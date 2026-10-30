//! IDL values in between Python and CDR.
//!
//! Marshalling goes through [`Value`] in two steps: Python objects are
//! checked and converted while attached to the interpreter, and the CDR
//! is written or read while detached from it, on whichever thread the ORB
//! uses. A request can then be sent, forwarded and sent again without
//! taking the GIL, and a servant thread holds the GIL only for the Python
//! part of an upcall.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Weak};

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList, PyString, PyTuple};
use rustorb_core::cdr::CdrEncode;
use rustorb_core::{CompletionStatus, InStream, OutStream, SystemException, ValueRef};

use crate::any;
use crate::error::{bad_param, corba, marshal, rt, sys_err};
use crate::orb::ObjRef;
use crate::tc::{Tc, TcCell, TcRef, value_type};

pub(crate) use rustorb_core::Value;

// --- valuetypes ------------------------------------------------------------------

/// The valuetypes converted while a scope is open, both ways, so that an
/// instance met twice is converted once (and sent once, as an
/// indirection), and cycles end.
#[derive(Default)]
struct Memo {
    /// By the address of the Python object, which the entry keeps alive.
    to_core: HashMap<usize, (Py<PyAny>, ValueRef)>,
    /// By [`ValueRef::key`], with the instance, kept alive likewise.
    to_py: HashMap<u64, (ValueRef, Py<PyAny>)>,
}

thread_local! {
    static MEMO: RefCell<Option<Memo>> = const { RefCell::new(None) };
}

/// Where the values of one request or reply are converted (arguments,
/// results, an exception). The outermost scope of a thread owns the memo;
/// when it ends, the instances made from Python objects are emptied, so
/// that cycles among them are freed.
pub(crate) struct ValueScope {
    owner: bool,
    release: bool,
}

impl ValueScope {
    /// Opens a scope whose values are not used after it ends.
    pub(crate) fn enter() -> ValueScope {
        ValueScope::open(true)
    }

    /// Opens a scope if none is, for a conversion outside any request.
    fn ensure() -> ValueScope {
        ValueScope::open(false)
    }

    fn open(release: bool) -> ValueScope {
        let owner = MEMO.with(|m| {
            let mut m = m.borrow_mut();
            let owner = m.is_none();
            if owner {
                *m = Some(Memo::default());
            }
            owner
        });
        ValueScope { owner, release }
    }
}

impl Drop for ValueScope {
    fn drop(&mut self) {
        if !self.owner {
            return;
        }
        let Some(memo) = MEMO.with(|m| m.borrow_mut().take()) else {
            return;
        };
        if self.release {
            for (_, r) in memo.to_core.values() {
                r.set_members(Vec::new());
            }
        }
    }
}

fn with_memo<R>(f: impl FnOnce(&mut Memo) -> R) -> R {
    MEMO.with(|m| f(m.borrow_mut().as_mut().expect("a value scope is open")))
}

/// A compiled valuetype: the declared one, or one found by repository id.
enum ValueTc<'a> {
    Borrowed(&'a Tc),
    Owned(TcRef),
}

impl Deref for ValueTc<'_> {
    type Target = Tc;

    fn deref(&self) -> &Tc {
        match self {
            ValueTc::Borrowed(tc) => tc,
            ValueTc::Owned(tc) => tc,
        }
    }
}

/// The valuetype `tc` stands for, through aliases and back references;
/// `None` if it is another type.
fn as_value_tc(tc: &Tc) -> Option<ValueTc<'_>> {
    match tc {
        Tc::Value { .. } => Some(ValueTc::Borrowed(tc)),
        Tc::Alias(content) => as_value_tc(content),
        Tc::Back(back) => {
            let target = back.upgrade()?;
            matches!(**target, Tc::Value { .. }).then_some(ValueTc::Owned(target))
        }
        _ => None,
    }
}

/// [`as_value_tc`] for a member type, which the result shares.
fn member_value_tc(tc: &TcRef) -> Option<ValueTc<'static>> {
    match &***tc {
        Tc::Value { .. } => Some(ValueTc::Owned(Arc::clone(tc))),
        Tc::Alias(content) => member_value_tc(content),
        Tc::Back(back) => {
            let target = back.upgrade()?;
            matches!(**target, Tc::Value { .. }).then_some(ValueTc::Owned(target))
        }
        _ => None,
    }
}

/// The compiled type of a value of repository id `id` sent where `tc` is
/// declared: `tc` itself, or the valuetype `id`.
fn actual_type<'a>(py: Python<'_>, tc: ValueTc<'a>, id: &str) -> PyResult<Option<ValueTc<'a>>> {
    match &*tc {
        Tc::Value { id: declared, .. } if declared == id => Ok(Some(tc)),
        _ => Ok(value_type(py, id)?.map(ValueTc::Owned)),
    }
}

/// A Python valuetype instance (or the value of a value box) as a core
/// value. `None` is the null value.
///
/// The instances it refers to are converted from a work list rather than
/// by recursion, so that long chains (linked lists) do not exhaust the
/// stack.
fn value_from_py(tc: &Tc, obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    let py = obj.py();
    if obj.is_none() {
        return Ok(Value::Valuetype(None));
    }
    if let Tc::ValueBox { core, content } = tc {
        let content = from_py(content, obj)?;
        return Ok(Value::Valuetype(Some(ValueRef::new(
            core.clone(),
            vec![content],
        ))));
    }
    let tc = as_value_tc(tc).ok_or_else(|| sys_err(py, &bad_param()))?;
    let _scope = ValueScope::ensure();
    let mut work = Vec::new();
    let first = core_instance(tc, obj, &mut work)?;
    while let Some((obj, r, tc)) = work.pop() {
        let Tc::Value { members, .. } = &*tc else {
            unreachable!("core_instance checks the type");
        };
        let mut values = Vec::with_capacity(members.len());
        for (name, mtc) in members {
            let member = obj.getattr(name.bind(py))?;
            values.push(match member_value_tc(mtc) {
                Some(vt) if !member.is_none() => {
                    Value::Valuetype(Some(core_instance(vt, &member, &mut work)?))
                }
                _ => from_py(mtc, &member)?,
            });
        }
        r.set_members(values);
    }
    Ok(Value::Valuetype(Some(first)))
}

/// The core instance for the Python instance `obj` declared as `tc`: the
/// one made before, or a new one whose members are left for `work`.
fn core_instance<'a, 'py>(
    tc: ValueTc<'a>,
    obj: &Bound<'py, PyAny>,
    work: &mut Vec<(Bound<'py, PyAny>, ValueRef, ValueTc<'a>)>,
) -> PyResult<ValueRef> {
    let py = obj.py();
    let key = obj.as_ptr() as usize;
    if let Some(r) = with_memo(|m| m.to_core.get(&key).map(|(_, r)| r.clone())) {
        return Ok(r);
    }
    let id: String = obj
        .getattr("_NP_RepositoryId")
        .and_then(|id| id.extract())
        .map_err(|_| sys_err(py, &bad_param()))?;
    let tc = actual_type(py, tc, &id)?.ok_or_else(|| sys_err(py, &bad_param()))?;
    let Tc::Value { core, .. } = &*tc else {
        return Err(sys_err(py, &bad_param()));
    };
    let r = ValueRef::new(core.clone(), Vec::new());
    with_memo(|m| m.to_core.insert(key, (obj.clone().unbind(), r.clone())));
    work.push((obj.clone(), r.clone(), tc));
    Ok(r)
}

/// A core valuetype instance (or value box) as Python: an instance made
/// by the value factory registered for its type (or, without one, of the
/// class of its type), with its state members set. Like
/// [`value_from_py`], it works from a list.
fn value_to_py<'py>(py: Python<'py>, tc: &Tc, v: Option<ValueRef>) -> PyResult<Bound<'py, PyAny>> {
    let Some(r) = v else {
        return Ok(py.None().into_bound(py));
    };
    if let Tc::ValueBox { content, .. } = tc {
        let mut members = r.members();
        if members.len() != 1 {
            return Err(sys_err(py, &bad_param()));
        }
        return to_py(py, content, members.remove(0));
    }
    let tc = as_value_tc(tc).ok_or_else(|| sys_err(py, &bad_param()))?;
    let _scope = ValueScope::ensure();
    let mut work = Vec::new();
    let first = py_instance(py, tc, &r, &mut work)?;
    while let Some((obj, values, tc)) = work.pop() {
        let Tc::Value { members, .. } = &*tc else {
            unreachable!("py_instance checks the type");
        };
        if values.len() != members.len() {
            return Err(sys_err(py, &bad_param()));
        }
        for ((name, mtc), v) in members.iter().zip(values) {
            let member = match (member_value_tc(mtc), v) {
                (Some(vt), Value::Valuetype(Some(r))) => py_instance(py, vt, &r, &mut work)?,
                (_, v) => to_py(py, mtc, v)?,
            };
            obj.setattr(name.bind(py), member)?;
        }
    }
    Ok(first)
}

/// The Python instance for the core instance `r` declared as `tc`: the
/// one made before, or a new one whose members are left for `work`.
fn py_instance<'a, 'py>(
    py: Python<'py>,
    tc: ValueTc<'a>,
    r: &ValueRef,
    work: &mut Vec<(Bound<'py, PyAny>, Vec<Value>, ValueTc<'a>)>,
) -> PyResult<Bound<'py, PyAny>> {
    if let Some(obj) = with_memo(|m| m.to_py.get(&r.key()).map(|(_, o)| o.clone_ref(py))) {
        return Ok(obj.into_bound(py));
    }
    let id = r.id().to_owned();
    let no_factory = || sys_err(py, &marshal(CompletionStatus::No));
    let tc = actual_type(py, tc, &id)?.ok_or_else(no_factory)?;
    let Tc::Value { class, .. } = &*tc else {
        return Err(sys_err(py, &bad_param()));
    };
    let factory = rt(py)?.call_method1("lookup_value_factory", (&id,))?;
    let obj = if !factory.is_none() {
        factory.call0()?
    } else if let Some(class) = class {
        let class = class.bind(py);
        class.call_method1("__new__", (class,))?
    } else {
        return Err(no_factory());
    };
    with_memo(|m| m.to_py.insert(r.key(), (r.clone(), obj.clone().unbind())));
    // The members are taken: what they refer to is converted once, and
    // the core instances are freed even when they form a cycle.
    work.push((obj.clone(), r.take_members(), tc));
    Ok(obj)
}

/// The entry of a union TypeCode the discriminator selects.
fn union_entry(labels: &[Option<i128>], default: Option<usize>, disc: &Value) -> Option<usize> {
    let key = disc.label_key();
    labels
        .iter()
        .position(|l| l.is_some() && *l == key)
        .or(default)
}

/// The type a back reference of a recursive type points to.
fn target(back: &Weak<TcCell>) -> Result<Arc<TcCell>, SystemException> {
    // The enclosing type holds the target alive while it is in use.
    back.upgrade().ok_or_else(|| {
        SystemException::new(
            rustorb_core::SystemExceptionKind::INTERNAL,
            0,
            CompletionStatus::No,
        )
    })
}

// --- Python to Value -----------------------------------------------------------

/// Converts a Python value of type `tc`. Errors are Python exceptions; the
/// caller turns those that are not CORBA exceptions into `BAD_PARAM`.
pub(crate) fn from_py(tc: &Tc, obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    let py = obj.py();
    Ok(match tc {
        Tc::Back(back) => from_py(&**target(back).map_err(|e| sys_err(py, &e))?, obj)?,
        Tc::Any => Value::Any(Box::new(any::any_from_py(obj)?)),
        Tc::TypeCode => Value::TypeCode(any::tc_from_py(obj)?),
        Tc::Void => Value::Void,
        Tc::Short => Value::Short(obj.extract()?),
        Tc::Long => Value::Long(obj.extract()?),
        Tc::UShort => Value::UShort(obj.extract()?),
        Tc::ULong => Value::ULong(obj.extract()?),
        Tc::LongLong => Value::LongLong(obj.extract()?),
        Tc::ULongLong => Value::ULongLong(obj.extract()?),
        Tc::Float => Value::Float(obj.extract::<f64>()? as f32),
        Tc::Double => Value::Double(obj.extract()?),
        Tc::Boolean => Value::Bool(obj.is_truthy()?),
        Tc::Octet => Value::Octet(obj.extract()?),
        Tc::Char => Value::Char(one_char(obj)?),
        Tc::WChar => {
            let c = u32::from(one_char(obj)?);
            Value::WChar(u16::try_from(c).map_err(|_| sys_err(py, &bad_param()))?)
        }
        Tc::String(bound) | Tc::WString(bound) => {
            let s: String = obj.extract()?;
            check_bound(py, s.chars().count(), *bound)?;
            Value::String(s)
        }
        Tc::Fixed { digits, scale } => fixed_from_py(obj, *digits, *scale)?,
        Tc::Sequence { content, bound } => {
            list_from_py(content, obj, |len| check_bound(py, len, *bound))?
        }
        Tc::Array { content, length } => list_from_py(content, obj, |len| {
            if len == *length as usize {
                Ok(())
            } else {
                Err(sys_err(py, &bad_param()))
            }
        })?,
        Tc::Alias(content) => from_py(content, obj)?,
        Tc::Struct { members, .. } => {
            let mut values = Vec::with_capacity(members.len());
            for (name, mtc) in members {
                values.push(from_py(mtc, &obj.getattr(name.bind(py))?)?);
            }
            Value::List(values)
        }
        Tc::Union {
            discriminator,
            labels,
            members,
            default,
            ..
        } => {
            let disc = from_py(discriminator, &obj.getattr("_d")?)?;
            let member = match union_entry(labels, *default, &disc) {
                Some(i) => Some((
                    i as u32,
                    Box::new(from_py(&members[i], &obj.getattr("_v")?)?),
                )),
                None => None,
            };
            Value::Union {
                disc: Box::new(disc),
                member,
            }
        }
        Tc::Enum { id, count, .. } => {
            if let Ok(parent) = obj.getattr("_parent_id") {
                if parent.extract::<String>()? != *id {
                    return Err(sys_err(py, &bad_param()));
                }
            }
            let v: u32 = obj.getattr("_v")?.extract()?;
            if v >= *count {
                return Err(sys_err(py, &bad_param()));
            }
            Value::Enum(v)
        }
        Tc::Objref { .. } => {
            if obj.is_none() {
                Value::Objref(None)
            } else {
                let native = obj.getattr("_obj")?;
                let native = native
                    .cast::<ObjRef>()
                    .map_err(|_| sys_err(py, &bad_param()))?;
                Value::Objref(Some(native.get().obj.clone()))
            }
        }
        Tc::Value { .. } | Tc::ValueBox { .. } => value_from_py(tc, obj)?,
        Tc::Unsupported(kind) => return Err(unsupported(py, kind)),
    })
}

/// A sequence or an array, whose length `check_len` checks. Sequences and
/// arrays of octet are `bytes` (a list of integers is accepted too), those
/// of char are `str` (or `bytes`); others are lists or tuples.
fn list_from_py(
    content: &Tc,
    obj: &Bound<'_, PyAny>,
    check_len: impl Fn(usize) -> PyResult<()>,
) -> PyResult<Value> {
    let py = obj.py();
    Ok(match content.unaliased() {
        Tc::Octet => {
            let bytes: Vec<u8> = if let Ok(b) = obj.cast::<PyBytes>() {
                b.as_bytes().to_vec()
            } else if obj.is_instance_of::<PyString>() {
                return Err(sys_err(py, &bad_param()));
            } else {
                obj.extract()?
            };
            check_len(bytes.len())?;
            Value::Octets(bytes.into())
        }
        Tc::Char => {
            let s: String = if let Ok(b) = obj.cast::<PyBytes>() {
                b.as_bytes().iter().map(|&c| char::from(c)).collect()
            } else {
                obj.extract()?
            };
            check_len(s.chars().count())?;
            Value::Chars(s)
        }
        _ => {
            if obj.is_instance_of::<PyString>() || obj.is_instance_of::<PyBytes>() {
                return Err(sys_err(py, &bad_param()));
            }
            let mut values = Vec::new();
            for item in obj.try_iter()? {
                values.push(from_py(content, &item?)?);
            }
            check_len(values.len())?;
            Value::List(values)
        }
    })
}

fn one_char(obj: &Bound<'_, PyAny>) -> PyResult<char> {
    let s = obj.cast::<PyString>()?.to_cow()?;
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(c),
        _ => Err(sys_err(obj.py(), &bad_param())),
    }
}

fn check_bound(py: Python<'_>, len: usize, bound: u32) -> PyResult<()> {
    if bound != 0 && len > bound as usize {
        return Err(sys_err(py, &bad_param()));
    }
    Ok(())
}

fn unsupported(py: Python<'_>, kind: &str) -> PyErr {
    let _ = kind;
    sys_err(
        py,
        &SystemException::new(
            rustorb_core::SystemExceptionKind::NO_IMPLEMENT,
            0,
            CompletionStatus::No,
        ),
    )
}

/// A `CORBA.fixed` (or any number) as the digits of `fixed<digits, scale>`.
fn fixed_from_py(obj: &Bound<'_, PyAny>, digits: u16, scale: i16) -> PyResult<Value> {
    let py = obj.py();
    let decimal = py
        .import("decimal")?
        .getattr("Decimal")?
        .call1((obj.str()?,))?;
    let text: String = decimal.call_method1("__format__", ("f",))?.extract()?;
    let (negative, text) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.as_str()),
    };
    let (int, frac) = text.split_once('.').unwrap_or((text, ""));
    let scale = usize::try_from(scale).map_err(|_| sys_err(py, &bad_param()))?;
    let frac = frac.trim_end_matches('0');
    if frac.len() > scale {
        return Err(sys_err(py, &bad_param()));
    }
    let int = int.trim_start_matches('0');
    let width = digits as usize;
    if int.len() + scale > width {
        return Err(sys_err(py, &bad_param()));
    }
    let mut all = "0".repeat(width - scale - int.len());
    all.push_str(int);
    all.push_str(frac);
    all.push_str(&"0".repeat(scale - frac.len()));
    let digits: Vec<u8> = all.bytes().map(|b| b - b'0').collect();
    let negative = negative && digits.iter().any(|&d| d != 0);
    Ok(Value::Fixed { digits, negative })
}

// --- Value to Python -----------------------------------------------------------

/// Converts a value of type `tc` to Python.
pub(crate) fn to_py<'py>(py: Python<'py>, tc: &Tc, value: Value) -> PyResult<Bound<'py, PyAny>> {
    Ok(match (tc, value) {
        (Tc::Back(back), v) => to_py(py, &**target(back).map_err(|e| sys_err(py, &e))?, v)?,
        (Tc::Any, Value::Any(a)) => any::any_to_py(py, *a)?,
        (Tc::TypeCode, Value::TypeCode(t)) => any::tc_to_py(py, &t)?,
        (Tc::Void, _) => py.None().into_bound(py),
        (Tc::Alias(content), v) => to_py(py, content, v)?,
        (_, Value::Bool(v)) => v.into_pyobject(py)?.to_owned().into_any(),
        (_, Value::Octet(v)) => v.into_pyobject(py)?.into_any(),
        (_, Value::Short(v)) => v.into_pyobject(py)?.into_any(),
        (_, Value::UShort(v)) => v.into_pyobject(py)?.into_any(),
        (_, Value::Long(v)) => v.into_pyobject(py)?.into_any(),
        (_, Value::ULong(v)) => v.into_pyobject(py)?.into_any(),
        (_, Value::LongLong(v)) => v.into_pyobject(py)?.into_any(),
        (_, Value::ULongLong(v)) => v.into_pyobject(py)?.into_any(),
        (_, Value::Float(v)) => f64::from(v).into_pyobject(py)?.into_any(),
        (_, Value::Double(v)) => v.into_pyobject(py)?.into_any(),
        (_, Value::Char(c)) => PyString::new(py, c.encode_utf8(&mut [0; 4])).into_any(),
        (_, Value::WChar(c)) => py
            .import("builtins")?
            .getattr("chr")?
            .call1((u32::from(c),))?,
        (_, Value::String(s)) | (_, Value::Chars(s)) => PyString::new(py, &s).into_any(),
        (_, Value::Octets(b)) => PyBytes::new(py, &b).into_any(),
        (Tc::Sequence { content, .. } | Tc::Array { content, .. }, Value::List(items)) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(to_py(py, content, item)?)?;
            }
            list.into_any()
        }
        (Tc::Struct { class, members, .. }, Value::List(items)) => {
            let mut args = Vec::with_capacity(items.len());
            for ((_, mtc), item) in members.iter().zip(items) {
                args.push(to_py(py, mtc, item)?);
            }
            class.bind(py).call1(PyTuple::new(py, args)?)?
        }
        (
            Tc::Union {
                class,
                discriminator,
                members,
                ..
            },
            Value::Union { disc, member },
        ) => {
            let d = to_py(py, discriminator, *disc)?;
            let v = match member {
                Some((i, v)) => to_py(py, &members[i as usize], *v)?,
                None => py.None().into_bound(py),
            };
            class.bind(py).call1((d, v))?
        }
        (Tc::Enum { items, .. }, Value::Enum(v)) => items.bind(py).get_item(v as usize)?,
        (Tc::Fixed { scale, .. }, Value::Fixed { digits, negative }) => {
            let text = fixed_text(&digits, *scale, negative);
            corba(py)?.getattr("fixed")?.call1((text,))?
        }
        (Tc::Objref { id }, Value::Objref(obj)) => match obj {
            None => py.None().into_bound(py),
            Some(obj) => rt(py)?
                .getattr("make_objref")?
                .call1((ObjRef { obj }, id.as_str()))?,
        },
        (Tc::Value { .. } | Tc::ValueBox { .. }, Value::Valuetype(v)) => value_to_py(py, tc, v)?,
        (Tc::Unsupported(kind), _) => return Err(unsupported(py, kind)),
        _ => return Err(sys_err(py, &bad_param())),
    })
}

fn fixed_text(digits: &[u8], scale: i16, negative: bool) -> String {
    let scale = usize::try_from(scale).unwrap_or(0).min(digits.len());
    let split = digits.len() - scale;
    let mut text = String::new();
    if negative {
        text.push('-');
    }
    let int: String = digits[..split]
        .iter()
        .map(|d| char::from(b'0' + d))
        .collect();
    let int = int.trim_start_matches('0');
    text.push_str(if int.is_empty() { "0" } else { int });
    if scale > 0 {
        text.push('.');
        text.extend(digits[split..].iter().map(|d| char::from(b'0' + d)));
    }
    text
}

// --- CDR -------------------------------------------------------------------------

fn mismatch() -> SystemException {
    // A Value that does not fit its TypeCode: from_py made it, so this
    // cannot happen.
    SystemException::new(
        rustorb_core::SystemExceptionKind::INTERNAL,
        0,
        CompletionStatus::No,
    )
}

/// Writes `value` of type `tc`.
pub(crate) fn encode(
    tc: &Tc,
    value: &Value,
    out: &mut OutStream<'_>,
) -> Result<(), SystemException> {
    match (tc, value) {
        (Tc::Back(back), v) => encode(&**target(back)?, v, out)?,
        (Tc::Any, Value::Any(a)) => out.write_any(a)?,
        (Tc::TypeCode, Value::TypeCode(t)) => out.write_typecode(t)?,
        (Tc::Void, _) => {}
        (Tc::Alias(content), v) => encode(content, v, out)?,
        (_, Value::Bool(v)) => v.encode(out)?,
        (_, Value::Octet(v)) => v.encode(out)?,
        (_, Value::Short(v)) => v.encode(out)?,
        (_, Value::UShort(v)) => v.encode(out)?,
        (_, Value::Long(v)) => v.encode(out)?,
        (_, Value::ULong(v)) => v.encode(out)?,
        (_, Value::LongLong(v)) => v.encode(out)?,
        (_, Value::ULongLong(v)) => v.encode(out)?,
        (_, Value::Float(v)) => v.encode(out)?,
        (_, Value::Double(v)) => v.encode(out)?,
        (_, Value::Char(c)) => {
            let bytes = out.code_sets().encode_string(c.encode_utf8(&mut [0; 4]))?;
            match bytes[..] {
                [b] => out.write_char(b),
                _ => return Err(data_conversion()),
            }
        }
        (_, Value::WChar(c)) => out.write_wchar(*c)?,
        (Tc::String(_), Value::String(s)) => out.write_string(s)?,
        (Tc::WString(_), Value::String(s)) => out.write_wstring(s)?,
        (Tc::Sequence { .. }, Value::Octets(b)) => out.write_octet_seq_shared(b)?,
        (Tc::Array { .. }, Value::Octets(b)) => out.write_octets(b),
        (Tc::Sequence { .. } | Tc::Array { .. }, Value::Chars(s)) => {
            let bytes = out.code_sets().encode_string(s)?;
            if bytes.len() != s.chars().count() {
                return Err(data_conversion());
            }
            if matches!(tc, Tc::Sequence { .. }) {
                out.write_len(bytes.len())?;
            }
            out.write_octets(&bytes);
        }
        (Tc::Sequence { content, .. }, Value::List(items)) => {
            out.write_len(items.len())?;
            for item in items {
                encode(content, item, out)?;
            }
        }
        (Tc::Array { content, .. }, Value::List(items)) => {
            for item in items {
                encode(content, item, out)?;
            }
        }
        (Tc::Struct { members, .. }, Value::List(items)) => {
            for ((_, mtc), item) in members.iter().zip(items) {
                encode(mtc, item, out)?;
            }
        }
        (
            Tc::Union {
                discriminator,
                members,
                ..
            },
            Value::Union { disc, member },
        ) => {
            encode(discriminator, disc, out)?;
            if let Some((i, v)) = member {
                encode(&members[*i as usize], v, out)?;
            }
        }
        (Tc::Enum { .. }, Value::Enum(v)) => v.encode(out)?,
        (Tc::Fixed { .. }, Value::Fixed { digits, negative }) => {
            // Packed decimal: two digits per octet, a leading zero digit
            // when the count is even, and the sign in the last half octet.
            let mut nibbles = Vec::with_capacity(digits.len() + 2);
            if digits.len() % 2 == 0 {
                nibbles.push(0);
            }
            nibbles.extend_from_slice(digits);
            nibbles.push(if *negative { 0xd } else { 0xc });
            let bytes: Vec<u8> = nibbles.chunks(2).map(|p| (p[0] << 4) | p[1]).collect();
            out.write_octets(&bytes);
        }
        (Tc::Objref { .. }, Value::Objref(obj)) => out.write_object(obj.as_ref())?,
        (Tc::Value { core, .. } | Tc::ValueBox { core, .. }, v) => out.write_value(core, v)?,
        _ => return Err(mismatch()),
    }
    Ok(())
}

fn data_conversion() -> SystemException {
    SystemException::new(
        rustorb_core::SystemExceptionKind::DATA_CONVERSION,
        0,
        CompletionStatus::No,
    )
}

/// Reads a value of type `tc`. Errors carry `completed`, the completion
/// status of the stream (yes for a reply, no for request arguments).
pub(crate) fn decode(
    tc: &Tc,
    input: &mut InStream<'_>,
    completed: CompletionStatus,
) -> Result<Value, SystemException> {
    let m = |_| marshal(completed);
    Ok(match tc {
        Tc::Back(back) => decode(&**target(back)?, input, completed)?,
        Tc::Any => Value::Any(Box::new(input.read_any()?)),
        Tc::TypeCode => Value::TypeCode(input.read_typecode()?),
        Tc::Void => Value::Void,
        Tc::Alias(content) => decode(content, input, completed)?,
        Tc::Short => Value::Short(input.read()?),
        Tc::Long => Value::Long(input.read()?),
        Tc::UShort => Value::UShort(input.read()?),
        Tc::ULong => Value::ULong(input.read()?),
        Tc::LongLong => Value::LongLong(input.read()?),
        Tc::ULongLong => Value::ULongLong(input.read()?),
        Tc::Float => Value::Float(input.read()?),
        Tc::Double => Value::Double(input.read()?),
        Tc::Boolean => Value::Bool(input.read()?),
        Tc::Octet => Value::Octet(input.read()?),
        Tc::Char => {
            let b = input.read_char().map_err(m)?;
            let s = input.code_sets().decode_string(&[b]).map_err(|mut e| {
                e.completed = completed;
                e
            })?;
            Value::Char(s.chars().next().ok_or_else(|| marshal(completed))?)
        }
        Tc::WChar => Value::WChar(input.read_wchar()?),
        Tc::String(bound) => {
            let s = input.read_string()?;
            check_read_bound(s.chars().count(), *bound, completed)?;
            Value::String(s)
        }
        Tc::WString(bound) => {
            let s = input.read_wstring()?;
            check_read_bound(s.chars().count(), *bound, completed)?;
            Value::String(s)
        }
        Tc::Fixed { digits, .. } => {
            let n = *digits as usize;
            let bytes = input.read_octets(n / 2 + 1).map_err(m)?;
            let mut nibbles: Vec<u8> = bytes.iter().flat_map(|b| [b >> 4, b & 0xf]).collect();
            let sign = nibbles.pop().unwrap_or(0xc);
            if n % 2 == 0 {
                nibbles.remove(0);
            }
            if nibbles.iter().any(|&d| d > 9) {
                return Err(marshal(completed));
            }
            Value::Fixed {
                digits: nibbles,
                negative: sign == 0xd,
            }
        }
        Tc::Sequence { content, bound } => {
            let len = input.read_len(content.min_size().max(1)).map_err(m)?;
            check_read_bound(len, *bound, completed)?;
            decode_items(content, input, len, completed)?
        }
        Tc::Array { content, length } => decode_items(content, input, *length as usize, completed)?,
        Tc::Struct { members, .. } => {
            let mut values = Vec::with_capacity(members.len());
            for (_, mtc) in members {
                values.push(decode(mtc, input, completed)?);
            }
            Value::List(values)
        }
        Tc::Union {
            discriminator,
            labels,
            members,
            default,
            ..
        } => {
            let disc = decode(discriminator, input, completed)?;
            let member = match union_entry(labels, *default, &disc) {
                Some(i) => Some((i as u32, Box::new(decode(&members[i], input, completed)?))),
                None => None,
            };
            Value::Union {
                disc: Box::new(disc),
                member,
            }
        }
        Tc::Enum { count, .. } => {
            let v: u32 = input.read()?;
            if v >= *count {
                return Err(marshal(completed));
            }
            Value::Enum(v)
        }
        Tc::Objref { .. } => Value::Objref(input.read_object()?),
        Tc::Value { core, .. } | Tc::ValueBox { core, .. } => input.read_value(core)?,
        Tc::Unsupported(_) => {
            return Err(SystemException::new(
                rustorb_core::SystemExceptionKind::NO_IMPLEMENT,
                0,
                completed,
            ));
        }
    })
}

fn decode_items(
    content: &Tc,
    input: &mut InStream<'_>,
    len: usize,
    completed: CompletionStatus,
) -> Result<Value, SystemException> {
    let m = |_| marshal(completed);
    Ok(match content.unaliased() {
        Tc::Octet => Value::Octets(input.read_octets_shared(len).map_err(m)?),
        Tc::Char => {
            let bytes = input.read_octets(len).map_err(m)?;
            Value::Chars(input.code_sets().decode_string(bytes).map_err(|mut e| {
                e.completed = completed;
                e
            })?)
        }
        _ => {
            let mut items = Vec::with_capacity(len.min(4096));
            for _ in 0..len {
                items.push(decode(content, input, completed)?);
            }
            Value::List(items)
        }
    })
}

fn check_read_bound(
    len: usize,
    bound: u32,
    completed: CompletionStatus,
) -> Result<(), SystemException> {
    if bound != 0 && len > bound as usize {
        return Err(marshal(completed));
    }
    Ok(())
}
