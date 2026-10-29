//! The ORB and object references.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use std::time::Duration;

use pyo3::exceptions::{PyLookupError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyTuple};
use rustorb_core::cdr::{ByteOrder, CdrVersion, Decoder, Encoder};
use rustorb_core::giop::code_set;
use rustorb_core::{
    CodeSets, CompletionStatus, Exception, InStream, ObjectRef, Orb as CoreOrb, OutStream,
    PolicyValue, ResolveError, SetOverrideType, SystemException, SystemExceptionKind, minor,
};

use crate::error::{marshal, marshal_err, sys_err};
use crate::poa::Poa;
use crate::tc::{CompiledOp, TcRef, compile_op, compile_tc};
use crate::value::{Value, ValueScope, decode, encode, from_py, to_py};

/// An ORB (`CORBA.ORB` in Python wraps it).
#[pyclass(frozen, module = "rustorb._native")]
pub(crate) struct Orb {
    pub(crate) orb: CoreOrb,
}

#[pymethods]
impl Orb {
    /// Creates an ORB from command line arguments. Returns the ORB and the
    /// arguments that are not ORB options.
    #[staticmethod]
    fn init(py: Python<'_>, args: Vec<String>) -> PyResult<(Orb, Vec<String>)> {
        let mut args = args;
        let orb = py
            .detach(|| CoreOrb::init(&mut args))
            .map_err(|e| sys_err(py, &e))?;
        Ok((Orb { orb }, args))
    }

    /// The reply timeout of calls, in seconds; None for no limit.
    fn set_call_timeout(&self, seconds: Option<f64>) -> PyResult<()> {
        self.orb.set_call_timeout(limit(seconds)?);
        Ok(())
    }

    fn call_timeout(&self) -> Option<f64> {
        self.orb.call_timeout().map(|t| t.as_secs_f64())
    }

    /// The timeout of new connections, in seconds; None for no limit.
    fn set_connect_timeout(&self, seconds: Option<f64>) -> PyResult<()> {
        self.orb.set_connect_timeout(limit(seconds)?);
        Ok(())
    }

    fn connect_timeout(&self) -> Option<f64> {
        self.orb.connect_timeout().map(|t| t.as_secs_f64())
    }

    fn string_to_object(&self, py: Python<'_>, s: &str) -> PyResult<Option<ObjRef>> {
        let obj = py
            .detach(|| self.orb.string_to_object(s))
            .map_err(|e| sys_err(py, &e))?;
        Ok(obj.map(|obj| ObjRef { obj }))
    }

    #[pyo3(signature = (obj))]
    fn object_to_string(&self, py: Python<'_>, obj: Option<PyRef<'_, ObjRef>>) -> PyResult<String> {
        self.orb
            .object_to_string(obj.as_ref().map(|o| &o.obj))
            .map_err(|e| sys_err(py, &e))
    }

    /// Raises `LookupError` for an unknown name (the Python layer turns it
    /// into `CORBA.ORB.InvalidName`).
    fn resolve_initial_references(&self, py: Python<'_>, name: &str) -> PyResult<ObjRef> {
        match py.detach(|| self.orb.resolve_initial_references(name)) {
            Ok(obj) => Ok(ObjRef { obj }),
            Err(ResolveError::InvalidName(name)) => Err(PyLookupError::new_err(name)),
            Err(ResolveError::System(e)) => Err(sys_err(py, &e)),
        }
    }

    fn list_initial_services(&self) -> Vec<String> {
        self.orb.list_initial_services()
    }

    /// `register_initial_reference` of a reference (None for nil).
    /// Raises `LookupError` for a name that is empty or taken.
    fn register_initial_reference(
        &self,
        py: Python<'_>,
        name: &str,
        obj: Option<PyRef<'_, ObjRef>>,
    ) -> PyResult<()> {
        resolve_err(
            py,
            self.orb
                .register_initial_reference(name, obj.as_ref().map(|o| &o.obj)),
        )
    }

    /// `register_initial_reference` of a Python object that has no
    /// reference (a local object). The ORB keeps it until it is destroyed.
    fn register_local_initial_reference(
        &self,
        py: Python<'_>,
        name: &str,
        obj: Py<PyAny>,
    ) -> PyResult<()> {
        resolve_err(
            py,
            self.orb
                .register_local_initial_reference(name, std::sync::Arc::new(obj)),
        )
    }

    /// The object of `register_local_initial_reference`, or None.
    fn resolve_local_initial_reference(&self, py: Python<'_>, name: &str) -> Option<Py<PyAny>> {
        let obj = self.orb.resolve_local_initial_reference(name)?;
        obj.downcast_ref::<Py<PyAny>>().map(|o| o.clone_ref(py))
    }

    /// `create_policy` with the value in a `CORBA.Any`: returns
    /// `(value, None)` (see `PolicyValue::raw_value`) or `(None, reason)`
    /// for a `PolicyError`.
    fn create_policy(
        &self,
        policy_type: u32,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<(Option<u64>, Option<i16>)> {
        let any = crate::any::any_from_py(value)?;
        Ok(match self.orb.create_policy(policy_type, &any) {
            Ok(p) => (Some(p.raw_value()), None),
            Err(e) => (None, Some(e.reason)),
        })
    }

    /// The ORB's `PolicyManager`: policies are `(type, value)` pairs.
    fn set_policy_overrides(
        &self,
        py: Python<'_>,
        policies: Vec<(u32, u64)>,
        add: bool,
    ) -> PyResult<()> {
        let (list, set_add) = policy_list(py, policies, add)?;
        self.orb
            .set_policy_overrides(&list, set_add)
            .map_err(|e| sys_err(py, &e))
    }

    fn policy_overrides(&self, types: Vec<u32>) -> Vec<(u32, u64)> {
        raw_policies(self.orb.policy_overrides(&types))
    }

    fn root_poa(&self, py: Python<'_>) -> PyResult<Poa> {
        let poa = py
            .detach(|| self.orb.root_poa())
            .map_err(|e| sys_err(py, &e))?;
        Ok(Poa { poa })
    }

    /// The host and port of each endpoint the server side publishes.
    fn endpoints(&self) -> Vec<(String, u16)> {
        self.orb.endpoints()
    }

    /// Waits for the shutdown, running MAIN_THREAD_MODEL requests. Python
    /// signal handlers run every 100 ms, so Ctrl-C interrupts it with
    /// `KeyboardInterrupt`.
    fn run(&self, py: Python<'_>) -> PyResult<()> {
        let tick = Some(std::time::Duration::from_millis(100));
        while !py.detach(|| self.orb.run_timeout(tick)) {
            py.check_signals()?;
        }
        Ok(())
    }

    fn work_pending(&self, py: Python<'_>) -> PyResult<bool> {
        py.detach(|| self.orb.work_pending())
            .map_err(|e| sys_err(py, &e))
    }

    fn perform_work(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.orb.perform_work())
            .map_err(|e| sys_err(py, &e))
    }

    fn shutdown(&self, py: Python<'_>, wait_for_completion: bool) -> PyResult<()> {
        py.detach(|| self.orb.shutdown(wait_for_completion))
            .map_err(|e| sys_err(py, &e))
    }

    fn destroy(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.orb.destroy())
            .map_err(|e| sys_err(py, &e))
    }

    /// Makes the object `obj` answer to the object key `key` and returns
    /// a reference with that key (`corbaloc::host:port/KEY`).
    fn register_object_key(
        &self,
        py: Python<'_>,
        key: &str,
        obj: PyRef<'_, ObjRef>,
    ) -> PyResult<ObjRef> {
        let obj = self
            .orb
            .register_object_key(key, &obj.obj)
            .map_err(|e| sys_err(py, &e))?;
        Ok(ObjRef { obj })
    }

    fn unregister_object_key(&self, key: &str) {
        self.orb.unregister_object_key(key);
    }

    /// Marshals `value`, of the type `tc` (a `CORBA.TypeCode`), on its own
    /// (`omniORB.cdrMarshal`): as an encapsulation in the byte order of
    /// the host when `little_endian` is None, else as a plain stream in
    /// that byte order. See [`MARSHAL_CODE_SETS`].
    #[pyo3(signature = (tc, value, little_endian=None))]
    fn cdr_marshal<'py>(
        &self,
        py: Python<'py>,
        tc: &Bound<'py, PyAny>,
        value: &Bound<'py, PyAny>,
        little_endian: Option<bool>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let compiled = compile_tc(tc)?;
        let _scope = ValueScope::enter();
        let value = from_py(&compiled, value).map_err(|e| marshal_err(py, e))?;
        let mut enc = match little_endian {
            None => Encoder::encapsulation(host_order(), CdrVersion::Giop1_2),
            Some(little) => Encoder::new(order(little), CdrVersion::Giop1_2),
        };
        let mut out = OutStream::standalone(&mut enc);
        out.set_code_sets(MARSHAL_CODE_SETS);
        encode(&compiled, &value, &mut out).map_err(|e| sys_err(py, &e))?;
        Ok(PyBytes::new(py, enc.as_bytes()))
    }

    /// Reads what [`cdr_marshal`](Orb::cdr_marshal) writes
    /// (`omniORB.cdrUnmarshal`). Object references read belong to this
    /// ORB. `MARSHAL` if `data` is too short or has bytes left over.
    #[pyo3(signature = (tc, data, little_endian=None))]
    fn cdr_unmarshal<'py>(
        &self,
        py: Python<'py>,
        tc: &Bound<'py, PyAny>,
        data: &[u8],
        little_endian: Option<bool>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let compiled = compile_tc(tc)?;
        let _scope = ValueScope::enter();
        let dec = match little_endian {
            None => Decoder::encapsulation(data, CdrVersion::Giop1_2)
                .map_err(|_| sys_err(py, &marshal(CompletionStatus::No)))?,
            Some(little) => Decoder::new(data, order(little), CdrVersion::Giop1_2),
        };
        let value = py
            .detach(|| {
                let mut input = InStream::standalone(dec, Some(&self.orb));
                input.set_code_sets(MARSHAL_CODE_SETS);
                let value = decode(&compiled, &mut input, CompletionStatus::No)?;
                if !input.is_empty() {
                    return Err(marshal(CompletionStatus::No));
                }
                Ok(value)
            })
            .map_err(|e| sys_err(py, &e))?;
        to_py(py, &compiled, value)
    }
}

/// The code sets of `cdr_marshal` and `cdr_unmarshal`: ISO 8859-1 and
/// UTF-16, the native code sets of omniORB, which marshals the same way.
/// The bytes are often sent to another ORB as an octet sequence (as
/// OpenRTM-aist's data ports do), with no code set negotiation.
const MARSHAL_CODE_SETS: CodeSets = CodeSets {
    char_data: code_set::ISO_8859_1,
    wchar_data: Some(code_set::UTF_16),
};

fn order(little: bool) -> ByteOrder {
    if little {
        ByteOrder::Little
    } else {
        ByteOrder::Big
    }
}

fn host_order() -> ByteOrder {
    order(cfg!(target_endian = "little"))
}

/// An object reference (the `_obj` of a `CORBA.Object`).
#[pyclass(frozen, module = "rustorb._native")]
pub(crate) struct ObjRef {
    pub(crate) obj: ObjectRef,
}

/// The outcome of an invocation, decoded but not yet in Python.
enum Outcome {
    Return(Vec<Value>),
    /// A declared user exception: its index in the operation and members.
    User(usize, Value),
}

#[pymethods]
impl ObjRef {
    /// The repository id in the IOR.
    fn type_id(&self) -> &str {
        self.obj.type_id()
    }

    /// Calls `op`, a `_rt.Operation`, with the `in` and `inout` arguments
    /// `args`. Returns None, the single result or a tuple of the results.
    fn invoke<'py>(
        &self,
        py: Python<'py>,
        op: &Bound<'py, PyAny>,
        args: &Bound<'py, PyTuple>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let compiled = compile_op(op)?;
        let op: &CompiledOp = compiled.get();
        if args.len() != op.in_types.len() {
            return Err(sys_err(
                py,
                &SystemException::new(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No),
            ));
        }
        let _scope = ValueScope::enter();
        let mut values = Vec::with_capacity(args.len());
        for (tc, arg) in op.in_types.iter().zip(args.iter()) {
            values.push(from_py(tc, &arg).map_err(|e| marshal_err(py, e))?);
        }
        let ins = &op.in_types;
        let write = |out: &mut rustorb_core::OutStream<'_>| -> Result<(), SystemException> {
            for (tc, v) in ins.iter().zip(&values) {
                encode(tc, v, out)?;
            }
            Ok(())
        };
        if op.oneway {
            py.detach(|| self.obj.request(&op.name).args(write).send_oneway())
                .map_err(|e| sys_err(py, &e))?;
            return Ok(py.None().into_bound(py));
        }
        let outcome = py
            .detach(|| invoke(&self.obj, op, write))
            .map_err(|e| sys_err(py, &e))?;
        match outcome {
            Outcome::Return(results) => results_to_py(py, &op.out_types, results),
            Outcome::User(i, members) => {
                let exc = to_py(py, &op.exceptions[i].1, members)?;
                Err(PyErr::from_value(exc))
            }
        }
    }

    fn is_a(&self, py: Python<'_>, repository_id: &str) -> PyResult<bool> {
        py.detach(|| self.obj.is_a(repository_id))
            .map_err(|e| exc_err(py, e))
    }

    fn non_existent(&self, py: Python<'_>) -> PyResult<bool> {
        py.detach(|| self.obj.non_existent())
            .map_err(|e| exc_err(py, e))
    }

    fn is_equivalent(&self, other: PyRef<'_, ObjRef>) -> bool {
        self.obj.is_equivalent(&other.obj)
    }

    /// `_hash`: the same for equivalent references, below `maximum`.
    fn hash(&self, maximum: u32) -> u32 {
        let mut h = DefaultHasher::new();
        for p in &self.obj.ior().profiles {
            p.tag.hash(&mut h);
            p.profile_data.hash(&mut h);
        }
        let maximum = u64::from(maximum).max(1);
        // The result is below `maximum`, so it fits.
        u32::try_from(h.finish() % maximum).unwrap_or(0)
    }

    /// The stringified IOR.
    fn ior_string(&self) -> String {
        self.obj.ior().to_string()
    }

    /// The reply timeout of calls on this reference (shared by the
    /// references made from it), in seconds: None unsets it, inf is no
    /// limit.
    fn set_call_timeout(&self, seconds: Option<f64>) -> PyResult<()> {
        self.obj.set_call_timeout(setting(seconds)?);
        Ok(())
    }

    fn call_timeout(&self) -> Option<f64> {
        seconds_of_setting(self.obj.call_timeout())
    }

    /// `_set_policy_overrides`: a new reference.
    fn set_policy_overrides(
        &self,
        py: Python<'_>,
        policies: Vec<(u32, u64)>,
        add: bool,
    ) -> PyResult<ObjRef> {
        let (list, set_add) = policy_list(py, policies, add)?;
        self.obj
            .set_policy_overrides(&list, set_add)
            .map(|obj| ObjRef { obj })
            .map_err(|e| sys_err(py, &e))
    }

    fn policy_overrides(&self, types: Vec<u32>) -> Vec<(u32, u64)> {
        raw_policies(self.obj.policy_overrides(&types))
    }

    /// `_get_client_policy`: the value, or None.
    fn client_policy(&self, policy_type: u32) -> Option<u64> {
        self.obj.client_policy(policy_type).map(|p| p.raw_value())
    }

    fn validate_connection(&self, py: Python<'_>) -> PyResult<bool> {
        py.detach(|| self.obj.validate_connection())
            .map_err(|e| exc_err(py, e))
    }
}

/// The `PolicyCurrent` of the calling thread.
#[pyfunction]
pub(crate) fn set_thread_policy_overrides(
    py: Python<'_>,
    policies: Vec<(u32, u64)>,
    add: bool,
) -> PyResult<()> {
    let (list, set_add) = policy_list(py, policies, add)?;
    rustorb_core::policy::set_thread_policy_overrides(&list, set_add).map_err(|e| sys_err(py, &e))
}

#[pyfunction]
pub(crate) fn thread_policy_overrides(types: Vec<u32>) -> Vec<(u32, u64)> {
    raw_policies(rustorb_core::policy::thread_policy_overrides(&types))
}

/// `(type, value)` pairs as policies; `NO_PERMISSION` for one rustorb
/// does not know (it cannot be an override).
fn policy_list(
    py: Python<'_>,
    policies: Vec<(u32, u64)>,
    add: bool,
) -> PyResult<(Vec<PolicyValue>, SetOverrideType)> {
    let list = policies
        .into_iter()
        .map(|(t, v)| {
            PolicyValue::from_raw(t, v).ok_or_else(|| {
                sys_err(
                    py,
                    &SystemException::new(
                        SystemExceptionKind::NO_PERMISSION,
                        0,
                        CompletionStatus::No,
                    ),
                )
            })
        })
        .collect::<PyResult<_>>()?;
    let set_add = if add {
        SetOverrideType::Add
    } else {
        SetOverrideType::Set
    };
    Ok((list, set_add))
}

fn raw_policies(policies: Vec<PolicyValue>) -> Vec<(u32, u64)> {
    policies
        .into_iter()
        .map(|p| (p.policy_type(), p.raw_value()))
        .collect()
}

fn resolve_err(py: Python<'_>, r: Result<(), ResolveError>) -> PyResult<()> {
    match r {
        Ok(()) => Ok(()),
        Err(ResolveError::InvalidName(name)) => Err(PyLookupError::new_err(name)),
        Err(ResolveError::System(e)) => Err(sys_err(py, &e)),
    }
}

/// The reply timeout of calls from the calling thread, in seconds: None
/// unsets it, inf is no limit.
#[pyfunction]
pub(crate) fn set_thread_call_timeout(seconds: Option<f64>) -> PyResult<()> {
    rustorb_core::set_thread_call_timeout(setting(seconds)?);
    Ok(())
}

#[pyfunction]
pub(crate) fn thread_call_timeout() -> Option<f64> {
    seconds_of_setting(rustorb_core::thread_call_timeout())
}

/// Seconds to a timeout: None, 0 or inf is no limit.
fn limit(seconds: Option<f64>) -> PyResult<Option<Duration>> {
    match seconds {
        None => Ok(None),
        Some(s) if s.is_nan() || s < 0.0 => {
            Err(PyValueError::new_err("timeout must not be negative"))
        }
        Some(s) if s == 0.0 || s.is_infinite() => Ok(None),
        Some(s) => Ok(Some(
            Duration::try_from_secs_f64(s).unwrap_or(Duration::MAX),
        )),
    }
}

/// Seconds to the setting of a thread or a reference: None unsets it.
fn setting(seconds: Option<f64>) -> PyResult<Option<Option<Duration>>> {
    seconds.map(|s| limit(Some(s))).transpose()
}

fn seconds_of_setting(setting: Option<Option<Duration>>) -> Option<f64> {
    setting.map(|t| t.map_or(f64::INFINITY, |t| t.as_secs_f64()))
}

/// Sends the request and decodes the reply, detached from Python.
fn invoke<F>(obj: &ObjectRef, op: &CompiledOp, write: F) -> Result<Outcome, SystemException>
where
    F: Fn(&mut rustorb_core::OutStream<'_>) -> Result<(), SystemException>,
{
    match obj.request(&op.name).args(write).invoke() {
        Ok(reply) => {
            let mut input = reply.input();
            let results = decode_all(&op.out_types, &mut input, CompletionStatus::Yes)?;
            Ok(Outcome::Return(results))
        }
        Err(Exception::System(e)) => Err(e),
        Err(Exception::User(u)) => {
            let Some(i) = op
                .exceptions
                .iter()
                .position(|(id, _)| id == u.repository_id())
            else {
                return Err(SystemException::new(
                    SystemExceptionKind::UNKNOWN,
                    minor::UNLISTED_USER_EXCEPTION,
                    CompletionStatus::Yes,
                ));
            };
            let mut input = u.input()?;
            let members = decode(&op.exceptions[i].1, &mut input, CompletionStatus::Yes)?;
            Ok(Outcome::User(i, members))
        }
    }
}

pub(crate) fn decode_all(
    types: &[TcRef],
    input: &mut rustorb_core::InStream<'_>,
    completed: CompletionStatus,
) -> Result<Vec<Value>, SystemException> {
    types
        .iter()
        .map(|tc| decode(tc, input, completed))
        .collect()
}

/// The results of an operation as Python returns them: None, the single
/// result, or a tuple.
fn results_to_py<'py>(
    py: Python<'py>,
    types: &[TcRef],
    results: Vec<Value>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut objs = Vec::with_capacity(results.len());
    for (tc, v) in types.iter().zip(results) {
        objs.push(to_py(py, tc, v)?);
    }
    Ok(match objs.len() {
        0 => py.None().into_bound(py),
        1 => objs.pop().expect("one result"),
        _ => PyTuple::new(py, objs)?.into_any(),
    })
}

fn exc_err(py: Python<'_>, e: Exception) -> PyErr {
    match e {
        Exception::System(e) => sys_err(py, &e),
        Exception::User(_) => sys_err(
            py,
            &SystemException::new(
                SystemExceptionKind::UNKNOWN,
                minor::UNLISTED_USER_EXCEPTION,
                CompletionStatus::Yes,
            ),
        ),
    }
}
