//! The caller's instances that stand for core valuetype instances.
//!
//! The C++ and Java anys hold their values in the core: inserting writes
//! the caller's instance to a stream that the core reads back, extracting
//! writes the core's instance to a stream that the caller reads. To keep
//! one instance one across anys (a valuetype shared among the arguments
//! of a request stays shared), the two are linked (docs/valuetype.md):
//!
//! - a core instance keeps the caller's instance it stands for as its
//!   native object ([`ValueRef::set_native`]), so that extracting it gives
//!   that instance back instead of a new one;
//! - a registry maps the caller's key of an instance to the core instance
//!   (weakly), so that inserting the instance again reuses that core
//!   instance (its state replaced by the current one).
//!
//! The core instance keeps the caller's instance alive, so a key in the
//! registry whose core instance is alive names a live instance of the
//! caller: the key cannot have been reused.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, LazyLock, Mutex};

use rustorb_core::{ValueRef, WeakValueRef};

use crate::value::ReleaseFn;

/// A reference of the caller to its instance, which the core keeps for a
/// core instance; released with `release` when the core instance is
/// dropped (on any thread).
#[derive(Debug)]
pub(crate) struct Host {
    pub(crate) key: u64,
    pub(crate) ptr: *mut c_void,
    release: ReleaseFn,
}

// The caller makes `release` callable from any thread.
unsafe impl Send for Host {}
unsafe impl Sync for Host {}

impl Drop for Host {
    fn drop(&mut self) {
        {
            let mut map = registry().lock().unwrap_or_else(|e| e.into_inner());
            if map.get(&self.key).is_some_and(WeakValueRef::is_dead) {
                map.remove(&self.key);
            }
        }
        if let Some(release) = self.release {
            unsafe { release(self.ptr) };
        }
    }
}

fn registry() -> &'static Mutex<HashMap<u64, WeakValueRef>> {
    static REGISTRY: LazyLock<Mutex<HashMap<u64, WeakValueRef>>> = LazyLock::new(Default::default);
    &REGISTRY
}

/// The caller's instance `v` stands for, if it has one.
pub(crate) fn host_of(v: &ValueRef) -> Option<Arc<Host>> {
    v.native()?.downcast::<Host>().ok()
}

/// The core instance the caller's instance `key` stands for, if one is
/// alive.
pub(crate) fn lookup(key: u64) -> Option<ValueRef> {
    let weak = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key)
        .cloned()?;
    let v = weak.upgrade()?;
    // Linked to the same instance still (not unlinked since).
    host_of(&v).is_some_and(|h| h.key == key).then_some(v)
}

/// Makes `v` stand for the caller's instance `key`, taking `ptr` (released
/// with `release`).
pub(crate) fn attach(v: &ValueRef, key: u64, ptr: *mut c_void, release: ReleaseFn) {
    v.set_native(Some(Arc::new(Host { key, ptr, release })));
    let old = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, v.downgrade());
    drop(old);
}

/// A reference the caller handed over: released when dropped, unless
/// taken.
#[derive(Debug)]
pub(crate) struct Owned {
    ptr: *mut c_void,
    release: ReleaseFn,
}

impl Owned {
    pub(crate) fn new(ptr: *mut c_void, release: ReleaseFn) -> Owned {
        Owned { ptr, release }
    }

    /// The pointer and its release function, no longer released here.
    pub(crate) fn take(self) -> (*mut c_void, ReleaseFn) {
        let me = std::mem::ManuallyDrop::new(self);
        (me.ptr, me.release)
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        if let Some(release) = self.release {
            unsafe { release(self.ptr) };
        }
    }
}

/// A new value written to the stream of an insertion: its slot, the
/// caller's key and, if the caller gave one, its reference.
#[derive(Debug)]
pub(crate) struct Written {
    pub(crate) slot: usize,
    pub(crate) key: u64,
    pub(crate) host: Option<Owned>,
}
