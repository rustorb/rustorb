//! The POA, POA managers, Python servants and servant managers.
//!
//! A Python servant is given to the POA as a [`PyServant`], which
//! implements the core's [`Servant`]: it decodes the arguments with the
//! operation's TypeCodes, attaches to Python only to call the method, and
//! encodes the results detached again. Each Python servant object has one
//! `PyServant` while the POA holds it, so the POA sees the same servant
//! every time (`servant_to_id`, UNIQUE_ID).

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyTuple};
use rustorb_core::{
    AdapterActivator, CompletionStatus, Cookie, IdAssignment, IdUniqueness, ImplicitActivation,
    InStream, Lifespan, ObjectId, Poa as CorePoa, PoaCurrent, PoaManager as CorePoaManager,
    PoaManagerState, Policy, RequestProcessing, Servant, ServantActivator, ServantLocator,
    ServantManager, ServantManagerError, ServantRetention, ServerReply, SystemException,
    SystemExceptionKind, ThreadPolicy, minor,
};

use crate::error::{corba, poa_err, portable_server, rt, sys, sys_err, sys_from_py};
use crate::orb::{ObjRef, decode_all};
use crate::tc::{CompiledOp, compile_op};
use crate::value::{Value, ValueScope, encode, from_py, to_py};

// --- registry of the Rust side of Python objects ---------------------------------

/// The `PyServant` of each Python servant object, by the object's address.
static SERVANTS: Mutex<Option<HashMap<usize, Weak<PyServant>>>> = Mutex::new(None);
/// The Python object of each adapter this module made (servants, servant
/// managers, adapter activators), by the adapter's address, to give the
/// object back to Python from the `Arc<dyn ...>` the POA returns.
static OBJECTS: Mutex<Option<HashMap<usize, Py<PyAny>>>> = Mutex::new(None);

fn register<T>(adapter: &Arc<T>, obj: &Py<PyAny>, py: Python<'_>) {
    let key = Arc::as_ptr(adapter) as *const () as usize;
    OBJECTS
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(key, obj.clone_ref(py));
}

fn unregister(addr: usize) {
    let removed = OBJECTS
        .lock()
        .unwrap()
        .as_mut()
        .and_then(|m| m.remove(&addr));
    // Dropped outside the lock: releasing the object may run Python code.
    drop(removed);
}

/// The Python object of an adapter made by this module.
fn python_object<'py, T: ?Sized>(py: Python<'py>, adapter: &Arc<T>) -> Option<Bound<'py, PyAny>> {
    let key = Arc::as_ptr(adapter) as *const () as usize;
    let objects = OBJECTS.lock().unwrap();
    objects.as_ref()?.get(&key).map(|obj| obj.bind(py).clone())
}

/// The Python object of a servant the POA returned.
fn servant_object<'py>(py: Python<'py>, servant: &Arc<dyn Servant>) -> PyResult<Bound<'py, PyAny>> {
    python_object(py, servant).ok_or_else(|| {
        sys_err(
            py,
            &sys(SystemExceptionKind::OBJ_ADAPTER, 0, CompletionStatus::No),
        )
    })
}

// --- servants --------------------------------------------------------------------

/// A Python servant (an instance of `PortableServer.Servant`) as the POA
/// sees it.
pub(crate) struct PyServant {
    obj: Py<PyAny>,
    repository_id: String,
    key: usize,
}

impl Drop for PyServant {
    fn drop(&mut self) {
        unregister(self as *const PyServant as usize);
        let mut servants = SERVANTS.lock().unwrap();
        if let Some(map) = servants.as_mut() {
            if map.get(&self.key).is_some_and(|w| w.upgrade().is_none()) {
                map.remove(&self.key);
            }
        }
    }
}

/// The `PyServant` of a Python servant object, made the first time.
pub(crate) fn servant_arc(obj: &Bound<'_, PyAny>) -> PyResult<Arc<dyn Servant>> {
    let py = obj.py();
    let base = portable_server(py)?.getattr("Servant")?;
    if !obj.is_instance(&base)? {
        return Err(sys_err(
            py,
            &sys(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No),
        ));
    }
    let key = obj.as_ptr() as usize;
    if let Some(s) = SERVANTS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&key))
        .and_then(Weak::upgrade)
    {
        return Ok(s);
    }
    let repository_id: String = obj.getattr("_NP_RepositoryId")?.extract()?;
    let servant = Arc::new(PyServant {
        obj: obj.clone().unbind(),
        repository_id,
        key,
    });
    register(&servant, &servant.obj, py);
    SERVANTS
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(key, Arc::downgrade(&servant));
    Ok(servant)
}

/// What the Python method of an upcall produced, before encoding.
enum Upcall {
    Return(Vec<Value>),
    User(usize, Value),
}

impl Servant for PyServant {
    fn repository_id(&self) -> &str {
        &self.repository_id
    }

    fn is_a(&self, repository_id: &str) -> bool {
        Python::attach(|py| {
            self.obj
                .bind(py)
                .call_method1("_is_a", (repository_id,))
                .and_then(|r| r.is_truthy())
                .unwrap_or_else(|e| {
                    e.print(py);
                    false
                })
        })
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        let op: Py<CompiledOp> = Python::attach(|py| {
            let class = self.obj.bind(py).get_type();
            let op = rt(py)
                .and_then(|rt| rt.getattr("operation"))
                .and_then(|f| f.call1((class, operation)))
                .map_err(|e| sys_from_py(py, &e))?;
            if op.is_none() {
                return Err(sys(
                    SystemExceptionKind::BAD_OPERATION,
                    minor::UNKNOWN_OPERATION,
                    CompletionStatus::No,
                ));
            }
            compile_op(&op)
                .map(Bound::unbind)
                .map_err(|e| sys_from_py(py, &e))
        })?;
        let op = op.get();
        let _scope = ValueScope::enter();
        let args = decode_all(&op.in_types, input, CompletionStatus::No)?;
        let outcome = Python::attach(|py| self.call(py, op, args))?;
        match outcome {
            Upcall::Return(results) => {
                let mut out = reply.output();
                for (tc, v) in op.out_types.iter().zip(&results) {
                    encode(tc, v, &mut out).map_err(maybe)?;
                }
            }
            Upcall::User(i, members) => {
                let (id, tc) = &op.exceptions[i];
                let mut out = reply.user_exception(id)?;
                encode(tc, &members, &mut out).map_err(maybe)?;
            }
        }
        Ok(())
    }
}

/// An error after the servant ran: completed maybe.
fn maybe(mut e: SystemException) -> SystemException {
    e.completed = CompletionStatus::Maybe;
    e
}

impl PyServant {
    /// Calls the method of `op` with `args` and converts what it returned
    /// or raised.
    fn call(
        &self,
        py: Python<'_>,
        op: &CompiledOp,
        args: Vec<Value>,
    ) -> Result<Upcall, SystemException> {
        let completed_no = |e: PyErr| {
            let mut e = sys_from_py(py, &e);
            if e.is(SystemExceptionKind::UNKNOWN) {
                e = sys(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No);
            }
            e
        };
        let mut py_args = Vec::with_capacity(args.len());
        for (tc, v) in op.in_types.iter().zip(args) {
            py_args.push(to_py(py, tc, v).map_err(completed_no)?);
        }
        let servant = self.obj.bind(py);
        // A servant without the method does not implement the operation.
        let method = servant
            .getattr(op.method.as_str())
            .map_err(|_| sys(SystemExceptionKind::NO_IMPLEMENT, 0, CompletionStatus::No))?;
        let result = PyTuple::new(py, py_args).and_then(|args| method.call1(args));
        match result {
            Ok(result) => self.results(py, op, &result).map_err(|e| {
                // The method returned something that does not match the
                // operation.
                let mut e = sys_from_py(py, &e);
                if e.is(SystemExceptionKind::UNKNOWN) {
                    e = sys(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::Maybe);
                }
                maybe(e)
            }),
            Err(err) => self.raised(py, op, err),
        }
    }

    fn results(
        &self,
        py: Python<'_>,
        op: &CompiledOp,
        result: &Bound<'_, PyAny>,
    ) -> PyResult<Upcall> {
        let types = &op.out_types;
        let mut values = Vec::with_capacity(types.len());
        match types.len() {
            0 => {}
            1 => values.push(from_py(&types[0], result)?),
            n => {
                let items = result.cast::<PyTuple>()?;
                if items.len() != n {
                    return Err(sys_err(
                        py,
                        &sys(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::Maybe),
                    ));
                }
                for (tc, item) in types.iter().zip(items.iter()) {
                    values.push(from_py(tc, &item)?);
                }
            }
        }
        Ok(Upcall::Return(values))
    }

    /// What the method raised: a declared user exception, a system
    /// exception, or anything else as `UNKNOWN` (with its traceback on
    /// standard error).
    fn raised(
        &self,
        py: Python<'_>,
        op: &CompiledOp,
        err: PyErr,
    ) -> Result<Upcall, SystemException> {
        let value = err.value(py);
        let declared = value
            .getattr("_NP_RepositoryId")
            .and_then(|id| id.extract::<String>())
            .ok()
            .and_then(|id| op.exceptions.iter().position(|(eid, _)| *eid == id));
        if let Some(i) = declared {
            let members = from_py(&op.exceptions[i].1, value).map_err(|e| {
                e.print(py);
                sys(SystemExceptionKind::UNKNOWN, 0, CompletionStatus::Maybe)
            })?;
            return Ok(Upcall::User(i, members));
        }
        let is = |name: &str| {
            corba(py)
                .and_then(|c| c.getattr(name))
                .map(|base| err.is_instance(py, &base))
                .unwrap_or(false)
        };
        if is("SystemException") {
            return Err(sys_from_py(py, &err));
        }
        // A Python error or an undeclared user exception.
        err.print(py);
        let minor = if is("UserException") {
            minor::UNLISTED_USER_EXCEPTION
        } else {
            0
        };
        Err(sys(
            SystemExceptionKind::UNKNOWN,
            minor,
            CompletionStatus::Maybe,
        ))
    }
}

// --- servant managers and adapter activators --------------------------------------

/// Calls a method of a Python servant manager, turning
/// `PortableServer.ForwardRequest` and CORBA system exceptions into a
/// [`ServantManagerError`].
fn manager_call<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
    method: &str,
    args: impl pyo3::call::PyCallArgs<'py>,
) -> Result<Bound<'py, PyAny>, ServantManagerError> {
    obj.call_method1(method, args).map_err(|err| {
        let forward = portable_server(py)
            .and_then(|ps| ps.getattr("ForwardRequest"))
            .map(|cls| err.is_instance(py, &cls))
            .unwrap_or(false);
        if forward {
            let target = err
                .value(py)
                .getattr("forward_reference")
                .and_then(|r| r.getattr("_obj"))
                .ok()
                .and_then(|r| r.cast_into::<ObjRef>().ok());
            if let Some(target) = target {
                return ServantManagerError::ForwardRequest(target.get().obj.clone());
            }
        }
        let e = sys_from_py(py, &err);
        if e.is(SystemExceptionKind::UNKNOWN) {
            err.print(py);
        }
        ServantManagerError::System(e)
    })
}

fn wrap_poa<'py>(py: Python<'py>, poa: &CorePoa) -> PyResult<Bound<'py, PyAny>> {
    portable_server(py)?
        .getattr("_wrap_poa")?
        .call1((Poa { poa: poa.clone() },))
}

struct PyActivator {
    obj: Py<PyAny>,
}

impl Drop for PyActivator {
    fn drop(&mut self) {
        unregister(self as *const PyActivator as usize);
    }
}

impl ServantActivator for PyActivator {
    fn incarnate(
        &self,
        id: &[u8],
        adapter: &CorePoa,
    ) -> Result<Arc<dyn Servant>, ServantManagerError> {
        Python::attach(|py| {
            let poa = wrap_poa(py, adapter)
                .map_err(|e| ServantManagerError::System(sys_from_py(py, &e)))?;
            let servant = manager_call(
                py,
                self.obj.bind(py),
                "incarnate",
                (PyBytes::new(py, id), poa),
            )?;
            servant_arc(&servant).map_err(|e| ServantManagerError::System(sys_from_py(py, &e)))
        })
    }

    fn etherealize(
        &self,
        id: &[u8],
        adapter: &CorePoa,
        servant: Arc<dyn Servant>,
        cleanup_in_progress: bool,
        remaining_activations: bool,
    ) {
        Python::attach(|py| {
            let call = || -> PyResult<()> {
                let poa = wrap_poa(py, adapter)?;
                let servant = servant_object(py, &servant)?;
                self.obj.bind(py).call_method1(
                    "etherealize",
                    (
                        PyBytes::new(py, id),
                        poa,
                        servant,
                        cleanup_in_progress,
                        remaining_activations,
                    ),
                )?;
                Ok(())
            };
            if let Err(e) = call() {
                e.print(py);
            }
            // The POA's reference goes now, while attached.
            drop(servant);
        });
    }
}

struct PyLocator {
    obj: Py<PyAny>,
}

impl Drop for PyLocator {
    fn drop(&mut self) {
        unregister(self as *const PyLocator as usize);
    }
}

impl ServantLocator for PyLocator {
    fn preinvoke(
        &self,
        id: &[u8],
        adapter: &CorePoa,
        operation: &str,
    ) -> Result<(Arc<dyn Servant>, Cookie), ServantManagerError> {
        Python::attach(|py| {
            let system = |e: PyErr| ServantManagerError::System(sys_from_py(py, &e));
            let poa = wrap_poa(py, adapter).map_err(system)?;
            let result = manager_call(
                py,
                self.obj.bind(py),
                "preinvoke",
                (PyBytes::new(py, id), poa, operation),
            )?;
            // The IDL to Python mapping returns the servant and the cookie
            // (the out argument) as a tuple.
            let (servant, cookie): (Bound<'_, PyAny>, Bound<'_, PyAny>) =
                result.extract().map_err(system)?;
            let servant = servant_arc(&servant).map_err(system)?;
            let cookie: Cookie = Box::new(cookie.unbind());
            Ok((servant, cookie))
        })
    }

    fn postinvoke(
        &self,
        id: &[u8],
        adapter: &CorePoa,
        operation: &str,
        cookie: Cookie,
        servant: Arc<dyn Servant>,
    ) {
        Python::attach(|py| {
            let call = || -> PyResult<()> {
                let poa = wrap_poa(py, adapter)?;
                let obj = servant_object(py, &servant)?;
                let cookie = match (cookie as Box<dyn Any>).downcast::<Py<PyAny>>() {
                    Ok(c) => c.bind(py).clone(),
                    Err(_) => py.None().into_bound(py),
                };
                self.obj.bind(py).call_method1(
                    "postinvoke",
                    (PyBytes::new(py, id), poa, operation, cookie, obj),
                )?;
                Ok(())
            };
            if let Err(e) = call() {
                e.print(py);
            }
            drop(servant);
        });
    }
}

struct PyAdapterActivator {
    obj: Py<PyAny>,
}

impl Drop for PyAdapterActivator {
    fn drop(&mut self) {
        unregister(self as *const PyAdapterActivator as usize);
    }
}

impl AdapterActivator for PyAdapterActivator {
    fn unknown_adapter(&self, parent: &CorePoa, name: &str) -> bool {
        Python::attach(|py| {
            let call = || -> PyResult<bool> {
                let poa = wrap_poa(py, parent)?;
                self.obj
                    .bind(py)
                    .call_method1("unknown_adapter", (poa, name))?
                    .is_truthy()
            };
            call().unwrap_or_else(|e| {
                e.print(py);
                false
            })
        })
    }
}

// --- the POA ---------------------------------------------------------------------

/// The policy type ids of `PortableServer` and the values of each, in the
/// order of their IDL enums.
fn policy(policy_type: u32, value: u32) -> Option<Policy> {
    Some(match (policy_type, value) {
        (16, 0) => Policy::Thread(ThreadPolicy::OrbCtrl),
        (16, 1) => Policy::Thread(ThreadPolicy::SingleThread),
        (16, 2) => Policy::Thread(ThreadPolicy::MainThread),
        (17, 0) => Policy::Lifespan(Lifespan::Transient),
        (17, 1) => Policy::Lifespan(Lifespan::Persistent),
        (18, 0) => Policy::IdUniqueness(IdUniqueness::Unique),
        (18, 1) => Policy::IdUniqueness(IdUniqueness::Multiple),
        (19, 0) => Policy::IdAssignment(IdAssignment::User),
        (19, 1) => Policy::IdAssignment(IdAssignment::System),
        (20, 0) => Policy::ImplicitActivation(ImplicitActivation::Implicit),
        (20, 1) => Policy::ImplicitActivation(ImplicitActivation::NoImplicit),
        (21, 0) => Policy::ServantRetention(ServantRetention::Retain),
        (21, 1) => Policy::ServantRetention(ServantRetention::NonRetain),
        (22, 0) => Policy::RequestProcessing(RequestProcessing::ActiveObjectMapOnly),
        (22, 1) => Policy::RequestProcessing(RequestProcessing::UseDefaultServant),
        (22, 2) => Policy::RequestProcessing(RequestProcessing::UseServantManager),
        _ => return None,
    })
}

fn objref(
    py: Python<'_>,
    r: Result<rustorb_core::ObjectRef, rustorb_core::PoaError>,
) -> PyResult<ObjRef> {
    r.map(|obj| ObjRef { obj }).map_err(|e| poa_err(py, e))
}

fn oid(
    py: Python<'_>,
    r: Result<ObjectId, rustorb_core::PoaError>,
) -> PyResult<Bound<'_, PyBytes>> {
    r.map(|id| PyBytes::new(py, &id))
        .map_err(|e| poa_err(py, e))
}

/// A POA (`PortableServer.POA` in Python wraps it). Calls that may wait
/// for requests detach from Python, so that servants can run meanwhile.
#[pyclass(frozen, module = "rustorb._native", eq)]
pub(crate) struct Poa {
    pub(crate) poa: CorePoa,
}

impl PartialEq for Poa {
    fn eq(&self, other: &Poa) -> bool {
        self.poa == other.poa
    }
}

#[pymethods]
impl Poa {
    fn the_name(&self) -> &str {
        self.poa.the_name()
    }

    fn the_parent(&self) -> Option<Poa> {
        self.poa.the_parent().map(|poa| Poa { poa })
    }

    fn the_children(&self) -> Vec<Poa> {
        self.poa
            .the_children()
            .into_iter()
            .map(|poa| Poa { poa })
            .collect()
    }

    fn the_poa_manager(&self) -> PoaManager {
        PoaManager {
            manager: self.poa.the_poa_manager(),
        }
    }

    /// `create_POA`. `policies` are `(policy_type, value)` pairs; one that
    /// rustorb does not know raises `InvalidPolicy` with its index.
    fn create_poa(
        &self,
        py: Python<'_>,
        name: &str,
        manager: Option<PyRef<'_, PoaManager>>,
        policies: Vec<(u32, u32)>,
    ) -> PyResult<Poa> {
        let mut list = Vec::with_capacity(policies.len());
        for (i, (t, v)) in policies.into_iter().enumerate() {
            let p = policy(t, v).ok_or_else(|| {
                poa_err(
                    py,
                    rustorb_core::PoaError::InvalidPolicy(u16::try_from(i).unwrap_or(u16::MAX)),
                )
            })?;
            list.push(p);
        }
        let manager = manager.map(|m| m.manager.clone());
        py.detach(|| self.poa.create_poa(name, manager.as_ref(), &list))
            .map(|poa| Poa { poa })
            .map_err(|e| poa_err(py, e))
    }

    fn find_poa(&self, py: Python<'_>, name: &str, activate_it: bool) -> PyResult<Poa> {
        py.detach(|| self.poa.find_poa(name, activate_it))
            .map(|poa| Poa { poa })
            .map_err(|e| poa_err(py, e))
    }

    fn destroy(
        &self,
        py: Python<'_>,
        etherealize_objects: bool,
        wait_for_completion: bool,
    ) -> PyResult<()> {
        py.detach(|| self.poa.destroy(etherealize_objects, wait_for_completion))
            .map_err(|e| poa_err(py, e))
    }

    fn activate_object<'py>(
        &self,
        py: Python<'py>,
        servant: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let servant = servant_arc(servant)?;
        oid(py, py.detach(|| self.poa.activate_object(servant)))
    }

    fn activate_object_with_id(
        &self,
        py: Python<'_>,
        id: &[u8],
        servant: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let servant = servant_arc(servant)?;
        py.detach(|| self.poa.activate_object_with_id(id, servant))
            .map_err(|e| poa_err(py, e))
    }

    fn deactivate_object(&self, py: Python<'_>, id: &[u8]) -> PyResult<()> {
        py.detach(|| self.poa.deactivate_object(id))
            .map_err(|e| poa_err(py, e))
    }

    fn servant_to_id<'py>(
        &self,
        py: Python<'py>,
        servant: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let servant = servant_arc(servant)?;
        oid(py, py.detach(|| self.poa.servant_to_id(&servant)))
    }

    fn servant_to_reference(&self, py: Python<'_>, servant: &Bound<'_, PyAny>) -> PyResult<ObjRef> {
        let servant = servant_arc(servant)?;
        objref(py, py.detach(|| self.poa.servant_to_reference(&servant)))
    }

    fn id_to_servant<'py>(&self, py: Python<'py>, id: &[u8]) -> PyResult<Bound<'py, PyAny>> {
        let servant = self.poa.id_to_servant(id).map_err(|e| poa_err(py, e))?;
        servant_object(py, &servant)
    }

    fn id_to_reference(&self, py: Python<'_>, id: &[u8]) -> PyResult<ObjRef> {
        objref(py, self.poa.id_to_reference(id))
    }

    fn reference_to_id<'py>(
        &self,
        py: Python<'py>,
        reference: PyRef<'_, ObjRef>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        oid(py, self.poa.reference_to_id(&reference.obj))
    }

    fn reference_to_servant<'py>(
        &self,
        py: Python<'py>,
        reference: PyRef<'_, ObjRef>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let servant = self
            .poa
            .reference_to_servant(&reference.obj)
            .map_err(|e| poa_err(py, e))?;
        servant_object(py, &servant)
    }

    fn create_reference(&self, py: Python<'_>, intf: &str) -> PyResult<ObjRef> {
        objref(py, self.poa.create_reference(intf))
    }

    fn create_reference_with_id(&self, py: Python<'_>, id: &[u8], intf: &str) -> PyResult<ObjRef> {
        objref(py, self.poa.create_reference_with_id(id, intf))
    }

    fn get_servant<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let servant = self.poa.get_servant().map_err(|e| poa_err(py, e))?;
        servant_object(py, &servant)
    }

    fn set_servant(&self, py: Python<'_>, servant: &Bound<'_, PyAny>) -> PyResult<()> {
        let servant = servant_arc(servant)?;
        py.detach(|| self.poa.set_servant(servant))
            .map_err(|e| poa_err(py, e))
    }

    /// The Python servant manager, or None.
    fn get_servant_manager<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        let manager = self.poa.get_servant_manager().map_err(|e| poa_err(py, e))?;
        Ok(match manager {
            None => None,
            Some(ServantManager::Activator(a)) => python_object(py, &a),
            Some(ServantManager::Locator(l)) => python_object(py, &l),
        })
    }

    /// `set_servant_manager` with a `ServantActivator` (`locator` false)
    /// or a `ServantLocator`.
    fn set_servant_manager(
        &self,
        py: Python<'_>,
        manager: &Bound<'_, PyAny>,
        locator: bool,
    ) -> PyResult<()> {
        let obj = manager.clone().unbind();
        let manager = if locator {
            let l = Arc::new(PyLocator { obj });
            register(&l, &l.obj, py);
            ServantManager::Locator(l)
        } else {
            let a = Arc::new(PyActivator { obj });
            register(&a, &a.obj, py);
            ServantManager::Activator(a)
        };
        py.detach(|| self.poa.set_servant_manager(manager))
            .map_err(|e| poa_err(py, e))
    }

    fn get_the_activator<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyAny>> {
        self.poa.the_activator().and_then(|a| python_object(py, &a))
    }

    fn set_the_activator(
        &self,
        py: Python<'_>,
        activator: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let activator: Option<Arc<dyn AdapterActivator>> = activator.map(|obj| {
            let a = Arc::new(PyAdapterActivator {
                obj: obj.clone().unbind(),
            });
            register(&a, &a.obj, py);
            a as Arc<dyn AdapterActivator>
        });
        py.detach(|| self.poa.set_the_activator(activator))
            .map_err(|e| poa_err(py, e))
    }
}

/// A POA manager (`PortableServer.POAManager` in Python wraps it).
#[pyclass(frozen, module = "rustorb._native", eq)]
pub(crate) struct PoaManager {
    manager: CorePoaManager,
}

impl PartialEq for PoaManager {
    fn eq(&self, other: &PoaManager) -> bool {
        self.manager == other.manager
    }
}

#[pymethods]
impl PoaManager {
    fn activate(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.manager.activate())
            .map_err(|e| poa_err(py, e))
    }

    fn hold_requests(&self, py: Python<'_>, wait_for_completion: bool) -> PyResult<()> {
        py.detach(|| self.manager.hold_requests(wait_for_completion))
            .map_err(|e| poa_err(py, e))
    }

    fn discard_requests(&self, py: Python<'_>, wait_for_completion: bool) -> PyResult<()> {
        py.detach(|| self.manager.discard_requests(wait_for_completion))
            .map_err(|e| poa_err(py, e))
    }

    fn deactivate(
        &self,
        py: Python<'_>,
        etherealize_objects: bool,
        wait_for_completion: bool,
    ) -> PyResult<()> {
        py.detach(|| {
            self.manager
                .deactivate(etherealize_objects, wait_for_completion)
        })
        .map_err(|e| poa_err(py, e))
    }

    /// The state, as the position in `POAManager::State` (HOLDING, ACTIVE,
    /// DISCARDING, INACTIVE).
    fn get_state(&self) -> u32 {
        match self.manager.state() {
            PoaManagerState::Holding => 0,
            PoaManagerState::Active => 1,
            PoaManagerState::Discarding => 2,
            PoaManagerState::Inactive => 3,
        }
    }
}

// --- PortableServer::Current -----------------------------------------------------

#[pyfunction]
pub(crate) fn current_poa(py: Python<'_>) -> PyResult<Poa> {
    PoaCurrent::get_poa()
        .map(|poa| Poa { poa })
        .map_err(|e| poa_err(py, e))
}

#[pyfunction]
pub(crate) fn current_object_id(py: Python<'_>) -> PyResult<Bound<'_, PyBytes>> {
    oid(py, PoaCurrent::get_object_id())
}

#[pyfunction]
pub(crate) fn current_reference(py: Python<'_>) -> PyResult<ObjRef> {
    objref(py, PoaCurrent::get_reference())
}

#[pyfunction]
pub(crate) fn current_servant(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let servant = PoaCurrent::get_servant().map_err(|e| poa_err(py, e))?;
    servant_object(py, &servant)
}
