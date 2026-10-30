//! `DynamicAny`: the core's [`DynAny`] for `rustorb.DynamicAny`, which
//! gives the Python classes of the mapping (`DynStruct`, `DynUnion`...).
//!
//! Values go in and out with the conversions of the generated code, for
//! the TypeCode that the Python side gives with each value: `CORBA.TC_long`
//! for `insert_long`, the type of the current component for `insert_val`.

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use rustorb_core::{
    Any, DynAny as CoreDynAny, DynAnyError, DynAnyFactory, TCKind, TypeCode, Value,
};

use crate::any::{any_from_py, any_to_py, tc_from_py, tc_to_py};
use crate::error::{marshal_err, sys_err};
use crate::tc::compile_tc;
use crate::value::{from_py, to_py};

static DYNAMIC_ANY: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// The Python exception for a DynAny error: the user exceptions of
/// `rustorb.DynamicAny`, or a system exception.
fn dyn_err(py: Python<'_>, e: DynAnyError) -> PyErr {
    let make = || -> PyResult<PyErr> {
        let (owner, name) = match &e {
            DynAnyError::System(e) => return Ok(sys_err(py, e)),
            DynAnyError::InvalidValue => (Some("DynAny"), "InvalidValue"),
            DynAnyError::TypeMismatch => (Some("DynAny"), "TypeMismatch"),
            DynAnyError::InconsistentTypeCode => (Some("DynAnyFactory"), "InconsistentTypeCode"),
            DynAnyError::MustTruncate => (None, "MustTruncate"),
        };
        let module = DYNAMIC_ANY
            .get_or_try_init(py, || {
                Ok::<_, PyErr>(py.import("rustorb.DynamicAny")?.unbind())
            })?
            .bind(py);
        let class = match owner {
            Some(owner) => module.getattr(owner)?.getattr(name)?,
            None => module.getattr(name)?,
        };
        Ok(PyErr::from_value(class.call0()?))
    };
    make().unwrap_or_else(|err| err)
}

fn kind_of(py: Python<'_>, k: u32) -> PyResult<TCKind> {
    TCKind::from_u32(k).ok_or_else(|| {
        sys_err(
            py,
            &rustorb_core::SystemException::new(
                rustorb_core::SystemExceptionKind::BAD_PARAM,
                0,
                rustorb_core::CompletionStatus::No,
            ),
        )
    })
}

/// A Python value of the type `tc` (a `CORBA.TypeCode`) as a core value.
fn value_from_py(tc: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<Value> {
    let py = tc.py();
    let compiled = compile_tc(tc)?;
    from_py(&compiled, value).map_err(|e| marshal_err(py, e))
}

fn value_to_py<'py>(tc: &Bound<'py, PyAny>, value: Value) -> PyResult<Bound<'py, PyAny>> {
    let compiled = compile_tc(tc)?;
    to_py(tc.py(), &compiled, value)
}

fn anys_from_py(values: &[Bound<'_, PyAny>]) -> PyResult<Vec<Any>> {
    values.iter().map(any_from_py).collect()
}

fn anys_to_py(py: Python<'_>, values: Vec<Any>) -> PyResult<Vec<Bound<'_, PyAny>>> {
    values.into_iter().map(|a| any_to_py(py, a)).collect()
}

/// A DynAny (`rustorb.DynamicAny.DynAny` and its subclasses wrap it).
#[pyclass(frozen, module = "rustorb._native")]
pub(crate) struct DynAny(CoreDynAny);

#[pymethods]
impl DynAny {
    /// `DynAnyFactory::create_dyn_any`, or with `truncate` false
    /// `create_dyn_any_without_truncation`.
    #[staticmethod]
    fn create(value: &Bound<'_, PyAny>, truncate: bool) -> PyResult<DynAny> {
        let py = value.py();
        let any = any_from_py(value)?;
        let d = if truncate {
            DynAnyFactory.create_dyn_any(&any)
        } else {
            DynAnyFactory.create_dyn_any_without_truncation(&any)
        };
        d.map(DynAny).map_err(|e| dyn_err(py, e))
    }

    /// `DynAnyFactory::create_dyn_any_from_type_code`.
    #[staticmethod]
    fn from_type_code(tc: &Bound<'_, PyAny>) -> PyResult<DynAny> {
        let py = tc.py();
        let tc = tc_from_py(tc)?;
        DynAnyFactory
            .create_dyn_any_from_type_code(&tc)
            .map(DynAny)
            .map_err(|e| dyn_err(py, e))
    }

    /// The unaliased kind, which tells the Python class.
    fn kind(&self) -> u32 {
        self.0.kind().as_u32()
    }

    /// Whether both are the same DynAny.
    fn same(&self, other: &DynAny) -> bool {
        self.0.ptr_eq(&other.0)
    }

    #[pyo3(name = "type")]
    fn type_code<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let tc = self.0.type_code().map_err(|e| dyn_err(py, e))?;
        tc_to_py(py, &tc)
    }

    fn assign(&self, py: Python<'_>, other: &DynAny) -> PyResult<()> {
        self.0.assign(&other.0).map_err(|e| dyn_err(py, e))
    }

    // The name of the IDL operation.
    #[allow(clippy::wrong_self_convention)]
    fn from_any(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let any = any_from_py(value)?;
        self.0.from_any(&any).map_err(|e| dyn_err(value.py(), e))
    }

    fn to_any<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let any = self.0.to_any().map_err(|e| dyn_err(py, e))?;
        any_to_py(py, any)
    }

    fn equal(&self, py: Python<'_>, other: &DynAny) -> PyResult<bool> {
        self.0.equal(&other.0).map_err(|e| dyn_err(py, e))
    }

    fn destroy(&self, py: Python<'_>) -> PyResult<()> {
        self.0.destroy().map_err(|e| dyn_err(py, e))
    }

    fn copy(&self, py: Python<'_>) -> PyResult<DynAny> {
        self.0.copy().map(DynAny).map_err(|e| dyn_err(py, e))
    }

    /// `insert_<type>`: the value of kind `kind`, converted as the
    /// TypeCode `tc` says.
    fn insert(&self, kind: u32, tc: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = tc.py();
        let kind = kind_of(py, kind)?;
        let value = value_from_py(tc, value)?;
        self.0.insert_basic(kind, value).map_err(|e| dyn_err(py, e))
    }

    /// `get_<type>`: the value of kind `kind`, converted as the TypeCode
    /// `tc` says.
    fn get<'py>(&self, kind: u32, tc: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let py = tc.py();
        let kind = kind_of(py, kind)?;
        let value = self.0.get_basic(kind).map_err(|e| dyn_err(py, e))?;
        value_to_py(tc, value)
    }

    /// `insert_val`: the value converted as the type of the current
    /// component (or of this DynAny) says.
    fn insert_val(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = value.py();
        let tc = self.target_type(py)?;
        let value = value_from_py(&tc, value)?;
        self.0
            .insert_basic(TCKind::Value, value)
            .map_err(|e| dyn_err(py, e))
    }

    fn get_val<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let value = self
            .0
            .get_basic(TCKind::Value)
            .map_err(|e| dyn_err(py, e))?;
        let tc = match &value {
            Value::Valuetype(Some(r)) => tc_to_py(py, r.typecode())?,
            _ => self.target_type(py)?,
        };
        value_to_py(&tc, value)
    }

    fn insert_dyn_any(&self, py: Python<'_>, value: &DynAny) -> PyResult<()> {
        self.0.insert_dyn_any(&value.0).map_err(|e| dyn_err(py, e))
    }

    fn get_dyn_any(&self, py: Python<'_>) -> PyResult<DynAny> {
        self.0.get_dyn_any().map(DynAny).map_err(|e| dyn_err(py, e))
    }

    fn no_implement(&self, py: Python<'_>) -> PyResult<()> {
        self.0.no_implement().map_err(|e| dyn_err(py, e))
    }

    /// `insert_<type>_seq`: `values` converted as the sequence TypeCode
    /// `tc` says.
    fn insert_seq(
        &self,
        kind: u32,
        tc: &Bound<'_, PyAny>,
        values: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let py = tc.py();
        let kind = kind_of(py, kind)?;
        let values = match value_from_py(tc, values)? {
            Value::List(items) => items,
            Value::Octets(b) => b.iter().copied().map(Value::Octet).collect(),
            Value::Chars(s) => s.chars().map(Value::Char).collect(),
            _ => Vec::new(),
        };
        self.0.insert_seq(kind, values).map_err(|e| dyn_err(py, e))
    }

    fn get_seq<'py>(&self, kind: u32, tc: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let py = tc.py();
        let kind = kind_of(py, kind)?;
        let values = self.0.get_seq(kind).map_err(|e| dyn_err(py, e))?;
        let value = match kind {
            TCKind::Octet => Value::Octets(
                values
                    .iter()
                    .filter_map(|v| match v {
                        Value::Octet(b) => Some(*b),
                        _ => None,
                    })
                    .collect(),
            ),
            TCKind::Char => Value::Chars(
                values
                    .iter()
                    .filter_map(|v| match v {
                        Value::Char(c) => Some(*c),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => Value::List(values),
        };
        value_to_py(tc, value)
    }

    fn seek(&self, py: Python<'_>, index: i32) -> PyResult<bool> {
        self.0.seek(index).map_err(|e| dyn_err(py, e))
    }

    fn rewind(&self, py: Python<'_>) -> PyResult<()> {
        self.0.rewind().map_err(|e| dyn_err(py, e))
    }

    fn next(&self, py: Python<'_>) -> PyResult<bool> {
        self.0.next().map_err(|e| dyn_err(py, e))
    }

    fn component_count(&self, py: Python<'_>) -> PyResult<u32> {
        self.0.component_count().map_err(|e| dyn_err(py, e))
    }

    fn current_component(&self, py: Python<'_>) -> PyResult<Option<DynAny>> {
        self.0
            .current_component()
            .map(|c| c.map(DynAny))
            .map_err(|e| dyn_err(py, e))
    }

    // DynFixed

    fn get_fixed_value(&self, py: Python<'_>) -> PyResult<String> {
        self.0.get_fixed_value().map_err(|e| dyn_err(py, e))
    }

    fn set_fixed_value(&self, py: Python<'_>, value: &str) -> PyResult<bool> {
        self.0.set_fixed_value(value).map_err(|e| dyn_err(py, e))
    }

    // DynEnum

    fn get_as_string(&self, py: Python<'_>) -> PyResult<String> {
        self.0.get_enum_as_string().map_err(|e| dyn_err(py, e))
    }

    fn set_as_string(&self, py: Python<'_>, value: &str) -> PyResult<()> {
        self.0.set_enum_as_string(value).map_err(|e| dyn_err(py, e))
    }

    fn get_as_ulong(&self, py: Python<'_>) -> PyResult<u32> {
        self.0.get_enum_as_ulong().map_err(|e| dyn_err(py, e))
    }

    fn set_as_ulong(&self, py: Python<'_>, value: u32) -> PyResult<()> {
        self.0.set_enum_as_ulong(value).map_err(|e| dyn_err(py, e))
    }

    // DynStruct, DynValue

    fn current_member_name(&self, py: Python<'_>) -> PyResult<String> {
        self.0.current_member_name().map_err(|e| dyn_err(py, e))
    }

    fn current_member_kind(&self, py: Python<'_>) -> PyResult<u32> {
        self.0
            .current_member_kind()
            .map(TCKind::as_u32)
            .map_err(|e| dyn_err(py, e))
    }

    /// The members as (name, CORBA.Any) pairs.
    fn get_members<'py>(&self, py: Python<'py>) -> PyResult<Vec<(String, Bound<'py, PyAny>)>> {
        let members = self.0.get_members().map_err(|e| dyn_err(py, e))?;
        members
            .into_iter()
            .map(|(n, a)| Ok((n, any_to_py(py, a)?)))
            .collect()
    }

    fn set_members(
        &self,
        py: Python<'_>,
        members: Vec<(String, Bound<'_, PyAny>)>,
    ) -> PyResult<()> {
        let members = members
            .iter()
            .map(|(n, a)| Ok((n.clone(), any_from_py(a)?)))
            .collect::<PyResult<Vec<_>>>()?;
        self.0.set_members(&members).map_err(|e| dyn_err(py, e))
    }

    fn get_members_as_dyn_any(&self, py: Python<'_>) -> PyResult<Vec<(String, DynAny)>> {
        let members = self
            .0
            .get_members_as_dyn_any()
            .map_err(|e| dyn_err(py, e))?;
        Ok(members.into_iter().map(|(n, d)| (n, DynAny(d))).collect())
    }

    fn set_members_as_dyn_any(
        &self,
        py: Python<'_>,
        members: Vec<(String, PyRef<'_, DynAny>)>,
    ) -> PyResult<()> {
        let members: Vec<_> = members
            .iter()
            .map(|(n, d)| (n.clone(), d.0.clone()))
            .collect();
        self.0
            .set_members_as_dyn_any(&members)
            .map_err(|e| dyn_err(py, e))
    }

    // DynUnion

    fn get_discriminator(&self, py: Python<'_>) -> PyResult<DynAny> {
        self.0
            .get_discriminator()
            .map(DynAny)
            .map_err(|e| dyn_err(py, e))
    }

    fn set_discriminator(&self, py: Python<'_>, d: &DynAny) -> PyResult<()> {
        self.0.set_discriminator(&d.0).map_err(|e| dyn_err(py, e))
    }

    fn set_to_default_member(&self, py: Python<'_>) -> PyResult<()> {
        self.0.set_to_default_member().map_err(|e| dyn_err(py, e))
    }

    fn set_to_no_active_member(&self, py: Python<'_>) -> PyResult<()> {
        self.0.set_to_no_active_member().map_err(|e| dyn_err(py, e))
    }

    fn has_no_active_member(&self, py: Python<'_>) -> PyResult<bool> {
        self.0.has_no_active_member().map_err(|e| dyn_err(py, e))
    }

    fn discriminator_kind(&self, py: Python<'_>) -> PyResult<u32> {
        self.0
            .discriminator_kind()
            .map(TCKind::as_u32)
            .map_err(|e| dyn_err(py, e))
    }

    fn member(&self, py: Python<'_>) -> PyResult<DynAny> {
        self.0.member().map(DynAny).map_err(|e| dyn_err(py, e))
    }

    fn member_name(&self, py: Python<'_>) -> PyResult<String> {
        self.0.member_name().map_err(|e| dyn_err(py, e))
    }

    fn member_kind(&self, py: Python<'_>) -> PyResult<u32> {
        self.0
            .member_kind()
            .map(TCKind::as_u32)
            .map_err(|e| dyn_err(py, e))
    }

    // DynSequence, DynArray

    fn get_length(&self, py: Python<'_>) -> PyResult<u32> {
        self.0.get_length().map_err(|e| dyn_err(py, e))
    }

    fn set_length(&self, py: Python<'_>, len: u32) -> PyResult<()> {
        self.0.set_length(len).map_err(|e| dyn_err(py, e))
    }

    fn get_elements<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let values = self.0.get_elements().map_err(|e| dyn_err(py, e))?;
        anys_to_py(py, values)
    }

    fn set_elements(&self, py: Python<'_>, values: Vec<Bound<'_, PyAny>>) -> PyResult<()> {
        let values = anys_from_py(&values)?;
        self.0.set_elements(&values).map_err(|e| dyn_err(py, e))
    }

    fn get_elements_as_dyn_any(&self, py: Python<'_>) -> PyResult<Vec<DynAny>> {
        let values = self
            .0
            .get_elements_as_dyn_any()
            .map_err(|e| dyn_err(py, e))?;
        Ok(values.into_iter().map(DynAny).collect())
    }

    fn set_elements_as_dyn_any(
        &self,
        py: Python<'_>,
        values: Vec<PyRef<'_, DynAny>>,
    ) -> PyResult<()> {
        let values: Vec<_> = values.iter().map(|d| d.0.clone()).collect();
        self.0
            .set_elements_as_dyn_any(&values)
            .map_err(|e| dyn_err(py, e))
    }

    // DynValueCommon, DynValueBox

    fn is_null(&self, py: Python<'_>) -> PyResult<bool> {
        self.0.is_null().map_err(|e| dyn_err(py, e))
    }

    fn set_to_null(&self, py: Python<'_>) -> PyResult<()> {
        self.0.set_to_null().map_err(|e| dyn_err(py, e))
    }

    fn set_to_value(&self, py: Python<'_>) -> PyResult<()> {
        self.0.set_to_value().map_err(|e| dyn_err(py, e))
    }

    fn get_boxed_value<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let value = self.0.get_boxed_value().map_err(|e| dyn_err(py, e))?;
        any_to_py(py, value)
    }

    fn set_boxed_value(&self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let any = any_from_py(value)?;
        self.0
            .set_boxed_value(&any)
            .map_err(|e| dyn_err(value.py(), e))
    }

    fn get_boxed_value_as_dyn_any(&self, py: Python<'_>) -> PyResult<DynAny> {
        self.0
            .get_boxed_value_as_dyn_any()
            .map(DynAny)
            .map_err(|e| dyn_err(py, e))
    }

    fn set_boxed_value_as_dyn_any(&self, py: Python<'_>, value: &DynAny) -> PyResult<()> {
        self.0
            .set_boxed_value_as_dyn_any(&value.0)
            .map_err(|e| dyn_err(py, e))
    }
}

impl DynAny {
    /// The type of the DynAny the insert and get operations work on.
    fn target_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let target = match self.0.current_component() {
            Ok(Some(c)) => c,
            _ => self.0.clone(),
        };
        let tc: TypeCode = target.type_code().map_err(|e| dyn_err(py, e))?;
        tc_to_py(py, &tc)
    }
}
