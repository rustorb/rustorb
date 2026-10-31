//! The DII and the DSI: `CORBA.Request` and `CORBA.ServerRequest` (in
//! `rustorb.CORBA`) on the core's [`DiiRequest`] and [`ServerRequest`].
//!
//! The values are `CORBA.Any`s, converted to the core's Anys and back.

use std::sync::Mutex;

use pyo3::prelude::*;
use rustorb_core::dii::{ArgMode, DiiRequest, NamedValue, ServerRequest};
use rustorb_core::{CompletionStatus, Exception, SystemException, SystemExceptionKind, minor};

use crate::any::{any_from_py, any_to_py, tc_from_py};
use crate::error::{sys, sys_err};
use crate::orb::ObjRef;
use crate::value::ValueScope;

fn mode(py: Python<'_>, flags: u32) -> PyResult<ArgMode> {
    ArgMode::from_flags(flags).ok_or_else(|| {
        sys_err(
            py,
            &sys(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No),
        )
    })
}

fn bad_inv_order() -> SystemException {
    sys(SystemExceptionKind::BAD_INV_ORDER, 0, CompletionStatus::No)
}

/// The native part of a `CORBA.Request`.
#[pyclass(frozen, module = "rustorb._native")]
pub(crate) struct Request(Mutex<DiiRequest>);

impl Request {
    /// The outcome of a call as Python sees it: a user exception is raised
    /// as the exception, decoded with the request's exception list.
    fn outcome(&self, py: Python<'_>, result: Result<(), Exception>) -> PyResult<()> {
        match result {
            Ok(()) => Ok(()),
            Err(Exception::System(e)) => Err(sys_err(py, &e)),
            Err(Exception::User(e)) => {
                let value = self.0.lock().unwrap().exception_value(&e);
                match value {
                    Ok(any) => {
                        let any = any_to_py(py, any)?;
                        Err(PyErr::from_value(any.call_method0("value")?))
                    }
                    Err(_) => Err(sys_err(
                        py,
                        &sys(
                            SystemExceptionKind::UNKNOWN,
                            minor::UNLISTED_USER_EXCEPTION,
                            CompletionStatus::Yes,
                        ),
                    )),
                }
            }
        }
    }
}

/// The arguments of a call: `(name, CORBA.Any, flags)` tuples. The Any of
/// an `out` argument gives only its type.
type PyArgs<'py> = Vec<(String, Bound<'py, PyAny>, u32)>;

impl Request {
    /// Sets the arguments, the result type and the user exceptions.
    fn prepare(
        &self,
        py: Python<'_>,
        args: PyArgs<'_>,
        result: &Bound<'_, PyAny>,
        exceptions: Vec<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let mut list = Vec::with_capacity(args.len());
        for (name, any, flags) in args {
            let mode = mode(py, flags)?;
            let nv = if mode == ArgMode::Out {
                NamedValue::out(name, tc_from_py(&any.call_method0("typecode")?)?)
            } else {
                NamedValue::new(name, any_from_py(&any)?, mode)
            };
            list.push(nv);
        }
        let result = tc_from_py(result)?;
        let exceptions = exceptions
            .iter()
            .map(tc_from_py)
            .collect::<PyResult<Vec<_>>>()?;
        let mut request = self.0.lock().unwrap();
        *request.arguments_mut() = list;
        request.set_return_type(result);
        for tc in exceptions {
            request.add_exception(tc);
        }
        Ok(())
    }

    /// The return value and the values of the `out` and `inout` arguments
    /// (`CORBA.Any`s; None for the `in` ones and an `out` one without a
    /// value), converted together so that valuetypes shared among them stay
    /// shared.
    fn outputs<'py>(&self, py: Python<'py>) -> PyResult<Outputs<'py>> {
        let (result, args) = {
            let request = self.0.lock().unwrap();
            (request.result().clone(), request.arguments().to_vec())
        };
        let _scope = ValueScope::enter();
        let result = any_to_py(py, result)?;
        let values = args
            .into_iter()
            .map(|nv| match nv.mode {
                // The values sent, which the scope of the call emptied
                // (valuetypes).
                ArgMode::In => Ok(None),
                _ if matches!(nv.value.value(), rustorb_core::Value::Void) => Ok(None),
                _ => any_to_py(py, nv.value).map(Some),
            })
            .collect::<PyResult<Vec<_>>>()?;
        Ok((result, values))
    }
}

type Outputs<'py> = (Bound<'py, PyAny>, Vec<Option<Bound<'py, PyAny>>>);

#[pymethods]
impl Request {
    #[new]
    fn new(target: PyRef<'_, ObjRef>, operation: &str) -> Request {
        Request(Mutex::new(target.obj.create_request(operation)))
    }

    /// Calls the operation with the arguments `args`, the result type
    /// `result` and the user exceptions `exceptions` (TypeCodes); returns
    /// the outputs: the return value and the values of the arguments.
    fn invoke<'py>(
        &self,
        py: Python<'py>,
        args: PyArgs<'py>,
        result: &Bound<'py, PyAny>,
        exceptions: Vec<Bound<'py, PyAny>>,
    ) -> PyResult<Outputs<'py>> {
        {
            let _scope = ValueScope::enter();
            self.prepare(py, args, result, exceptions)?;
            let outcome = py.detach(|| self.0.lock().unwrap().invoke());
            self.outcome(py, outcome)?;
        }
        self.outputs(py)
    }

    fn send_oneway(
        &self,
        py: Python<'_>,
        args: PyArgs<'_>,
        result: &Bound<'_, PyAny>,
        exceptions: Vec<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let _scope = ValueScope::enter();
        self.prepare(py, args, result, exceptions)?;
        py.detach(|| self.0.lock().unwrap().send_oneway())
            .map_err(|e| sys_err(py, &e))
    }

    fn send_deferred(
        &self,
        py: Python<'_>,
        args: PyArgs<'_>,
        result: &Bound<'_, PyAny>,
        exceptions: Vec<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        // The values are written after the scope ends: it keeps them.
        let _scope = ValueScope::ensure();
        self.prepare(py, args, result, exceptions)?;
        py.detach(|| self.0.lock().unwrap().send_deferred())
            .map_err(|e| sys_err(py, &e))
    }

    fn poll_response(&self, py: Python<'_>) -> PyResult<bool> {
        self.0
            .lock()
            .unwrap()
            .poll_response()
            .map_err(|e| sys_err(py, &e))
    }

    /// Waits for the response of `send_deferred`; returns the outputs.
    fn get_response<'py>(&self, py: Python<'py>) -> PyResult<Outputs<'py>> {
        let outcome = py.detach(|| self.0.lock().unwrap().get_response());
        self.outcome(py, outcome)?;
        self.outputs(py)
    }
}

/// The native part of a `CORBA.ServerRequest`, valid while the servant's
/// `invoke` runs.
#[pyclass(frozen, module = "rustorb._native")]
pub(crate) struct NativeServerRequest {
    operation: String,
    inner: Mutex<Option<ServerRequest>>,
}

impl NativeServerRequest {
    pub(crate) fn new(request: ServerRequest) -> NativeServerRequest {
        NativeServerRequest {
            operation: request.operation().to_owned(),
            inner: Mutex::new(Some(request)),
        }
    }

    /// The request back, once `invoke` has returned.
    pub(crate) fn take(&self) -> Option<ServerRequest> {
        self.inner.lock().unwrap().take()
    }

    fn with<T>(
        &self,
        py: Python<'_>,
        f: impl FnOnce(&mut ServerRequest) -> PyResult<T>,
    ) -> PyResult<T> {
        let mut inner = self.inner.lock().unwrap();
        match inner.as_mut() {
            Some(request) => f(request),
            // Kept after invoke returned.
            None => Err(sys_err(py, &bad_inv_order())),
        }
    }
}

#[pymethods]
impl NativeServerRequest {
    fn operation(&self) -> &str {
        &self.operation
    }

    /// Reads the `in` and `inout` arguments of the parameters
    /// `(name, CORBA.TypeCode, flags)`: their values, `CORBA.Any`s in
    /// order, None for the `out` ones.
    fn arguments<'py>(
        &self,
        py: Python<'py>,
        params: Vec<(String, Bound<'py, PyAny>, u32)>,
    ) -> PyResult<Vec<Option<Bound<'py, PyAny>>>> {
        let mut list = Vec::with_capacity(params.len());
        for (name, tc, flags) in &params {
            list.push(NamedValue::out(name.clone(), tc_from_py(tc)?));
            list.last_mut().unwrap().mode = mode(py, *flags)?;
        }
        let read = self.with(py, |request| {
            request
                .arguments(list)
                .map(|args| args.to_vec())
                .map_err(|e| sys_err(py, &e))
        })?;
        read.into_iter()
            .map(|nv| match nv.mode {
                ArgMode::Out => Ok(None),
                _ => any_to_py(py, nv.value).map(Some),
            })
            .collect()
    }

    /// Sets the values of the `out` and `inout` parameters, `CORBA.Any`s
    /// in the order of the parameters (None for the `in` ones).
    fn set_out_values(
        &self,
        py: Python<'_>,
        values: Vec<Option<Bound<'_, PyAny>>>,
    ) -> PyResult<()> {
        let mut anys = Vec::with_capacity(values.len());
        for v in &values {
            anys.push(v.as_ref().map(any_from_py).transpose()?);
        }
        self.with(py, |request| {
            let Some(params) = request.parameters_mut() else {
                return Err(sys_err(py, &bad_inv_order()));
            };
            for (nv, any) in params.iter_mut().zip(anys) {
                if let (true, Some(any)) = (nv.mode != ArgMode::In, any) {
                    nv.value = any;
                }
            }
            Ok(())
        })
    }

    fn set_result(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let any = any_from_py(value)?;
        self.with(py, |request| {
            request.set_result(any).map_err(|e| sys_err(py, &e))
        })
    }

    fn set_exception(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let any = any_from_py(value)?;
        self.with(py, |request| {
            request.set_exception(any).map_err(|e| sys_err(py, &e))
        })
    }
}
