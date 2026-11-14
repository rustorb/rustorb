//! The DII and the DSI (`rorb_request`, `rorb_server_request`) on the
//! core's [`DiiRequest`] and [`ServerRequest`] (`docs/dii-dsi.md`).
//!
//! The values go in and out as anys (`rorb_any`). A request is made for
//! one call: the caller (the C++ `CORBA::Request`) keeps its own list of
//! arguments and makes a new `rorb_request` each time it sends it.
//!
//! A DSI servant is an ordinary caller servant (`rorb_servant_get`): its
//! `dispatch` makes a `rorb_server_request` from the arguments stream,
//! before reading anything from it, and ends with
//! `rorb_server_request_reply`.

use std::ffi::c_char;
use std::sync::Mutex;

use rustorb_core::dii::{ArgMode, DiiRequest, NamedValue, ServerRequest};
use rustorb_core::{
    Any, CompletionStatus, Exception as CoreException, ObjectRef, SystemException,
    SystemExceptionKind, TypeCode, minor,
};

use crate::servant::ReplyFfi;
use crate::stream::RorbIn;
use crate::{
    Exception, OK, Status, USER_EXCEPTION, bad_param, guard, into_handle, release_handle,
    set_system, str_arg, sys,
};

fn bad_inv_order() -> SystemException {
    sys(SystemExceptionKind::BAD_INV_ORDER, 0, CompletionStatus::No)
}

fn mode(flags: u32) -> Result<ArgMode, SystemException> {
    ArgMode::from_flags(flags).ok_or_else(bad_param)
}

// --- the client side (rorb_request) ----------------------------------------------

/// `rorb_request`: a [`DiiRequest`] and the user exception of its last
/// call.
#[derive(Debug)]
pub struct FfiRequest {
    request: DiiRequest,
    exception: Option<Any>,
}

/// The request behind a handle; the lock is not poisoned by a panic the
/// guard caught.
type Handle = Mutex<FfiRequest>;

unsafe fn request<'a>(req: *mut Handle) -> std::sync::MutexGuard<'a, FfiRequest> {
    unsafe { &*req }.lock().unwrap_or_else(|e| e.into_inner())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_new(
    target: *mut ObjectRef,
    operation: *const c_char,
    len: usize,
    out: *mut *mut Handle,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let target = unsafe { &*target };
        match unsafe { str_arg(operation, len) } {
            Ok(op) => {
                let r = FfiRequest {
                    request: target.create_request(op),
                    exception: None,
                };
                unsafe { *out = into_handle(Mutex::new(r)) };
                OK
            }
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_release(req: *mut Handle) {
    unsafe { release_handle(req) }
}

/// Adds an argument: `value` is its value, or for an `out` argument
/// only its type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_add_arg(
    req: *mut Handle,
    name: *const c_char,
    len: usize,
    flags: u32,
    value: *const Any,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let r = (|| {
            let name = unsafe { str_arg(name, len) }?;
            let mode = mode(flags)?;
            let value = unsafe { &*value };
            let nv = match mode {
                ArgMode::Out => NamedValue::out(name, value.typecode().clone()),
                _ => NamedValue::new(name, value.clone(), mode),
            };
            unsafe { request(req) }.request.arguments_mut().push(nv);
            Ok(())
        })();
        match r {
            Ok(()) => OK,
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_set_return_type(req: *mut Handle, tc: *const TypeCode) {
    let tc = unsafe { &*tc }.clone();
    unsafe { request(req) }.request.set_return_type(tc);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_add_exception(req: *mut Handle, tc: *const TypeCode) {
    let tc = unsafe { &*tc }.clone();
    unsafe { request(req) }.request.add_exception(tc);
}

/// The timeout of the calls, as `rorb_invocation.timeout_us`: negative
/// for the policies', 0 for none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_set_timeout(req: *mut Handle, us: i64) {
    if us < 0 {
        return;
    }
    let us = u64::try_from(us).unwrap_or(0);
    let timeout = (us > 0).then(|| std::time::Duration::from_micros(us));
    unsafe { request(req) }.request.set_timeout(timeout);
}

/// The outcome of a call: a user exception is kept as an any (with the
/// TypeCode of the exception list) and is [`USER_EXCEPTION`]; one that is
/// not listed is `UNKNOWN`.
fn outcome(r: &mut FfiRequest, result: Result<(), CoreException>, exc: *mut Exception) -> Status {
    r.exception = None;
    match result {
        Ok(()) => OK,
        Err(CoreException::System(e)) => unsafe { set_system(exc, &e) },
        Err(CoreException::User(u)) => match r.request.exception_value(&u) {
            Ok(any) => {
                r.exception = Some(any);
                USER_EXCEPTION
            }
            Err(e) if e.kind() == Some(SystemExceptionKind::UNKNOWN) => unsafe {
                set_system(
                    exc,
                    &sys(
                        SystemExceptionKind::UNKNOWN,
                        minor::UNLISTED_USER_EXCEPTION,
                        CompletionStatus::Yes,
                    ),
                )
            },
            Err(e) => unsafe { set_system(exc, &e) },
        },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_invoke(req: *mut Handle, exc: *mut Exception) -> Status {
    guard(exc, || {
        let mut r = unsafe { request(req) };
        crate::stack::commit();
        let result = r.request.invoke();
        outcome(&mut r, result, exc)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_send_oneway(req: *mut Handle, exc: *mut Exception) -> Status {
    guard(exc, || {
        match unsafe { request(req) }.request.send_oneway() {
            Ok(()) => OK,
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_send_deferred(
    req: *mut Handle,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        match unsafe { request(req) }.request.send_deferred() {
            Ok(()) => OK,
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

/// Sets `*ready` to whether the response of `rorb_request_send_deferred`
/// has arrived.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_poll_response(
    req: *mut Handle,
    ready: *mut i32,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        match unsafe { request(req) }.request.poll_response() {
            Ok(v) => {
                unsafe { *ready = i32::from(v) };
                OK
            }
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

/// Waits for the response of `rorb_request_send_deferred`; the outcome is
/// that of `rorb_request_invoke`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_get_response(
    req: *mut Handle,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let mut r = unsafe { request(req) };
        let result = r.request.get_response();
        outcome(&mut r, result, exc)
    })
}

/// The return value (its type only before a call): a new handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_result(req: *mut Handle) -> *mut Any {
    into_handle(unsafe { request(req) }.request.result().clone())
}

/// The value of argument `index` (that sent for an `in` one, that received
/// for the others after a call): a new handle, NULL when there is no such
/// argument.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_argument(req: *mut Handle, index: usize) -> *mut Any {
    match unsafe { request(req) }.request.arguments().get(index) {
        Some(nv) => into_handle(nv.value.clone()),
        None => std::ptr::null_mut(),
    }
}

/// The user exception of the last call (after `RORB_USER_EXCEPTION`): a
/// new handle, NULL when there was none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_request_exception(req: *mut Handle) -> *mut Any {
    match &unsafe { request(req) }.exception {
        Some(any) => into_handle(any.clone()),
        None => std::ptr::null_mut(),
    }
}

// --- the server side (rorb_server_request) ---------------------------------------

/// `rorb_server_request`: a [`ServerRequest`] and the parameters given
/// before its arguments are read.
#[derive(Debug)]
pub struct FfiServerRequest {
    /// `None` once the reply is written.
    request: Option<ServerRequest>,
    params: Vec<NamedValue>,
}

impl FfiServerRequest {
    fn request(&mut self) -> Result<&mut ServerRequest, SystemException> {
        self.request.as_mut().ok_or_else(bad_inv_order)
    }
}

type ServerHandle = Mutex<FfiServerRequest>;

unsafe fn server_request<'a>(sr: *mut ServerHandle) -> std::sync::MutexGuard<'a, FfiServerRequest> {
    unsafe { &*sr }.lock().unwrap_or_else(|e| e.into_inner())
}

fn status(exc: *mut Exception, r: Result<(), SystemException>) -> Status {
    match r {
        Ok(()) => OK,
        Err(e) => unsafe { set_system(exc, &e) },
    }
}

/// The request whose arguments `args` (the stream `dispatch` got, nothing
/// read from it yet) holds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_server_request_new(
    args: *mut RorbIn,
    operation: *const c_char,
    len: usize,
    out: *mut *mut ServerHandle,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let input = unsafe { &mut *args };
        let r = unsafe { str_arg(operation, len) }
            .and_then(|op| ServerRequest::new(op, input.stream()));
        match r {
            Ok(request) => {
                let sr = FfiServerRequest {
                    request: Some(request),
                    params: Vec::new(),
                };
                unsafe { *out = into_handle(Mutex::new(sr)) };
                OK
            }
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_server_request_release(sr: *mut ServerHandle) {
    unsafe { release_handle(sr) }
}

/// Adds a parameter, of type `tc`, for `rorb_server_request_arguments`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_server_request_add_param(
    sr: *mut ServerHandle,
    name: *const c_char,
    len: usize,
    flags: u32,
    tc: *const TypeCode,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let r = (|| {
            let name = unsafe { str_arg(name, len) }?;
            let mode = mode(flags)?;
            let mut nv = NamedValue::out(name, unsafe { &*tc }.clone());
            nv.mode = mode;
            unsafe { server_request(sr) }.params.push(nv);
            Ok(())
        })();
        status(exc, r)
    })
}

/// Reads the `in` and `inout` arguments of the parameters added. Once
/// only (`BAD_INV_ORDER`); arguments that do not match are `MARSHAL`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_server_request_arguments(
    sr: *mut ServerHandle,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let mut s = unsafe { server_request(sr) };
        let params = std::mem::take(&mut s.params);
        let r = s.request().and_then(|r| r.arguments(params).map(drop));
        status(exc, r)
    })
}

/// The value of parameter `index` after `rorb_server_request_arguments`
/// (a new handle); NULL for an `out` one not set, or no such parameter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_server_request_argument(
    sr: *mut ServerHandle,
    index: usize,
) -> *mut Any {
    let mut s = unsafe { server_request(sr) };
    let Ok(r) = s.request() else {
        return std::ptr::null_mut();
    };
    match r.parameters_mut().and_then(|p| p.get(index)) {
        Some(nv) if !matches!(nv.value.value(), rustorb_core::Value::Void) => {
            into_handle(nv.value.clone())
        }
        _ => std::ptr::null_mut(),
    }
}

/// Sets the value of the `out` or `inout` parameter `index`, of a type
/// equivalent to the parameter's (else `BAD_PARAM`, as for an `in` one or
/// no such parameter). `BAD_INV_ORDER` before the arguments are read.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_server_request_set_argument(
    sr: *mut ServerHandle,
    index: usize,
    value: *const Any,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let value = unsafe { &*value };
        let mut s = unsafe { server_request(sr) };
        let r = s.request().and_then(|r| {
            let params = r.parameters_mut().ok_or_else(bad_inv_order)?;
            let nv = params.get_mut(index).ok_or_else(bad_param)?;
            if nv.mode == ArgMode::In || !nv.value.typecode().equivalent(value.typecode()) {
                return Err(bad_param());
            }
            nv.value = value.clone();
            Ok(())
        });
        status(exc, r)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_server_request_set_result(
    sr: *mut ServerHandle,
    value: *const Any,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let value = unsafe { &*value }.clone();
        let mut s = unsafe { server_request(sr) };
        let r = s.request().and_then(|r| r.set_result(value));
        status(exc, r)
    })
}

/// Makes the reply the user exception `value` (an any of an exception
/// type, else `BAD_PARAM`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_server_request_set_exception(
    sr: *mut ServerHandle,
    value: *const Any,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let value = unsafe { &*value }.clone();
        let mut s = unsafe { server_request(sr) };
        let r = s.request().and_then(|r| r.set_exception(value));
        status(exc, r)
    })
}

/// Writes the reply of the dispatch `reply` belongs to: the user
/// exception, or the result and the `out` and `inout` values. Once only
/// (`BAD_INV_ORDER`). A value that does not fit its type (an `out`
/// argument not set, say) is `BAD_PARAM`; the dispatch then replies with
/// that exception.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_server_request_reply(
    sr: *mut ServerHandle,
    reply: *mut ReplyFfi,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let Some(request) = unsafe { server_request(sr) }.request.take() else {
            return unsafe { set_system(exc, &bad_inv_order()) };
        };
        let server_reply = unsafe { &mut *reply }.server_reply();
        status(exc, request.write_reply(unsafe { &mut *server_reply }))
    })
}
