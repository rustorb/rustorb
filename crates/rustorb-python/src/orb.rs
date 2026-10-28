//! The ORB and object references.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use pyo3::exceptions::PyLookupError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyTuple};
use rustorb_core::cdr::{ByteOrder, CdrVersion, Decoder, Encoder};
use rustorb_core::giop::code_set;
use rustorb_core::{
    CodeSets, CompletionStatus, Exception, InStream, ObjectRef, Orb as CoreOrb, OutStream,
    ResolveError, SystemException, SystemExceptionKind, minor,
};

use crate::error::{marshal, marshal_err, sys_err};
use crate::poa::Poa;
use crate::tc::{CompiledOp, TcRef, compile_op, compile_tc};
use crate::value::{Value, decode, encode, from_py, to_py};

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
