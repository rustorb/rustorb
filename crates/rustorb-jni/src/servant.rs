//! Java servants: each one has a [`Peer`], whose address is the `self` of
//! the C ABI (`rorb_servant_get`).
//!
//! The peer keeps a weak reference to the servant, and a strong one while
//! the C side holds references to it (between `add_ref` and the matching
//! `remove_ref`), so that a servant the POA holds is not collected. The
//! servant's delegate (org.rustorb.ServantDelegateImpl) owns the peer and
//! frees it once the servant is unreachable, which can only happen when
//! the C side holds none.

use std::ffi::{c_char, c_void};
use std::sync::Mutex;

use jni::JNIEnv;
use jni::objects::{GlobalRef, JClass, JObject, JString, WeakRef};
use jni::sys::{jint, jlong, jvalue};

use crate::sys::*;
use crate::{
    BOOLEAN, INT, JResult, Thrown, VM, cache, call_bridge, handle, java_string, ptr, run,
    rust_string,
};

pub(crate) struct Peer {
    weak: WeakRef,
    state: Mutex<PeerState>,
}

#[derive(Default)]
struct PeerState {
    strong: Option<GlobalRef>,
    refs: usize,
}

static VTABLE: rorb_servant_vtable = rorb_servant_vtable {
    dispatch,
    is_a,
    add_ref,
    remove_ref,
};

/// The JNIEnv of the calling thread, attaching it (as a daemon, detached
/// when the thread exits) if it is not attached yet.
fn thread_env() -> Option<JNIEnv<'static>> {
    VM.get()?.attach_current_thread_as_daemon().ok()
}

impl Peer {
    fn strong(&self) -> Option<GlobalRef> {
        self.state.lock().unwrap().strong.clone()
    }
}

impl std::fmt::Debug for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Peer").finish_non_exhaustive()
    }
}

unsafe extern "C" fn add_ref(this: *mut c_void) {
    let peer = unsafe { &*(this as *const Peer) };
    let mut state = peer.state.lock().unwrap();
    if state.refs == 0 {
        if let Some(env) = thread_env() {
            state.strong = peer.weak.upgrade_global(&env).ok().flatten();
        }
    }
    state.refs += 1;
}

unsafe extern "C" fn remove_ref(this: *mut c_void) {
    let peer = unsafe { &*(this as *const Peer) };
    let dropped = {
        let mut state = peer.state.lock().unwrap();
        state.refs = state.refs.saturating_sub(1);
        if state.refs == 0 {
            state.strong.take()
        } else {
            None
        }
    };
    // Dropping the global reference calls the JVM; not under the lock.
    drop(dropped);
}

unsafe extern "C" fn dispatch(
    this: *mut c_void,
    op: *const c_char,
    op_len: usize,
    r#in: *mut rorb_in,
    reply: *mut rorb_server_reply,
) -> rorb_status {
    let peer = unsafe { &*(this as *const Peer) };
    let (Some(mut env), Some(servant)) = (thread_env(), peer.strong()) else {
        unsafe { system_reply(reply, "IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0") };
        return RORB_SYSTEM_EXCEPTION;
    };
    let result = env.with_local_frame(8, |env| -> JResult<i32> {
        let op = java_string(env, op, op_len)?;
        let v = call_bridge(
            env,
            cache().dispatch,
            INT,
            &[
                jvalue {
                    l: servant.as_obj().as_raw(),
                },
                jvalue { l: op.as_raw() },
                jvalue { j: r#in as jlong },
                jvalue { j: reply as jlong },
            ],
        )?;
        Ok(v.i()?)
    });
    match result {
        Ok(st) => st,
        Err(Thrown) => {
            let _ = env.exception_clear();
            unsafe { system_reply(reply, "IDL:omg.org/CORBA/UNKNOWN:1.0") };
            RORB_SYSTEM_EXCEPTION
        }
    }
}

unsafe fn system_reply(reply: *mut rorb_server_reply, id: &str) {
    unsafe {
        rorb_server_reply_system_exception(reply, id.as_ptr() as *const c_char, id.len(), 0, 2)
    };
}

unsafe extern "C" fn is_a(this: *mut c_void, id: *const c_char, len: usize) -> i32 {
    let peer = unsafe { &*(this as *const Peer) };
    let (Some(mut env), Some(servant)) = (thread_env(), peer.strong()) else {
        return 0;
    };
    let result = env.with_local_frame(4, |env| -> JResult<bool> {
        let id = java_string(env, id, len)?;
        let v = call_bridge(
            env,
            cache().is_a,
            BOOLEAN,
            &[
                jvalue {
                    l: servant.as_obj().as_raw(),
                },
                jvalue { l: id.as_raw() },
            ],
        )?;
        Ok(v.z()?)
    });
    match result {
        Ok(b) => b as i32,
        Err(Thrown) => {
            let _ = env.exception_clear();
            0
        }
    }
}

/// The Java servant of a `self` the C side returned with a reference for
/// the caller (`rorb_poa_id_to_servant`...), as a local reference; gives
/// that reference back.
pub(crate) fn servant_object<'local>(
    env: &mut JNIEnv<'local>,
    this: *mut c_void,
) -> JResult<JObject<'local>> {
    if this.is_null() {
        return Ok(JObject::null());
    }
    let peer = unsafe { &*(this as *const Peer) };
    let obj = match peer.strong() {
        Some(g) => env.new_local_ref(g.as_obj())?,
        None => JObject::null(),
    };
    unsafe { remove_ref(this) };
    Ok(obj)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_servantPeerNew(
    mut env: JNIEnv,
    _: JClass,
    servant: JObject,
) -> jlong {
    run(&mut env, 0, |env| {
        let Some(weak) = env.new_weak_ref(&servant)? else {
            return crate::throw_system(env, "IDL:omg.org/CORBA/BAD_PARAM:1.0", 0, 1);
        };
        let peer = Box::new(Peer {
            weak,
            state: Mutex::new(PeerState::default()),
        });
        Ok(handle(Box::into_raw(peer)))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_servantPeerFree(_: JNIEnv, _: JClass, peer: jlong) {
    let p: *mut Peer = ptr(peer);
    if !p.is_null() {
        drop(unsafe { Box::from_raw(p) });
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_servantGet(
    mut env: JNIEnv,
    _: JClass,
    peer: jlong,
    repository_id: JString,
) -> jlong {
    run(&mut env, 0, |env| {
        let id = rust_string(env, &repository_id)?;
        let s =
            unsafe { rorb_servant_get(ptr(peer), id.as_ptr() as *const c_char, id.len(), &VTABLE) };
        Ok(handle(s))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_servantRelease(
    _: JNIEnv,
    _: JClass,
    servant: jlong,
) {
    unsafe { rorb_servant_release(ptr(servant)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_servantCurrentReference(
    _: JNIEnv,
    _: JClass,
    servant: jlong,
) -> jlong {
    let mut obj = std::ptr::null_mut();
    if unsafe { rorb_servant_current_reference(ptr(servant), &mut obj) } != 0 {
        handle(obj)
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_replyOutput(
    _: JNIEnv,
    _: JClass,
    reply: jlong,
) -> jlong {
    handle(unsafe { rorb_server_reply_output(ptr(reply)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_replyUserException(
    mut env: JNIEnv,
    _: JClass,
    reply: jlong,
    id: JString,
) -> jlong {
    run(&mut env, 0, |env| {
        let id = rust_string(env, &id)?;
        Ok(handle(unsafe {
            rorb_server_reply_user_exception(ptr(reply), id.as_ptr() as *const c_char, id.len())
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_replySystemException(
    mut env: JNIEnv,
    _: JClass,
    reply: jlong,
    id: JString,
    minor: jint,
    completed: jint,
) {
    run(&mut env, (), |env| {
        let id = rust_string(env, &id)?;
        unsafe {
            rorb_server_reply_system_exception(
                ptr(reply),
                id.as_ptr() as *const c_char,
                id.len(),
                minor as u32,
                completed as u32,
            )
        };
        Ok(())
    })
}
