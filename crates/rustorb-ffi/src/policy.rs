//! `ORB::create_policy`, `ORB::register_initial_reference` and the policy
//! overrides of object references, the calling thread and the ORB.
//! Policies cross the ABI as a type and a value
//! ([`PolicyValue::raw_value`]).

use std::ffi::{c_char, c_void};

use rustorb_core::{
    Any, CompletionStatus, Exception as CoreException, ObjectRef, Orb, PolicyType, PolicyValue,
    ResolveError, SetOverrideType, SystemException, SystemExceptionKind, policy,
};

use crate::{
    Exception, OK, Status, guard, into_handle, local, set_local, set_system, str_arg, sys,
};

/// Calls `each` with each policy.
type EachPolicyFn = extern "C" fn(*mut c_void, u32, u64);

/// The `count` policies of `types` and `values`, and the override mode.
/// `BAD_PARAM` for a mode other than 0 (SET) and 1 (ADD), or a policy
/// rustorb does not know.
unsafe fn policy_list(
    types: *const u32,
    values: *const u64,
    count: usize,
    set_add: u32,
) -> Result<(Vec<PolicyValue>, SetOverrideType), SystemException> {
    let bad_param = || sys(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No);
    let set_add = match set_add {
        0 => SetOverrideType::Set,
        1 => SetOverrideType::Add,
        _ => return Err(bad_param()),
    };
    if count == 0 {
        return Ok((Vec::new(), set_add));
    }
    let types = unsafe { std::slice::from_raw_parts(types, count) };
    let values = unsafe { std::slice::from_raw_parts(values, count) };
    let list = types
        .iter()
        .zip(values)
        .map(|(&t, &v)| PolicyValue::from_raw(t, v).ok_or_else(bad_param))
        .collect::<Result<_, _>>()?;
    Ok((list, set_add))
}

unsafe fn type_list<'a>(types: *const u32, count: usize) -> &'a [PolicyType] {
    if count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(types, count) }
    }
}

fn each_policy(policies: Vec<PolicyValue>, context: *mut c_void, each: EachPolicyFn) {
    for p in policies {
        each(context, p.policy_type(), p.raw_value());
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_create_policy(
    _orb: *mut Orb,
    policy_type: u32,
    value: *const Any,
    raw_value: *mut u64,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        match policy::create_policy(policy_type, unsafe { &*value }) {
            Ok(p) => {
                unsafe { *raw_value = p.raw_value() };
                OK
            }
            Err(e) => unsafe { set_local(exc, local::POLICY_ERROR, e.reason as u16 as u32) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_register_initial_reference(
    orb: *mut Orb,
    name: *const c_char,
    len: usize,
    obj: *mut ObjectRef,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let orb = unsafe { &*orb };
        let name = match unsafe { str_arg(name, len) } {
            Ok(n) => n,
            Err(_) => return unsafe { set_local(exc, local::INVALID_NAME, 0) },
        };
        match orb.register_initial_reference(name, unsafe { obj.as_ref() }) {
            Ok(()) => OK,
            Err(ResolveError::InvalidName(_)) => unsafe { set_local(exc, local::INVALID_NAME, 0) },
            Err(ResolveError::System(e)) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_set_policy_overrides(
    orb: *mut Orb,
    types: *const u32,
    values: *const u64,
    count: usize,
    set_add: u32,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let result = unsafe { policy_list(types, values, count, set_add) }
            .and_then(|(list, set_add)| unsafe { &*orb }.set_policy_overrides(&list, set_add));
        match result {
            Ok(()) => OK,
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_get_policy_overrides(
    orb: *mut Orb,
    types: *const u32,
    count: usize,
    context: *mut c_void,
    each: EachPolicyFn,
) {
    let types = unsafe { type_list(types, count) };
    each_policy(unsafe { &*orb }.policy_overrides(types), context, each);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_set_thread_policy_overrides(
    types: *const u32,
    values: *const u64,
    count: usize,
    set_add: u32,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let result = unsafe { policy_list(types, values, count, set_add) }
            .and_then(|(list, set_add)| policy::set_thread_policy_overrides(&list, set_add));
        match result {
            Ok(()) => OK,
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_thread_policy_overrides(
    types: *const u32,
    count: usize,
    context: *mut c_void,
    each: EachPolicyFn,
) {
    let types = unsafe { type_list(types, count) };
    each_policy(policy::thread_policy_overrides(types), context, each);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_set_policy_overrides(
    obj: *mut ObjectRef,
    types: *const u32,
    values: *const u64,
    count: usize,
    set_add: u32,
    result: *mut *mut ObjectRef,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let r = unsafe { policy_list(types, values, count, set_add) }
            .and_then(|(list, set_add)| unsafe { &*obj }.set_policy_overrides(&list, set_add));
        match r {
            Ok(o) => {
                unsafe { *result = into_handle(o) };
                OK
            }
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_get_policy_overrides(
    obj: *mut ObjectRef,
    types: *const u32,
    count: usize,
    context: *mut c_void,
    each: EachPolicyFn,
) {
    let types = unsafe { type_list(types, count) };
    each_policy(unsafe { &*obj }.policy_overrides(types), context, each);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_get_client_policy(
    obj: *mut ObjectRef,
    policy_type: u32,
    raw_value: *mut u64,
) -> i32 {
    match unsafe { &*obj }.client_policy(policy_type) {
        Some(p) => {
            unsafe { *raw_value = p.raw_value() };
            1
        }
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_validate_connection(
    obj: *mut ObjectRef,
    result: *mut i32,
    exc: *mut Exception,
) -> Status {
    guard(exc, || match unsafe { &*obj }.validate_connection() {
        Ok(v) => {
            unsafe { *result = i32::from(v) };
            OK
        }
        Err(CoreException::System(e)) => unsafe { set_system(exc, &e) },
        Err(CoreException::User(_)) => unsafe {
            set_system(
                exc,
                &sys(SystemExceptionKind::UNKNOWN, 0, CompletionStatus::No),
            )
        },
    })
}

/// An object of the binding registered as an initial reference: `release`
/// is called once the ORB drops it.
struct LocalRef {
    object: *mut c_void,
    release: extern "C" fn(*mut c_void),
}

// The binding's object is shared between threads by the binding itself.
unsafe impl Send for LocalRef {}
unsafe impl Sync for LocalRef {}

impl Drop for LocalRef {
    fn drop(&mut self) {
        (self.release)(self.object);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_register_local_initial_reference(
    orb: *mut Orb,
    name: *const c_char,
    len: usize,
    object: *mut c_void,
    release: extern "C" fn(*mut c_void),
    exc: *mut Exception,
) -> Status {
    let local = LocalRef { object, release };
    guard(exc, || {
        let orb = unsafe { &*orb };
        let name = match unsafe { str_arg(name, len) } {
            Ok(n) => n,
            Err(_) => return unsafe { set_local(exc, local::INVALID_NAME, 0) },
        };
        match orb.register_local_initial_reference(name, std::sync::Arc::new(local)) {
            Ok(()) => OK,
            Err(ResolveError::InvalidName(_)) => unsafe { set_local(exc, local::INVALID_NAME, 0) },
            Err(ResolveError::System(e)) => unsafe { set_system(exc, &e) },
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_orb_resolve_local_initial_reference(
    orb: *mut Orb,
    name: *const c_char,
    len: usize,
    context: *mut c_void,
    each: extern "C" fn(*mut c_void, *mut c_void),
) -> i32 {
    let orb = unsafe { &*orb };
    let Ok(name) = (unsafe { str_arg(name, len) }) else {
        return 0;
    };
    let Some(obj) = orb.resolve_local_initial_reference(name) else {
        return 0;
    };
    match obj.downcast_ref::<LocalRef>() {
        Some(local) => {
            each(context, local.object);
            1
        }
        None => 0,
    }
}
