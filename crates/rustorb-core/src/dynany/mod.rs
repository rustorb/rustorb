//! `DynamicAny`: building and taking apart values of types known only by
//! their TypeCode.
//!
//! A [`DynAny`] is a tree of components that follows the TypeCode: a
//! struct, an exception, a sequence, an array, a union, a valuetype or a
//! value box has components, which are DynAnys themselves and share their
//! state with it; the other types have none and hold their value. Every
//! constructed DynAny has a current position, which the `insert_*` and
//! `get_*` operations work on.
//!
//! [`DynAnyFactory`] makes DynAnys from an [`Any`] or a [`TypeCode`]. The
//! operations of the derived interfaces of the IDL (`DynStruct`,
//! `DynUnion`...) are all on [`DynAny`]; on a DynAny of another kind they
//! are `BAD_OPERATION`, as a failed narrow would be in a language mapping.
//! [`DynAny::kind`] tells which apply.
//!
//! Valuetype instances that are shared in the value, or refer to each
//! other in cycles, are one DynAny each, reached from every place the
//! instance is; [`DynAny::to_any`] gives them back as shared instances.
//! Such cycles keep the DynAnys alive until [`DynAny::destroy`].

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::typecode::{TCKind, TypeCode};
use crate::value::state_types;
use crate::{
    Any, CompletionStatus, Fixed, LongDouble, SystemException, SystemExceptionKind, Value, ValueRef,
};

#[cfg(test)]
mod tests;

/// How deep the components of a DynAny made from a TypeCode may nest.
const MAX_DEPTH: usize = 256;

/// Why a DynAny operation failed: the exceptions of `DynamicAny`, or a
/// system exception (`OBJECT_NOT_EXIST` after [`DynAny::destroy`],
/// `BAD_OPERATION` for an operation of another kind of DynAny,
/// `NO_IMPLEMENT` for abstract interfaces).
#[derive(Debug, Clone, PartialEq)]
pub enum DynAnyError {
    /// `DynAny::InvalidValue`: the value is not one the operation accepts
    /// (out of bounds, no current component...).
    InvalidValue,
    /// `DynAny::TypeMismatch`: the type does not fit.
    TypeMismatch,
    /// `DynAnyFactory::InconsistentTypeCode`: no DynAny can have this
    /// type.
    InconsistentTypeCode,
    /// `MustTruncate`: a valuetype instance is of a type derived from the
    /// declared one, and truncation was refused.
    MustTruncate,
    /// A system exception.
    System(SystemException),
}

impl fmt::Display for DynAnyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DynAnyError::InvalidValue => f.write_str("DynAny::InvalidValue"),
            DynAnyError::TypeMismatch => f.write_str("DynAny::TypeMismatch"),
            DynAnyError::InconsistentTypeCode => f.write_str("DynAnyFactory::InconsistentTypeCode"),
            DynAnyError::MustTruncate => f.write_str("MustTruncate"),
            DynAnyError::System(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DynAnyError {}

impl From<SystemException> for DynAnyError {
    fn from(e: SystemException) -> DynAnyError {
        DynAnyError::System(e)
    }
}

fn system(kind: SystemExceptionKind) -> DynAnyError {
    DynAnyError::System(SystemException::new(kind, 0, CompletionStatus::No))
}

type Result<T> = std::result::Result<T, DynAnyError>;

/// `DynamicAny::DynAnyFactory`, the initial reference `DynAnyFactory`.
/// It keeps no state.
#[derive(Debug, Clone, Copy, Default)]
pub struct DynAnyFactory;

impl DynAnyFactory {
    /// A DynAny with the type and the value of `value`. A valuetype
    /// instance of a type derived from the declared one is truncated to
    /// the declared type.
    pub fn create_dyn_any(&self, value: &Any) -> Result<DynAny> {
        DynAny::top(value.typecode(), value.value(), true)
    }

    /// [`create_dyn_any`](DynAnyFactory::create_dyn_any), but
    /// `MustTruncate` where a valuetype would be truncated.
    pub fn create_dyn_any_without_truncation(&self, value: &Any) -> Result<DynAny> {
        DynAny::top(value.typecode(), value.value(), false)
    }

    /// A DynAny of type `tc` with the default value: zero, false, empty
    /// strings and sequences, the first enumerator, a union with the
    /// first label value of its TypeCode, nil references, `null` valuetypes, an empty any and a
    /// `null` TypeCode, and structs and arrays of those.
    /// `InconsistentTypeCode` for `Principal`, `native`, abstract and
    /// local interfaces, an enum without enumerators and
    /// an unresolved recursive TypeCode.
    pub fn create_dyn_any_from_type_code(&self, tc: &TypeCode) -> Result<DynAny> {
        let life = Arc::new(Life::default());
        let state = default_state(tc, &life, 0)?;
        Ok(DynAny::node(tc.clone(), &life, false, state))
    }

    /// DynAnys of the values, each as
    /// [`create_dyn_any`](DynAnyFactory::create_dyn_any) or, without
    /// `allow_truncate`, as
    /// [`create_dyn_any_without_truncation`](DynAnyFactory::create_dyn_any_without_truncation).
    pub fn create_multiple_dyn_anys(
        &self,
        values: &[Any],
        allow_truncate: bool,
    ) -> Result<Vec<DynAny>> {
        values
            .iter()
            .map(|v| DynAny::top(v.typecode(), v.value(), allow_truncate))
            .collect()
    }

    /// The values of the DynAnys.
    pub fn create_multiple_anys(&self, values: &[DynAny]) -> Result<Vec<Any>> {
        values.iter().map(DynAny::to_any).collect()
    }
}

/// The lifetime of the DynAnys of one tree: [`DynAny::destroy`] on the
/// top one ends it.
#[derive(Default)]
struct Life {
    destroyed: AtomicBool,
}

/// `DynamicAny::DynAny` and the interfaces derived from it. Clones are
/// the same object, as references are.
#[derive(Clone)]
pub struct DynAny(Arc<Node>);

struct Node {
    /// The type as given, aliases included; the operations look at the
    /// unaliased type.
    tc: TypeCode,
    life: Arc<Life>,
    /// A component of another DynAny: destroy() does nothing.
    component: bool,
    state: Mutex<State>,
}

/// The state of a DynAny, by the kind of its unaliased type.
enum State {
    /// Types without components: the value.
    Basic(Value),
    /// Structs, exceptions, sequences and arrays: the members or
    /// elements.
    Items { items: Vec<DynAny>, pos: i32 },
    /// Unions: the discriminator and, when the discriminator selects one,
    /// the index of the member and the member.
    Union {
        disc: DynAny,
        member: Option<(u32, DynAny)>,
        pos: i32,
    },
    /// Valuetypes and value boxes: `None` for `null`, else the state
    /// members (the boxed value of a value box).
    Value {
        members: Option<Vec<DynAny>>,
        pos: i32,
    },
}

impl State {
    fn items(items: Vec<DynAny>) -> State {
        let pos = if items.is_empty() { -1 } else { 0 };
        State::Items { items, pos }
    }

    fn value(members: Option<Vec<DynAny>>) -> State {
        let pos = match &members {
            Some(m) if !m.is_empty() => 0,
            _ => -1,
        };
        State::Value { members, pos }
    }
}

impl fmt::Debug for DynAny {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DynAny({:?})", self.0.tc)
    }
}

/// Whether DynAnys of this (unaliased) kind have components.
fn is_constructed(kind: TCKind) -> bool {
    matches!(
        kind,
        TCKind::Struct
            | TCKind::Except
            | TCKind::Union
            | TCKind::Sequence
            | TCKind::Array
            | TCKind::Value
            | TCKind::ValueBox
            | TCKind::Event
    )
}

fn member_type(tc: &TypeCode, i: u32) -> Result<TypeCode> {
    tc.member_type(i)
        .map_err(|_| system(SystemExceptionKind::BAD_TYPECODE))
}

fn content_type(tc: &TypeCode) -> Result<TypeCode> {
    tc.content_type()
        .map_err(|_| system(SystemExceptionKind::BAD_TYPECODE))
}

fn bad_value() -> DynAnyError {
    system(SystemExceptionKind::BAD_PARAM)
}

// --- default values ------------------------------------------------------------

/// The state of a DynAny of type `tc` made from the TypeCode alone.
fn default_state(tc: &TypeCode, life: &Arc<Life>, depth: usize) -> Result<State> {
    if depth > MAX_DEPTH || tc.unaliased().is_recursive_placeholder() {
        return Err(DynAnyError::InconsistentTypeCode);
    }
    let t = tc.unaliased();
    Ok(match t.kind() {
        TCKind::Struct | TCKind::Except => {
            let n = t.member_count().unwrap_or(0);
            let items = (0..n)
                .map(|i| default_child(&member_type(&t, i)?, life, depth))
                .collect::<Result<_>>()?;
            State::items(items)
        }
        TCKind::Sequence => State::items(Vec::new()),
        TCKind::Array => {
            let content = content_type(&t)?;
            let n = t.length().unwrap_or(0);
            let items = (0..n)
                .map(|_| default_child(&content, life, depth))
                .collect::<Result<_>>()?;
            State::items(items)
        }
        TCKind::Union => {
            let (disc, member) = union_first(&t)?;
            let disc_tc = t
                .discriminator_type()
                .map_err(|_| DynAnyError::InconsistentTypeCode)?;
            let disc = DynAny::node(disc_tc, life, true, State::Basic(disc));
            let member = match member {
                Some(i) => Some((i, default_child(&member_type(&t, i)?, life, depth)?)),
                None => None,
            };
            State::Union {
                disc,
                member,
                pos: 0,
            }
        }
        TCKind::Value | TCKind::ValueBox | TCKind::Event => State::value(None),
        kind => State::Basic(default_basic(&t, kind)?),
    })
}

/// A component made from its TypeCode.
fn default_child(tc: &TypeCode, life: &Arc<Life>, depth: usize) -> Result<DynAny> {
    let state = default_state(tc, life, depth + 1)?;
    Ok(DynAny::node(tc.clone(), life, true, state))
}

fn default_basic(t: &TypeCode, kind: TCKind) -> Result<Value> {
    Ok(match kind {
        TCKind::Null | TCKind::Void => Value::Void,
        TCKind::Short => Value::Short(0),
        TCKind::UShort => Value::UShort(0),
        TCKind::Long => Value::Long(0),
        TCKind::ULong => Value::ULong(0),
        TCKind::LongLong => Value::LongLong(0),
        TCKind::ULongLong => Value::ULongLong(0),
        TCKind::Float => Value::Float(0.0),
        TCKind::Double => Value::Double(0.0),
        TCKind::LongDouble => Value::LongDouble(LongDouble::ZERO),
        TCKind::Boolean => Value::Bool(false),
        TCKind::Char => Value::Char('\0'),
        TCKind::WChar => Value::WChar(0),
        TCKind::Octet => Value::Octet(0),
        TCKind::String | TCKind::WString => Value::String(String::new()),
        TCKind::Enum => {
            if t.member_count().unwrap_or(0) == 0 {
                return Err(DynAnyError::InconsistentTypeCode);
            }
            Value::Enum(0)
        }
        TCKind::Fixed => Value::Fixed {
            digits: vec![0; t.fixed_digits().unwrap_or(0) as usize],
            negative: false,
        },
        TCKind::Objref | TCKind::Component | TCKind::Home => Value::Objref(None),
        TCKind::TypeCode => Value::TypeCode(TypeCode::null()),
        TCKind::Any => Value::Any(Box::default()),
        _ => return Err(DynAnyError::InconsistentTypeCode),
    })
}

/// The labels of a union, by member: `None` for the default member.
fn labels(t: &TypeCode) -> Vec<Option<i128>> {
    (0..t.member_count().unwrap_or(0))
        .map(|i| t.member_label_value(i).ok().flatten())
        .collect()
}

/// A discriminator value no label of the union `t` uses, if there is one.
fn unused_discriminator(t: &TypeCode) -> Option<Value> {
    let disc = t.discriminator_type().ok()?;
    let used: Vec<i128> = labels(t).into_iter().flatten().collect();
    // Labels are finitely many: counting up from 0 finds a free value
    // unless the type has few values and they are all used.
    (0..=used.len() as i128)
        .find(|l| !used.contains(l))
        .and_then(|l| disc.label_to_value(l))
}

/// The discriminator value and the member of a union made from its
/// TypeCode: the first label value, or an unused value (the default
/// member) if the union has no label but the default.
fn union_first(t: &TypeCode) -> Result<(Value, Option<u32>)> {
    let first = labels(t).into_iter().flatten().next();
    let disc = match first {
        Some(l) => t
            .discriminator_type()
            .ok()
            .and_then(|d| d.label_to_value(l)),
        None => unused_discriminator(t),
    }
    .ok_or(DynAnyError::InconsistentTypeCode)?;
    let member = t.union_member_for(&disc).ok().flatten();
    Ok((disc, member))
}

// --- from values -----------------------------------------------------------------

/// Makes DynAnys from values, one for each valuetype instance.
struct FromValue<'a> {
    life: &'a Arc<Life>,
    truncate: bool,
    /// The valuetype instances made so far, by instance and declared type.
    values: HashMap<(u64, String), DynAny>,
    depth: usize,
}

impl FromValue<'_> {
    fn node(&mut self, tc: &TypeCode, value: &Value, component: bool) -> Result<DynAny> {
        if let Some(d) = self
            .instance_key(tc, value)
            .and_then(|k| self.values.get(&k))
        {
            return Ok(d.clone());
        }
        let d = DynAny::node(tc.clone(), self.life, component, State::Basic(Value::Void));
        self.fill(&d, value)?;
        Ok(d)
    }

    /// The key of a valuetype instance in `values`.
    fn instance_key(&self, tc: &TypeCode, value: &Value) -> Option<(u64, String)> {
        let t = tc.unaliased();
        match (t.kind(), value) {
            (TCKind::Value | TCKind::ValueBox | TCKind::Event, Value::Valuetype(Some(r))) => {
                Some((r.key(), t.id().unwrap_or_default().to_owned()))
            }
            _ => None,
        }
    }

    /// Gives `d` the state of `value`, of the type of `d`. On an error `d`
    /// is left as it was.
    fn fill(&mut self, d: &DynAny, value: &Value) -> Result<()> {
        let state = match (self.instance_key(&d.0.tc, value), value) {
            (Some(key), Value::Valuetype(Some(r))) => {
                // Registered before its members are made, which may refer
                // to it.
                self.values.insert(key, d.clone());
                State::value(Some(self.valuetype_members(&d.unaliased(), r)?))
            }
            _ => self.state(&d.0.tc, value)?,
        };
        *d.lock() = state;
        Ok(())
    }

    fn state(&mut self, tc: &TypeCode, value: &Value) -> Result<State> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(system(SystemExceptionKind::BAD_PARAM));
        }
        let state = self.state_inner(&tc.unaliased(), value);
        self.depth -= 1;
        state
    }

    fn state_inner(&mut self, t: &TypeCode, value: &Value) -> Result<State> {
        Ok(match (t.kind(), value) {
            (TCKind::Struct | TCKind::Except, Value::List(items)) => {
                let n = t.member_count().unwrap_or(0);
                if items.len() != n as usize {
                    return Err(bad_value());
                }
                let items = items
                    .iter()
                    .enumerate()
                    .map(|(i, v)| self.node(&member_type(t, i as u32)?, v, true))
                    .collect::<Result<_>>()?;
                State::items(items)
            }
            (TCKind::Sequence | TCKind::Array, _) => {
                let content = content_type(t)?;
                let values: Vec<Value> = match value {
                    Value::List(items) => items.clone(),
                    Value::Octets(b) => b.iter().map(|&b| Value::Octet(b)).collect(),
                    Value::Chars(s) => s.chars().map(Value::Char).collect(),
                    _ => return Err(bad_value()),
                };
                let n = t.length().unwrap_or(0) as usize;
                let fits = if t.kind() == TCKind::Sequence {
                    n == 0 || values.len() <= n
                } else {
                    values.len() == n
                };
                if !fits {
                    return Err(bad_value());
                }
                let items = values
                    .iter()
                    .map(|v| self.node(&content, v, true))
                    .collect::<Result<_>>()?;
                State::items(items)
            }
            (TCKind::Union, Value::Union { disc, member }) => {
                let disc_tc = t.discriminator_type().map_err(|_| bad_value())?;
                let selected = t.union_member_for(disc).map_err(|_| bad_value())?;
                if disc.label_key().is_none() || selected != member.as_ref().map(|(i, _)| *i) {
                    return Err(bad_value());
                }
                let disc = self.node(&disc_tc, disc, true)?;
                let member = match member {
                    Some((i, v)) => Some((*i, self.node(&member_type(t, *i)?, v, true)?)),
                    None => None,
                };
                State::Union {
                    disc,
                    member,
                    pos: 0,
                }
            }
            (TCKind::Value | TCKind::ValueBox | TCKind::Event, Value::Valuetype(None)) => {
                State::value(None)
            }
            (kind, v) if !is_constructed(kind) => State::Basic(check_basic(t, kind, v)?),
            _ => return Err(bad_value()),
        })
    }

    /// The members of the instance `r` as a value of the declared type
    /// `t`: those of a derived type's own members are cut off, if
    /// truncation is allowed.
    fn valuetype_members(&mut self, t: &TypeCode, r: &ValueRef) -> Result<Vec<DynAny>> {
        let types = state_types(t);
        let actual = r.typecode().unaliased();
        let mut values = r.members();
        if actual.id().ok() != t.id().ok() {
            if !self.truncate {
                return Err(DynAnyError::MustTruncate);
            }
            // The members of the base types come first.
            if values.len() < types.len() {
                return Err(bad_value());
            }
            values.truncate(types.len());
        }
        if values.len() != types.len() {
            return Err(bad_value());
        }
        types
            .iter()
            .zip(&values)
            .map(|(tc, v)| self.node(tc, v, true))
            .collect()
    }
}

/// `value` if it is a value of the type `t` (of kind `kind`, without
/// components).
fn check_basic(t: &TypeCode, kind: TCKind, value: &Value) -> Result<Value> {
    let ok = match (kind, value) {
        (TCKind::Null | TCKind::Void, _) => return Ok(Value::Void),
        (TCKind::Short, Value::Short(_))
        | (TCKind::UShort, Value::UShort(_))
        | (TCKind::Long, Value::Long(_))
        | (TCKind::ULong, Value::ULong(_))
        | (TCKind::LongLong, Value::LongLong(_))
        | (TCKind::ULongLong, Value::ULongLong(_))
        | (TCKind::Float, Value::Float(_))
        | (TCKind::Double, Value::Double(_))
        | (TCKind::LongDouble, Value::LongDouble(_))
        | (TCKind::Boolean, Value::Bool(_))
        | (TCKind::Char, Value::Char(_))
        | (TCKind::WChar, Value::WChar(_))
        | (TCKind::Octet, Value::Octet(_))
        | (TCKind::Objref | TCKind::Component | TCKind::Home, Value::Objref(_))
        | (TCKind::TypeCode, Value::TypeCode(_))
        | (TCKind::Any, Value::Any(_)) => true,
        (TCKind::String | TCKind::WString, Value::String(s)) => {
            fits_bound(s, t.length().unwrap_or(0))
        }
        (TCKind::Enum, Value::Enum(v)) => *v < t.member_count().unwrap_or(0),
        (TCKind::Fixed, Value::Fixed { digits, .. }) => {
            digits.len() == t.fixed_digits().unwrap_or(0) as usize && digits.iter().all(|&d| d < 10)
        }
        (TCKind::AbstractInterface | TCKind::Native, _) => {
            return Err(system(SystemExceptionKind::NO_IMPLEMENT));
        }
        _ => false,
    };
    if ok {
        Ok(value.clone())
    } else {
        Err(bad_value())
    }
}

fn fits_bound(s: &str, bound: u32) -> bool {
    bound == 0 || s.chars().count() <= bound as usize
}

// --- to values -------------------------------------------------------------------

/// Makes values from DynAnys, one instance for each valuetype DynAny.
#[derive(Default)]
struct ToValue {
    values: HashMap<usize, ValueRef>,
}

impl ToValue {
    fn value(&mut self, d: &DynAny) -> Result<Value> {
        d.check()?;
        let t = d.0.tc.unaliased();
        if let Some(r) = self.values.get(&d.key()) {
            return Ok(Value::Valuetype(Some(r.clone())));
        }
        // Copies of what is needed, so that no lock is held while the
        // components are converted.
        enum Parts {
            Basic(Value),
            Items(Vec<DynAny>),
            Union(DynAny, Option<(u32, DynAny)>),
            Value(Option<Vec<DynAny>>),
        }
        d.sync_union();
        let parts = match &*d.lock() {
            State::Basic(v) => Parts::Basic(v.clone()),
            State::Items { items, .. } => Parts::Items(items.clone()),
            State::Union { disc, member, .. } => Parts::Union(disc.clone(), member.clone()),
            State::Value { members, .. } => Parts::Value(members.clone()),
        };
        Ok(match parts {
            Parts::Basic(v) => v,
            Parts::Items(items) => {
                let content = match t.kind() {
                    TCKind::Sequence | TCKind::Array => {
                        t.content_type().ok().map(|c| c.unaliased().kind())
                    }
                    _ => None,
                };
                let values = items
                    .iter()
                    .map(|i| self.value(i))
                    .collect::<Result<Vec<_>>>()?;
                match content {
                    Some(TCKind::Octet) => Value::Octets(
                        values
                            .iter()
                            .map(|v| match v {
                                Value::Octet(b) => *b,
                                _ => 0,
                            })
                            .collect(),
                    ),
                    Some(TCKind::Char) => Value::Chars(
                        values
                            .iter()
                            .map(|v| match v {
                                Value::Char(c) => *c,
                                _ => '\0',
                            })
                            .collect(),
                    ),
                    _ => Value::List(values),
                }
            }
            Parts::Union(disc, member) => Value::Union {
                disc: Box::new(self.value(&disc)?),
                member: match member {
                    Some((i, m)) => Some((i, Box::new(self.value(&m)?))),
                    None => None,
                },
            },
            Parts::Value(None) => Value::Valuetype(None),
            Parts::Value(Some(members)) => {
                // Registered before its members are converted, which may
                // refer to it.
                let r = ValueRef::new(t.clone(), Vec::new());
                self.values.insert(d.key(), r.clone());
                let values = members
                    .iter()
                    .map(|m| self.value(m))
                    .collect::<Result<Vec<_>>>()?;
                r.set_members(values);
                Value::Valuetype(Some(r))
            }
        })
    }
}

// --- the DynAny operations ---------------------------------------------------------

macro_rules! basic_ops {
    ($($insert:ident, $get:ident, $ty:ty, $kind:ident, $variant:ident;)*) => {
        $(
            #[doc = concat!("`", stringify!($insert), "`: see [`DynAny::insert_basic`].")]
            pub fn $insert(&self, v: $ty) -> Result<()> {
                self.insert_basic(TCKind::$kind, Value::$variant(v))
            }

            #[doc = concat!("`", stringify!($get), "`: see [`DynAny::get_basic`].")]
            pub fn $get(&self) -> Result<$ty> {
                match self.get_basic(TCKind::$kind)? {
                    Value::$variant(v) => Ok(v),
                    _ => Err(DynAnyError::TypeMismatch),
                }
            }
        )*
    };
}

macro_rules! seq_ops {
    ($($insert:ident, $get:ident, $ty:ty, $kind:ident, $variant:ident;)*) => {
        $(
            #[doc = concat!("`", stringify!($insert), "`: see [`DynAny::insert_seq`].")]
            pub fn $insert(&self, v: &[$ty]) -> Result<()> {
                self.insert_seq(TCKind::$kind, v.iter().map(|x| Value::$variant(x.clone())).collect())
            }

            #[doc = concat!("`", stringify!($get), "`: see [`DynAny::get_seq`].")]
            pub fn $get(&self) -> Result<Vec<$ty>> {
                self.get_seq(TCKind::$kind)?
                    .into_iter()
                    .map(|v| match v {
                        Value::$variant(x) => Ok(x),
                        _ => Err(DynAnyError::TypeMismatch),
                    })
                    .collect()
            }
        )*
    };
}

impl DynAny {
    fn node(tc: TypeCode, life: &Arc<Life>, component: bool, state: State) -> DynAny {
        DynAny(Arc::new(Node {
            tc,
            life: Arc::clone(life),
            component,
            state: Mutex::new(state),
        }))
    }

    fn top(tc: &TypeCode, value: &Value, truncate: bool) -> Result<DynAny> {
        let life = Arc::new(Life::default());
        let mut from = FromValue {
            life: &life,
            truncate,
            values: HashMap::new(),
            depth: 0,
        };
        from.node(tc, value, false)
    }

    /// A component of `self` with the type and value of `value`.
    fn child(&self, tc: &TypeCode, value: &Value) -> Result<DynAny> {
        FromValue {
            life: &self.0.life,
            truncate: true,
            values: HashMap::new(),
            depth: 0,
        }
        .node(tc, value, true)
    }

    /// A component of `self` with the default value of `tc`.
    fn default_child(&self, tc: &TypeCode) -> Result<DynAny> {
        default_child(tc, &self.0.life, 0)
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn key(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }

    /// `OBJECT_NOT_EXIST` once the tree is destroyed.
    fn check(&self) -> Result<()> {
        if self.0.life.destroyed.load(Ordering::Acquire) {
            return Err(system(SystemExceptionKind::OBJECT_NOT_EXIST));
        }
        Ok(())
    }

    fn unaliased(&self) -> TypeCode {
        self.0.tc.unaliased()
    }

    /// The unaliased kind of the type, which tells which interface of
    /// `DynamicAny` the DynAny is: `DynStruct` for structs and exceptions,
    /// `DynFixed`, `DynEnum`, `DynUnion`, `DynSequence`, `DynArray`,
    /// `DynValue` (valuetypes and eventtypes), `DynValueBox`, plain
    /// `DynAny` for the others.
    pub fn kind(&self) -> TCKind {
        self.0.tc.unaliased().kind()
    }

    /// `BAD_OPERATION` unless the DynAny is of one of `kinds`.
    fn expect(&self, kinds: &[TCKind]) -> Result<TypeCode> {
        self.check()?;
        let t = self.unaliased();
        if kinds.contains(&t.kind()) {
            Ok(t)
        } else {
            Err(system(SystemExceptionKind::BAD_OPERATION))
        }
    }

    /// Whether both are the same DynAny.
    pub fn ptr_eq(&self, other: &DynAny) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    // --- DynAny ---------------------------------------------------------------

    /// `type`: the TypeCode of the DynAny, aliases included.
    pub fn type_code(&self) -> Result<TypeCode> {
        self.check()?;
        Ok(self.0.tc.clone())
    }

    /// `assign`: gives this DynAny the value of `other`. `TypeMismatch`
    /// if the types are not equivalent.
    pub fn assign(&self, other: &DynAny) -> Result<()> {
        self.check()?;
        let value = other.to_any()?;
        self.from_any(&value)
    }

    /// `from_any`: gives this DynAny the value of `value`. `TypeMismatch`
    /// if the types are not equivalent; `InvalidValue` if the value does
    /// not fit the type.
    pub fn from_any(&self, value: &Any) -> Result<()> {
        self.check()?;
        if !value.typecode().equivalent(&self.0.tc) {
            return Err(DynAnyError::TypeMismatch);
        }
        self.fill(value.value()).map_err(|e| match e {
            DynAnyError::System(_) => DynAnyError::InvalidValue,
            e => e,
        })
    }

    /// Gives this DynAny the value `value`, its components in its tree.
    fn fill(&self, value: &Value) -> Result<()> {
        FromValue {
            life: &self.0.life,
            truncate: true,
            values: HashMap::new(),
            depth: 0,
        }
        .fill(self, value)
    }

    /// `to_any`: the value as an Any of the type of the DynAny.
    pub fn to_any(&self) -> Result<Any> {
        let value = ToValue::default().value(self)?;
        Ok(Any::new(self.0.tc.clone(), value))
    }

    /// `equal`: whether both have equivalent types and equal values.
    pub fn equal(&self, other: &DynAny) -> Result<bool> {
        let a = self.to_any()?;
        let b = other.to_any()?;
        Ok(a.typecode().equivalent(b.typecode()) && a.value() == b.value())
    }

    /// `destroy`: destroys a DynAny made by the factory or by
    /// [`copy`](DynAny::copy) and its components; using them afterwards
    /// is `OBJECT_NOT_EXIST`. Destroying a component does nothing.
    pub fn destroy(&self) -> Result<()> {
        self.check()?;
        if self.0.component {
            return Ok(());
        }
        self.0.life.destroyed.store(true, Ordering::Release);
        // Breaks the cycles that valuetype instances may form.
        let mut stack = vec![self.clone()];
        while let Some(d) = stack.pop() {
            match std::mem::replace(&mut *d.lock(), State::Basic(Value::Void)) {
                State::Basic(v) => v.break_cycles(),
                State::Items { items, .. } => stack.extend(items),
                State::Union { disc, member, .. } => {
                    stack.push(disc);
                    stack.extend(member.map(|(_, m)| m));
                }
                State::Value { members, .. } => stack.extend(members.into_iter().flatten()),
            }
        }
        Ok(())
    }

    /// `copy`: a new DynAny with the type and the value of this one.
    pub fn copy(&self) -> Result<DynAny> {
        let value = self.to_any()?;
        DynAny::top(value.typecode(), value.value(), true)
    }

    /// The DynAny the `insert_*` and `get_*` operations work on: this one
    /// if its type has no components, else the current component
    /// (`InvalidValue` if there is none).
    fn target(&self) -> Result<DynAny> {
        self.check()?;
        if !is_constructed(self.kind()) {
            return Ok(self.clone());
        }
        self.current_component()?.ok_or(DynAnyError::InvalidValue)
    }

    /// `insert_<type>` with the value of the type `kind` (`Objref` for
    /// `insert_reference`, `Value` for `insert_val`): sets the value of
    /// the [target](DynAny) if its unaliased type is of that kind, else
    /// `TypeMismatch`. A string longer than a bounded string type is
    /// `InvalidValue`.
    pub fn insert_basic(&self, kind: TCKind, value: Value) -> Result<()> {
        let target = self.target()?;
        let t = target.unaliased();
        let actual = t.kind();
        let same = actual == kind
            || (kind == TCKind::Objref && matches!(actual, TCKind::Component | TCKind::Home))
            || (kind == TCKind::Value && matches!(actual, TCKind::ValueBox | TCKind::Event));
        if !same {
            return Err(DynAnyError::TypeMismatch);
        }
        match value {
            Value::Valuetype(v) => target.insert_valuetype(&t, v),
            value => {
                if let Value::String(s) = &value {
                    if !fits_bound(s, t.length().unwrap_or(0)) {
                        return Err(DynAnyError::InvalidValue);
                    }
                }
                let value = check_basic(&t, actual, &value).map_err(|e| match e {
                    DynAnyError::System(_) => DynAnyError::TypeMismatch,
                    e => e,
                })?;
                *target.lock() = State::Basic(value);
                Ok(())
            }
        }
    }

    fn insert_valuetype(&self, t: &TypeCode, v: Option<ValueRef>) -> Result<()> {
        if let Some(r) = &v {
            if !is_assignable_to(r.typecode(), t) {
                return Err(DynAnyError::TypeMismatch);
            }
        }
        self.fill(&Value::Valuetype(v))
    }

    /// `get_<type>`: the value of the [target](DynAny) if its unaliased
    /// type is of the kind `kind`, else `TypeMismatch`.
    pub fn get_basic(&self, kind: TCKind) -> Result<Value> {
        let target = self.target()?;
        let actual = target.kind();
        let same = actual == kind
            || (kind == TCKind::Objref && matches!(actual, TCKind::Component | TCKind::Home))
            || (kind == TCKind::Value && matches!(actual, TCKind::ValueBox | TCKind::Event));
        if !same {
            return Err(DynAnyError::TypeMismatch);
        }
        if kind == TCKind::Value {
            return ToValue::default().value(&target);
        }
        match &*target.lock() {
            State::Basic(v) => Ok(v.clone()),
            _ => Err(DynAnyError::TypeMismatch),
        }
    }

    basic_ops! {
        insert_boolean, get_boolean, bool, Boolean, Bool;
        insert_octet, get_octet, u8, Octet, Octet;
        insert_char, get_char, char, Char, Char;
        insert_short, get_short, i16, Short, Short;
        insert_ushort, get_ushort, u16, UShort, UShort;
        insert_long, get_long, i32, Long, Long;
        insert_ulong, get_ulong, u32, ULong, ULong;
        insert_float, get_float, f32, Float, Float;
        insert_double, get_double, f64, Double, Double;
        insert_longdouble, get_longdouble, LongDouble, LongDouble, LongDouble;
        insert_longlong, get_longlong, i64, LongLong, LongLong;
        insert_ulonglong, get_ulonglong, u64, ULongLong, ULongLong;
        insert_wchar, get_wchar, u16, WChar, WChar;
        insert_typecode, get_typecode, TypeCode, TypeCode, TypeCode;
        insert_reference, get_reference, Option<crate::ObjectRef>, Objref, Objref;
        insert_val, get_val, Option<ValueRef>, Value, Valuetype;
    }

    /// `insert_string`.
    pub fn insert_string(&self, v: &str) -> Result<()> {
        self.insert_basic(TCKind::String, Value::String(v.to_owned()))
    }

    /// `get_string`.
    pub fn get_string(&self) -> Result<String> {
        match self.get_basic(TCKind::String)? {
            Value::String(s) => Ok(s),
            _ => Err(DynAnyError::TypeMismatch),
        }
    }

    /// `insert_wstring`.
    pub fn insert_wstring(&self, v: &str) -> Result<()> {
        self.insert_basic(TCKind::WString, Value::String(v.to_owned()))
    }

    /// `get_wstring`.
    pub fn get_wstring(&self) -> Result<String> {
        match self.get_basic(TCKind::WString)? {
            Value::String(s) => Ok(s),
            _ => Err(DynAnyError::TypeMismatch),
        }
    }

    /// `insert_any`.
    pub fn insert_any(&self, v: &Any) -> Result<()> {
        self.insert_basic(TCKind::Any, Value::Any(Box::new(v.clone())))
    }

    /// `get_any`.
    pub fn get_any(&self) -> Result<Any> {
        match self.get_basic(TCKind::Any)? {
            Value::Any(a) => Ok(*a),
            _ => Err(DynAnyError::TypeMismatch),
        }
    }

    /// `insert_dyn_any`: [`insert_any`](DynAny::insert_any) with the
    /// value of `v`.
    pub fn insert_dyn_any(&self, v: &DynAny) -> Result<()> {
        self.insert_any(&v.to_any()?)
    }

    /// `get_dyn_any`: a new DynAny of the value in the any.
    pub fn get_dyn_any(&self) -> Result<DynAny> {
        let a = self.get_any()?;
        DynAny::top(a.typecode(), a.value(), true)
    }

    /// `insert_abstract` / `get_abstract`: `NO_IMPLEMENT`.
    pub fn no_implement(&self) -> Result<()> {
        self.check()?;
        Err(system(SystemExceptionKind::NO_IMPLEMENT))
    }

    /// The DynAny the `insert_<type>_seq` and `get_<type>_seq`
    /// operations work on: this one if it is a sequence or array of
    /// elements of the kind `kind` or has no components, else the current
    /// component.
    fn seq_target(&self, kind: TCKind) -> Result<DynAny> {
        self.check()?;
        if self.seq_of(kind).is_ok() {
            return Ok(self.clone());
        }
        self.target()
    }

    /// `insert_<type>_seq`: sets the elements of the target (this DynAny
    /// if it is a sequence or array of that kind or has no components,
    /// else its current component), a sequence or array of elements of
    /// the kind `kind`, else
    /// `TypeMismatch`. Too many elements for a bounded sequence, or not
    /// as many as an array has, is `InvalidValue`. The position becomes
    /// the first element.
    pub fn insert_seq(&self, kind: TCKind, values: Vec<Value>) -> Result<()> {
        let target = self.seq_target(kind)?;
        let t = target.seq_of(kind)?;
        let content = content_type(&t)?;
        let len = values.len();
        target.check_length(&t, len)?;
        let items = values
            .iter()
            .map(|v| target.child(&content, v))
            .collect::<Result<Vec<_>>>()
            .map_err(|_| DynAnyError::TypeMismatch)?;
        *target.lock() = State::items(items);
        Ok(())
    }

    /// `get_<type>_seq`: the elements of the target (this DynAny if it is
    /// a sequence or array of that kind or has no components, else its
    /// current component), a sequence or array of elements of the kind
    /// `kind`, else
    /// `TypeMismatch`.
    pub fn get_seq(&self, kind: TCKind) -> Result<Vec<Value>> {
        let target = self.seq_target(kind)?;
        target.seq_of(kind)?;
        let items = match &*target.lock() {
            State::Items { items, .. } => items.clone(),
            _ => return Err(DynAnyError::TypeMismatch),
        };
        let mut to = ToValue::default();
        items.iter().map(|i| to.value(i)).collect()
    }

    /// The unaliased type if it is a sequence or array of `kind`.
    fn seq_of(&self, kind: TCKind) -> Result<TypeCode> {
        let t = self.unaliased();
        if !matches!(t.kind(), TCKind::Sequence | TCKind::Array)
            || content_type(&t)?.unaliased().kind() != kind
        {
            return Err(DynAnyError::TypeMismatch);
        }
        Ok(t)
    }

    fn check_length(&self, t: &TypeCode, len: usize) -> Result<()> {
        let n = t.length().unwrap_or(0) as usize;
        let ok = if t.kind() == TCKind::Sequence {
            n == 0 || len <= n
        } else {
            len == n
        };
        if ok {
            Ok(())
        } else {
            Err(DynAnyError::InvalidValue)
        }
    }

    seq_ops! {
        insert_boolean_seq, get_boolean_seq, bool, Boolean, Bool;
        insert_octet_seq, get_octet_seq, u8, Octet, Octet;
        insert_char_seq, get_char_seq, char, Char, Char;
        insert_short_seq, get_short_seq, i16, Short, Short;
        insert_ushort_seq, get_ushort_seq, u16, UShort, UShort;
        insert_long_seq, get_long_seq, i32, Long, Long;
        insert_ulong_seq, get_ulong_seq, u32, ULong, ULong;
        insert_float_seq, get_float_seq, f32, Float, Float;
        insert_double_seq, get_double_seq, f64, Double, Double;
        insert_longdouble_seq, get_longdouble_seq, LongDouble, LongDouble, LongDouble;
        insert_longlong_seq, get_longlong_seq, i64, LongLong, LongLong;
        insert_ulonglong_seq, get_ulonglong_seq, u64, ULongLong, ULongLong;
        insert_wchar_seq, get_wchar_seq, u16, WChar, WChar;
    }

    // --- iteration ------------------------------------------------------------

    /// Keeps a union's member in step with its discriminator, which may
    /// have been changed through its component: a discriminator that
    /// selects another member gets that member with its default value.
    fn sync_union(&self) {
        let disc = match &*self.lock() {
            State::Union { disc, .. } => disc.clone(),
            _ => return,
        };
        let Ok(value) = disc.to_any() else { return };
        let t = self.unaliased();
        let selected = t.union_member_for(value.value()).ok().flatten();
        let current = match &*self.lock() {
            State::Union { member, .. } => member.as_ref().map(|(i, _)| *i),
            _ => return,
        };
        if selected == current {
            return;
        }
        // A member with several labels is one entry of the TypeCode per
        // label, with the same name: it stays active, with its value.
        if let (Some(i), Some(j)) = (selected, current) {
            if same_member(&t, i, j) {
                if let State::Union {
                    member: Some(m), ..
                } = &mut *self.lock()
                {
                    m.0 = i;
                }
                return;
            }
        }
        let member = selected.and_then(|i| {
            let tc = member_type(&t, i).ok()?;
            Some((i, self.default_child(&tc).ok()?))
        });
        if let State::Union { member: m, pos, .. } = &mut *self.lock() {
            *m = member;
            if *pos >= if m.is_some() { 2 } else { 1 } {
                *pos = -1;
            }
        }
    }

    /// `component_count`: the members of a struct, exception or
    /// valuetype (0 for `null`), the elements of a sequence or array, 2
    /// for a union with an active member and 1 without, 1 for a value
    /// box that is not `null`, 0 for the other types.
    pub fn component_count(&self) -> Result<u32> {
        self.check()?;
        self.sync_union();
        Ok(match &*self.lock() {
            State::Basic(_) => 0,
            State::Items { items, .. } => items.len() as u32,
            State::Union { member, .. } => 1 + u32::from(member.is_some()),
            State::Value { members, .. } => members.as_ref().map_or(0, |m| m.len() as u32),
        })
    }

    fn pos_mut(state: &mut State) -> Option<&mut i32> {
        match state {
            State::Basic(_) => None,
            State::Items { pos, .. } | State::Union { pos, .. } | State::Value { pos, .. } => {
                Some(pos)
            }
        }
    }

    /// `seek`: makes `index` the current position. True if there is a
    /// component there; else the position becomes -1 and the result is
    /// false.
    pub fn seek(&self, index: i32) -> Result<bool> {
        let count = self.component_count()? as i32;
        let mut state = self.lock();
        let Some(pos) = DynAny::pos_mut(&mut state) else {
            return Ok(false);
        };
        if (0..count).contains(&index) {
            *pos = index;
            Ok(true)
        } else {
            *pos = -1;
            Ok(false)
        }
    }

    /// `rewind`: `seek(0)`.
    pub fn rewind(&self) -> Result<()> {
        self.seek(0).map(|_| ())
    }

    /// `next`: moves to the next component (from -1 to the first). True
    /// if there is one; else the position becomes -1 and the result is
    /// false.
    pub fn next(&self) -> Result<bool> {
        let count = self.component_count()? as i32;
        let mut state = self.lock();
        let Some(pos) = DynAny::pos_mut(&mut state) else {
            return Ok(false);
        };
        if *pos + 1 < count {
            *pos += 1;
            Ok(true)
        } else {
            *pos = -1;
            Ok(false)
        }
    }

    /// The current position, -1 when there is no current component.
    pub fn position(&self) -> Result<i32> {
        self.check()?;
        self.sync_union();
        Ok(DynAny::pos_mut(&mut self.lock()).map_or(-1, |p| *p))
    }

    /// `current_component`: the component at the current position,
    /// `None` at -1. `TypeMismatch` for a DynAny that cannot have
    /// components (`DynEnum`, `DynFixed`, basic types), for an exception
    /// without members and for a `null` valuetype or value box.
    pub fn current_component(&self) -> Result<Option<DynAny>> {
        self.check()?;
        let t = self.unaliased();
        let kind = t.kind();
        if !is_constructed(kind) || (kind == TCKind::Except && t.member_count() == Ok(0)) {
            return Err(DynAnyError::TypeMismatch);
        }
        self.sync_union();
        let state = self.lock();
        if let State::Value { members: None, .. } = &*state {
            return Err(DynAnyError::TypeMismatch);
        }
        Ok(match &*state {
            State::Basic(_) => None,
            State::Items { items, pos } => usize::try_from(*pos)
                .ok()
                .and_then(|p| items.get(p))
                .cloned(),
            State::Union { disc, member, pos } => match pos {
                0 => Some(disc.clone()),
                1 => member.as_ref().map(|(_, m)| m.clone()),
                _ => None,
            },
            State::Value { members, pos } => usize::try_from(*pos)
                .ok()
                .and_then(|p| members.as_ref()?.get(p))
                .cloned(),
        })
    }

    // --- DynFixed ---------------------------------------------------------------

    /// `DynFixed::get_value`: the value as a fixed-point literal without
    /// the `d` suffix and without zeros at the end of the fraction, such
    /// as `-123.45`, `-12.5` or `0`.
    pub fn get_fixed_value(&self) -> Result<String> {
        let t = self.expect(&[TCKind::Fixed])?;
        let State::Basic(Value::Fixed { digits, negative }) = &*self.lock() else {
            return Err(DynAnyError::TypeMismatch);
        };
        let scale = u16::try_from(t.fixed_scale().unwrap_or(0)).unwrap_or(0);
        let f = Fixed::from_digits(digits, scale, *negative).ok_or_else(bad_value)?;
        // Without the zeros at the end of the fraction, as omniORB.
        let s = f.to_string();
        Ok(if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s
        })
    }

    /// `DynFixed::set_value`: sets the value from a fixed-point literal
    /// (an optional sign, digits with an optional point, an optional `d`
    /// or `D`, blanks around). `TypeMismatch` if it is not one,
    /// `InvalidValue` if its integer part has too many digits. Digits
    /// after the scale are cut off, and the result is then false.
    pub fn set_fixed_value(&self, literal: &str) -> Result<bool> {
        let t = self.expect(&[TCKind::Fixed])?;
        let f: Fixed = literal
            .trim()
            .parse()
            .map_err(|_| DynAnyError::TypeMismatch)?;
        let digits = t.fixed_digits().unwrap_or(0);
        let scale = u16::try_from(t.fixed_scale().unwrap_or(0)).unwrap_or(0);
        // Exact if the digits cut off are all zero.
        let exact = f.scale() <= scale
            || f.digits()[f.digits().len() - (f.scale() - scale) as usize..]
                .iter()
                .all(|&d| d == 0);
        let fitted = f
            .to_digits(digits, scale)
            .ok_or(DynAnyError::InvalidValue)?;
        let negative = f.is_negative() && fitted.iter().any(|&d| d != 0);
        *self.lock() = State::Basic(Value::Fixed {
            digits: fitted,
            negative,
        });
        Ok(exact)
    }

    // --- DynEnum --------------------------------------------------------------

    /// `DynEnum::get_as_ulong`: the position of the enumerator.
    pub fn get_enum_as_ulong(&self) -> Result<u32> {
        self.expect(&[TCKind::Enum])?;
        match &*self.lock() {
            State::Basic(Value::Enum(v)) => Ok(*v),
            _ => Err(DynAnyError::TypeMismatch),
        }
    }

    /// `DynEnum::set_as_ulong`: `InvalidValue` if there is no enumerator
    /// at `v`.
    pub fn set_enum_as_ulong(&self, v: u32) -> Result<()> {
        let t = self.expect(&[TCKind::Enum])?;
        if v >= t.member_count().unwrap_or(0) {
            return Err(DynAnyError::InvalidValue);
        }
        *self.lock() = State::Basic(Value::Enum(v));
        Ok(())
    }

    /// `DynEnum::get_as_string`: the name of the enumerator.
    pub fn get_enum_as_string(&self) -> Result<String> {
        let t = self.expect(&[TCKind::Enum])?;
        let v = self.get_enum_as_ulong()?;
        Ok(t.member_name(v).unwrap_or_default().to_owned())
    }

    /// `DynEnum::set_as_string`: `InvalidValue` if no enumerator has the
    /// name.
    pub fn set_enum_as_string(&self, name: &str) -> Result<()> {
        let t = self.expect(&[TCKind::Enum])?;
        let v = (0..t.member_count().unwrap_or(0))
            .find(|&i| t.member_name(i) == Ok(name))
            .ok_or(DynAnyError::InvalidValue)?;
        *self.lock() = State::Basic(Value::Enum(v));
        Ok(())
    }

    // --- DynStruct and DynValue -------------------------------------------------

    const MEMBERED: [TCKind; 4] = [TCKind::Struct, TCKind::Except, TCKind::Value, TCKind::Event];

    /// The names and types of the members of a struct, exception or
    /// valuetype (with those of its base types first).
    fn member_types(t: &TypeCode) -> Vec<(String, TypeCode)> {
        match t.kind() {
            TCKind::Value | TCKind::Event => value_member_names(t)
                .into_iter()
                .zip(state_types(t))
                .collect(),
            _ => (0..t.member_count().unwrap_or(0))
                .map(|i| {
                    (
                        t.member_name(i).unwrap_or_default().to_owned(),
                        t.member_type(i).unwrap_or_else(|_| TypeCode::null()),
                    )
                })
                .collect(),
        }
    }

    /// `current_member_name` of `DynStruct` and `DynValue`.
    /// `TypeMismatch` without members (or `null`), `InvalidValue` at
    /// position -1.
    pub fn current_member_name(&self) -> Result<String> {
        let t = self.expect(&Self::MEMBERED)?;
        let i = self.current_member(&t)?;
        Ok(DynAny::member_types(&t).swap_remove(i).0)
    }

    /// `current_member_kind` of `DynStruct` and `DynValue`: the kind of
    /// the member's type, aliases included.
    pub fn current_member_kind(&self) -> Result<TCKind> {
        let t = self.expect(&Self::MEMBERED)?;
        let i = self.current_member(&t)?;
        Ok(DynAny::member_types(&t).swap_remove(i).1.kind())
    }

    fn current_member(&self, t: &TypeCode) -> Result<usize> {
        let count = self.component_count()? as i32;
        // An exception without members, a null valuetype.
        if DynAny::member_types(t).is_empty() || count == 0 {
            return Err(DynAnyError::TypeMismatch);
        }
        let pos = self.position()?;
        if pos < 0 || pos >= count {
            return Err(DynAnyError::InvalidValue);
        }
        Ok(pos as usize)
    }

    fn member_items(&self) -> Result<Vec<DynAny>> {
        Ok(match &*self.lock() {
            State::Items { items, .. } => items.clone(),
            State::Value { members, .. } => members.clone().ok_or(DynAnyError::InvalidValue)?,
            _ => return Err(DynAnyError::TypeMismatch),
        })
    }

    /// `get_members` of `DynStruct` and `DynValue`: the names and values
    /// of the members. `InvalidValue` for a `null` valuetype.
    pub fn get_members(&self) -> Result<Vec<(String, Any)>> {
        self.get_members_as_dyn_any()?
            .into_iter()
            .map(|(n, d)| Ok((n, d.to_any()?)))
            .collect()
    }

    /// `get_members_as_dyn_any`: the names and the components.
    pub fn get_members_as_dyn_any(&self) -> Result<Vec<(String, DynAny)>> {
        let t = self.expect(&Self::MEMBERED)?;
        let names = DynAny::member_types(&t).into_iter().map(|(n, _)| n);
        Ok(names.zip(self.member_items()?).collect())
    }

    /// `set_members` of `DynStruct` and `DynValue`: as many values as
    /// there are members (else `InvalidValue`), each with the name of its
    /// member or an empty one and a type equivalent to the member's
    /// (else `TypeMismatch`). A `null` valuetype is no longer `null`.
    pub fn set_members(&self, values: &[(String, Any)]) -> Result<()> {
        let t = self.expect(&Self::MEMBERED)?;
        let types = DynAny::member_types(&t);
        if values.len() != types.len() {
            return Err(DynAnyError::InvalidValue);
        }
        let mut items = Vec::with_capacity(values.len());
        for ((name, tc), (n, v)) in types.iter().zip(values) {
            if (!n.is_empty() && n != name) || !v.typecode().equivalent(tc) {
                return Err(DynAnyError::TypeMismatch);
            }
            items.push(
                self.child(tc, v.value())
                    .map_err(|_| DynAnyError::InvalidValue)?,
            );
        }
        *self.lock() = match t.kind() {
            TCKind::Value | TCKind::Event => State::value(Some(items)),
            _ => State::items(items),
        };
        Ok(())
    }

    /// `set_members_as_dyn_any`: [`set_members`](DynAny::set_members)
    /// with the values of the DynAnys.
    pub fn set_members_as_dyn_any(&self, values: &[(String, DynAny)]) -> Result<()> {
        let values = values
            .iter()
            .map(|(n, d)| Ok((n.clone(), d.to_any()?)))
            .collect::<Result<Vec<_>>>()?;
        self.set_members(&values)
    }

    // --- DynUnion ---------------------------------------------------------------

    /// `DynUnion::get_discriminator`: the discriminator component.
    pub fn get_discriminator(&self) -> Result<DynAny> {
        self.expect(&[TCKind::Union])?;
        self.sync_union();
        match &*self.lock() {
            State::Union { disc, .. } => Ok(disc.clone()),
            _ => Err(DynAnyError::TypeMismatch),
        }
    }

    /// `DynUnion::set_discriminator`: sets the discriminator to the value
    /// of `d` (`TypeMismatch` if its type is not equivalent to the
    /// discriminator type). The member it selects keeps its value if it
    /// was already active and gets its default value otherwise; the
    /// position becomes 1 if there is an active member, else 0.
    pub fn set_discriminator(&self, d: &DynAny) -> Result<()> {
        let t = self.expect(&[TCKind::Union])?;
        let disc_tc = t
            .discriminator_type()
            .map_err(|_| DynAnyError::TypeMismatch)?;
        let value = d.to_any()?;
        if !value.typecode().equivalent(&disc_tc) {
            return Err(DynAnyError::TypeMismatch);
        }
        self.set_disc_value(value.value().clone())
    }

    fn set_disc_value(&self, value: Value) -> Result<()> {
        let disc = self.get_discriminator()?;
        *disc.lock() = State::Basic(value);
        self.sync_union();
        if let State::Union { member, pos, .. } = &mut *self.lock() {
            *pos = if member.is_some() { 1 } else { 0 };
        }
        Ok(())
    }

    /// `DynUnion::set_to_default_member`: sets the discriminator to a
    /// value no label uses, which selects the default member.
    /// `TypeMismatch` if the union has no default member.
    pub fn set_to_default_member(&self) -> Result<()> {
        let t = self.expect(&[TCKind::Union])?;
        if t.default_index().unwrap_or(-1) < 0 {
            return Err(DynAnyError::TypeMismatch);
        }
        if self.is_default_selected(&t)? {
            if let State::Union { pos, .. } = &mut *self.lock() {
                *pos = 0;
            }
            return Ok(());
        }
        let value = unused_discriminator(&t).ok_or(DynAnyError::TypeMismatch)?;
        self.set_disc_value(value)?;
        if let State::Union { pos, .. } = &mut *self.lock() {
            *pos = 0;
        }
        Ok(())
    }

    fn is_default_selected(&self, t: &TypeCode) -> Result<bool> {
        self.sync_union();
        let current = match &*self.lock() {
            State::Union { member, .. } => member.as_ref().map(|(i, _)| *i as i32),
            _ => None,
        };
        Ok(current.is_some() && current == t.default_index().ok())
    }

    /// `DynUnion::set_to_no_active_member`: sets the discriminator to a
    /// value no label uses. `TypeMismatch` if the union has a default
    /// member or its labels use every value of the discriminator type.
    pub fn set_to_no_active_member(&self) -> Result<()> {
        let t = self.expect(&[TCKind::Union])?;
        if t.default_index().unwrap_or(-1) >= 0 {
            return Err(DynAnyError::TypeMismatch);
        }
        if !self.has_no_active_member()? {
            let value = unused_discriminator(&t).ok_or(DynAnyError::TypeMismatch)?;
            self.set_disc_value(value)?;
        }
        if let State::Union { pos, .. } = &mut *self.lock() {
            *pos = 0;
        }
        Ok(())
    }

    /// `DynUnion::has_no_active_member`.
    pub fn has_no_active_member(&self) -> Result<bool> {
        self.expect(&[TCKind::Union])?;
        self.sync_union();
        Ok(matches!(&*self.lock(), State::Union { member: None, .. }))
    }

    /// `DynUnion::discriminator_kind`: the kind of the discriminator
    /// type, aliases included.
    pub fn discriminator_kind(&self) -> Result<TCKind> {
        let t = self.expect(&[TCKind::Union])?;
        Ok(t.discriminator_type()
            .map_err(|_| DynAnyError::TypeMismatch)?
            .kind())
    }

    fn active_member(&self) -> Result<(u32, DynAny)> {
        self.expect(&[TCKind::Union])?;
        self.sync_union();
        match &*self.lock() {
            State::Union {
                member: Some((i, m)),
                ..
            } => Ok((*i, m.clone())),
            _ => Err(DynAnyError::InvalidValue),
        }
    }

    /// `DynUnion::member`: the active member. `InvalidValue` without one.
    pub fn member(&self) -> Result<DynAny> {
        Ok(self.active_member()?.1)
    }

    /// `DynUnion::member_name`: the name of the active member.
    /// `InvalidValue` without one.
    pub fn member_name(&self) -> Result<String> {
        let (i, _) = self.active_member()?;
        Ok(self
            .unaliased()
            .member_name(i)
            .unwrap_or_default()
            .to_owned())
    }

    /// `DynUnion::member_kind`: the kind of the type of the active member,
    /// aliases included. `InvalidValue` without one.
    pub fn member_kind(&self) -> Result<TCKind> {
        let (i, _) = self.active_member()?;
        Ok(member_type(&self.unaliased(), i)?.kind())
    }

    // --- DynSequence and DynArray -------------------------------------------------

    /// `DynSequence::get_length`.
    pub fn get_length(&self) -> Result<u32> {
        self.expect(&[TCKind::Sequence])?;
        self.component_count()
    }

    /// `DynSequence::set_length`: new elements get their default value.
    /// When the sequence grows and there was no current position, the
    /// first new element becomes current; when it shrinks past the
    /// current position, the position becomes -1. Longer than the bound
    /// is `InvalidValue`.
    pub fn set_length(&self, len: u32) -> Result<()> {
        let t = self.expect(&[TCKind::Sequence])?;
        self.check_length(&t, len as usize)?;
        let content = content_type(&t)?;
        let old = self.component_count()? as usize;
        let mut new_items = Vec::new();
        for _ in old..len as usize {
            new_items.push(self.default_child(&content)?);
        }
        if let State::Items { items, pos } = &mut *self.lock() {
            let len = len as usize;
            if len < items.len() {
                items.truncate(len);
                if *pos >= len as i32 {
                    *pos = -1;
                }
            } else if len > items.len() {
                if *pos < 0 {
                    *pos = items.len() as i32;
                }
                items.extend(new_items);
            }
        }
        Ok(())
    }

    /// `get_elements` of `DynSequence` and `DynArray`.
    pub fn get_elements(&self) -> Result<Vec<Any>> {
        self.get_elements_as_dyn_any()?
            .iter()
            .map(DynAny::to_any)
            .collect()
    }

    /// `get_elements_as_dyn_any`: the components.
    pub fn get_elements_as_dyn_any(&self) -> Result<Vec<DynAny>> {
        self.expect(&[TCKind::Sequence, TCKind::Array])?;
        self.member_items()
    }

    /// `set_elements` of `DynSequence` and `DynArray`: `TypeMismatch` if
    /// a value's type is not equivalent to the element type,
    /// `InvalidValue` if there are more than the bound of a sequence or
    /// not as many as an array has.
    pub fn set_elements(&self, values: &[Any]) -> Result<()> {
        let t = self.expect(&[TCKind::Sequence, TCKind::Array])?;
        let content = content_type(&t)?;
        if values.iter().any(|v| !v.typecode().equivalent(&content)) {
            return Err(DynAnyError::TypeMismatch);
        }
        self.check_length(&t, values.len())?;
        let items = values
            .iter()
            .map(|v| self.child(&content, v.value()))
            .collect::<Result<Vec<_>>>()
            .map_err(|_| DynAnyError::InvalidValue)?;
        *self.lock() = State::items(items);
        Ok(())
    }

    /// `set_elements_as_dyn_any`: [`set_elements`](DynAny::set_elements)
    /// with the values of the DynAnys.
    pub fn set_elements_as_dyn_any(&self, values: &[DynAny]) -> Result<()> {
        let values = values
            .iter()
            .map(DynAny::to_any)
            .collect::<Result<Vec<_>>>()?;
        self.set_elements(&values)
    }

    // --- DynValueCommon, DynValueBox -------------------------------------------------

    const VALUES: [TCKind; 3] = [TCKind::Value, TCKind::ValueBox, TCKind::Event];

    /// `DynValueCommon::is_null`.
    pub fn is_null(&self) -> Result<bool> {
        self.expect(&Self::VALUES)?;
        Ok(matches!(&*self.lock(), State::Value { members: None, .. }))
    }

    /// `DynValueCommon::set_to_null`.
    pub fn set_to_null(&self) -> Result<()> {
        self.expect(&Self::VALUES)?;
        *self.lock() = State::value(None);
        Ok(())
    }

    /// `DynValueCommon::set_to_value`: a `null` value gets members with
    /// their default values; a value that is not `null` is left as it is.
    pub fn set_to_value(&self) -> Result<()> {
        let t = self.expect(&Self::VALUES)?;
        if !self.is_null()? {
            return Ok(());
        }
        let members = state_types(&t)
            .iter()
            .map(|tc| self.default_child(tc))
            .collect::<Result<Vec<_>>>()?;
        *self.lock() = State::value(Some(members));
        Ok(())
    }

    /// `DynValueBox::get_boxed_value`. `InvalidValue` if `null`.
    pub fn get_boxed_value(&self) -> Result<Any> {
        self.get_boxed_value_as_dyn_any()?.to_any()
    }

    /// `DynValueBox::get_boxed_value_as_dyn_any`: the component.
    /// `InvalidValue` if `null`.
    pub fn get_boxed_value_as_dyn_any(&self) -> Result<DynAny> {
        self.expect(&[TCKind::ValueBox])?;
        match &*self.lock() {
            State::Value {
                members: Some(m), ..
            } => m.first().cloned().ok_or(DynAnyError::InvalidValue),
            _ => Err(DynAnyError::InvalidValue),
        }
    }

    /// `DynValueBox::set_boxed_value`: `TypeMismatch` if the type of
    /// `value` is not equivalent to the boxed type. A `null` value box
    /// is no longer `null`.
    pub fn set_boxed_value(&self, value: &Any) -> Result<()> {
        let t = self.expect(&[TCKind::ValueBox])?;
        let content = content_type(&t)?;
        if !value.typecode().equivalent(&content) {
            return Err(DynAnyError::TypeMismatch);
        }
        let item = self
            .child(&content, value.value())
            .map_err(|_| DynAnyError::InvalidValue)?;
        *self.lock() = State::value(Some(vec![item]));
        Ok(())
    }

    /// `DynValueBox::set_boxed_value_as_dyn_any`.
    pub fn set_boxed_value_as_dyn_any(&self, value: &DynAny) -> Result<()> {
        self.set_boxed_value(&value.to_any()?)
    }
}

/// Whether the members `i` and `j` of the union `t` are one member with
/// several labels.
fn same_member(t: &TypeCode, i: u32, j: u32) -> bool {
    let (Ok(a), Ok(b)) = (t.member_name(i), t.member_name(j)) else {
        return false;
    };
    let (Ok(ta), Ok(tb)) = (t.member_type(i), t.member_type(j)) else {
        return false;
    };
    let (lo, hi) = (i.min(j), i.max(j));
    // The labels of one member are next to each other.
    !a.is_empty() && a == b && ta.equal(&tb) && (lo..=hi).all(|k| t.member_name(k) == Ok(a))
}

/// The names of the state members of a valuetype, those of its base types
/// first, in the order of [`state_types`].
fn value_member_names(tc: &TypeCode) -> Vec<String> {
    let mut chain = vec![tc.clone()];
    while let Some(Some(base)) = chain.last().map(|t| t.concrete_base_type().ok().flatten()) {
        if chain.len() > MAX_DEPTH {
            break;
        }
        chain.push(base.unaliased());
    }
    chain
        .iter()
        .rev()
        .flat_map(|t| {
            (0..t.member_count().unwrap_or(0))
                .map(move |i| t.member_name(i).unwrap_or_default().to_owned())
        })
        .collect()
}

/// Whether an instance of `actual` may be the value of a DynAny of type
/// `expected`: the same type or a type derived from it.
fn is_assignable_to(actual: &TypeCode, expected: &TypeCode) -> bool {
    let id = expected.id().unwrap_or_default();
    let mut t = Some(actual.unaliased());
    let mut n = 0;
    while let Some(tc) = t {
        if tc.id().unwrap_or_default() == id {
            return true;
        }
        n += 1;
        if n > MAX_DEPTH {
            break;
        }
        t = tc.concrete_base_type().ok().flatten();
    }
    false
}
