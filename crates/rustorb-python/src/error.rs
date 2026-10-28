//! CORBA exceptions between Rust and Python, and the Python modules of the
//! runtime the native module calls back into.

use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyDict;
use rustorb_core::{CompletionStatus, PoaError, SystemException, SystemExceptionKind};

static CORBA: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
static RT: PyOnceLock<Py<PyModule>> = PyOnceLock::new();
static PORTABLE_SERVER: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

fn module<'py>(
    py: Python<'py>,
    cell: &PyOnceLock<Py<PyModule>>,
    name: &str,
) -> PyResult<Bound<'py, PyModule>> {
    cell.get_or_try_init(py, || Ok::<_, PyErr>(py.import(name)?.unbind()))
        .map(|m| m.bind(py).clone())
}

/// `rustorb.CORBA`.
pub(crate) fn corba(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    module(py, &CORBA, "rustorb.CORBA")
}

/// `rustorb._rt`.
pub(crate) fn rt(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    module(py, &RT, "rustorb._rt")
}

/// `rustorb.PortableServer`.
pub(crate) fn portable_server(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    module(py, &PORTABLE_SERVER, "rustorb.PortableServer")
}

pub(crate) fn sys(
    kind: SystemExceptionKind,
    minor: u32,
    completed: CompletionStatus,
) -> SystemException {
    SystemException::new(kind, minor, completed)
}

pub(crate) fn bad_param() -> SystemException {
    sys(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No)
}

pub(crate) fn bad_typecode() -> SystemException {
    sys(SystemExceptionKind::BAD_TYPECODE, 0, CompletionStatus::No)
}

pub(crate) fn marshal(completed: CompletionStatus) -> SystemException {
    sys(SystemExceptionKind::MARSHAL, 0, completed)
}

pub(crate) fn completion_item<'py>(
    py: Python<'py>,
    completed: CompletionStatus,
) -> PyResult<Bound<'py, PyAny>> {
    let name = match completed {
        CompletionStatus::Yes => "COMPLETED_YES",
        CompletionStatus::No => "COMPLETED_NO",
        CompletionStatus::Maybe => "COMPLETED_MAYBE",
    };
    corba(py)?.getattr(name)
}

/// The Python exception for a system exception: the class of
/// `CORBA.system_exceptions` with its repository id, `CORBA.UNKNOWN` for
/// one rustorb does not know.
pub(crate) fn sys_err(py: Python<'_>, e: &SystemException) -> PyErr {
    let make = || -> PyResult<PyErr> {
        let corba = corba(py)?;
        let classes = corba.getattr("system_exceptions")?.cast_into::<PyDict>()?;
        let class = match classes.get_item(&e.repository_id)? {
            Some(class) => class,
            None => corba.getattr("UNKNOWN")?,
        };
        let value = class.call1((e.minor, completion_item(py, e.completed)?))?;
        Ok(PyErr::from_value(value))
    };
    make().unwrap_or_else(|err| err)
}

/// The system exception a Python exception stands for: a
/// `CORBA.SystemException` as it is, anything else `UNKNOWN`.
pub(crate) fn sys_from_py(py: Python<'_>, err: &PyErr) -> SystemException {
    let value = err.value(py);
    let convert = || -> PyResult<Option<SystemException>> {
        let base = corba(py)?.getattr("SystemException")?;
        if !value.is_instance(&base)? {
            return Ok(None);
        }
        let repository_id: String = value.getattr("_NP_RepositoryId")?.extract()?;
        let minor: u32 = value.getattr("minor")?.extract()?;
        let completed: u32 = value.getattr("completed")?.getattr("_v")?.extract()?;
        let completed = match completed {
            0 => CompletionStatus::Yes,
            1 => CompletionStatus::No,
            _ => CompletionStatus::Maybe,
        };
        Ok(Some(SystemException {
            repository_id,
            minor,
            completed,
        }))
    };
    match convert() {
        Ok(Some(e)) => e,
        _ => sys(SystemExceptionKind::UNKNOWN, 0, CompletionStatus::Maybe),
    }
}

/// Conversion of a Python error met while marshalling: a CORBA system
/// exception stays, anything else (a value of the wrong type) becomes
/// `BAD_PARAM`.
pub(crate) fn marshal_err(py: Python<'_>, err: PyErr) -> PyErr {
    match corba(py).and_then(|c| c.getattr("Exception")) {
        Ok(base) if err.is_instance(py, &base) => err,
        _ => {
            let e = sys_err(py, &bad_param());
            e.set_cause(py, Some(err));
            e
        }
    }
}

/// The Python exception for a POA error: the user exceptions of
/// `PortableServer.POA`, `POAManager` and `Current`.
pub(crate) fn poa_err(py: Python<'_>, e: PoaError) -> PyErr {
    let make = || -> PyResult<PyErr> {
        let ps = portable_server(py)?;
        let (owner, name) = match &e {
            PoaError::System(e) => return Ok(sys_err(py, e)),
            PoaError::AdapterAlreadyExists => ("POA", "AdapterAlreadyExists"),
            PoaError::AdapterNonExistent => ("POA", "AdapterNonExistent"),
            PoaError::InvalidPolicy(_) => ("POA", "InvalidPolicy"),
            PoaError::ServantAlreadyActive => ("POA", "ServantAlreadyActive"),
            PoaError::ObjectAlreadyActive => ("POA", "ObjectAlreadyActive"),
            PoaError::ServantNotActive => ("POA", "ServantNotActive"),
            PoaError::ObjectNotActive => ("POA", "ObjectNotActive"),
            PoaError::WrongPolicy => ("POA", "WrongPolicy"),
            PoaError::WrongAdapter => ("POA", "WrongAdapter"),
            PoaError::NoServant => ("POA", "NoServant"),
            PoaError::AdapterInactive => ("POAManager", "AdapterInactive"),
            PoaError::NoContext => ("Current", "NoContext"),
        };
        let class = ps.getattr(owner)?.getattr(name)?;
        let value = match e {
            PoaError::InvalidPolicy(index) => class.call1((index,))?,
            _ => class.call0()?,
        };
        Ok(PyErr::from_value(value))
    };
    make().unwrap_or_else(|err| err)
}
