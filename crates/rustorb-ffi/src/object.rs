//! Object references (`rorb_object` is a boxed [`ObjectRef`]) and
//! invocations.

use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::ffi::{c_char, c_void};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_set_call_timeout(obj: *mut ObjectRef, us: i64) {
    unsafe { &*obj }.set_call_timeout(crate::orb::setting_us(us));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_call_timeout(obj: *mut ObjectRef) -> i64 {
    crate::orb::us_of_setting(unsafe { &*obj }.call_timeout())
}

type WriteFn = extern "C" fn(*mut c_void, *mut RorbOut) -> Status;
type ReadFn = extern "C" fn(*mut c_void, *mut RorbIn) -> Status;
type UserExceptionFn = extern "C" fn(*mut c_void, *const c_char, usize, *mut RorbIn) -> Status;

/// `rorb_invocation`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
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

/// What [`send`] got back.
pub(crate) enum Sent {
    Oneway,
    Reply(rustorb_core::Reply),
    User(rustorb_core::UserException),
}

/// Sends the request of `inv`, its arguments written by `write_args`.
/// `Err` is the status to return: a system exception (set in `exc`), or
/// `CALLBACK_FAILED` when the callback failed.
pub(crate) unsafe fn send(
    obj: &ObjectRef,
    inv: &Invocation,
    exc: *mut Exception,
) -> Result<Sent, Status> {
    let operation = match unsafe { str_arg(inv.operation, inv.operation_len) } {
        Ok(op) => op,
        Err(e) => return Err(unsafe { set_system(exc, &e) }),
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
            Ok(()) => Ok(Sent::Oneway),
            Err(_) if failed.get() => Err(CALLBACK_FAILED),
            Err(e) => Err(unsafe { set_system(exc, &e) }),
        };
    }
    match request.invoke() {
        Ok(reply) => Ok(Sent::Reply(reply)),
        Err(CoreException::System(_)) if failed.get() => Err(CALLBACK_FAILED),
        Err(CoreException::System(e)) => Err(unsafe { set_system(exc, &e) }),
        Err(CoreException::User(u)) => Ok(Sent::User(u)),
    }
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
        let sent = match unsafe { send(obj, inv, exc) } {
            Ok(sent) => sent,
            Err(status) => return status,
        };
        unsafe { read_sent(inv, sent, exc) }
    })
}

/// Hands what an invocation got back to the read callbacks of `inv`.
unsafe fn read_sent(inv: &Invocation, sent: Sent, exc: *mut Exception) -> Status {
    match sent {
        Sent::Oneway => OK,
        Sent::Reply(reply) => {
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
        Sent::User(u) => {
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
}

/// `rorb_sendc_done_fn`.
type DoneFn = extern "C" fn(*mut c_void, Status, *const Exception);

/// An invocation of [`rorb_object_sendc`], used on other threads until its
/// `done` has been called.
struct Pending(Invocation);

// SAFETY: the caller of rorb_object_sendc makes the context and the
// callbacks usable from any thread until `done` is called.
unsafe impl Send for Pending {}
unsafe impl Sync for Pending {}

/// A pointer handed to the thread that runs a callback (crate::offload).
struct SendPtr<T>(*mut T);

// SAFETY: used by one thread at a time, while its owner waits.
unsafe impl<T> Send for SendPtr<T> {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_sendc(
    obj: *mut ObjectRef,
    inv: *const Invocation,
    done: Option<DoneFn>,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let obj = unsafe { &*obj };
        let inv = unsafe { *inv };
        let Some(done) = done else {
            return unsafe { set_system(exc, &crate::bad_param()) };
        };
        if inv.oneway != 0 {
            return unsafe { set_system(exc, &crate::bad_param()) };
        }
        let operation = match unsafe { str_arg(inv.operation, inv.operation_len) } {
            Ok(op) => op.to_owned(),
            Err(e) => return unsafe { set_system(exc, &e) },
        };
        // The operation name is copied: its pointer is not kept.
        let pending = Arc::new(Pending(Invocation {
            operation: std::ptr::null(),
            operation_len: 0,
            ..inv
        }));
        // Set when write_args failed.
        let failed = Arc::new(AtomicBool::new(false));
        let mut request = obj.request_callback(&operation);
        if let Some(write_args) = inv.write_args {
            let pending = Arc::clone(&pending);
            let failed = Arc::clone(&failed);
            request = request.args(move |out: &mut OutStream<'_>| {
                // Written on a thread of the runtime: the caller's callback
                // runs elsewhere (crate::offload).
                let out = SendPtr(out as *mut OutStream<'_>);
                let pending = &pending;
                let failed = &failed;
                crate::offload::run(move || {
                    let out = out;
                    // SAFETY: the stream outlives this call, which run
                    // waits for, and nothing else uses it meanwhile.
                    let mut o = Out::new(unsafe { &mut *out.0 });
                    if write_args(pending.0.context, o.as_ptr()) != OK {
                        failed.store(true, Ordering::Relaxed);
                        return Err(sys(SystemExceptionKind::UNKNOWN, 0, CompletionStatus::No));
                    }
                    match o.error {
                        Some(e) => Err(e),
                        None => Ok(()),
                    }
                })
            });
        }
        if inv.timeout_us >= 0 {
            let timeout = u64::try_from(inv.timeout_us).unwrap_or(0);
            request = request.timeout((timeout > 0).then(|| Duration::from_micros(timeout)));
        }
        let sent = request.send(move |result| {
            let inv = &pending.0;
            let mut e = crate::Exception::empty();
            let status = match result {
                Err(CoreException::System(_)) if failed.load(Ordering::Relaxed) => CALLBACK_FAILED,
                Err(CoreException::System(s)) => unsafe { set_system(&mut e, &s) },
                Ok(reply) => unsafe { read_sent(inv, Sent::Reply(reply), &mut e) },
                Err(CoreException::User(u)) => unsafe { read_sent(inv, Sent::User(u), &mut e) },
            };
            done(inv.context, status, &e);
            unsafe { crate::rorb_exception_clear(&mut e) };
        });
        match sent {
            Ok(()) => OK,
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}
