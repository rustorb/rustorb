//! Servant managers (`ServantActivator`, `ServantLocator`) and adapter
//! activators implemented by the caller through a table of callbacks.
//!
//! Like servants, each one holds a reference to the caller's object
//! (`add_ref` when the POA takes it, `remove_ref` when the POA lets it go).
//! The objects the POA holds are remembered by address, so that
//! `get_servant_manager` and `the_activator` give back the caller's object.

use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::sync::{Arc, Mutex};

use rustorb_core::{
    AdapterActivator, CompletionStatus, Cookie, ObjectRef, Poa, Servant, ServantActivator,
    ServantLocator, ServantManager, ServantManagerError, SystemException, SystemExceptionKind,
};

use crate::poa::poa_call;
use crate::servant::{FfiServant, caller_self};
use crate::{Exception, OK, Status, completion_status, into_handle, release_handle, str_arg, sys};

/// `rorb_servant_activator_vtable`.
#[repr(C)]
#[derive(Debug)]
pub struct ActivatorVtable {
    pub incarnate: extern "C" fn(
        *mut c_void,
        *const u8,
        usize,
        *mut Poa,
        *mut *mut Arc<FfiServant>,
        *mut ManagerError,
    ) -> Status,
    pub etherealize:
        Option<extern "C" fn(*mut c_void, *const u8, usize, *mut Poa, *mut c_void, i32, i32)>,
    pub add_ref: extern "C" fn(*mut c_void),
    pub remove_ref: extern "C" fn(*mut c_void),
}

/// `rorb_servant_locator_vtable`.
#[repr(C)]
#[derive(Debug)]
pub struct LocatorVtable {
    #[allow(clippy::type_complexity)]
    pub preinvoke: extern "C" fn(
        *mut c_void,
        *const u8,
        usize,
        *mut Poa,
        *const c_char,
        usize,
        *mut *mut Arc<FfiServant>,
        *mut *mut c_void,
        *mut ManagerError,
    ) -> Status,
    #[allow(clippy::type_complexity)]
    pub postinvoke: Option<
        extern "C" fn(
            *mut c_void,
            *const u8,
            usize,
            *mut Poa,
            *const c_char,
            usize,
            *mut c_void,
            *mut c_void,
        ),
    >,
    pub add_ref: extern "C" fn(*mut c_void),
    pub remove_ref: extern "C" fn(*mut c_void),
}

/// `rorb_adapter_activator_vtable`.
#[repr(C)]
#[derive(Debug)]
pub struct AdapterActivatorVtable {
    pub unknown_adapter: extern "C" fn(*mut c_void, *mut Poa, *const c_char, usize) -> i32,
    pub add_ref: extern "C" fn(*mut c_void),
    pub remove_ref: extern "C" fn(*mut c_void),
}

/// `rorb_manager_error`: what `incarnate` or `preinvoke` raised.
#[derive(Debug, Default)]
pub struct ManagerError(Option<ServantManagerError>);

/// `RORB_SERVANT_ACTIVATOR` and `RORB_SERVANT_LOCATOR`.
const ACTIVATOR: i32 = 1;
const LOCATOR: i32 = 2;

/// The caller's object of each live manager, by the address of its Rust
/// side.
static LIVE: Mutex<Option<HashMap<usize, usize>>> = Mutex::new(None);

fn register(addr: usize, this: *mut c_void) {
    LIVE.lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(addr, this as usize);
}

fn unregister(addr: usize) {
    if let Some(live) = LIVE.lock().unwrap().as_mut() {
        live.remove(&addr);
    }
}

fn caller_of<T: ?Sized>(manager: &Arc<T>) -> Option<*mut c_void> {
    let addr = Arc::as_ptr(manager) as *const () as usize;
    let live = LIVE.lock().unwrap();
    live.as_ref()?.get(&addr).map(|&p| p as *mut c_void)
}

/// The reference counting callbacks every table has.
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

refs!(ActivatorVtable, LocatorVtable, AdapterActivatorVtable);

/// One of the caller's objects with its callbacks.
struct Caller<V: Refs> {
    this: *mut c_void,
    vtable: *const V,
}

// The caller's managers are called from any upcall thread, as the header
// says.
unsafe impl<V: Refs> Send for Caller<V> {}
unsafe impl<V: Refs> Sync for Caller<V> {}

impl<V: Refs> Caller<V> {
    fn new(this: *mut c_void, vtable: *const V) -> Arc<Self> {
        (unsafe { &*vtable }.add_ref())(this);
        let c = Arc::new(Caller { this, vtable });
        register(Arc::as_ptr(&c) as usize, this);
        c
    }

    fn vtable(&self) -> &V {
        unsafe { &*self.vtable }
    }

    /// The caller's object with a reference added for the caller.
    fn get(&self) -> *mut c_void {
        (self.vtable().add_ref())(self.this);
        self.this
    }
}

impl<V: Refs> Drop for Caller<V> {
    fn drop(&mut self) {
        unregister(self as *const Self as usize);
        (self.vtable().remove_ref())(self.this);
    }
}

/// A handle of `adapter` for the duration of a callback.
struct Borrowed(*mut Poa);

impl Borrowed {
    fn new(adapter: &Poa) -> Borrowed {
        Borrowed(into_handle(adapter.clone()))
    }
}

impl Drop for Borrowed {
    fn drop(&mut self) {
        unsafe { release_handle(self.0) };
    }
}

/// The servant a callback gave (one handle reference), or the error it
/// raised.
fn provided(
    status: Status,
    servant: *mut Arc<FfiServant>,
    error: ManagerError,
) -> Result<Arc<dyn Servant>, ServantManagerError> {
    let servant = (!servant.is_null()).then(|| *unsafe { Box::from_raw(servant) });
    if status != OK {
        return Err(error.0.unwrap_or_else(|| {
            ServantManagerError::System(sys(
                SystemExceptionKind::UNKNOWN,
                0,
                CompletionStatus::Maybe,
            ))
        }));
    }
    match servant {
        Some(s) => Ok(s),
        None => Err(ServantManagerError::System(sys(
            SystemExceptionKind::OBJ_ADAPTER,
            0,
            CompletionStatus::No,
        ))),
    }
}

impl ServantActivator for Caller<ActivatorVtable> {
    fn incarnate(&self, id: &[u8], adapter: &Poa) -> Result<Arc<dyn Servant>, ServantManagerError> {
        let poa = Borrowed::new(adapter);
        let mut servant = std::ptr::null_mut();
        let mut error = ManagerError::default();
        let status = (self.vtable().incarnate)(
            self.this,
            id.as_ptr(),
            id.len(),
            poa.0,
            &mut servant,
            &mut error,
        );
        provided(status, servant, error)
    }

    fn etherealize(
        &self,
        id: &[u8],
        adapter: &Poa,
        servant: Arc<dyn Servant>,
        cleanup_in_progress: bool,
        remaining_activations: bool,
    ) {
        let Some(etherealize) = self.vtable().etherealize else {
            return;
        };
        let poa = Borrowed::new(adapter);
        etherealize(
            self.this,
            id.as_ptr(),
            id.len(),
            poa.0,
            caller_self(&servant),
            i32::from(cleanup_in_progress),
            i32::from(remaining_activations),
        );
        // The POA's reference to the servant goes after the callback.
        drop(servant);
    }
}

/// The caller's cookie.
struct CookiePtr(*mut c_void);

unsafe impl Send for CookiePtr {}

impl ServantLocator for Caller<LocatorVtable> {
    fn preinvoke(
        &self,
        id: &[u8],
        adapter: &Poa,
        operation: &str,
    ) -> Result<(Arc<dyn Servant>, Cookie), ServantManagerError> {
        let poa = Borrowed::new(adapter);
        let mut servant = std::ptr::null_mut();
        let mut cookie = std::ptr::null_mut();
        let mut error = ManagerError::default();
        let status = (self.vtable().preinvoke)(
            self.this,
            id.as_ptr(),
            id.len(),
            poa.0,
            operation.as_ptr() as *const c_char,
            operation.len(),
            &mut servant,
            &mut cookie,
            &mut error,
        );
        let servant = provided(status, servant, error)?;
        Ok((servant, Box::new(CookiePtr(cookie))))
    }

    fn postinvoke(
        &self,
        id: &[u8],
        adapter: &Poa,
        operation: &str,
        cookie: Cookie,
        servant: Arc<dyn Servant>,
    ) {
        let Some(postinvoke) = self.vtable().postinvoke else {
            return;
        };
        let cookie = cookie
            .downcast::<CookiePtr>()
            .map_or(std::ptr::null_mut(), |c| c.0);
        let poa = Borrowed::new(adapter);
        postinvoke(
            self.this,
            id.as_ptr(),
            id.len(),
            poa.0,
            operation.as_ptr() as *const c_char,
            operation.len(),
            cookie,
            caller_self(&servant),
        );
        drop(servant);
    }
}

impl AdapterActivator for Caller<AdapterActivatorVtable> {
    fn unknown_adapter(&self, parent: &Poa, name: &str) -> bool {
        let poa = Borrowed::new(parent);
        (self.vtable().unknown_adapter)(
            self.this,
            poa.0,
            name.as_ptr() as *const c_char,
            name.len(),
        ) != 0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_manager_error_forward(error: *mut ManagerError, obj: *mut ObjectRef) {
    let error = unsafe { &mut *error };
    error.0 = Some(match unsafe { obj.as_ref() } {
        Some(obj) => ServantManagerError::ForwardRequest(obj.clone()),
        // A nil forward reference.
        None => ServantManagerError::System(sys(
            SystemExceptionKind::BAD_PARAM,
            0,
            CompletionStatus::No,
        )),
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_manager_error_system(
    error: *mut ManagerError,
    id: *const c_char,
    len: usize,
    minor: u32,
    completed: u32,
) {
    let repository_id = unsafe { str_arg(id, len) }
        .unwrap_or("IDL:omg.org/CORBA/UNKNOWN:1.0")
        .to_owned();
    unsafe { &mut *error }.0 = Some(ServantManagerError::System(SystemException {
        repository_id,
        minor,
        completed: completion_status(completed),
    }));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_set_servant_activator(
    poa: *mut Poa,
    this: *mut c_void,
    vtable: *const ActivatorVtable,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || {
            let manager = Caller::<ActivatorVtable>::new(this, vtable);
            unsafe { &*poa }.set_servant_manager(ServantManager::Activator(manager))
        },
        |()| {},
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_set_servant_locator(
    poa: *mut Poa,
    this: *mut c_void,
    vtable: *const LocatorVtable,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || {
            let manager = Caller::<LocatorVtable>::new(this, vtable);
            unsafe { &*poa }.set_servant_manager(ServantManager::Locator(manager))
        },
        |()| {},
    )
}

/// The caller's object of a manager the POA holds, with a reference added
/// for the caller, or NULL for none or one of another kind.
fn manager_object(manager: &ServantManager) -> (i32, *mut c_void) {
    match manager {
        ServantManager::Activator(a) => (
            ACTIVATOR,
            caller_of(a).map_or(std::ptr::null_mut(), |_| {
                // Registered, so `a` is one of ours.
                let c =
                    unsafe { &*(Arc::as_ptr(a) as *const () as *const Caller<ActivatorVtable>) };
                c.get()
            }),
        ),
        ServantManager::Locator(l) => (
            LOCATOR,
            caller_of(l).map_or(std::ptr::null_mut(), |_| {
                let c = unsafe { &*(Arc::as_ptr(l) as *const () as *const Caller<LocatorVtable>) };
                c.get()
            }),
        ),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_get_servant_manager(
    poa: *mut Poa,
    kind: *mut i32,
    this: *mut *mut c_void,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || unsafe { &*poa }.get_servant_manager(),
        |m| {
            let (k, p) = m.as_ref().map_or((0, std::ptr::null_mut()), manager_object);
            unsafe {
                *kind = k;
                *this = p;
            }
        },
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_set_the_activator(
    poa: *mut Poa,
    this: *mut c_void,
    vtable: *const AdapterActivatorVtable,
    exc: *mut Exception,
) -> Status {
    poa_call(
        exc,
        || {
            let activator: Option<Arc<dyn AdapterActivator>> = if this.is_null() {
                None
            } else {
                Some(Caller::<AdapterActivatorVtable>::new(this, vtable))
            };
            unsafe { &*poa }.set_the_activator(activator)
        },
        |()| {},
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_poa_the_activator(poa: *mut Poa) -> *mut c_void {
    let Some(a) = unsafe { &*poa }.the_activator() else {
        return std::ptr::null_mut();
    };
    match caller_of(&a) {
        Some(_) => {
            let c = unsafe {
                &*(Arc::as_ptr(&a) as *const () as *const Caller<AdapterActivatorVtable>)
            };
            c.get()
        }
        None => std::ptr::null_mut(),
    }
}
