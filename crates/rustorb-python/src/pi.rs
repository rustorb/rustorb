//! Portable Interceptors and `IOP::Codec` (docs/portable-interceptors.md):
//! the core's interceptor traits implemented by Python objects, and the
//! native halves of the info objects, `PICurrent` and the codecs.
//! `rustorb.PortableInterceptor` and `rustorb.IOP` wrap them in the
//! classes of the IDL to Python mapping.
//!
//! An info object borrows the core's for the time of one interception
//! point: it holds a pointer that the point clears before it returns, so
//! an info object kept past its point raises `BAD_INV_ORDER`. Uses of the
//! pointer run with the GIL held, which orders them. A use can run Python
//! code (a servant's `is_a`, the constructor of an exception), and so let
//! another thread in, so the point counts the uses running and waits for
//! them, with the GIL released, before it returns; the mutable
//! `ORBInitInfo` takes one use at a time.

use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use pyo3::exceptions::{PyIndexError, PyLookupError, PyValueError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyBytes;
use rustorb_core::iop::{
    Codec, CodecError, CodecFactory, Encoding, ServiceContext, TaggedComponent,
};
use rustorb_core::pi::{
    ClientRequestInfo, ClientRequestInterceptor, InterceptError, Interceptor, IorInfo,
    IorInterceptor, OrbInitInfo, OrbInitializer, Parameter, PiCurrent, ServerRequestInfo,
    ServerRequestInterceptor, minor,
};
use rustorb_core::{
    CompletionStatus, PolicyValue, ResolveError, SystemException, SystemExceptionKind,
};

use crate::any::{any_from_py, any_to_py, tc_from_py, tc_to_py};
use crate::error::{sys_err, sys_from_py};
use crate::orb::ObjRef;

static PI: PyOnceLock<Py<PyModule>> = PyOnceLock::new();

/// `rustorb.PortableInterceptor`.
fn pi_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    PI.get_or_try_init(py, || {
        Ok::<_, PyErr>(py.import("rustorb.PortableInterceptor")?.unbind())
    })
    .map(|m| m.bind(py).clone())
}

fn invalid_point() -> SystemException {
    SystemException::new(
        SystemExceptionKind::BAD_INV_ORDER,
        minor::INVALID_POINT,
        CompletionStatus::No,
    )
}

/// A pointer to an info object of the core, valid while its point runs
/// (see the module's documentation), and how many calls use it now.
struct Borrowed<T> {
    ptr: AtomicPtr<T>,
    users: AtomicUsize,
}

/// One use of a [`Borrowed`] pointer.
struct Use<'a>(&'a AtomicUsize);

impl Drop for Use<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl<T> Borrowed<T> {
    fn new(ptr: *mut T) -> Borrowed<T> {
        Borrowed {
            ptr: AtomicPtr::new(ptr),
            users: AtomicUsize::new(0),
        }
    }

    /// The pointer, valid until the `Use` is dropped; `exclusive` refuses
    /// a second use at the same time (`BAD_INV_ORDER`).
    fn enter(&self, py: Python<'_>, exclusive: bool) -> PyResult<(*mut T, Use<'_>)> {
        let ptr = self.ptr.load(Ordering::Acquire);
        if ptr.is_null() {
            return Err(sys_err(py, &invalid_point()));
        }
        let before = self.users.fetch_add(1, Ordering::AcqRel);
        let usage = Use(&self.users);
        if exclusive && before > 0 {
            return Err(sys_err(py, &invalid_point()));
        }
        Ok((ptr, usage))
    }

    /// Ends the point: no new use starts, and the ones running (Python
    /// code run by a use can let another thread in) finish first. Waits
    /// with the GIL released, as those uses need it.
    fn close(&self, py: Python<'_>) {
        self.ptr.store(std::ptr::null_mut(), Ordering::Release);
        while self.users.load(Ordering::Acquire) > 0 {
            py.detach(std::thread::yield_now);
        }
    }
}

fn sys(py: Python<'_>) -> impl Fn(SystemException) -> PyErr + '_ {
    move |e| sys_err(py, &e)
}

fn policy(p: PolicyValue) -> (u32, u64) {
    (p.policy_type(), p.raw_value())
}

fn parameters<'py>(
    py: Python<'py>,
    params: Vec<Parameter>,
) -> PyResult<Vec<(Bound<'py, PyAny>, u32)>> {
    params
        .into_iter()
        .map(|p| {
            let mode = match p.mode {
                rustorb_core::dii::ArgMode::In => 0,
                rustorb_core::dii::ArgMode::Out => 1,
                rustorb_core::dii::ArgMode::InOut => 2,
            };
            Ok((any_to_py(py, p.argument)?, mode))
        })
        .collect()
}

fn context(sc: ServiceContext) -> (u32, Vec<u8>) {
    (sc.context_id, sc.context_data)
}

fn component(c: TaggedComponent) -> (u32, Vec<u8>) {
    (c.tag, c.component_data)
}

// --- request info ------------------------------------------------------------

/// The native half of `PortableInterceptor.ClientRequestInfo`.
#[pyclass(frozen, module = "rustorb._native")]
pub(crate) struct ClientInfo {
    ri: Borrowed<ClientRequestInfo<'static>>,
}

impl ClientInfo {
    fn with<R>(
        &self,
        py: Python<'_>,
        f: impl FnOnce(&ClientRequestInfo<'_>) -> PyResult<R>,
    ) -> PyResult<R> {
        let (ptr, _use) = self.ri.enter(py, false)?;
        // SAFETY: see the module's documentation; `close` waits for this
        // use, and the closure cannot keep what it borrows, as it works for
        // any lifetime.
        f(unsafe { &*ptr })
    }
}

#[pymethods]
impl ClientInfo {
    fn request_id(&self, py: Python<'_>) -> PyResult<u32> {
        self.with(py, |ri| Ok(ri.request_id()))
    }

    fn operation(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |ri| Ok(ri.operation().to_owned()))
    }

    fn arguments<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, u32)>> {
        let params = self.with(py, |ri| ri.arguments().map_err(sys(py)))?;
        parameters(py, params)
    }

    fn exceptions<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let tcs = self.with(py, |ri| ri.exceptions().map_err(sys(py)))?;
        tcs.iter().map(|tc| tc_to_py(py, tc)).collect()
    }

    fn contexts(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.with(py, |ri| ri.contexts().map_err(sys(py)))
    }

    fn operation_context(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.with(py, |ri| ri.operation_context().map_err(sys(py)))
    }

    fn result<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let any = self.with(py, |ri| ri.result().map_err(sys(py)))?;
        any_to_py(py, any)
    }

    fn response_expected(&self, py: Python<'_>) -> PyResult<bool> {
        self.with(py, |ri| Ok(ri.response_expected()))
    }

    fn sync_scope(&self, py: Python<'_>) -> PyResult<i16> {
        self.with(py, |ri| Ok(ri.sync_scope()))
    }

    fn reply_status(&self, py: Python<'_>) -> PyResult<i16> {
        self.with(py, |ri| ri.reply_status().map_err(sys(py)))
    }

    fn forward_reference(&self, py: Python<'_>) -> PyResult<ObjRef> {
        self.with(py, |ri| {
            Ok(ObjRef {
                obj: ri.forward_reference().map_err(sys(py))?,
            })
        })
    }

    /// Raises `IndexError` for `InvalidSlot`.
    fn get_slot<'py>(&self, py: Python<'py>, id: u32) -> PyResult<Bound<'py, PyAny>> {
        let any = self.with(py, |ri| {
            ri.get_slot(id)
                .map_err(|_| PyIndexError::new_err("InvalidSlot"))
        })?;
        any_to_py(py, any)
    }

    fn get_request_service_context(&self, py: Python<'_>, id: u32) -> PyResult<(u32, Vec<u8>)> {
        self.with(py, |ri| {
            ri.get_request_service_context(id)
                .map(context)
                .map_err(sys(py))
        })
    }

    fn get_reply_service_context(&self, py: Python<'_>, id: u32) -> PyResult<(u32, Vec<u8>)> {
        self.with(py, |ri| {
            ri.get_reply_service_context(id)
                .map(context)
                .map_err(sys(py))
        })
    }

    fn target(&self, py: Python<'_>) -> PyResult<ObjRef> {
        self.with(py, |ri| {
            Ok(ObjRef {
                obj: ri.target().clone(),
            })
        })
    }

    fn effective_target(&self, py: Python<'_>) -> PyResult<ObjRef> {
        self.with(py, |ri| {
            Ok(ObjRef {
                obj: ri.effective_target().clone(),
            })
        })
    }

    fn effective_profile(&self, py: Python<'_>) -> PyResult<(u32, Vec<u8>)> {
        self.with(py, |ri| {
            let p = ri.effective_profile();
            Ok((p.tag, p.profile_data.clone()))
        })
    }

    fn received_exception<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let any = self.with(py, |ri| ri.received_exception().map_err(sys(py)))?;
        any_to_py(py, any)
    }

    fn received_exception_id(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |ri| ri.received_exception_id().map_err(sys(py)))
    }

    fn get_effective_component(&self, py: Python<'_>, tag: u32) -> PyResult<(u32, Vec<u8>)> {
        self.with(py, |ri| {
            ri.get_effective_component(tag)
                .map(component)
                .map_err(sys(py))
        })
    }

    fn get_effective_components(&self, py: Python<'_>, tag: u32) -> PyResult<Vec<(u32, Vec<u8>)>> {
        self.with(py, |ri| {
            ri.get_effective_components(tag)
                .map(|cs| cs.into_iter().map(component).collect())
                .map_err(sys(py))
        })
    }

    /// `(type, value)`, or None when no policy of the type is in effect.
    fn get_request_policy(&self, py: Python<'_>, policy_type: u32) -> PyResult<Option<(u32, u64)>> {
        self.with(py, |ri| {
            ri.get_request_policy(policy_type)
                .map(|p| p.map(policy))
                .map_err(sys(py))
        })
    }

    fn add_request_service_context(
        &self,
        py: Python<'_>,
        id: u32,
        data: Vec<u8>,
        replace: bool,
    ) -> PyResult<()> {
        self.with(py, |ri| {
            ri.add_request_service_context(
                ServiceContext {
                    context_id: id,
                    context_data: data,
                },
                replace,
            )
            .map_err(sys(py))
        })
    }
}

/// The native half of `PortableInterceptor.ServerRequestInfo`.
#[pyclass(frozen, module = "rustorb._native")]
pub(crate) struct ServerInfo {
    ri: Borrowed<ServerRequestInfo<'static>>,
}

impl ServerInfo {
    fn with<R>(
        &self,
        py: Python<'_>,
        f: impl FnOnce(&ServerRequestInfo<'_>) -> PyResult<R>,
    ) -> PyResult<R> {
        let (ptr, _use) = self.ri.enter(py, false)?;
        // SAFETY: as in `ClientInfo::with`.
        f(unsafe { &*ptr })
    }
}

#[pymethods]
impl ServerInfo {
    fn request_id(&self, py: Python<'_>) -> PyResult<u32> {
        self.with(py, |ri| Ok(ri.request_id()))
    }

    fn operation(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |ri| Ok(ri.operation().to_owned()))
    }

    fn arguments<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, u32)>> {
        let params = self.with(py, |ri| ri.arguments().map_err(sys(py)))?;
        parameters(py, params)
    }

    fn exceptions<'py>(&self, py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let tcs = self.with(py, |ri| ri.exceptions().map_err(sys(py)))?;
        tcs.iter().map(|tc| tc_to_py(py, tc)).collect()
    }

    fn contexts(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.with(py, |ri| ri.contexts().map_err(sys(py)))
    }

    fn operation_context(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.with(py, |ri| ri.operation_context().map_err(sys(py)))
    }

    fn result<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let any = self.with(py, |ri| ri.result().map_err(sys(py)))?;
        any_to_py(py, any)
    }

    fn response_expected(&self, py: Python<'_>) -> PyResult<bool> {
        self.with(py, |ri| Ok(ri.response_expected()))
    }

    fn sync_scope(&self, py: Python<'_>) -> PyResult<i16> {
        self.with(py, |ri| Ok(ri.sync_scope()))
    }

    fn reply_status(&self, py: Python<'_>) -> PyResult<i16> {
        self.with(py, |ri| ri.reply_status().map_err(sys(py)))
    }

    fn forward_reference(&self, py: Python<'_>) -> PyResult<ObjRef> {
        self.with(py, |ri| {
            Ok(ObjRef {
                obj: ri.forward_reference().map_err(sys(py))?,
            })
        })
    }

    /// Raises `IndexError` for `InvalidSlot`.
    fn get_slot<'py>(&self, py: Python<'py>, id: u32) -> PyResult<Bound<'py, PyAny>> {
        let any = self.with(py, |ri| {
            ri.get_slot(id)
                .map_err(|_| PyIndexError::new_err("InvalidSlot"))
        })?;
        any_to_py(py, any)
    }

    /// Raises `IndexError` for `InvalidSlot`.
    fn set_slot(&self, py: Python<'_>, id: u32, data: &Bound<'_, PyAny>) -> PyResult<()> {
        let any = any_from_py(data)?;
        self.with(py, |ri| {
            ri.set_slot(id, any)
                .map_err(|_| PyIndexError::new_err("InvalidSlot"))
        })
    }

    fn get_request_service_context(&self, py: Python<'_>, id: u32) -> PyResult<(u32, Vec<u8>)> {
        self.with(py, |ri| {
            ri.get_request_service_context(id)
                .map(context)
                .map_err(sys(py))
        })
    }

    fn get_reply_service_context(&self, py: Python<'_>, id: u32) -> PyResult<(u32, Vec<u8>)> {
        self.with(py, |ri| {
            ri.get_reply_service_context(id)
                .map(context)
                .map_err(sys(py))
        })
    }

    fn add_reply_service_context(
        &self,
        py: Python<'_>,
        id: u32,
        data: Vec<u8>,
        replace: bool,
    ) -> PyResult<()> {
        self.with(py, |ri| {
            ri.add_reply_service_context(
                ServiceContext {
                    context_id: id,
                    context_data: data,
                },
                replace,
            )
            .map_err(sys(py))
        })
    }

    fn sending_exception<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let any = self.with(py, |ri| ri.sending_exception().map_err(sys(py)))?;
        any_to_py(py, any)
    }

    fn sending_exception_id(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |ri| ri.sending_exception_id().map_err(sys(py)))
    }

    fn object_id<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let id = self.with(py, |ri| ri.object_id().map_err(sys(py)))?;
        Ok(PyBytes::new(py, &id))
    }

    fn adapter_id<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let id = self.with(py, |ri| ri.adapter_id().map_err(sys(py)))?;
        Ok(PyBytes::new(py, &id))
    }

    fn adapter_name(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.with(py, |ri| ri.adapter_name().map_err(sys(py)))
    }

    fn orb_id(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |ri| Ok(ri.orb_id().to_owned()))
    }

    fn server_id(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |ri| Ok(ri.server_id().to_owned()))
    }

    fn target_most_derived_interface(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |ri| ri.target_most_derived_interface().map_err(sys(py)))
    }

    fn target_is_a(&self, py: Python<'_>, id: &str) -> PyResult<bool> {
        // The servant's is_a may be Python code: it runs with the GIL held,
        // as everything else here.
        self.with(py, |ri| ri.target_is_a(id).map_err(sys(py)))
    }

    fn get_server_policy(&self, py: Python<'_>, policy_type: u32) -> PyResult<(u32, u64)> {
        self.with(py, |ri| {
            ri.get_server_policy(policy_type)
                .map(policy)
                .map_err(sys(py))
        })
    }
}

/// The native half of `PortableInterceptor.IORInfo`.
#[pyclass(frozen, module = "rustorb._native")]
pub(crate) struct IorInfoHandle {
    info: Borrowed<IorInfo<'static>>,
}

impl IorInfoHandle {
    fn with<R>(&self, py: Python<'_>, f: impl FnOnce(&IorInfo<'_>) -> PyResult<R>) -> PyResult<R> {
        let (ptr, _use) = self.info.enter(py, false)?;
        // SAFETY: as in `ClientInfo::with`.
        f(unsafe { &*ptr })
    }
}

#[pymethods]
impl IorInfoHandle {
    fn get_effective_policy(&self, py: Python<'_>, policy_type: u32) -> PyResult<(u32, u64)> {
        self.with(py, |info| {
            info.get_effective_policy(policy_type)
                .map(policy)
                .map_err(sys(py))
        })
    }

    fn adapter_name(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.with(py, |info| Ok(info.adapter_name().to_vec()))
    }

    fn add_ior_component(&self, py: Python<'_>, tag: u32, data: Vec<u8>) -> PyResult<()> {
        self.with(py, |info| {
            info.add_ior_component(TaggedComponent {
                tag,
                component_data: data,
            });
            Ok(())
        })
    }

    fn add_ior_component_to_profile(
        &self,
        py: Python<'_>,
        tag: u32,
        data: Vec<u8>,
        profile_id: u32,
    ) -> PyResult<()> {
        self.with(py, |info| {
            info.add_ior_component_to_profile(
                TaggedComponent {
                    tag,
                    component_data: data,
                },
                profile_id,
            )
            .map_err(sys(py))
        })
    }
}

/// The native half of `PortableInterceptor.ORBInitInfo`.
#[pyclass(frozen, module = "rustorb._native")]
pub(crate) struct InitInfo {
    info: Borrowed<OrbInitInfo<'static>>,
}

impl InitInfo {
    fn with<R>(
        &self,
        py: Python<'_>,
        f: impl FnOnce(&mut OrbInitInfo<'_>) -> PyResult<R>,
    ) -> PyResult<R> {
        let (ptr, _use) = self.info.enter(py, true)?;
        // SAFETY: as in `ClientInfo::with`; the initializer has the info
        // mutably for the time of its call, and this use is the only one.
        f(unsafe { &mut *ptr })
    }
}

/// `add_*_interceptor`'s result: `LookupError` for `DuplicateName`.
fn duplicate(r: Result<(), rustorb_core::pi::DuplicateName>) -> PyResult<()> {
    r.map_err(|e| PyLookupError::new_err(e.0))
}

#[pymethods]
impl InitInfo {
    fn arguments(&self, py: Python<'_>) -> PyResult<Vec<String>> {
        self.with(py, |info| Ok(info.arguments().to_vec()))
    }

    fn orb_id(&self, py: Python<'_>) -> PyResult<String> {
        self.with(py, |info| Ok(info.orb_id().to_owned()))
    }

    fn codec_factory(&self, py: Python<'_>) -> PyResult<NativeCodecFactory> {
        self.with(py, |info| {
            Ok(NativeCodecFactory {
                factory: info.codec_factory(),
            })
        })
    }

    fn pi_current(&self, py: Python<'_>) -> PyResult<NativePiCurrent> {
        self.with(py, |info| {
            Ok(NativePiCurrent {
                current: info.pi_current(),
            })
        })
    }

    /// Raises `LookupError` for `InvalidName`.
    fn register_initial_reference(
        &self,
        py: Python<'_>,
        name: &str,
        obj: Option<PyRef<'_, ObjRef>>,
    ) -> PyResult<()> {
        self.with(py, |info| {
            match info.register_initial_reference(name, obj.as_ref().map(|o| &o.obj)) {
                Ok(()) => Ok(()),
                Err(ResolveError::InvalidName(name)) => Err(PyLookupError::new_err(name)),
                Err(ResolveError::System(e)) => Err(sys_err(py, &e)),
            }
        })
    }

    /// `register_initial_reference` of a local object, as
    /// `Orb.register_local_initial_reference`.
    fn register_local_initial_reference(
        &self,
        py: Python<'_>,
        name: &str,
        obj: Py<PyAny>,
    ) -> PyResult<()> {
        self.with(py, |info| {
            match info
                .orb()
                .register_local_initial_reference(name, Arc::new(obj))
            {
                Ok(()) => Ok(()),
                Err(ResolveError::InvalidName(name)) => Err(PyLookupError::new_err(name)),
                Err(ResolveError::System(e)) => Err(sys_err(py, &e)),
            }
        })
    }

    /// Raises `LookupError` for `InvalidName`.
    fn resolve_initial_references(&self, py: Python<'_>, name: &str) -> PyResult<ObjRef> {
        self.with(py, |info| match info.resolve_initial_references(name) {
            Ok(obj) => Ok(ObjRef { obj }),
            Err(ResolveError::InvalidName(name)) => Err(PyLookupError::new_err(name)),
            Err(ResolveError::System(e)) => Err(sys_err(py, &e)),
        })
    }

    /// The local object of `register_local_initial_reference`, or None.
    fn resolve_local_initial_reference(
        &self,
        py: Python<'_>,
        name: &str,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.with(py, |info| {
            Ok(info
                .orb()
                .resolve_local_initial_reference(name)
                .and_then(|o| o.downcast_ref::<Py<PyAny>>().map(|o| o.clone_ref(py))))
        })
    }

    fn add_client_request_interceptor(
        &self,
        py: Python<'_>,
        interceptor: Py<PyAny>,
        name: String,
    ) -> PyResult<()> {
        let i = Arc::new(PyInterceptor {
            obj: interceptor,
            name,
        });
        self.with(py, |info| duplicate(info.add_client_request_interceptor(i)))
    }

    fn add_server_request_interceptor(
        &self,
        py: Python<'_>,
        interceptor: Py<PyAny>,
        name: String,
    ) -> PyResult<()> {
        let i = Arc::new(PyInterceptor {
            obj: interceptor,
            name,
        });
        self.with(py, |info| duplicate(info.add_server_request_interceptor(i)))
    }

    fn add_ior_interceptor(
        &self,
        py: Python<'_>,
        interceptor: Py<PyAny>,
        name: String,
    ) -> PyResult<()> {
        let i = Arc::new(PyInterceptor {
            obj: interceptor,
            name,
        });
        self.with(py, |info| duplicate(info.add_ior_interceptor(i)))
    }

    fn allocate_slot_id(&self, py: Python<'_>) -> PyResult<u32> {
        self.with(py, |info| Ok(info.allocate_slot_id()))
    }

    fn register_policy_factory(&self, py: Python<'_>, policy_type: u32) -> PyResult<()> {
        self.with(py, |info| {
            info.register_policy_factory(policy_type).map_err(sys(py))
        })
    }
}

// --- PICurrent and the codecs -----------------------------------------------

/// The native half of `PortableInterceptor.Current`.
#[pyclass(frozen, module = "rustorb._native")]
pub(crate) struct NativePiCurrent {
    pub(crate) current: PiCurrent,
}

#[pymethods]
impl NativePiCurrent {
    /// Raises `IndexError` for `InvalidSlot`.
    fn get_slot<'py>(&self, py: Python<'py>, id: u32) -> PyResult<Bound<'py, PyAny>> {
        let any = self
            .current
            .get_slot(id)
            .map_err(|_| PyIndexError::new_err("InvalidSlot"))?;
        any_to_py(py, any)
    }

    /// Raises `IndexError` for `InvalidSlot`.
    fn set_slot(&self, data: &Bound<'_, PyAny>, id: u32) -> PyResult<()> {
        let any = any_from_py(data)?;
        self.current
            .set_slot(id, any)
            .map_err(|_| PyIndexError::new_err("InvalidSlot"))
    }
}

/// The native half of `IOP.CodecFactory`.
#[pyclass(frozen, module = "rustorb._native")]
pub(crate) struct NativeCodecFactory {
    pub(crate) factory: CodecFactory,
}

#[pymethods]
impl NativeCodecFactory {
    /// Raises `LookupError` for `UnknownEncoding`.
    fn create_codec(&self, format: i16, major: u8, minor: u8) -> PyResult<NativeCodec> {
        let encoding = Encoding {
            format,
            major_version: major,
            minor_version: minor,
        };
        self.factory
            .create_codec(encoding)
            .map(|codec| NativeCodec { codec })
            .map_err(|e| PyLookupError::new_err(e.to_string()))
    }
}

/// The native half of `IOP.Codec`. Its errors are `ValueError`s whose
/// argument names the user exception of `IOP.Codec`.
#[pyclass(frozen, module = "rustorb._native")]
pub(crate) struct NativeCodec {
    codec: Codec,
}

fn codec_err(e: CodecError) -> PyErr {
    PyValueError::new_err(match e {
        CodecError::InvalidTypeForEncoding => "InvalidTypeForEncoding",
        CodecError::FormatMismatch => "FormatMismatch",
        CodecError::TypeMismatch => "TypeMismatch",
    })
}

#[pymethods]
impl NativeCodec {
    fn encode<'py>(
        &self,
        py: Python<'py>,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let any = any_from_py(data).map_err(|_| codec_err(CodecError::InvalidTypeForEncoding))?;
        let bytes = self.codec.encode(&any).map_err(codec_err)?;
        Ok(PyBytes::new(py, &bytes))
    }

    fn decode<'py>(&self, py: Python<'py>, data: &[u8]) -> PyResult<Bound<'py, PyAny>> {
        let any = self.codec.decode(data).map_err(codec_err)?;
        any_to_py(py, any)
    }

    fn encode_value<'py>(
        &self,
        py: Python<'py>,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let any = any_from_py(data).map_err(|_| codec_err(CodecError::InvalidTypeForEncoding))?;
        let bytes = self.codec.encode_value(&any).map_err(codec_err)?;
        Ok(PyBytes::new(py, &bytes))
    }

    fn decode_value<'py>(
        &self,
        py: Python<'py>,
        data: &[u8],
        tc: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let tc = tc_from_py(tc)?;
        let any = self.codec.decode_value(data, &tc).map_err(codec_err)?;
        any_to_py(py, any)
    }
}

// --- Python objects as the core's interceptors -------------------------------

/// An interceptor written in Python: the Python object, wrapped by
/// `rustorb.PortableInterceptor` so that its points take the info objects
/// of the mapping.
struct PyInterceptor {
    obj: Py<PyAny>,
    name: String,
}

/// What a Python exception raised at a point stands for:
/// `PortableInterceptor.ForwardRequest` as it is (where the point allows
/// it), anything else a system exception (`UNKNOWN` if it is not one).
fn intercept_err(py: Python<'_>, err: &PyErr) -> InterceptError {
    let forward = || -> PyResult<Option<rustorb_core::ObjectRef>> {
        let class = pi_module(py)?.getattr("ForwardRequest")?;
        let value = err.value(py);
        if !value.is_instance(&class)? {
            return Ok(None);
        }
        let target = value.getattr("forward")?.call_method0("_orb_object")?;
        let target: PyRef<'_, ObjRef> = target.extract()?;
        Ok(Some(target.obj.clone()))
    };
    match forward() {
        Ok(Some(obj)) => InterceptError::ForwardRequest(obj),
        _ => InterceptError::System(sys_from_py(py, err)),
    }
}

/// `ForwardRequest` where the point cannot raise it is `BAD_INV_ORDER`.
fn system_only(e: InterceptError) -> SystemException {
    match e {
        InterceptError::System(e) => e,
        InterceptError::ForwardRequest(_) => invalid_point(),
    }
}

impl PyInterceptor {
    /// Calls `point` of the wrapper with a new `ClientInfo` for `ri`.
    fn client(&self, point: &str, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        Python::attach(|py| {
            let ptr: *const ClientRequestInfo<'_> = ri;
            let info = Py::new(
                py,
                ClientInfo {
                    ri: Borrowed::new(ptr.cast::<ClientRequestInfo<'static>>().cast_mut()),
                },
            )
            .map_err(|e| intercept_err(py, &e))?;
            let r = self.obj.bind(py).call_method1(point, (info.clone_ref(py),));
            info.get().ri.close(py);
            r.map(|_| ()).map_err(|e| intercept_err(py, &e))
        })
    }

    fn server(&self, point: &str, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        Python::attach(|py| {
            let ptr: *const ServerRequestInfo<'_> = ri;
            let info = Py::new(
                py,
                ServerInfo {
                    ri: Borrowed::new(ptr.cast::<ServerRequestInfo<'static>>().cast_mut()),
                },
            )
            .map_err(|e| intercept_err(py, &e))?;
            let r = self.obj.bind(py).call_method1(point, (info.clone_ref(py),));
            info.get().ri.close(py);
            r.map(|_| ()).map_err(|e| intercept_err(py, &e))
        })
    }
}

impl Interceptor for PyInterceptor {
    fn name(&self) -> &str {
        &self.name
    }

    fn destroy(&self) {
        Python::attach(|py| {
            if let Err(e) = self.obj.bind(py).call_method0("destroy") {
                e.print(py);
            }
        });
    }
}

impl ClientRequestInterceptor for PyInterceptor {
    fn send_request(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        self.client("send_request", ri)
    }

    fn receive_reply(&self, ri: &ClientRequestInfo<'_>) -> Result<(), SystemException> {
        self.client("receive_reply", ri).map_err(system_only)
    }

    fn receive_exception(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        self.client("receive_exception", ri)
    }

    fn receive_other(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        self.client("receive_other", ri)
    }
}

impl ServerRequestInterceptor for PyInterceptor {
    fn receive_request_service_contexts(
        &self,
        ri: &ServerRequestInfo<'_>,
    ) -> Result<(), InterceptError> {
        self.server("receive_request_service_contexts", ri)
    }

    fn receive_request(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        self.server("receive_request", ri)
    }

    fn send_reply(&self, ri: &ServerRequestInfo<'_>) -> Result<(), SystemException> {
        self.server("send_reply", ri).map_err(system_only)
    }

    fn send_exception(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        self.server("send_exception", ri)
    }

    fn send_other(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        self.server("send_other", ri)
    }
}

impl IorInterceptor for PyInterceptor {
    fn establish_components(&self, info: &IorInfo<'_>) -> Result<(), SystemException> {
        Python::attach(|py| {
            let ptr: *const IorInfo<'_> = info;
            let handle = Py::new(
                py,
                IorInfoHandle {
                    info: Borrowed::new(ptr.cast::<IorInfo<'static>>().cast_mut()),
                },
            )
            .map_err(|e| sys_from_py(py, &e))?;
            let r = self
                .obj
                .bind(py)
                .call_method1("establish_components", (handle.clone_ref(py),));
            handle.get().info.close(py);
            r.map(|_| ()).map_err(|e| sys_from_py(py, &e))
        })
    }
}

/// An `ORBInitializer` written in Python, wrapped as the interceptors are.
pub(crate) struct PyInitializer {
    pub(crate) obj: Py<PyAny>,
}

impl PyInitializer {
    fn call(&self, point: &str, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        Python::attach(|py| {
            let ptr: *mut OrbInitInfo<'_> = info;
            let handle = Py::new(
                py,
                InitInfo {
                    info: Borrowed::new(ptr.cast::<OrbInitInfo<'static>>()),
                },
            )
            .map_err(|e| sys_from_py(py, &e))?;
            let r = self
                .obj
                .bind(py)
                .call_method1(point, (handle.clone_ref(py),));
            handle.get().info.close(py);
            r.map(|_| ()).map_err(|e| sys_from_py(py, &e))
        })
    }
}

impl OrbInitializer for PyInitializer {
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        self.call("pre_init", info)
    }

    fn post_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        self.call("post_init", info)
    }
}
