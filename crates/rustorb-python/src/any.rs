//! `any` and `TypeCode` values: `CORBA.Any` and `CORBA.TypeCode` to and
//! from the core's [`Any`] and [`TypeCode`].
//!
//! A Python TypeCode is a graph of `CORBA.TypeCode` objects, recursive
//! types being cycles. It becomes a core TypeCode by walking the graph,
//! recursive references becoming [`TypeCode::recursive`] placeholders that
//! the enclosing struct or union resolves. The result is kept in the
//! `_NP_native` attribute of the Python object.
//!
//! A core TypeCode becomes a Python one in `_rt.typecode_from_nodes`,
//! which gets the nodes of the TypeCode as tuples. Named types that the
//! generated code defines (with an equal TypeCode) are replaced by the
//! generated TypeCode, so values get the generated classes; others get
//! classes made on the fly.

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};
use rustorb_core::{Any, TCKind, TypeCode, TypeCodeError};

use crate::error::{bad_param, bad_typecode, corba, rt, sys_err};
use crate::tc::{compile_tc, label_key};
use crate::value::{from_py, to_py};

/// The core TypeCode cached on a Python TypeCode.
#[pyclass(frozen, module = "rustorb._native")]
#[derive(Debug)]
pub(crate) struct NativeTypeCode(pub(crate) TypeCode);

/// A `CORBA.Any` as a core [`Any`].
pub(crate) fn any_from_py(obj: &Bound<'_, PyAny>) -> PyResult<Any> {
    let py = obj.py();
    let (Ok(t), Ok(v)) = (obj.getattr("_t"), obj.getattr("_v")) else {
        return Err(sys_err(py, &bad_param()));
    };
    let tc = tc_from_py(&t)?;
    let compiled = compile_tc(&t)?;
    let value = from_py(&compiled, &v)?;
    Ok(Any::new(tc, value))
}

/// A core [`Any`] as a `CORBA.Any`.
pub(crate) fn any_to_py(py: Python<'_>, any: Any) -> PyResult<Bound<'_, PyAny>> {
    let (tc, value) = any.into_parts();
    let t = tc_to_py(py, &tc)?;
    let compiled = compile_tc(&t)?;
    let v = to_py(py, &compiled, value)?;
    corba(py)?.getattr("Any")?.call1((t, v))
}

/// A `CORBA.TypeCode` as a core [`TypeCode`].
pub(crate) fn tc_from_py(obj: &Bound<'_, PyAny>) -> PyResult<TypeCode> {
    if let Ok(native) = obj.getattr("_NP_native") {
        if let Ok(native) = native.cast::<NativeTypeCode>() {
            return Ok(native.get().0.clone());
        }
    }
    let tc = ToCore::default().convert(obj)?;
    // A TypeCode whose members are not set yet (declared, not defined)
    // fails above, so what is cached is complete.
    let _ = obj.setattr("_NP_native", NativeTypeCode(tc.clone()));
    Ok(tc)
}

fn tc_err(py: Python<'_>) -> impl Fn(TypeCodeError) -> PyErr + '_ {
    move |_| sys_err(py, &bad_typecode())
}

#[derive(Default)]
struct ToCore {
    /// The structs and unions being converted, by address, with their
    /// repository ids: a reference to one of them is recursive.
    open: HashMap<usize, String>,
}

impl ToCore {
    fn convert(&mut self, obj: &Bound<'_, PyAny>) -> PyResult<TypeCode> {
        let py = obj.py();
        let key = obj.as_ptr() as usize;
        if let Some(id) = self.open.get(&key) {
            return Ok(TypeCode::recursive(id));
        }
        if let Ok(native) = obj.getattr("_NP_native") {
            if let Ok(native) = native.cast::<NativeTypeCode>() {
                return Ok(native.get().0.clone());
            }
        }
        let err = tc_err(py);
        let Ok(k) = obj.getattr("_k").and_then(|k| k.getattr("_v")) else {
            return Err(sys_err(py, &bad_param()));
        };
        let k: u32 = k.extract()?;
        let kind = TCKind::from_u32(k).ok_or_else(|| sys_err(py, &bad_typecode()))?;
        let id = || -> PyResult<String> { obj.getattr("_id")?.extract() };
        let name = || -> PyResult<String> { obj.getattr("_name")?.extract() };
        let length = || -> PyResult<u32> { obj.getattr("_length")?.extract() };
        if kind.is_simple() {
            return TypeCode::simple(kind).map_err(&err);
        }
        Ok(match kind {
            TCKind::String => TypeCode::string(length()?),
            TCKind::WString => TypeCode::wstring(length()?),
            TCKind::Fixed => TypeCode::fixed(
                obj.getattr("_digits")?.extract()?,
                obj.getattr("_scale")?.extract()?,
            )
            .map_err(&err)?,
            TCKind::Sequence => {
                let content = self.convert(&obj.getattr("_content")?)?;
                TypeCode::sequence(&content, length()?).map_err(&err)?
            }
            TCKind::Array => {
                let content = self.convert(&obj.getattr("_content")?)?;
                TypeCode::array(&content, length()?).map_err(&err)?
            }
            TCKind::Alias | TCKind::ValueBox => {
                let content = self.convert(&obj.getattr("_content")?)?;
                if kind == TCKind::Alias {
                    TypeCode::alias(&id()?, &name()?, &content).map_err(&err)?
                } else {
                    TypeCode::value_box(&id()?, &name()?, &content).map_err(&err)?
                }
            }
            TCKind::Objref
            | TCKind::AbstractInterface
            | TCKind::LocalInterface
            | TCKind::Native
            | TCKind::Component
            | TCKind::Home => TypeCode::interface(kind, &id()?, &name()?).map_err(&err)?,
            TCKind::Enum => {
                let members: Vec<String> = obj.getattr("_members")?.extract()?;
                TypeCode::enum_tc(&id()?, &name()?, &members).map_err(&err)?
            }
            TCKind::Struct | TCKind::Except => {
                let members = obj.getattr("_members")?;
                if members.is_none() {
                    // Declared but never defined.
                    return Err(sys_err(py, &bad_typecode()));
                }
                let id = id()?;
                self.open.insert(key, id.clone());
                let mut converted = Vec::new();
                let result = (|| -> PyResult<()> {
                    for m in members.try_iter()? {
                        let (n, t): (String, Bound<'_, PyAny>) = m?.extract()?;
                        converted.push((n, self.convert(&t)?));
                    }
                    Ok(())
                })();
                self.open.remove(&key);
                result?;
                if kind == TCKind::Struct {
                    TypeCode::struct_tc(&id, &name()?, &converted).map_err(&err)?
                } else {
                    TypeCode::exception(&id, &name()?, &converted).map_err(&err)?
                }
            }
            TCKind::Union => {
                let members = obj.getattr("_members")?;
                if members.is_none() {
                    return Err(sys_err(py, &bad_typecode()));
                }
                let id = id()?;
                let disc = self.convert(&obj.getattr("_disc")?)?;
                let default: i64 = obj.getattr("_default")?.extract()?;
                let labels: Vec<Bound<'_, PyAny>> = obj
                    .getattr("_labels")?
                    .try_iter()?
                    .collect::<PyResult<_>>()?;
                self.open.insert(key, id.clone());
                let mut converted = Vec::new();
                let result = (|| -> PyResult<()> {
                    for (i, m) in members.try_iter()?.enumerate() {
                        let (n, t): (String, Bound<'_, PyAny>) = m?.extract()?;
                        let label = if i as i64 == default {
                            None
                        } else {
                            let l = labels.get(i).ok_or_else(|| sys_err(py, &bad_typecode()))?;
                            Some(label_key(l)?)
                        };
                        converted.push((label, n, self.convert(&t)?));
                    }
                    Ok(())
                })();
                self.open.remove(&key);
                result?;
                TypeCode::union_tc(&id, &name()?, &disc, &converted).map_err(&err)?
            }
            _ => {
                return Err(sys_err(
                    py,
                    &rustorb_core::SystemException::new(
                        rustorb_core::SystemExceptionKind::NO_IMPLEMENT,
                        0,
                        rustorb_core::CompletionStatus::No,
                    ),
                ));
            }
        })
    }
}

/// A core [`TypeCode`] as a `CORBA.TypeCode`.
pub(crate) fn tc_to_py<'py>(py: Python<'py>, tc: &TypeCode) -> PyResult<Bound<'py, PyAny>> {
    let mut nodes = ToPy {
        py,
        nodes: Vec::new(),
        index: HashMap::new(),
    };
    nodes.node(tc)?;
    let list = PyList::empty(py);
    for n in nodes.nodes {
        list.append(n.expect("every node is filled"))?;
    }
    rt(py)?.getattr("typecode_from_nodes")?.call1((list,))
}

/// The nodes of a core TypeCode as tuples, the kind first, children as
/// indexes into the list (see `_rt.typecode_from_nodes`).
struct ToPy<'py> {
    py: Python<'py>,
    nodes: Vec<Option<Bound<'py, PyTuple>>>,
    index: HashMap<(usize, u32), usize>,
}

impl<'py> ToPy<'py> {
    fn node(&mut self, tc: &TypeCode) -> PyResult<usize> {
        let key = tc.node_key();
        if let Some(&i) = self.index.get(&key) {
            return Ok(i);
        }
        let i = self.nodes.len();
        self.nodes.push(None);
        self.index.insert(key, i);
        let py = self.py;
        let err = tc_err(py);
        let kind = tc.kind();
        let k = kind.as_u32();
        let id = || tc.id().map_err(&err);
        let name = || tc.name().map_err(&err);
        let tuple = match kind {
            _ if tc.is_recursive_placeholder() => return Err(sys_err(py, &bad_typecode())),
            _ if kind.is_simple() => (k,).into_pyobject(py)?,
            TCKind::String | TCKind::WString => {
                (k, tc.length().map_err(&err)?).into_pyobject(py)?
            }
            TCKind::Fixed => (
                k,
                tc.fixed_digits().map_err(&err)?,
                tc.fixed_scale().map_err(&err)?,
            )
                .into_pyobject(py)?,
            TCKind::Sequence | TCKind::Array => {
                let content = self.node(&tc.content_type().map_err(&err)?)?;
                (k, content, tc.length().map_err(&err)?).into_pyobject(py)?
            }
            TCKind::Alias | TCKind::ValueBox => {
                let content = self.node(&tc.content_type().map_err(&err)?)?;
                (k, id()?, name()?, content).into_pyobject(py)?
            }
            TCKind::Enum => {
                let count = tc.member_count().map_err(&err)?;
                let names = (0..count)
                    .map(|m| tc.member_name(m).map(str::to_owned))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(&err)?;
                (k, id()?, name()?, names).into_pyobject(py)?
            }
            TCKind::Struct | TCKind::Except => {
                let count = tc.member_count().map_err(&err)?;
                let mut members = Vec::new();
                for m in 0..count {
                    let mt = self.node(&tc.member_type(m).map_err(&err)?)?;
                    members.push((tc.member_name(m).map_err(&err)?.to_owned(), mt));
                }
                (k, id()?, name()?, members).into_pyobject(py)?
            }
            TCKind::Union => {
                let disc = self.node(&tc.discriminator_type().map_err(&err)?)?;
                let count = tc.member_count().map_err(&err)?;
                let mut members = Vec::new();
                for m in 0..count {
                    let mt = self.node(&tc.member_type(m).map_err(&err)?)?;
                    let label = tc.member_label_value(m).map_err(&err)?;
                    members.push((label, tc.member_name(m).map_err(&err)?.to_owned(), mt));
                }
                let default = tc.default_index().map_err(&err)?;
                (k, id()?, name()?, disc, default, members).into_pyobject(py)?
            }
            // Interfaces, native and the types the binding does not
            // marshal (valuetypes) keep their id and name.
            _ => (k, id().unwrap_or_default(), name().unwrap_or_default()).into_pyobject(py)?,
        };
        self.nodes[i] = Some(tuple);
        Ok(i)
    }
}
