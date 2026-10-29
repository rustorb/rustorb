//! TypeCodes of the Python runtime (`rustorb.CORBA.TypeCode`) compiled
//! into a Rust form that the marshalling code walks.
//!
//! Generated code describes every type with a `CORBA.TypeCode`. Reading its
//! attributes for every value would be slow, so the TypeCodes an operation
//! uses are compiled once into [`Tc`] trees and kept, with the operation, in
//! [`CompiledOp`]. A recursive type refers back to its enclosing type with a
//! weak [`Tc::Back`], so that the TypeCodes compiled for the value of an
//! `any` (which are not kept) are freed with it.

use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use pyo3::prelude::*;
use pyo3::types::{PyString, PyTuple};

use rustorb_core::TypeCode;

use crate::any::tc_from_py;
use crate::error::{bad_typecode, sys_err};

/// A compiled TypeCode, shared by the types that contain it.
pub(crate) type TcRef = Arc<TcCell>;

/// A [`Tc`] that is filled in after it is created, so that recursive types
/// can refer to themselves.
#[derive(Debug, Default)]
pub(crate) struct TcCell(OnceLock<Tc>);

impl Deref for TcCell {
    type Target = Tc;

    fn deref(&self) -> &Tc {
        self.0.get().expect("TypeCode used before it was compiled")
    }
}

/// The TCKind values, in the order of `CORBA.TCKind`.
mod kind {
    pub const NULL: u32 = 0;
    pub const VOID: u32 = 1;
    pub const SHORT: u32 = 2;
    pub const LONG: u32 = 3;
    pub const USHORT: u32 = 4;
    pub const ULONG: u32 = 5;
    pub const FLOAT: u32 = 6;
    pub const DOUBLE: u32 = 7;
    pub const BOOLEAN: u32 = 8;
    pub const CHAR: u32 = 9;
    pub const OCTET: u32 = 10;
    pub const ANY: u32 = 11;
    pub const TYPECODE: u32 = 12;
    pub const OBJREF: u32 = 14;
    pub const STRUCT: u32 = 15;
    pub const UNION: u32 = 16;
    pub const ENUM: u32 = 17;
    pub const STRING: u32 = 18;
    pub const SEQUENCE: u32 = 19;
    pub const ARRAY: u32 = 20;
    pub const ALIAS: u32 = 21;
    pub const EXCEPT: u32 = 22;
    pub const LONGLONG: u32 = 23;
    pub const ULONGLONG: u32 = 24;
    pub const WCHAR: u32 = 26;
    pub const WSTRING: u32 = 27;
    pub const FIXED: u32 = 28;
    pub const VALUE: u32 = 29;
    pub const VALUE_BOX: u32 = 30;
    pub const ABSTRACT_INTERFACE: u32 = 32;
    pub const LOCAL_INTERFACE: u32 = 33;
}

/// A type, as the marshalling code needs it.
#[derive(Debug)]
pub(crate) enum Tc {
    /// `void` and `null`: no data.
    Void,
    Short,
    Long,
    UShort,
    ULong,
    LongLong,
    ULongLong,
    Float,
    Double,
    Boolean,
    Char,
    WChar,
    Octet,
    /// `string`, with its bound (0 for none).
    String(u32),
    /// `wstring`, with its bound (0 for none).
    WString(u32),
    Fixed {
        digits: u16,
        scale: i16,
    },
    Sequence {
        content: TcRef,
        bound: u32,
    },
    Array {
        content: TcRef,
        length: u32,
    },
    Alias(TcRef),
    /// A struct or an exception: the Python class and the members, with
    /// the names of their Python attributes.
    Struct {
        class: Py<PyAny>,
        members: Vec<(Py<PyString>, TcRef)>,
    },
    /// A union. `entries` are the case labels of the TypeCode, each with
    /// its member; `default` is the index of the `default` entry.
    Union {
        class: Py<PyAny>,
        discriminator: TcRef,
        labels: Vec<Option<i128>>,
        members: Vec<TcRef>,
        default: Option<usize>,
    },
    /// An enum: its enumerators (`EnumItem`s) in order.
    Enum {
        id: String,
        items: Py<PyTuple>,
        count: u32,
    },
    /// An object reference to the interface `id` (an abstract or local
    /// interface is marshalled the same way).
    Objref {
        id: String,
    },
    /// A valuetype: its repository id, the class of its values (if it
    /// has one), its core TypeCode, which marshals it, and its state
    /// members, those of its base types first, with the names of their
    /// Python attributes.
    Value {
        id: String,
        class: Option<Py<PyAny>>,
        core: TypeCode,
        members: Vec<(Py<PyString>, TcRef)>,
    },
    /// A value box: its core TypeCode and the boxed type.
    ValueBox {
        core: TypeCode,
        content: TcRef,
    },
    /// `any`.
    Any,
    /// `TypeCode`.
    TypeCode,
    /// A reference back to an enclosing type of a recursive type, which
    /// holds it alive.
    Back(Weak<TcCell>),
    /// A type the binding cannot marshal (valuetypes, `long double`...).
    Unsupported(String),
}

impl Tc {
    /// The type with the aliases removed.
    pub(crate) fn unaliased(&self) -> &Tc {
        let mut tc = self;
        while let Tc::Alias(content) = tc {
            tc = content;
        }
        tc
    }

    /// The smallest size of a value on the wire, to check sequence lengths
    /// before allocating.
    pub(crate) fn min_size(&self) -> usize {
        match self.unaliased() {
            Tc::Void | Tc::Struct { .. } | Tc::Union { .. } | Tc::Back(_) | Tc::Unsupported(_) => 0,
            Tc::Array { content, length } => content.min_size().saturating_mul(*length as usize),
            Tc::Boolean | Tc::Char | Tc::Octet | Tc::WChar => 1,
            Tc::Short | Tc::UShort => 2,
            Tc::LongLong | Tc::ULongLong | Tc::Double => 8,
            Tc::Fixed { digits, .. } => (*digits as usize + 2) / 2,
            _ => 4,
        }
    }
}

/// An operation of an interface, compiled from a `_rt.Operation`.
#[pyclass(frozen, module = "rustorb._native")]
#[derive(Debug)]
pub(crate) struct CompiledOp {
    pub(crate) name: String,
    /// The name of the servant method: the operation name, with a leading
    /// `_` for a Python keyword.
    pub(crate) method: String,
    pub(crate) in_types: Vec<TcRef>,
    pub(crate) out_types: Vec<TcRef>,
    /// The user exceptions: repository id and type.
    pub(crate) exceptions: Vec<(String, TcRef)>,
    pub(crate) oneway: bool,
}

/// Compiled TypeCodes by the address of the Python object, which the entry
/// keeps alive so that the address is not reused.
static CACHE: Mutex<Option<Compiled>> = Mutex::new(None);

/// Compiled TypeCodes by the address of the Python object, with the object.
type Compiled = HashMap<usize, (Py<PyAny>, TcRef)>;

/// The compiled form of `op` (a `_rt.Operation`), made once and kept in its
/// `_native` slot.
pub(crate) fn compile_op<'py>(op: &Bound<'py, PyAny>) -> PyResult<Bound<'py, CompiledOp>> {
    let py = op.py();
    let cached = op.getattr("_native")?;
    if let Ok(compiled) = cached.cast_into::<CompiledOp>() {
        return Ok(compiled);
    }
    let mut c = Compiler::default();
    let name: String = op.getattr("name")?.extract()?;
    let keyword = py
        .import("keyword")?
        .getattr("iskeyword")?
        .call1((&name,))?;
    let method = if keyword.is_truthy()? {
        format!("_{name}")
    } else {
        name.clone()
    };
    let in_types = c.list(&op.getattr("in_types")?)?;
    let out_types = c.list(&op.getattr("out_types")?)?;
    let mut exceptions = Vec::new();
    for tc in op.getattr("exceptions")?.try_iter()? {
        let tc = tc?;
        let id: String = tc.getattr("_id")?.extract()?;
        exceptions.push((id, c.tc(&tc)?));
    }
    let oneway = op.getattr("oneway")?.is_truthy()?;
    c.commit();
    let compiled = Bound::new(
        py,
        CompiledOp {
            name,
            method,
            in_types,
            out_types,
            exceptions,
            oneway,
        },
    )?;
    op.setattr("_native", &compiled)?;
    Ok(compiled)
}

/// Compiled valuetypes by repository id, for values whose type is not the
/// declared one: those registered by generated code, and those found in
/// `_rt.typecodes` when first met.
static VALUE_TYPES: Mutex<Option<HashMap<String, TcRef>>> = Mutex::new(None);

/// Registers the valuetype `tc` (a `CORBA.TypeCode`): values of it can be
/// sent where a base type is declared, and read where one is expected.
pub(crate) fn register_value_type(tc: &Bound<'_, PyAny>) -> PyResult<()> {
    let core = tc_from_py(tc)?;
    let mut c = Compiler::default();
    let compiled = c.tc(tc)?;
    c.commit();
    let id: String = tc.getattr("_id")?.extract()?;
    rustorb_core::register_value_type(&core);
    VALUE_TYPES
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(id, compiled);
    Ok(())
}

/// The compiled valuetype `id`: registered, or defined by generated code.
pub(crate) fn value_type(py: Python<'_>, id: &str) -> PyResult<Option<TcRef>> {
    if let Some(tc) = VALUE_TYPES.lock().unwrap().as_ref().and_then(|m| m.get(id)) {
        return Ok(Some(Arc::clone(tc)));
    }
    let typecodes = crate::error::rt(py)?.getattr("typecodes")?;
    let tc = typecodes.call_method1("get", (id,))?;
    if tc.is_none() || tc.getattr("_k")?.getattr("_v")?.extract::<u32>()? != kind::VALUE {
        return Ok(None);
    }
    register_value_type(&tc)?;
    Ok(VALUE_TYPES
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(id))
        .cloned())
}

/// Compiles TypeCodes, sharing the ones already compiled. The new ones go to
/// the cache only when the whole operation compiled, so that a failure (a
/// struct that was declared but never defined) leaves no half-filled cells.
#[derive(Default)]
struct Compiler {
    new: Compiled,
    /// The TypeCodes being compiled, which a reference to is a reference
    /// back to an enclosing type.
    open: HashSet<usize>,
}

/// Compiles the TypeCode of the value of an `any`. The result is not
/// cached (such TypeCodes are often made for one value), except for the
/// parts that operations already compiled.
pub(crate) fn compile_tc(tc: &Bound<'_, PyAny>) -> PyResult<TcRef> {
    Compiler::default().tc(tc)
}

impl Compiler {
    fn commit(self) {
        let mut cache = CACHE.lock().unwrap();
        cache.get_or_insert_with(HashMap::new).extend(self.new);
    }

    fn list(&mut self, tcs: &Bound<'_, PyAny>) -> PyResult<Vec<TcRef>> {
        tcs.try_iter()?.map(|tc| self.tc(&tc?)).collect()
    }

    fn tc(&mut self, tc: &Bound<'_, PyAny>) -> PyResult<TcRef> {
        let key = tc.as_ptr() as usize;
        if let Some((_, cell)) = self.new.get(&key) {
            if self.open.contains(&key) {
                let back = TcCell::default();
                back.0
                    .set(Tc::Back(Arc::downgrade(cell)))
                    .expect("a new cell is empty");
                return Ok(Arc::new(back));
            }
            return Ok(Arc::clone(cell));
        }
        if let Some((_, cell)) = CACHE.lock().unwrap().as_ref().and_then(|c| c.get(&key)) {
            return Ok(Arc::clone(cell));
        }
        let cell = Arc::new(TcCell::default());
        self.new
            .insert(key, (tc.clone().unbind(), Arc::clone(&cell)));
        self.open.insert(key);
        let compiled = self.build(tc);
        self.open.remove(&key);
        let compiled = compiled?;
        cell.0
            .set(compiled)
            .expect("a TypeCode cell is filled once");
        Ok(cell)
    }

    fn build(&mut self, tc: &Bound<'_, PyAny>) -> PyResult<Tc> {
        let k: u32 = tc.getattr("_k")?.getattr("_v")?.extract()?;
        let length = || -> PyResult<u32> { tc.getattr("_length")?.extract() };
        let id = || -> PyResult<String> { tc.getattr("_id")?.extract() };
        let class = || -> PyResult<Py<PyAny>> {
            let class = tc.getattr("_class")?;
            if class.is_none() {
                // Declared (forward or recursive) but never defined.
                return Err(sys_err(tc.py(), &bad_typecode()));
            }
            Ok(class.unbind())
        };
        Ok(match k {
            kind::NULL | kind::VOID => Tc::Void,
            kind::SHORT => Tc::Short,
            kind::LONG => Tc::Long,
            kind::USHORT => Tc::UShort,
            kind::ULONG => Tc::ULong,
            kind::LONGLONG => Tc::LongLong,
            kind::ULONGLONG => Tc::ULongLong,
            kind::FLOAT => Tc::Float,
            kind::DOUBLE => Tc::Double,
            kind::BOOLEAN => Tc::Boolean,
            kind::CHAR => Tc::Char,
            kind::WCHAR => Tc::WChar,
            kind::OCTET => Tc::Octet,
            kind::ANY => Tc::Any,
            kind::TYPECODE => Tc::TypeCode,
            kind::STRING => Tc::String(length()?),
            kind::WSTRING => Tc::WString(length()?),
            kind::FIXED => Tc::Fixed {
                digits: tc.getattr("_digits")?.extract()?,
                scale: tc.getattr("_scale")?.extract()?,
            },
            kind::SEQUENCE => Tc::Sequence {
                content: self.tc(&tc.getattr("_content")?)?,
                bound: length()?,
            },
            kind::ARRAY => Tc::Array {
                content: self.tc(&tc.getattr("_content")?)?,
                length: length()?,
            },
            kind::ALIAS => Tc::Alias(self.tc(&tc.getattr("_content")?)?),
            kind::STRUCT | kind::EXCEPT => {
                let class = class()?;
                let iskeyword = tc.py().import("keyword")?.getattr("iskeyword")?;
                let mut members = Vec::new();
                for m in tc.getattr("_members")?.try_iter()? {
                    let (name, mtc): (String, Bound<'_, PyAny>) = m?.extract()?;
                    // The attribute of the Python class (`_from` for `from`).
                    let attr = if iskeyword.call1((&name,))?.is_truthy()? {
                        format!("_{name}")
                    } else {
                        name
                    };
                    members.push((PyString::intern(tc.py(), &attr).unbind(), self.tc(&mtc)?));
                }
                Tc::Struct { class, members }
            }
            kind::UNION => {
                let class = class()?;
                let discriminator = self.tc(&tc.getattr("_disc")?)?;
                let default: i64 = tc.getattr("_default")?.extract()?;
                let default = usize::try_from(default).ok();
                let mut labels = Vec::new();
                for (i, label) in tc.getattr("_labels")?.try_iter()?.enumerate() {
                    labels.push(if Some(i) == default {
                        None
                    } else {
                        Some(label_key(&label?)?)
                    });
                }
                let mut members = Vec::new();
                for m in tc.getattr("_members")?.try_iter()? {
                    let (_, mtc): (Bound<'_, PyAny>, Bound<'_, PyAny>) = m?.extract()?;
                    members.push(self.tc(&mtc)?);
                }
                Tc::Union {
                    class,
                    discriminator,
                    labels,
                    members,
                    default,
                }
            }
            kind::ENUM => {
                let items = class()?.bind(tc.py()).getattr("_items")?;
                let items = items.cast_into::<PyTuple>()?;
                Tc::Enum {
                    id: id()?,
                    count: u32::try_from(items.len()).unwrap_or(u32::MAX),
                    items: items.unbind(),
                }
            }
            kind::VALUE => {
                let py = tc.py();
                let core = tc_from_py(tc)?;
                let class = tc.getattr("_class")?;
                let class = (!class.is_none()).then(|| class.unbind());
                let mut members = Vec::new();
                let base = tc.getattr("_base")?;
                if !base.is_none() && base.getattr("_k")?.getattr("_v")?.extract::<u32>()? == k {
                    let base = self.tc(&base)?;
                    if let Tc::Value { members: m, .. } = &**base {
                        members.extend(m.iter().map(|(n, t)| (n.clone_ref(py), Arc::clone(t))));
                    }
                }
                let iskeyword = py.import("keyword")?.getattr("iskeyword")?;
                for m in tc.getattr("_members")?.try_iter()? {
                    let m = m?;
                    let name: String = m.get_item(0)?.extract()?;
                    let attr = if iskeyword.call1((&name,))?.is_truthy()? {
                        format!("_{name}")
                    } else {
                        name
                    };
                    let mtc = self.tc(&m.get_item(1)?)?;
                    members.push((PyString::intern(py, &attr).unbind(), mtc));
                }
                Tc::Value {
                    id: id()?,
                    class,
                    core,
                    members,
                }
            }
            kind::VALUE_BOX => Tc::ValueBox {
                core: tc_from_py(tc)?,
                content: self.tc(&tc.getattr("_content")?)?,
            },
            kind::OBJREF | kind::ABSTRACT_INTERFACE | kind::LOCAL_INTERFACE => {
                Tc::Objref { id: id()? }
            }
            _ => Tc::Unsupported(tc.getattr("_k")?.str()?.to_string()),
        })
    }
}

/// The value of a union label or discriminator, as a number: integers as
/// they are, `boolean` as 0 or 1, `char` as its code point and enumerators
/// as their position.
pub(crate) fn label_key(label: &Bound<'_, PyAny>) -> PyResult<i128> {
    if let Ok(v) = label.getattr("_v") {
        return v.extract();
    }
    if let Ok(s) = label.cast::<PyString>() {
        let s = s.to_cow()?;
        let mut chars = s.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return Ok(i128::from(u32::from(c)));
        }
    }
    label.extract()
}
