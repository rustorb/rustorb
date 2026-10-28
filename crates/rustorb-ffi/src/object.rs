//! Object references (`rorb_object` is a boxed [`ObjectRef`]) and
//! invocations.

use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::ffi::{c_char, c_void};
use std::hash::{Hash, Hasher};
use std::time::Duration;

use rustorb_core::{
    CompletionStatus, Exception as CoreException, ObjectRef, Orb, OutStream, SystemException,
    SystemExceptionKind, minor,
};

use crate::stream::{In, Out, RorbIn, RorbOut};
use crate::{
    CALLBACK_FAILED, Exception, OK, SYSTEM_EXCEPTION, Status, USER_EXCEPTION, guard, into_handle,
    release_handle, set_system, str_arg, sys,
};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_clone(obj: *mut ObjectRef) -> *mut ObjectRef {
    into_handle(unsafe { &*obj }.clone())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_release(obj: *mut ObjectRef) {
    unsafe { release_handle(obj) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_orb(obj: *mut ObjectRef) -> *mut Orb {
    into_handle(unsafe { &*obj }.orb().clone())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_type_id(
    obj: *mut ObjectRef,
    len: *mut usize,
) -> *const c_char {
    let id = unsafe { &*obj }.type_id();
    unsafe { *len = id.len() };
    id.as_ptr() as *const c_char
}

/// A user exception where the caller expected none is `UNKNOWN`.
fn unlisted() -> SystemException {
    sys(
        SystemExceptionKind::UNKNOWN,
        minor::UNLISTED_USER_EXCEPTION,
        CompletionStatus::Yes,
    )
}

fn system(e: CoreException) -> SystemException {
    match e {
        CoreException::System(e) => e,
        CoreException::User(_) => unlisted(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_is_a(
    obj: *mut ObjectRef,
    id: *const c_char,
    len: usize,
    result: *mut i32,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let obj = unsafe { &*obj };
        let r = unsafe { str_arg(id, len) }.and_then(|id| obj.is_a(id).map_err(system));
        match r {
            Ok(v) => {
                unsafe { *result = i32::from(v) };
                OK
            }
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_non_existent(
    obj: *mut ObjectRef,
    result: *mut i32,
    exc: *mut Exception,
) -> Status {
    guard(exc, || match unsafe { &*obj }.non_existent() {
        Ok(v) => {
            unsafe { *result = i32::from(v) };
            OK
        }
        Err(e) => unsafe { set_system(exc, &system(e)) },
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_is_equivalent(a: *mut ObjectRef, b: *mut ObjectRef) -> i32 {
    match (unsafe { a.as_ref() }, unsafe { b.as_ref() }) {
        (Some(a), Some(b)) => i32::from(a.is_equivalent(b)),
        (None, None) => 1,
        _ => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_hash(obj: *mut ObjectRef, maximum: u32) -> u32 {
    let mut h = DefaultHasher::new();
    for p in &unsafe { &*obj }.ior().profiles {
        p.tag.hash(&mut h);
        p.profile_data.hash(&mut h);
    }
    let maximum = u64::from(maximum).max(1);
    u32::try_from(h.finish() % maximum).unwrap_or(0)
}

type WriteFn = extern "C" fn(*mut c_void, *mut RorbOut) -> Status;
type ReadFn = extern "C" fn(*mut c_void, *mut RorbIn) -> Status;
type UserExceptionFn = extern "C" fn(*mut c_void, *const c_char, usize, *mut RorbIn) -> Status;

/// `rorb_invocation`.
#[repr(C)]
#[derive(Debug)]
pub struct Invocation {
    pub operation: *const c_char,
    pub operation_len: usize,
    pub oneway: i32,
    pub timeout_us: i64,
    pub context: *mut c_void,
    pub write_args: Option<WriteFn>,
    pub read_reply: Option<ReadFn>,
    pub read_user_exception: Option<UserExceptionFn>,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_invoke(
    obj: *mut ObjectRef,
    inv: *const Invocation,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let obj = unsafe { &*obj };
        let inv = unsafe { &*inv };
        let operation = match unsafe { str_arg(inv.operation, inv.operation_len) } {
            Ok(op) => op,
            Err(e) => return unsafe { set_system(exc, &e) },
        };
        // Set when a callback failed, so that its placeholder exception is
        // not reported.
        let failed = Cell::new(false);
        let write = |out: &mut OutStream<'_>| -> Result<(), SystemException> {
            let Some(write_args) = inv.write_args else {
                return Ok(());
            };
            let mut o = Out::new(out);
            if write_args(inv.context, o.as_ptr()) != OK {
                failed.set(true);
                return Err(sys(SystemExceptionKind::UNKNOWN, 0, CompletionStatus::No));
            }
            match o.error {
                Some(e) => Err(e),
                None => Ok(()),
            }
        };
        let mut request = obj.request(operation).args(write);
        if inv.timeout_us >= 0 {
            let timeout = u64::try_from(inv.timeout_us).unwrap_or(0);
            request = request.timeout((timeout > 0).then(|| Duration::from_micros(timeout)));
        }
        if inv.oneway != 0 {
            return match request.send_oneway() {
                Ok(()) => OK,
                Err(_) if failed.get() => CALLBACK_FAILED,
                Err(e) => unsafe { set_system(exc, &e) },
            };
        }
        match request.invoke() {
            Ok(reply) => {
                let Some(read_reply) = inv.read_reply else {
                    return OK;
                };
                let mut stream = reply.input();
                let mut input = In::new(&mut stream, CompletionStatus::Yes);
                if read_reply(inv.context, input.as_ptr()) != OK {
                    return CALLBACK_FAILED;
                }
                match input.error {
                    Some(e) => unsafe { set_system(exc, &e) },
                    None => OK,
                }
            }
            Err(CoreException::System(_)) if failed.get() => CALLBACK_FAILED,
            Err(CoreException::System(e)) => unsafe { set_system(exc, &e) },
            Err(CoreException::User(u)) => {
                let Some(read_user_exception) = inv.read_user_exception else {
                    return unsafe { set_system(exc, &unlisted()) };
                };
                let mut stream = match u.input() {
                    Ok(s) => s,
                    Err(e) => return unsafe { set_system(exc, &e) },
                };
                let id = u.repository_id();
                let mut input = In::new(&mut stream, CompletionStatus::Yes);
                match read_user_exception(
                    inv.context,
                    id.as_ptr() as *const c_char,
                    id.len(),
                    input.as_ptr(),
                ) {
                    USER_EXCEPTION => match input.error {
                        Some(e) => unsafe { set_system(exc, &e) },
                        None => USER_EXCEPTION,
                    },
                    SYSTEM_EXCEPTION => unsafe { set_system(exc, &unlisted()) },
                    _ => CALLBACK_FAILED,
                }
            }
        }
    })
}
