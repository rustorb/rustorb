//! Servants implemented in C (`rorb_servant` is a boxed
//! `Arc<FfiServant>`) and the replies they write.
//!
//! Each servant object of the caller (`self`) has one [`FfiServant`] while
//! the POA or a handle holds it, found again by `self` in a registry. The
//! `FfiServant` holds a reference to `self` (`add_ref` when made,
//! `remove_ref` when dropped); the caller's object only points back
//! weakly, through the registry, so there is no cycle.

use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::sync::{Arc, Mutex, Weak};

use rustorb_core::{
    CompletionStatus, InStream, ObjectRef, PoaCurrent, Servant, ServerReply, SystemException,
    SystemExceptionKind,
};

use crate::stream::{In, Out, RorbIn, RorbOut};
use crate::{OK, SYSTEM_EXCEPTION, Status, completion_status, str_arg, sys};

/// `rorb_servant_vtable`.
#[repr(C)]
#[derive(Debug)]
pub struct ServantVtable {
    pub dispatch:
        extern "C" fn(*mut c_void, *const c_char, usize, *mut RorbIn, *mut ReplyFfi) -> Status,
    pub is_a: extern "C" fn(*mut c_void, *const c_char, usize) -> i32,
    pub add_ref: extern "C" fn(*mut c_void),
    pub remove_ref: extern "C" fn(*mut c_void),
}

/// A servant of the caller, as the POA sees it.
#[derive(Debug)]
pub struct FfiServant {
    this: *mut c_void,
    vtable: *const ServantVtable,
    repository_id: String,
}

// The caller's servants are called from any upcall thread, as the header
// says.
unsafe impl Send for FfiServant {}
unsafe impl Sync for FfiServant {}

#[derive(Default)]
struct Registry {
    /// The `FfiServant` of each caller object.
    by_self: HashMap<usize, Weak<FfiServant>>,
    /// The caller object of each live `FfiServant`, by its address.
    by_addr: HashMap<usize, usize>,
}

static REGISTRY: Mutex<Option<Registry>> = Mutex::new(None);

impl FfiServant {
    fn vtable(&self) -> &ServantVtable {
        unsafe { &*self.vtable }
    }
}

impl Drop for FfiServant {
    fn drop(&mut self) {
        {
            let mut reg = REGISTRY.lock().unwrap();
            if let Some(reg) = reg.as_mut() {
                reg.by_addr.remove(&(self as *const FfiServant as usize));
                let key = self.this as usize;
                if reg.by_self.get(&key).is_some_and(|w| w.upgrade().is_none()) {
                    reg.by_self.remove(&key);
                }
            }
        }
        // Outside the lock: the caller's object may go now, and release
        // other servants.
        (self.vtable().remove_ref)(self.this);
    }
}

impl Servant for FfiServant {
    fn repository_id(&self) -> &str {
        &self.repository_id
    }

    fn is_a(&self, repository_id: &str) -> bool {
        (self.vtable().is_a)(
            self.this,
            repository_id.as_ptr() as *const c_char,
            repository_id.len(),
        ) != 0
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        let mut input = In::new(input, CompletionStatus::No);
        let mut r = ReplyFfi {
            out: Out::reply(reply),
            system: None,
        };
        let status = (self.vtable().dispatch)(
            self.this,
            operation.as_ptr() as *const c_char,
            operation.len(),
            input.as_ptr(),
            &mut r,
        );
        match status {
            OK => {
                if let Some(e) = input.error {
                    return Err(e);
                }
                // The operation has run.
                if let Some(mut e) = r.out.error {
                    e.completed = CompletionStatus::Yes;
                    return Err(e);
                }
                Ok(())
            }
            SYSTEM_EXCEPTION => Err(r.system.unwrap_or_else(unknown)),
            _ => Err(unknown()),
        }
    }
}

fn unknown() -> SystemException {
    sys(SystemExceptionKind::UNKNOWN, 0, CompletionStatus::Maybe)
}

/// `rorb_server_reply`.
#[derive(Debug)]
pub struct ReplyFfi {
    out: Out<'static, 'static>,
    system: Option<SystemException>,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_servant_get(
    this: *mut c_void,
    repository_id: *const c_char,
    len: usize,
    vtable: *const ServantVtable,
) -> *mut Arc<FfiServant> {
    let repository_id = unsafe { str_arg(repository_id, len) }
        .unwrap_or_default()
        .to_owned();
    let mut reg = REGISTRY.lock().unwrap();
    let reg = reg.get_or_insert_with(Registry::default);
    let key = this as usize;
    if let Some(s) = reg.by_self.get(&key).and_then(Weak::upgrade) {
        return Box::into_raw(Box::new(s));
    }
    let vt = unsafe { &*vtable };
    (vt.add_ref)(this);
    let servant = Arc::new(FfiServant {
        this,
        vtable,
        repository_id,
    });
    reg.by_self.insert(key, Arc::downgrade(&servant));
    reg.by_addr.insert(Arc::as_ptr(&servant) as usize, key);
    Box::into_raw(Box::new(servant))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_servant_release(servant: *mut Arc<FfiServant>) {
    if !servant.is_null() {
        drop(unsafe { Box::from_raw(servant) });
    }
}

/// The servant behind a handle, as the POA takes it.
pub(crate) unsafe fn servant_arg(servant: *mut Arc<FfiServant>) -> Arc<dyn Servant> {
    let s: Arc<FfiServant> = Arc::clone(unsafe { &*servant });
    s
}

/// The caller's object of a servant from the POA, with a reference added
/// for the caller; `None` if it is not one of the caller's servants.
pub(crate) fn caller_object(servant: &Arc<dyn Servant>) -> Option<*mut c_void> {
    let addr = Arc::as_ptr(servant) as *const () as usize;
    let reg = REGISTRY.lock().unwrap();
    reg.as_ref()?.by_addr.get(&addr)?;
    // `servant` keeps the FfiServant at `addr` alive.
    let s = unsafe { &*(addr as *const FfiServant) };
    (s.vtable().add_ref)(s.this);
    Some(s.this)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_servant_current_reference(
    servant: *mut Arc<FfiServant>,
    obj: *mut *mut ObjectRef,
) -> i32 {
    let Ok(current) = PoaCurrent::get_servant() else {
        return 0;
    };
    let mine = Arc::as_ptr(unsafe { &*servant }) as *const ();
    if Arc::as_ptr(&current) as *const () != mine {
        return 0;
    }
    match PoaCurrent::get_reference() {
        Ok(r) => {
            unsafe { *obj = crate::into_handle(r) };
            1
        }
        Err(_) => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_server_reply_output(reply: *mut ReplyFfi) -> *mut RorbOut {
    unsafe { &mut *reply }.out.as_ptr()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_server_reply_user_exception(
    reply: *mut ReplyFfi,
    id: *const c_char,
    len: usize,
) -> *mut RorbOut {
    let r = unsafe { &mut *reply };
    let result = unsafe { str_arg(id, len) }.and_then(|id| {
        let server_reply = r.out.server_reply();
        unsafe { &mut *server_reply }.user_exception(id).map(drop)
    });
    r.out.error = result.err();
    r.out.as_ptr()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_server_reply_system_exception(
    reply: *mut ReplyFfi,
    id: *const c_char,
    len: usize,
    minor: u32,
    completed: u32,
) {
    let r = unsafe { &mut *reply };
    let repository_id = unsafe { str_arg(id, len) }
        .unwrap_or("IDL:omg.org/CORBA/UNKNOWN:1.0")
        .to_owned();
    r.system = Some(SystemException {
        repository_id,
        minor,
        completed: completion_status(completed),
    });
}
