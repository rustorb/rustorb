//! Stable `extern "C"` ABI layer over the rustorb core.
//!
//! The C++ binding (`cpp/include/rustorb`) is built on this crate, and so
//! will the Java one be; the Python binding uses PyO3 directly instead. The
//! functions are declared, with the rules every caller follows (handles,
//! errors, strings, the lifetime of streams), in
//! `include/rustorb/rustorb.h`.
//!
//! - [`orb`]: `ORB_init` and the ORB operations.
//! - [`object`]: object references, and invocations whose arguments and
//!   results the caller writes and reads in callbacks.
//! - [`stream`]: the CDR streams the callbacks get. Their errors are
//!   sticky, so generated code does not check every call.
//! - [`servant`]: servants implemented by the caller through a table of
//!   callbacks, and the replies they write.
//! - [`poa`]: the POA, its manager and `PortableServer::Current`.
//! - [`any`]: TypeCodes, and anys whose values the caller puts in and
//!   takes out through the same streams.

// Every function takes raw pointers from C; what they must point to is
// documented once, in the header.
#![allow(clippy::missing_safety_doc)]

pub mod any;
pub mod object;
pub mod orb;
pub mod poa;
pub mod servant;
pub mod stream;

use std::ffi::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};

use rustorb_core::{CompletionStatus, PoaError, SystemException, SystemExceptionKind};

/// `rorb_status`.
pub type Status = i32;

/// Success.
pub const OK: Status = 0;
/// A system exception, in the [`Exception`].
pub const SYSTEM_EXCEPTION: Status = 1;
/// A user exception, taken by the caller's callback.
pub const USER_EXCEPTION: Status = 2;
/// An exception of a local interface, named by [`Exception::local`].
pub const LOCAL_EXCEPTION: Status = 3;
/// A callback failed and keeps its own error.
pub const CALLBACK_FAILED: Status = 4;

/// The version of the ABI in the header (`RORB_ABI_VERSION`).
pub const ABI_VERSION: u32 = 1;

/// Values of [`Exception::local`] (`RORB_ADAPTER_ALREADY_EXISTS`, ...).
pub mod local {
    pub const ADAPTER_ALREADY_EXISTS: u32 = 1;
    pub const ADAPTER_NON_EXISTENT: u32 = 2;
    pub const INVALID_POLICY: u32 = 3;
    pub const SERVANT_ALREADY_ACTIVE: u32 = 4;
    pub const OBJECT_ALREADY_ACTIVE: u32 = 5;
    pub const SERVANT_NOT_ACTIVE: u32 = 6;
    pub const OBJECT_NOT_ACTIVE: u32 = 7;
    pub const WRONG_POLICY: u32 = 8;
    pub const WRONG_ADAPTER: u32 = 9;
    pub const ADAPTER_INACTIVE: u32 = 10;
    pub const NO_SERVANT: u32 = 11;
    pub const NO_CONTEXT: u32 = 12;
    pub const INVALID_NAME: u32 = 13;
    pub const TC_BAD_KIND: u32 = 14;
    pub const TC_BOUNDS: u32 = 15;
}

/// `rorb_exception`.
#[repr(C)]
#[derive(Debug)]
pub struct Exception {
    pub repository_id: *mut c_char,
    pub repository_id_len: usize,
    pub minor: u32,
    pub completed: u32,
    pub local: u32,
    pub index: u32,
}

/// `rorb_bytes`.
#[repr(C)]
#[derive(Debug)]
pub struct Bytes {
    pub data: *mut u8,
    pub len: usize,
}

impl Bytes {
    pub(crate) fn new(v: Vec<u8>) -> Bytes {
        let boxed = v.into_boxed_slice();
        let len = boxed.len();
        let data = Box::into_raw(boxed) as *mut u8;
        Bytes { data, len }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rorb_abi_version() -> u32 {
    ABI_VERSION
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_bytes_free(bytes: Bytes) {
    if !bytes.data.is_null() {
        let slice = std::ptr::slice_from_raw_parts_mut(bytes.data, bytes.len);
        drop(unsafe { Box::from_raw(slice) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_exception_clear(exc: *mut Exception) {
    let Some(exc) = (unsafe { exc.as_mut() }) else {
        return;
    };
    if !exc.repository_id.is_null() {
        // Allocated by set_system with one byte more for the NUL.
        let slice = std::ptr::slice_from_raw_parts_mut(
            exc.repository_id as *mut u8,
            exc.repository_id_len + 1,
        );
        drop(unsafe { Box::from_raw(slice) });
    }
    exc.repository_id = std::ptr::null_mut();
    exc.repository_id_len = 0;
    exc.minor = 0;
    exc.completed = 0;
    exc.local = 0;
    exc.index = 0;
}

pub(crate) fn completion_code(c: CompletionStatus) -> u32 {
    match c {
        CompletionStatus::Yes => 0,
        CompletionStatus::No => 1,
        CompletionStatus::Maybe => 2,
    }
}

pub(crate) fn completion_status(code: u32) -> CompletionStatus {
    match code {
        0 => CompletionStatus::Yes,
        1 => CompletionStatus::No,
        _ => CompletionStatus::Maybe,
    }
}

/// Fills `exc` with a system exception and returns [`SYSTEM_EXCEPTION`].
pub(crate) unsafe fn set_system(exc: *mut Exception, e: &SystemException) -> Status {
    if let Some(exc) = unsafe { exc.as_mut() } {
        unsafe { rorb_exception_clear(exc) };
        let mut id = e.repository_id.clone().into_bytes();
        let len = id.len();
        id.push(0);
        exc.repository_id = Box::into_raw(id.into_boxed_slice()) as *mut c_char;
        exc.repository_id_len = len;
        exc.minor = e.minor;
        exc.completed = completion_code(e.completed);
    }
    SYSTEM_EXCEPTION
}

/// Fills `exc` with an exception of a local interface and returns
/// [`LOCAL_EXCEPTION`].
pub(crate) unsafe fn set_local(exc: *mut Exception, code: u32, index: u32) -> Status {
    if let Some(exc) = unsafe { exc.as_mut() } {
        unsafe { rorb_exception_clear(exc) };
        exc.local = code;
        exc.index = index;
    }
    LOCAL_EXCEPTION
}

pub(crate) unsafe fn set_poa_error(exc: *mut Exception, e: &PoaError) -> Status {
    let code = match e {
        PoaError::AdapterAlreadyExists => local::ADAPTER_ALREADY_EXISTS,
        PoaError::AdapterNonExistent => local::ADAPTER_NON_EXISTENT,
        PoaError::InvalidPolicy(i) => {
            return unsafe { set_local(exc, local::INVALID_POLICY, u32::from(*i)) };
        }
        PoaError::ServantAlreadyActive => local::SERVANT_ALREADY_ACTIVE,
        PoaError::ObjectAlreadyActive => local::OBJECT_ALREADY_ACTIVE,
        PoaError::ServantNotActive => local::SERVANT_NOT_ACTIVE,
        PoaError::ObjectNotActive => local::OBJECT_NOT_ACTIVE,
        PoaError::WrongPolicy => local::WRONG_POLICY,
        PoaError::WrongAdapter => local::WRONG_ADAPTER,
        PoaError::AdapterInactive => local::ADAPTER_INACTIVE,
        PoaError::NoServant => local::NO_SERVANT,
        PoaError::NoContext => local::NO_CONTEXT,
        PoaError::System(e) => return unsafe { set_system(exc, e) },
    };
    unsafe { set_local(exc, code, 0) }
}

pub(crate) fn sys(
    kind: SystemExceptionKind,
    minor: u32,
    completed: CompletionStatus,
) -> SystemException {
    SystemException::new(kind, minor, completed)
}

pub(crate) fn bad_param() -> SystemException {
    sys(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No)
}

/// A UTF-8 string argument; NULL is the empty string.
pub(crate) unsafe fn str_arg<'a>(p: *const c_char, len: usize) -> Result<&'a str, SystemException> {
    std::str::from_utf8(unsafe { bytes_arg(p as *const u8, len) }).map_err(|_| {
        sys(
            SystemExceptionKind::DATA_CONVERSION,
            0,
            CompletionStatus::No,
        )
    })
}

/// A byte string argument; NULL is the empty string.
pub(crate) unsafe fn bytes_arg<'a>(p: *const u8, len: usize) -> &'a [u8] {
    if p.is_null() || len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(p, len) }
    }
}

/// Runs `f`, turning a panic into `INTERNAL` rather than unwinding into C.
pub(crate) fn guard(exc: *mut Exception, f: impl FnOnce() -> Status) -> Status {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(_) => unsafe {
            set_system(
                exc,
                &sys(SystemExceptionKind::INTERNAL, 0, CompletionStatus::Maybe),
            )
        },
    }
}

/// Moves a value to the heap for C.
pub(crate) fn into_handle<T>(v: T) -> *mut T {
    Box::into_raw(Box::new(v))
}

/// Drops a handle made by [`into_handle`].
pub(crate) unsafe fn release_handle<T>(p: *mut T) {
    if !p.is_null() {
        drop(unsafe { Box::from_raw(p) });
    }
}
