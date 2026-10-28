//! The POA (`rorb_poa` is a boxed [`Poa`]), its manager (`rorb_poa_manager`,
//! a boxed [`PoaManager`]) and `PortableServer::Current`.

use std::ffi::{c_char, c_void};
use std::sync::Arc;

use rustorb_core::{
    CompletionStatus, IdAssignment, IdUniqueness, ImplicitActivation, Lifespan, ObjectRef, Poa,
    PoaCurrent, PoaError, PoaManager, PoaManagerState, Policy, RequestProcessing, Servant,
    ServantRetention, SystemExceptionKind, ThreadPolicy,
};

use crate::servant::{FfiServant, caller_object, servant_arg};
use crate::{
    Bytes, Exception, OK, Status, bytes_arg, guard, into_handle, local, release_handle, set_local,
    set_poa_error, str_arg, sys,
};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_clone(poa: *mut Poa) -> *mut Poa {
    into_handle(unsafe { &*poa }.clone())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_release(poa: *mut Poa) {
    unsafe { release_handle(poa) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_name(poa: *mut Poa, len: *mut usize) -> *const c_char {
    let name = unsafe { &*poa }.the_name();
    unsafe { *len = name.len() };
    name.as_ptr() as *const c_char
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_parent(poa: *mut Poa) -> *mut Poa {
    unsafe { &*poa }
        .the_parent()
        .map_or(std::ptr::null_mut(), into_handle)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_children(
    poa: *mut Poa,
    context: *mut c_void,
    each: extern "C" fn(*mut c_void, *mut Poa),
) {
    for child in unsafe { &*poa }.the_children() {
        each(context, into_handle(child));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_the_manager(poa: *mut Poa) -> *mut PoaManager {
    into_handle(unsafe { &*poa }.the_poa_manager())
}

/// `rorb_policy`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PolicyFfi {
    pub type_: u32,
    pub value: u32,
}

const THREAD_POLICY_ID: u32 = 16;
const LIFESPAN_POLICY_ID: u32 = 17;
const ID_UNIQUENESS_POLICY_ID: u32 = 18;
const ID_ASSIGNMENT_POLICY_ID: u32 = 19;
const IMPLICIT_ACTIVATION_POLICY_ID: u32 = 20;
const SERVANT_RETENTION_POLICY_ID: u32 = 21;
const REQUEST_PROCESSING_POLICY_ID: u32 = 22;

fn policy(p: PolicyFfi) -> Option<Policy> {
    let v = p.value;
    Some(match p.type_ {
        THREAD_POLICY_ID => Policy::Thread(match v {
            0 => ThreadPolicy::OrbCtrl,
            1 => ThreadPolicy::SingleThread,
            2 => ThreadPolicy::MainThread,
            _ => return None,
        }),
        LIFESPAN_POLICY_ID => Policy::Lifespan(match v {
            0 => Lifespan::Transient,
            1 => Lifespan::Persistent,
            _ => return None,
        }),
        ID_UNIQUENESS_POLICY_ID => Policy::IdUniqueness(match v {
            0 => IdUniqueness::Unique,
            1 => IdUniqueness::Multiple,
            _ => return None,
        }),
        ID_ASSIGNMENT_POLICY_ID => Policy::IdAssignment(match v {
            0 => IdAssignment::User,
            1 => IdAssignment::System,
            _ => return None,
        }),
        IMPLICIT_ACTIVATION_POLICY_ID => Policy::ImplicitActivation(match v {
            0 => ImplicitActivation::Implicit,
            1 => ImplicitActivation::NoImplicit,
            _ => return None,
        }),
        SERVANT_RETENTION_POLICY_ID => Policy::ServantRetention(match v {
            0 => ServantRetention::Retain,
            1 => ServantRetention::NonRetain,
            _ => return None,
        }),
        REQUEST_PROCESSING_POLICY_ID => Policy::RequestProcessing(match v {
            0 => RequestProcessing::ActiveObjectMapOnly,
            1 => RequestProcessing::UseDefaultServant,
            2 => RequestProcessing::UseServantManager,
            _ => return None,
        }),
        _ => return None,
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_policies(poa: *mut Poa, policies: *mut PolicyFfi) {
    let p = unsafe { &*poa }.policies();
    let values = [
        (
            THREAD_POLICY_ID,
            match p.thread {
                ThreadPolicy::OrbCtrl => 0,
                ThreadPolicy::SingleThread => 1,
                ThreadPolicy::MainThread => 2,
            },
        ),
        (
            LIFESPAN_POLICY_ID,
            match p.lifespan {
                Lifespan::Transient => 0,
                Lifespan::Persistent => 1,
            },
        ),
        (
            ID_UNIQUENESS_POLICY_ID,
            match p.id_uniqueness {
                IdUniqueness::Unique => 0,
                IdUniqueness::Multiple => 1,
            },
        ),
        (
            ID_ASSIGNMENT_POLICY_ID,
            match p.id_assignment {
                IdAssignment::User => 0,
                IdAssignment::System => 1,
            },
        ),
        (
            IMPLICIT_ACTIVATION_POLICY_ID,
            match p.implicit_activation {
                ImplicitActivation::Implicit => 0,
                ImplicitActivation::NoImplicit => 1,
            },
        ),
        (
            SERVANT_RETENTION_POLICY_ID,
            match p.servant_retention {
                ServantRetention::Retain => 0,
                ServantRetention::NonRetain => 1,
            },
        ),
        (
            REQUEST_PROCESSING_POLICY_ID,
            match p.request_processing {
                RequestProcessing::ActiveObjectMapOnly => 0,
                RequestProcessing::UseDefaultServant => 1,
                RequestProcessing::UseServantManager => 2,
            },
        ),
    ];
    for (i, (type_, value)) in values.into_iter().enumerate() {
        unsafe { *policies.add(i) = PolicyFfi { type_, value } };
    }
}

/// Runs a POA operation and reports its error.
fn poa_call<T>(
    exc: *mut Exception,
    f: impl FnOnce() -> Result<T, PoaError>,
    done: impl FnOnce(T),
) -> Status {
    guard(exc, || match f() {
        Ok(v) => {
            done(v);
            OK
        }
        Err(e) => unsafe { set_poa_error(exc, &e) },
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_create_poa(
    poa: *mut Poa,
    name: *const c_char,
    len: usize,
    manager: *mut PoaManager,
    policies: *const PolicyFfi,
    count: usize,
    child: *mut *mut Poa,
    exc: *mut Exception,
) -> Status {
    let list = if policies.is_null() || count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(policies, count) }
    };
    let mut converted = Vec::with_capacity(list.len());
    for (i, p) in list.iter().enumerate() {
        match policy(*p) {
            Some(p) => converted.push(p),
            None => {
                return unsafe {
                    set_local(exc, local::INVALID_POLICY, u32::try_from(i).unwrap_or(0))
                };
            }
        }
    }
    poa_call(
        exc,
        || {
            let name = unsafe { str_arg(name, len) }?;
            unsafe { &*poa }.create_poa(name, unsafe { manager.as_ref() }, &converted)
        },
        |p| unsafe { *child = into_handle(p) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_find_poa(
    poa: *mut Poa,
    name: *const c_char,
    len: usize,
    activate_it: i32,
    child: *mut *mut Poa,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || {
            let name = unsafe { str_arg(name, len) }?;
            unsafe { &*poa }.find_poa(name, activate_it != 0)
        },
        |p| unsafe { *child = into_handle(p) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_destroy(
    poa: *mut Poa,
    etherealize: i32,
    wait_for_completion: i32,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*poa }.destroy(etherealize != 0, wait_for_completion != 0),
        |()| {},
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_activate_object(
    poa: *mut Poa,
    servant: *mut Arc<FfiServant>,
    id: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*poa }.activate_object(unsafe { servant_arg(servant) }),
        |v| unsafe { *id = Bytes::new(v) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_activate_object_with_id(
    poa: *mut Poa,
    id: *const u8,
    len: usize,
    servant: *mut Arc<FfiServant>,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || {
            unsafe { &*poa }.activate_object_with_id(unsafe { bytes_arg(id, len) }, unsafe {
                servant_arg(servant)
            })
        },
        |()| {},
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_deactivate_object(
    poa: *mut Poa,
    id: *const u8,
    len: usize,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*poa }.deactivate_object(unsafe { bytes_arg(id, len) }),
        |()| {},
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_servant_to_id(
    poa: *mut Poa,
    servant: *mut Arc<FfiServant>,
    id: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*poa }.servant_to_id(&unsafe { servant_arg(servant) }),
        |v| unsafe { *id = Bytes::new(v) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_servant_to_reference(
    poa: *mut Poa,
    servant: *mut Arc<FfiServant>,
    obj: *mut *mut ObjectRef,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*poa }.servant_to_reference(&unsafe { servant_arg(servant) }),
        |r| unsafe { *obj = into_handle(r) },
    )
}

/// The caller's object of a servant from the POA, or `OBJ_ADAPTER`.
fn caller(servant: Arc<dyn Servant>) -> Result<*mut c_void, PoaError> {
    caller_object(&servant).ok_or_else(|| {
        PoaError::System(sys(
            SystemExceptionKind::OBJ_ADAPTER,
            0,
            CompletionStatus::No,
        ))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_id_to_servant(
    poa: *mut Poa,
    id: *const u8,
    len: usize,
    this: *mut *mut c_void,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || caller(unsafe { &*poa }.id_to_servant(unsafe { bytes_arg(id, len) })?),
        |s| unsafe { *this = s },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_id_to_reference(
    poa: *mut Poa,
    id: *const u8,
    len: usize,
    obj: *mut *mut ObjectRef,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*poa }.id_to_reference(unsafe { bytes_arg(id, len) }),
        |r| unsafe { *obj = into_handle(r) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_reference_to_id(
    poa: *mut Poa,
    obj: *mut ObjectRef,
    id: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*poa }.reference_to_id(unsafe { &*obj }),
        |v| unsafe { *id = Bytes::new(v) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_reference_to_servant(
    poa: *mut Poa,
    obj: *mut ObjectRef,
    this: *mut *mut c_void,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || caller(unsafe { &*poa }.reference_to_servant(unsafe { &*obj })?),
        |s| unsafe { *this = s },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_create_reference(
    poa: *mut Poa,
    intf: *const c_char,
    len: usize,
    obj: *mut *mut ObjectRef,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*poa }.create_reference(unsafe { str_arg(intf, len) }?),
        |r| unsafe { *obj = into_handle(r) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_create_reference_with_id(
    poa: *mut Poa,
    id: *const u8,
    id_len: usize,
    intf: *const c_char,
    len: usize,
    obj: *mut *mut ObjectRef,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || {
            unsafe { &*poa }.create_reference_with_id(unsafe { bytes_arg(id, id_len) }, unsafe {
                str_arg(intf, len)
            }?)
        },
        |r| unsafe { *obj = into_handle(r) },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_get_servant(
    poa: *mut Poa,
    this: *mut *mut c_void,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || caller(unsafe { &*poa }.get_servant()?),
        |s| unsafe { *this = s },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_set_servant(
    poa: *mut Poa,
    servant: *mut Arc<FfiServant>,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*poa }.set_servant(unsafe { servant_arg(servant) }),
        |()| {},
    )
}

// --- PortableServer::Current ----------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_current_get_poa(poa: *mut *mut Poa, exc: *mut Exception) -> Status {
    poa_call(exc, PoaCurrent::get_poa, |p| unsafe {
        *poa = into_handle(p)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_current_get_object_id(id: *mut Bytes, exc: *mut Exception) -> Status {
    poa_call(exc, PoaCurrent::get_object_id, |v| unsafe {
        *id = Bytes::new(v)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_current_get_reference(
    obj: *mut *mut ObjectRef,
    exc: *mut Exception,
) -> Status {
    poa_call(exc, PoaCurrent::get_reference, |r| unsafe {
        *obj = into_handle(r)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_current_get_servant(
    this: *mut *mut c_void,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || caller(PoaCurrent::get_servant()?),
        |s| unsafe { *this = s },
    )
}

// --- POAManager -----------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_manager_clone(manager: *mut PoaManager) -> *mut PoaManager {
    into_handle(unsafe { &*manager }.clone())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_manager_release(manager: *mut PoaManager) {
    unsafe { release_handle(manager) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_manager_get_state(manager: *mut PoaManager) -> u32 {
    match unsafe { &*manager }.state() {
        PoaManagerState::Holding => 0,
        PoaManagerState::Active => 1,
        PoaManagerState::Discarding => 2,
        PoaManagerState::Inactive => 3,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_manager_activate(
    manager: *mut PoaManager,
    exc: *mut Exception,
) -> Status {
    poa_call(exc, || unsafe { &*manager }.activate(), |()| {})
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_manager_hold_requests(
    manager: *mut PoaManager,
    wait_for_completion: i32,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*manager }.hold_requests(wait_for_completion != 0),
        |()| {},
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_manager_discard_requests(
    manager: *mut PoaManager,
    wait_for_completion: i32,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*manager }.discard_requests(wait_for_completion != 0),
        |()| {},
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_manager_deactivate(
    manager: *mut PoaManager,
    etherealize: i32,
    wait_for_completion: i32,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*manager }.deactivate(etherealize != 0, wait_for_completion != 0),
        |()| {},
    )
}
