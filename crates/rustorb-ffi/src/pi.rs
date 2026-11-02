//! Portable Interceptors and `IOP::Codec` (docs/portable-interceptors.md).
//!
//! Interceptors and ORB initializers are the caller's objects behind a
//! table of callbacks, reference counted like servant managers. Each point
//! gets a handle of the core's info object, valid only during the callback
//! (the bindings check that themselves), and returns a status, after
//! `rorb_pi_error_*` when it raises.

use std::ffi::{c_char, c_int, c_void};
use std::sync::Arc;

use rustorb_core::iop::{Codec, CodecError, Encoding, ServiceContext, TaggedComponent};
use rustorb_core::pi::{
    ClientRequestInfo, ClientRequestInterceptor, InterceptError, Interceptor, IorInfo,
    IorInterceptor, OrbInitInfo, OrbInitializer, Parameter, PiCurrent, ServerRequestInfo,
    ServerRequestInterceptor,
};
use rustorb_core::{
    Any, CompletionStatus, ObjectRef, Orb, OrbOptions, PolicyValue, ResolveError, SystemException,
    SystemExceptionKind, TypeCode,
};

use crate::{
    Bytes, Exception, OK, Status, completion_status, guard, into_handle, local, set_local,
    set_system, str_arg, sys,
};

// --- errors of the caller's points -------------------------------------------

/// `rorb_pi_error`: what a point raised.
#[derive(Debug, Default)]
pub struct PiError(Option<InterceptError>);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_pi_error_forward(error: *mut PiError, obj: *mut ObjectRef) {
    let error = unsafe { &mut *error };
    error.0 = Some(match unsafe { obj.as_ref() } {
        Some(obj) => InterceptError::ForwardRequest(obj.clone()),
        None => {
            InterceptError::System(sys(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No))
        }
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_pi_error_system(
    error: *mut PiError,
    id: *const c_char,
    len: usize,
    minor: u32,
    completed: u32,
) {
    let error = unsafe { &mut *error };
    let id = unsafe { str_arg(id, len) }.unwrap_or("IDL:omg.org/CORBA/UNKNOWN:1.0");
    error.0 = Some(InterceptError::System(SystemException {
        repository_id: id.to_owned(),
        minor,
        completed: completion_status(completed),
    }));
}

/// What a point that returned `status` raised.
fn raised(status: Status, error: PiError) -> Result<(), InterceptError> {
    if status == OK {
        return Ok(());
    }
    Err(error.0.unwrap_or_else(|| {
        InterceptError::System(sys(
            SystemExceptionKind::UNKNOWN,
            0,
            CompletionStatus::Maybe,
        ))
    }))
}

fn invalid_point() -> SystemException {
    sys(
        SystemExceptionKind::BAD_INV_ORDER,
        rustorb_core::pi::minor::INVALID_POINT,
        CompletionStatus::No,
    )
}

/// `ForwardRequest` where the point cannot raise it is `BAD_INV_ORDER`.
fn system_only(r: Result<(), InterceptError>) -> Result<(), SystemException> {
    match r {
        Ok(()) => Ok(()),
        Err(InterceptError::System(e)) => Err(e),
        Err(InterceptError::ForwardRequest(_)) => Err(invalid_point()),
    }
}

// --- the caller's objects ------------------------------------------------------

/// The info objects as the callbacks see them.
pub type ClientInfo = ClientRequestInfo<'static>;
pub type ServerInfo = ServerRequestInfo<'static>;
pub type IorInfoHandle = IorInfo<'static>;
pub type InitInfo = OrbInitInfo<'static>;

type Point<I> = Option<extern "C" fn(*mut c_void, *const I, *mut PiError) -> Status>;

/// `rorb_client_interceptor_vtable`.
#[repr(C)]
#[derive(Debug)]
pub struct ClientVtable {
    pub send_request: Point<ClientInfo>,
    pub receive_reply: Point<ClientInfo>,
    pub receive_exception: Point<ClientInfo>,
    pub receive_other: Point<ClientInfo>,
    pub destroy: Option<extern "C" fn(*mut c_void)>,
    pub add_ref: extern "C" fn(*mut c_void),
    pub remove_ref: extern "C" fn(*mut c_void),
}

/// `rorb_server_interceptor_vtable`.
#[repr(C)]
#[derive(Debug)]
pub struct ServerVtable {
    pub receive_request_service_contexts: Point<ServerInfo>,
    pub receive_request: Point<ServerInfo>,
    pub send_reply: Point<ServerInfo>,
    pub send_exception: Point<ServerInfo>,
    pub send_other: Point<ServerInfo>,
    pub destroy: Option<extern "C" fn(*mut c_void)>,
    pub add_ref: extern "C" fn(*mut c_void),
    pub remove_ref: extern "C" fn(*mut c_void),
}

/// `rorb_ior_interceptor_vtable`.
#[repr(C)]
#[derive(Debug)]
pub struct IorVtable {
    pub establish_components: Point<IorInfoHandle>,
    pub destroy: Option<extern "C" fn(*mut c_void)>,
    pub add_ref: extern "C" fn(*mut c_void),
    pub remove_ref: extern "C" fn(*mut c_void),
}

/// `rorb_orb_initializer_vtable`.
#[repr(C)]
#[derive(Debug)]
pub struct InitializerVtable {
    pub pre_init: Option<extern "C" fn(*mut c_void, *mut InitInfo, *mut PiError) -> Status>,
    pub post_init: Option<extern "C" fn(*mut c_void, *mut InitInfo, *mut PiError) -> Status>,
    pub add_ref: extern "C" fn(*mut c_void),
    pub remove_ref: extern "C" fn(*mut c_void),
}

/// `rorb_orb_initializer`: an initializer of `rorb_orb_init_pi`.
#[repr(C)]
#[derive(Debug)]
pub struct InitializerEntry {
    pub this: *mut c_void,
    pub vtable: *const InitializerVtable,
}

trait Refs {
    fn add_ref(&self) -> extern "C" fn(*mut c_void);
    fn remove_ref(&self) -> extern "C" fn(*mut c_void);
}

macro_rules! refs {
    ($($vt:ty),*) => {$(
        impl Refs for $vt {
            fn add_ref(&self) -> extern "C" fn(*mut c_void) {
                self.add_ref
            }
            fn remove_ref(&self) -> extern "C" fn(*mut c_void) {
                self.remove_ref
            }
        }
    )*};
}

refs!(ClientVtable, ServerVtable, IorVtable, InitializerVtable);

/// One of the caller's objects with its callbacks and, for an
/// interceptor, its name.
struct Caller<V: Refs> {
    this: *mut c_void,
    vtable: *const V,
    name: String,
}

// The caller's objects are called from any thread, as the header says.
unsafe impl<V: Refs> Send for Caller<V> {}
unsafe impl<V: Refs> Sync for Caller<V> {}

impl<V: Refs> Caller<V> {
    fn new(this: *mut c_void, vtable: *const V, name: String) -> Arc<Self> {
        (unsafe { &*vtable }.add_ref())(this);
        Arc::new(Caller { this, vtable, name })
    }

    fn vtable(&self) -> &V {
        unsafe { &*self.vtable }
    }
}

impl<V: Refs> Drop for Caller<V> {
    fn drop(&mut self) {
        (self.vtable().remove_ref())(self.this);
    }
}

fn call<I>(this: *mut c_void, point: Point<I>, info: &I) -> Result<(), InterceptError> {
    let Some(point) = point else {
        return Ok(());
    };
    let mut error = PiError::default();
    let status = point(this, info, &mut error);
    raised(status, error)
}

/// The info as the `'static` type of the callbacks: valid only during
/// them, as the header says.
fn erase<T, U>(info: &T) -> &U {
    unsafe { &*(info as *const T).cast::<U>() }
}

macro_rules! interceptor {
    ($vt:ty) => {
        impl Interceptor for Caller<$vt> {
            fn name(&self) -> &str {
                &self.name
            }

            fn destroy(&self) {
                if let Some(destroy) = self.vtable().destroy {
                    destroy(self.this);
                }
            }
        }
    };
}

interceptor!(ClientVtable);
interceptor!(ServerVtable);
interceptor!(IorVtable);

impl ClientRequestInterceptor for Caller<ClientVtable> {
    fn send_request(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        call(self.this, self.vtable().send_request, erase(ri))
    }

    fn receive_reply(&self, ri: &ClientRequestInfo<'_>) -> Result<(), SystemException> {
        system_only(call(self.this, self.vtable().receive_reply, erase(ri)))
    }

    fn receive_exception(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        call(self.this, self.vtable().receive_exception, erase(ri))
    }

    fn receive_other(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        call(self.this, self.vtable().receive_other, erase(ri))
    }
}

impl ServerRequestInterceptor for Caller<ServerVtable> {
    fn receive_request_service_contexts(
        &self,
        ri: &ServerRequestInfo<'_>,
    ) -> Result<(), InterceptError> {
        call(
            self.this,
            self.vtable().receive_request_service_contexts,
            erase(ri),
        )
    }

    fn receive_request(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        call(self.this, self.vtable().receive_request, erase(ri))
    }

    fn send_reply(&self, ri: &ServerRequestInfo<'_>) -> Result<(), SystemException> {
        system_only(call(self.this, self.vtable().send_reply, erase(ri)))
    }

    fn send_exception(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        call(self.this, self.vtable().send_exception, erase(ri))
    }

    fn send_other(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        call(self.this, self.vtable().send_other, erase(ri))
    }
}

impl IorInterceptor for Caller<IorVtable> {
    fn establish_components(&self, info: &IorInfo<'_>) -> Result<(), SystemException> {
        system_only(call(
            self.this,
            self.vtable().establish_components,
            erase(info),
        ))
    }
}

impl Caller<InitializerVtable> {
    fn init(
        &self,
        point: Option<extern "C" fn(*mut c_void, *mut InitInfo, *mut PiError) -> Status>,
        info: &mut OrbInitInfo<'_>,
    ) -> Result<(), SystemException> {
        let Some(point) = point else {
            return Ok(());
        };
        let mut error = PiError::default();
        let ptr = (info as *mut OrbInitInfo<'_>).cast::<InitInfo>();
        let status = point(self.this, ptr, &mut error);
        system_only(raised(status, error))
    }
}

impl OrbInitializer for Caller<InitializerVtable> {
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        self.init(self.vtable().pre_init, info)
    }

    fn post_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        self.init(self.vtable().post_init, info)
    }
}

/// `PortableInterceptor::register_orb_initializer`: for the ORBs created
/// after it in this process. The vtable must outlive them.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_register_orb_initializer(
    this: *mut c_void,
    vtable: *const InitializerVtable,
) {
    rustorb_core::pi::register_orb_initializer(Caller::new(this, vtable, String::new()));
}

/// `ORB_init` with ORB id `orb_id` (which `-ORBid` overrides) and these
/// initializers, run after the registered ones.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_init_pi(
    argc: *mut c_int,
    argv: *mut *mut c_char,
    orb_id: *const c_char,
    orb_id_len: usize,
    initializers: *const InitializerEntry,
    count: usize,
    orb: *mut *mut Orb,
    exc: *mut Exception,
) -> Status {
    let orb_id = match unsafe { str_arg(orb_id, orb_id_len) } {
        Ok(id) => id.to_owned(),
        Err(e) => return unsafe { set_system(exc, &e) },
    };
    let entries: &[InitializerEntry] = if initializers.is_null() || count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(initializers, count) }
    };
    let inits: Vec<Arc<dyn OrbInitializer>> = entries
        .iter()
        .map(|e| Caller::new(e.this, e.vtable, String::new()) as Arc<dyn OrbInitializer>)
        .collect();
    unsafe {
        crate::orb::init_with(argc, argv, orb, exc, move |args| {
            let mut options = OrbOptions {
                arguments: args.clone(),
                orb_id,
                initializers: inits,
                ..OrbOptions::default()
            };
            options.parse_args(args)?;
            Orb::with_options(options)
        })
    }
}

// --- helpers of the accessors --------------------------------------------------

/// Called with each string of a list.
pub type EachString = extern "C" fn(*mut c_void, *const c_char, usize);
/// Called with each tagged component (or service context) of a list.
pub type EachBytes = extern "C" fn(*mut c_void, u32, *const u8, usize);
/// Called with each parameter: an any handle the callee takes, and the
/// mode (0 in, 1 out, 2 inout).
pub type EachParameter = extern "C" fn(*mut c_void, *mut Any, u32);
/// Called with each TypeCode, a handle the callee takes.
pub type EachTypeCode = extern "C" fn(*mut c_void, *mut TypeCode);

fn result<T>(exc: *mut Exception, r: Result<T, SystemException>, out: impl FnOnce(T)) -> Status {
    match r {
        Ok(v) => {
            out(v);
            OK
        }
        Err(e) => unsafe { set_system(exc, &e) },
    }
}

fn each_string(context: *mut c_void, each: EachString, list: &[String]) {
    for s in list {
        each(context, s.as_ptr() as *const c_char, s.len());
    }
}

fn each_parameter(context: *mut c_void, each: EachParameter, list: Vec<Parameter>) {
    for p in list {
        let mode = match p.mode {
            rustorb_core::dii::ArgMode::In => 0,
            rustorb_core::dii::ArgMode::Out => 1,
            rustorb_core::dii::ArgMode::InOut => 2,
        };
        each(context, into_handle(p.argument), mode);
    }
}

fn policy_out(p: PolicyValue, raw: *mut u64) {
    unsafe { *raw = p.raw_value() };
}

unsafe fn bytes_out(out: *mut Bytes, v: Vec<u8>) {
    unsafe { *out = Bytes::new(v) };
}

fn invalid_slot(exc: *mut Exception) -> Status {
    unsafe { set_local(exc, local::INVALID_SLOT, 0) }
}

/// Accessors that both request infos have, as `rorb_cri_*` and
/// `rorb_sri_*`.
macro_rules! request_info {
    ($info:ty, $request_id:ident, $operation:ident, $response_expected:ident,
     $sync_scope:ident, $reply_status:ident, $forward_reference:ident, $get_slot:ident,
     $get_request_sc:ident, $get_reply_sc:ident, $arguments:ident, $exceptions:ident,
     $result:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $request_id(ri: *const $info) -> u32 {
            unsafe { &*ri }.request_id()
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $operation(ri: *const $info, len: *mut usize) -> *const c_char {
            let op = unsafe { &*ri }.operation();
            unsafe { *len = op.len() };
            op.as_ptr() as *const c_char
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $response_expected(ri: *const $info) -> i32 {
            i32::from(unsafe { &*ri }.response_expected())
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $sync_scope(ri: *const $info) -> i16 {
            unsafe { &*ri }.sync_scope()
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $reply_status(
            ri: *const $info,
            status: *mut i16,
            exc: *mut Exception,
        ) -> Status {
            guard(exc, || {
                result(exc, unsafe { &*ri }.reply_status(), |s| unsafe {
                    *status = s
                })
            })
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $forward_reference(
            ri: *const $info,
            obj: *mut *mut ObjectRef,
            exc: *mut Exception,
        ) -> Status {
            guard(exc, || {
                result(exc, unsafe { &*ri }.forward_reference(), |o| unsafe {
                    *obj = into_handle(o)
                })
            })
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $get_slot(
            ri: *const $info,
            id: u32,
            any: *mut *mut Any,
            exc: *mut Exception,
        ) -> Status {
            guard(exc, || match unsafe { &*ri }.get_slot(id) {
                Ok(a) => {
                    unsafe { *any = into_handle(a) };
                    OK
                }
                Err(_) => invalid_slot(exc),
            })
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $get_request_sc(
            ri: *const $info,
            id: u32,
            data: *mut Bytes,
            exc: *mut Exception,
        ) -> Status {
            guard(exc, || {
                result(
                    exc,
                    unsafe { &*ri }.get_request_service_context(id),
                    |sc| unsafe { bytes_out(data, sc.context_data) },
                )
            })
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $get_reply_sc(
            ri: *const $info,
            id: u32,
            data: *mut Bytes,
            exc: *mut Exception,
        ) -> Status {
            guard(exc, || {
                result(
                    exc,
                    unsafe { &*ri }.get_reply_service_context(id),
                    |sc| unsafe { bytes_out(data, sc.context_data) },
                )
            })
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $arguments(
            ri: *const $info,
            context: *mut c_void,
            each: EachParameter,
            exc: *mut Exception,
        ) -> Status {
            guard(exc, || {
                result(exc, unsafe { &*ri }.arguments(), |list| {
                    each_parameter(context, each, list)
                })
            })
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $exceptions(
            ri: *const $info,
            context: *mut c_void,
            each: EachTypeCode,
            exc: *mut Exception,
        ) -> Status {
            guard(exc, || {
                result(exc, unsafe { &*ri }.exceptions(), |list| {
                    for tc in list {
                        each(context, into_handle(tc));
                    }
                })
            })
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $result(
            ri: *const $info,
            any: *mut *mut Any,
            exc: *mut Exception,
        ) -> Status {
            guard(exc, || {
                result(exc, unsafe { &*ri }.result(), |a| unsafe {
                    *any = into_handle(a)
                })
            })
        }
    };
}

request_info!(
    ClientInfo,
    rorb_cri_request_id,
    rorb_cri_operation,
    rorb_cri_response_expected,
    rorb_cri_sync_scope,
    rorb_cri_reply_status,
    rorb_cri_forward_reference,
    rorb_cri_get_slot,
    rorb_cri_get_request_service_context,
    rorb_cri_get_reply_service_context,
    rorb_cri_arguments,
    rorb_cri_exceptions,
    rorb_cri_result
);

request_info!(
    ServerInfo,
    rorb_sri_request_id,
    rorb_sri_operation,
    rorb_sri_response_expected,
    rorb_sri_sync_scope,
    rorb_sri_reply_status,
    rorb_sri_forward_reference,
    rorb_sri_get_slot,
    rorb_sri_get_request_service_context,
    rorb_sri_get_reply_service_context,
    rorb_sri_arguments,
    rorb_sri_exceptions,
    rorb_sri_result
);

// --- ClientRequestInfo ------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_cri_target(ri: *const ClientInfo) -> *mut ObjectRef {
    into_handle(unsafe { &*ri }.target().clone())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_cri_effective_target(ri: *const ClientInfo) -> *mut ObjectRef {
    into_handle(unsafe { &*ri }.effective_target().clone())
}

/// The tag, and the profile data in `*data`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_cri_effective_profile(
    ri: *const ClientInfo,
    data: *mut Bytes,
) -> u32 {
    let p = unsafe { &*ri }.effective_profile();
    unsafe { bytes_out(data, p.profile_data.clone()) };
    p.tag
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_cri_received_exception(
    ri: *const ClientInfo,
    any: *mut *mut Any,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(exc, unsafe { &*ri }.received_exception(), |a| unsafe {
            *any = into_handle(a)
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_cri_received_exception_id(
    ri: *const ClientInfo,
    id: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(exc, unsafe { &*ri }.received_exception_id(), |s| unsafe {
            bytes_out(id, s.into_bytes())
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_cri_get_effective_component(
    ri: *const ClientInfo,
    tag: u32,
    data: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(
            exc,
            unsafe { &*ri }.get_effective_component(tag),
            |c| unsafe { bytes_out(data, c.component_data) },
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_cri_get_effective_components(
    ri: *const ClientInfo,
    tag: u32,
    context: *mut c_void,
    each: EachBytes,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(exc, unsafe { &*ri }.get_effective_components(tag), |list| {
            for c in list {
                each(
                    context,
                    c.tag,
                    c.component_data.as_ptr(),
                    c.component_data.len(),
                );
            }
        })
    })
}

/// 1 with `*raw` set when a policy of the type is in effect, else 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_cri_get_request_policy(
    ri: *const ClientInfo,
    policy_type: u32,
    found: *mut i32,
    raw: *mut u64,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(exc, unsafe { &*ri }.get_request_policy(policy_type), |p| {
            unsafe { *found = i32::from(p.is_some()) };
            if let Some(p) = p {
                policy_out(p, raw);
            }
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_cri_add_request_service_context(
    ri: *const ClientInfo,
    id: u32,
    data: *const u8,
    len: usize,
    replace: i32,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let sc = ServiceContext {
            context_id: id,
            context_data: unsafe { crate::bytes_arg(data, len) }.to_vec(),
        };
        result(
            exc,
            unsafe { &*ri }.add_request_service_context(sc, replace != 0),
            |()| (),
        )
    })
}

// --- ServerRequestInfo ------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_sri_set_slot(
    ri: *const ServerInfo,
    id: u32,
    any: *const Any,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let value = unsafe { any.as_ref() }.cloned().unwrap_or_default();
        match unsafe { &*ri }.set_slot(id, value) {
            Ok(()) => OK,
            Err(_) => invalid_slot(exc),
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_sri_add_reply_service_context(
    ri: *const ServerInfo,
    id: u32,
    data: *const u8,
    len: usize,
    replace: i32,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let sc = ServiceContext {
            context_id: id,
            context_data: unsafe { crate::bytes_arg(data, len) }.to_vec(),
        };
        result(
            exc,
            unsafe { &*ri }.add_reply_service_context(sc, replace != 0),
            |()| (),
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_sri_sending_exception(
    ri: *const ServerInfo,
    any: *mut *mut Any,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(exc, unsafe { &*ri }.sending_exception(), |a| unsafe {
            *any = into_handle(a)
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_sri_sending_exception_id(
    ri: *const ServerInfo,
    id: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(exc, unsafe { &*ri }.sending_exception_id(), |s| unsafe {
            bytes_out(id, s.into_bytes())
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_sri_object_id(
    ri: *const ServerInfo,
    id: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(exc, unsafe { &*ri }.object_id(), |v| unsafe {
            bytes_out(id, v)
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_sri_adapter_id(
    ri: *const ServerInfo,
    id: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(exc, unsafe { &*ri }.adapter_id(), |v| unsafe {
            bytes_out(id, v)
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_sri_adapter_name(
    ri: *const ServerInfo,
    context: *mut c_void,
    each: EachString,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(exc, unsafe { &*ri }.adapter_name(), |names| {
            each_string(context, each, &names)
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_sri_orb_id(ri: *const ServerInfo, len: *mut usize) -> *const c_char {
    let s = unsafe { &*ri }.orb_id();
    unsafe { *len = s.len() };
    s.as_ptr() as *const c_char
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_sri_server_id(
    ri: *const ServerInfo,
    len: *mut usize,
) -> *const c_char {
    let s = unsafe { &*ri }.server_id();
    unsafe { *len = s.len() };
    s.as_ptr() as *const c_char
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_sri_target_most_derived_interface(
    ri: *const ServerInfo,
    id: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(
            exc,
            unsafe { &*ri }.target_most_derived_interface(),
            |s| unsafe { bytes_out(id, s.into_bytes()) },
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_sri_target_is_a(
    ri: *const ServerInfo,
    id: *const c_char,
    len: usize,
    is_a: *mut i32,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let id = match unsafe { str_arg(id, len) } {
            Ok(id) => id,
            Err(e) => return unsafe { set_system(exc, &e) },
        };
        result(exc, unsafe { &*ri }.target_is_a(id), |b| unsafe {
            *is_a = i32::from(b)
        })
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_sri_get_server_policy(
    ri: *const ServerInfo,
    policy_type: u32,
    raw: *mut u64,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(exc, unsafe { &*ri }.get_server_policy(policy_type), |p| {
            policy_out(p, raw)
        })
    })
}

// --- IORInfo ----------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_ior_info_get_effective_policy(
    info: *const IorInfoHandle,
    policy_type: u32,
    raw: *mut u64,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        result(
            exc,
            unsafe { &*info }.get_effective_policy(policy_type),
            |p| policy_out(p, raw),
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_ior_info_adapter_name(
    info: *const IorInfoHandle,
    context: *mut c_void,
    each: EachString,
) {
    each_string(context, each, unsafe { &*info }.adapter_name());
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_ior_info_add_ior_component(
    info: *const IorInfoHandle,
    tag: u32,
    data: *const u8,
    len: usize,
) {
    unsafe { &*info }.add_ior_component(TaggedComponent {
        tag,
        component_data: unsafe { crate::bytes_arg(data, len) }.to_vec(),
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_ior_info_add_ior_component_to_profile(
    info: *const IorInfoHandle,
    tag: u32,
    data: *const u8,
    len: usize,
    profile_id: u32,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let c = TaggedComponent {
            tag,
            component_data: unsafe { crate::bytes_arg(data, len) }.to_vec(),
        };
        result(
            exc,
            unsafe { &*info }.add_ior_component_to_profile(c, profile_id),
            |()| (),
        )
    })
}

// --- ORBInitInfo ------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_init_info_arguments(
    info: *const InitInfo,
    context: *mut c_void,
    each: EachString,
) {
    each_string(context, each, unsafe { &*info }.arguments());
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_init_info_orb_id(
    info: *const InitInfo,
    len: *mut usize,
) -> *const c_char {
    let s = unsafe { &*info }.orb_id();
    unsafe { *len = s.len() };
    s.as_ptr() as *const c_char
}

/// The ORB being created (a new handle). Calls on it may not work yet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_init_info_orb(info: *const InitInfo) -> *mut Orb {
    into_handle(unsafe { &*info }.orb().clone())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_init_info_register_initial_reference(
    info: *const InitInfo,
    name: *const c_char,
    len: usize,
    obj: *mut ObjectRef,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let Ok(name) = (unsafe { str_arg(name, len) }) else {
            return unsafe { set_local(exc, local::INVALID_NAME, 0) };
        };
        match unsafe { &*info }.register_initial_reference(name, unsafe { obj.as_ref() }) {
            Ok(()) => OK,
            Err(ResolveError::InvalidName(_)) => unsafe { set_local(exc, local::INVALID_NAME, 0) },
            Err(ResolveError::System(e)) => unsafe { set_system(exc, &e) },
        }
    })
}

/// Only in post_init (BAD_INV_ORDER in pre_init).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_init_info_resolve_initial_references(
    info: *const InitInfo,
    name: *const c_char,
    len: usize,
    obj: *mut *mut ObjectRef,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let Ok(name) = (unsafe { str_arg(name, len) }) else {
            return unsafe { set_local(exc, local::INVALID_NAME, 0) };
        };
        match unsafe { &*info }.resolve_initial_references(name) {
            Ok(o) => {
                unsafe { *obj = into_handle(o) };
                OK
            }
            Err(ResolveError::InvalidName(_)) => unsafe { set_local(exc, local::INVALID_NAME, 0) },
            Err(ResolveError::System(e)) => unsafe { set_system(exc, &e) },
        }
    })
}

fn added(exc: *mut Exception, r: Result<(), rustorb_core::pi::DuplicateName>) -> Status {
    match r {
        Ok(()) => OK,
        Err(_) => unsafe { set_local(exc, local::DUPLICATE_NAME, 0) },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_init_info_add_client_request_interceptor(
    info: *mut InitInfo,
    name: *const c_char,
    len: usize,
    this: *mut c_void,
    vtable: *const ClientVtable,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let name = match unsafe { str_arg(name, len) } {
            Ok(n) => n.to_owned(),
            Err(e) => return unsafe { set_system(exc, &e) },
        };
        let i = Caller::new(this, vtable, name);
        added(exc, unsafe { &mut *info }.add_client_request_interceptor(i))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_init_info_add_server_request_interceptor(
    info: *mut InitInfo,
    name: *const c_char,
    len: usize,
    this: *mut c_void,
    vtable: *const ServerVtable,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let name = match unsafe { str_arg(name, len) } {
            Ok(n) => n.to_owned(),
            Err(e) => return unsafe { set_system(exc, &e) },
        };
        let i = Caller::new(this, vtable, name);
        added(exc, unsafe { &mut *info }.add_server_request_interceptor(i))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_init_info_add_ior_interceptor(
    info: *mut InitInfo,
    name: *const c_char,
    len: usize,
    this: *mut c_void,
    vtable: *const IorVtable,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let name = match unsafe { str_arg(name, len) } {
            Ok(n) => n.to_owned(),
            Err(e) => return unsafe { set_system(exc, &e) },
        };
        let i = Caller::new(this, vtable, name);
        added(exc, unsafe { &mut *info }.add_ior_interceptor(i))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_init_info_allocate_slot_id(info: *mut InitInfo) -> u32 {
    unsafe { &mut *info }.allocate_slot_id()
}

/// PICurrent with the slots allocated so far.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_init_info_pi_current(info: *const InitInfo) -> *mut PiCurrent {
    into_handle(unsafe { &*info }.pi_current())
}

// --- PICurrent ----------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_pi_current(orb: *mut Orb) -> *mut PiCurrent {
    into_handle(unsafe { &*orb }.pi_current())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_pi_current_release(current: *mut PiCurrent) {
    unsafe { crate::release_handle(current) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_pi_current_get_slot(
    current: *const PiCurrent,
    id: u32,
    any: *mut *mut Any,
    exc: *mut Exception,
) -> Status {
    guard(exc, || match unsafe { &*current }.get_slot(id) {
        Ok(a) => {
            unsafe { *any = into_handle(a) };
            OK
        }
        Err(_) => invalid_slot(exc),
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_pi_current_set_slot(
    current: *const PiCurrent,
    id: u32,
    any: *const Any,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let value = unsafe { any.as_ref() }.cloned().unwrap_or_default();
        match unsafe { &*current }.set_slot(id, value) {
            Ok(()) => OK,
            Err(_) => invalid_slot(exc),
        }
    })
}

// --- Codec ----------------------------------------------------------------------------

fn codec_of(
    factory: rustorb_core::iop::CodecFactory,
    format: i16,
    major: u8,
    minor: u8,
    codec: *mut *mut Codec,
    exc: *mut Exception,
) -> Status {
    let encoding = Encoding {
        format,
        major_version: major,
        minor_version: minor,
    };
    match factory.create_codec(encoding) {
        Ok(c) => {
            unsafe { *codec = into_handle(c) };
            OK
        }
        Err(_) => unsafe { set_local(exc, local::UNKNOWN_ENCODING, 0) },
    }
}

/// `CodecFactory::create_codec` of the ORB; RORB_UNKNOWN_ENCODING but for
/// ENCODING_CDR_ENCAPS 1.0 to 1.2.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_create_codec(
    orb: *mut Orb,
    format: i16,
    major: u8,
    minor: u8,
    codec: *mut *mut Codec,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        codec_of(
            unsafe { &*orb }.codec_factory(),
            format,
            major,
            minor,
            codec,
            exc,
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_init_info_create_codec(
    info: *const InitInfo,
    format: i16,
    major: u8,
    minor: u8,
    codec: *mut *mut Codec,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        codec_of(
            unsafe { &*info }.codec_factory(),
            format,
            major,
            minor,
            codec,
            exc,
        )
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_codec_release(codec: *mut Codec) {
    unsafe { crate::release_handle(codec) }
}

fn codec_err(exc: *mut Exception, e: CodecError) -> Status {
    let code = match e {
        CodecError::InvalidTypeForEncoding => local::CODEC_INVALID_TYPE_FOR_ENCODING,
        CodecError::FormatMismatch => local::CODEC_FORMAT_MISMATCH,
        CodecError::TypeMismatch => local::CODEC_TYPE_MISMATCH,
    };
    unsafe { set_local(exc, code, 0) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_codec_encode(
    codec: *const Codec,
    any: *const Any,
    value_only: i32,
    data: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let codec = unsafe { &*codec };
        let any = unsafe { &*any };
        let r = if value_only != 0 {
            codec.encode_value(any)
        } else {
            codec.encode(any)
        };
        match r {
            Ok(v) => {
                unsafe { bytes_out(data, v) };
                OK
            }
            Err(e) => codec_err(exc, e),
        }
    })
}

/// `decode` with `tc` NULL, else `decode_value` of a value of type `tc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_codec_decode(
    codec: *const Codec,
    data: *const u8,
    len: usize,
    tc: *const TypeCode,
    any: *mut *mut Any,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let codec = unsafe { &*codec };
        let data = unsafe { crate::bytes_arg(data, len) };
        let r = match unsafe { tc.as_ref() } {
            Some(tc) => codec.decode_value(data, tc),
            None => codec.decode(data),
        };
        match r {
            Ok(a) => {
                unsafe { *any = into_handle(a) };
                OK
            }
            Err(e) => codec_err(exc, e),
        }
    })
}
