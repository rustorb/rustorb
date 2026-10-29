//! `CORBA::ORB`: `rorb_orb` is a boxed [`Orb`].

use std::ffi::{CStr, c_char, c_int, c_void};
use std::sync::Mutex;
use std::time::Duration;

use rustorb_core::{ObjectRef, Orb, Poa, ResolveError};

use crate::{
    Bytes, Exception, OK, Status, guard, into_handle, local, release_handle, set_local, set_system,
    str_arg,
};

/// The first ORB initialised and not destroyed: its RootPOA is the
/// default POA of servants.
static DEFAULT_ORB: Mutex<Option<Orb>> = Mutex::new(None);

/// The first ORB initialised and not destroyed, if any.
pub(crate) fn default_orb() -> Option<Orb> {
    DEFAULT_ORB.lock().unwrap().clone()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_init(
    argc: *mut c_int,
    argv: *mut *mut c_char,
    orb: *mut *mut Orb,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let count = if argc.is_null() || argv.is_null() {
            0
        } else {
            usize::try_from(unsafe { *argc }).unwrap_or(0)
        };
        let ptrs: Vec<*mut c_char> = (0..count).map(|i| unsafe { *argv.add(i) }).collect();
        let strings: Vec<String> = ptrs
            .iter()
            .map(|&p| {
                if p.is_null() {
                    String::new()
                } else {
                    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
                }
            })
            .collect();
        // Keep argv[0] out of the parser, which would keep it anyway.
        let mut args: Vec<String> = strings.iter().skip(1).cloned().collect();
        let new_orb = match Orb::init(&mut args) {
            Ok(o) => o,
            Err(e) => return unsafe { set_system(exc, &e) },
        };
        if count > 0 {
            // The arguments left are a subsequence of the original ones.
            let mut kept = vec![ptrs[0]];
            let mut rest = args.iter().peekable();
            for (i, s) in strings.iter().enumerate().skip(1) {
                if rest.peek().is_some_and(|r| *r == s) {
                    kept.push(ptrs[i]);
                    rest.next();
                }
            }
            for (i, p) in kept.iter().enumerate() {
                unsafe { *argv.add(i) = *p };
            }
            if kept.len() < count {
                unsafe { *argv.add(kept.len()) = std::ptr::null_mut() };
            }
            unsafe { *argc = c_int::try_from(kept.len()).unwrap_or(0) };
        }
        let mut default = DEFAULT_ORB.lock().unwrap();
        if default.is_none() {
            *default = Some(new_orb.clone());
        }
        unsafe { *orb = into_handle(new_orb) };
        OK
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_clone(orb: *mut Orb) -> *mut Orb {
    into_handle(unsafe { &*orb }.clone())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_release(orb: *mut Orb) {
    unsafe { release_handle(orb) }
}

/// A nullable object handle for C.
pub(crate) fn object_handle(obj: Option<ObjectRef>) -> *mut ObjectRef {
    obj.map_or(std::ptr::null_mut(), into_handle)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_string_to_object(
    orb: *mut Orb,
    s: *const c_char,
    len: usize,
    obj: *mut *mut ObjectRef,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let orb = unsafe { &*orb };
        let result = unsafe { str_arg(s, len) }.and_then(|s| orb.string_to_object(s));
        match result {
            Ok(o) => {
                unsafe { *obj = object_handle(o) };
                OK
            }
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_object_to_string(
    orb: *mut Orb,
    obj: *mut ObjectRef,
    s: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let orb = unsafe { &*orb };
        match orb.object_to_string(unsafe { obj.as_ref() }) {
            Ok(text) => {
                unsafe { *s = Bytes::new(text.into_bytes()) };
                OK
            }
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_resolve_initial_references(
    orb: *mut Orb,
    name: *const c_char,
    len: usize,
    obj: *mut *mut ObjectRef,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let orb = unsafe { &*orb };
        let name = match unsafe { str_arg(name, len) } {
            Ok(n) => n,
            Err(_) => return unsafe { set_local(exc, local::INVALID_NAME, 0) },
        };
        match orb.resolve_initial_references(name) {
            Ok(o) => {
                unsafe { *obj = into_handle(o) };
                OK
            }
            Err(ResolveError::InvalidName(_)) => unsafe { set_local(exc, local::INVALID_NAME, 0) },
            Err(ResolveError::System(e)) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_list_initial_services(
    orb: *mut Orb,
    context: *mut c_void,
    each: extern "C" fn(*mut c_void, *const c_char, usize),
) {
    let orb = unsafe { &*orb };
    for name in orb.list_initial_services() {
        each(context, name.as_ptr() as *const c_char, name.len());
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_root_poa(
    orb: *mut Orb,
    poa: *mut *mut Poa,
    exc: *mut Exception,
) -> Status {
    guard(exc, || match unsafe { &*orb }.root_poa() {
        Ok(p) => {
            unsafe { *poa = into_handle(p) };
            OK
        }
        Err(e) => unsafe { set_system(exc, &e) },
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_default_root_poa(poa: *mut *mut Poa, exc: *mut Exception) -> Status {
    guard(exc, || {
        let orb = DEFAULT_ORB.lock().unwrap().clone();
        let Some(orb) = orb else {
            return unsafe {
                set_system(
                    exc,
                    &crate::sys(
                        rustorb_core::SystemExceptionKind::OBJ_ADAPTER,
                        0,
                        rustorb_core::CompletionStatus::No,
                    ),
                )
            };
        };
        match orb.root_poa() {
            Ok(p) => {
                unsafe { *poa = into_handle(p) };
                OK
            }
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_run(orb: *mut Orb) {
    let orb = unsafe { &*orb };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| orb.run()));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_work_pending(
    orb: *mut Orb,
    pending: *mut i32,
    exc: *mut Exception,
) -> Status {
    guard(exc, || match unsafe { &*orb }.work_pending() {
        Ok(p) => {
            unsafe { *pending = i32::from(p) };
            OK
        }
        Err(e) => unsafe { set_system(exc, &e) },
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_perform_work(orb: *mut Orb, exc: *mut Exception) -> Status {
    guard(exc, || match unsafe { &*orb }.perform_work() {
        Ok(()) => OK,
        Err(e) => unsafe { set_system(exc, &e) },
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_shutdown(
    orb: *mut Orb,
    wait_for_completion: i32,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        match unsafe { &*orb }.shutdown(wait_for_completion != 0) {
            Ok(()) => OK,
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_destroy(orb: *mut Orb, exc: *mut Exception) -> Status {
    guard(exc, || {
        let orb = unsafe { &*orb };
        match orb.destroy() {
            Ok(()) => {
                let mut default = DEFAULT_ORB.lock().unwrap();
                if default.as_ref() == Some(orb) {
                    *default = None;
                }
                OK
            }
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

/// Microseconds of the C ABI: 0 for no limit, negative clamped to 0.
pub(crate) fn duration_us(us: i64) -> Option<Duration> {
    (us > 0).then(|| Duration::from_micros(us.unsigned_abs()))
}

pub(crate) fn us_of(timeout: Option<Duration>) -> i64 {
    timeout.map_or(0, |t| {
        i64::try_from(t.as_micros()).unwrap_or(i64::MAX).max(1)
    })
}

/// The three-state setting of a thread or a reference: negative unsets it.
pub(crate) fn setting_us(us: i64) -> Option<Option<Duration>> {
    (us >= 0).then(|| duration_us(us))
}

pub(crate) fn us_of_setting(setting: Option<Option<Duration>>) -> i64 {
    setting.map_or(-1, us_of)
}

/// A new handle of the default ORB, or NULL before `rorb_orb_init`.
#[unsafe(no_mangle)]
pub extern "C" fn rorb_default_orb() -> *mut Orb {
    default_orb().map_or(std::ptr::null_mut(), into_handle)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_set_call_timeout(orb: *mut Orb, us: i64) {
    unsafe { &*orb }.set_call_timeout(duration_us(us));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_call_timeout(orb: *mut Orb) -> i64 {
    us_of(unsafe { &*orb }.call_timeout())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_set_connect_timeout(orb: *mut Orb, us: i64) {
    unsafe { &*orb }.set_connect_timeout(duration_us(us));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_connect_timeout(orb: *mut Orb) -> i64 {
    us_of(unsafe { &*orb }.connect_timeout())
}

#[unsafe(no_mangle)]
pub extern "C" fn rorb_set_thread_call_timeout(us: i64) {
    rustorb_core::set_thread_call_timeout(setting_us(us));
}

#[unsafe(no_mangle)]
pub extern "C" fn rorb_thread_call_timeout() -> i64 {
    us_of_setting(rustorb_core::thread_call_timeout())
}
