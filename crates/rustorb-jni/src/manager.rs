//! `org.rustorb.Native.poa*` for servant managers and adapter activators,
//! and `managerError*`.
//!
//! The `self` of a Java servant manager or adapter activator is a
//! [`Held`]: a global reference to the Java object with the count of
//! references the C side holds, freed when that count drops to zero. The
//! callbacks call `org.rustorb.Bridge`, which runs the Java method and
//! reports what it raised with `managerError*`.

use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicUsize, Ordering};

use jni::JNIEnv;
use jni::objects::{GlobalRef, JClass, JObject, JString};
use jni::sys::{jboolean, jint, jlong, jobject, jvalue};

use crate::servant::{borrowed_servant, thread_env};
use crate::sys::*;
use crate::{BOOLEAN, Exc, JResult, LONG, Thrown, VOID, cache, call_bridge, java_string, ptr, run};

struct Held {
    obj: GlobalRef,
    refs: AtomicUsize,
}

impl Held {
    /// A new holder of obj, with one reference for the caller.
    fn make(env: &mut JNIEnv, obj: &JObject) -> JResult<*mut c_void> {
        let held = Box::new(Held {
            obj: env.new_global_ref(obj)?,
            refs: AtomicUsize::new(1),
        });
        Ok(Box::into_raw(held) as *mut c_void)
    }

    fn get<'a>(this: *mut c_void) -> &'a Held {
        unsafe { &*(this as *const Held) }
    }
}

unsafe extern "C" fn add_ref(this: *mut c_void) {
    Held::get(this).refs.fetch_add(1, Ordering::Relaxed);
}

unsafe extern "C" fn remove_ref(this: *mut c_void) {
    if Held::get(this).refs.fetch_sub(1, Ordering::AcqRel) == 1 {
        // Dropping the global reference attaches the thread if needed.
        drop(unsafe { Box::from_raw(this as *mut Held) });
    }
}

/// The Java object of a holder the C side returned with a reference added
/// for the caller, as a local reference; gives that reference back.
fn take<'local>(env: &mut JNIEnv<'local>, this: *mut c_void) -> JResult<JObject<'local>> {
    if this.is_null() {
        return Ok(JObject::null());
    }
    let obj = env.new_local_ref(Held::get(this).obj.as_obj());
    unsafe { remove_ref(this) };
    Ok(obj?)
}

/// Runs a callback body with the JNIEnv of the calling thread, clearing any
/// Java exception it leaves (Bridge catches them all).
fn upcall<T>(default: T, f: impl FnOnce(&mut JNIEnv) -> JResult<T>) -> T {
    let Some(mut env) = thread_env() else {
        return default;
    };
    match env.with_local_frame(16, |env| f(env)) {
        Ok(v) => v,
        Err(Thrown) => {
            let _ = env.exception_clear();
            default
        }
    }
}

fn object_id<'local>(
    env: &mut JNIEnv<'local>,
    id: *const u8,
    len: usize,
) -> JResult<JObject<'local>> {
    let slice = if id.is_null() || len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(id, len) }
    };
    Ok(env.byte_array_from_slice(slice)?.into())
}

fn unknown(error: *mut rorb_manager_error) -> rorb_status {
    let id = "IDL:omg.org/CORBA/UNKNOWN:1.0";
    unsafe { rorb_manager_error_system(error, id.as_ptr() as *const c_char, id.len(), 0, 2) };
    RORB_SYSTEM_EXCEPTION
}

unsafe extern "C" fn incarnate(
    this: *mut c_void,
    id: *const u8,
    id_len: usize,
    adapter: *mut rorb_poa,
    servant: *mut *mut rorb_servant,
    error: *mut rorb_manager_error,
) -> rorb_status {
    let h = upcall(None, |env| {
        let oid = object_id(env, id, id_len)?;
        let v = call_bridge(
            env,
            cache().incarnate,
            LONG,
            &[
                jvalue {
                    l: Held::get(this).obj.as_obj().as_raw(),
                },
                jvalue { l: oid.as_raw() },
                jvalue {
                    j: adapter as jlong,
                },
                jvalue { j: error as jlong },
            ],
        )?;
        Ok(Some(v.j()?))
    });
    match h {
        Some(0) => RORB_SYSTEM_EXCEPTION,
        Some(h) => {
            unsafe { *servant = ptr(h) };
            RORB_OK
        }
        None => unknown(error),
    }
}

unsafe extern "C" fn etherealize(
    this: *mut c_void,
    id: *const u8,
    id_len: usize,
    adapter: *mut rorb_poa,
    servant: *mut c_void,
    cleanup_in_progress: i32,
    remaining_activations: i32,
) {
    upcall((), |env| {
        let oid = object_id(env, id, id_len)?;
        let servant = borrowed_servant(env, servant)?;
        call_bridge(
            env,
            cache().etherealize,
            VOID,
            &[
                jvalue {
                    l: Held::get(this).obj.as_obj().as_raw(),
                },
                jvalue { l: oid.as_raw() },
                jvalue {
                    j: adapter as jlong,
                },
                jvalue {
                    l: servant.as_raw(),
                },
                jvalue {
                    z: (cleanup_in_progress != 0) as jboolean,
                },
                jvalue {
                    z: (remaining_activations != 0) as jboolean,
                },
            ],
        )?;
        Ok(())
    })
}

static ACTIVATOR: rorb_servant_activator_vtable = rorb_servant_activator_vtable {
    incarnate,
    etherealize: Some(etherealize),
    add_ref,
    remove_ref,
};

unsafe extern "C" fn preinvoke(
    this: *mut c_void,
    id: *const u8,
    id_len: usize,
    adapter: *mut rorb_poa,
    op: *const c_char,
    op_len: usize,
    servant: *mut *mut rorb_servant,
    cookie: *mut *mut c_void,
    error: *mut rorb_manager_error,
) -> rorb_status {
    let result = upcall(None, |env| {
        let oid = object_id(env, id, id_len)?;
        let op = java_string(env, op, op_len)?;
        let cookie_out = env.new_object_array(1, "java/lang/Object", JObject::null())?;
        let v = call_bridge(
            env,
            cache().preinvoke,
            LONG,
            &[
                jvalue {
                    l: Held::get(this).obj.as_obj().as_raw(),
                },
                jvalue { l: oid.as_raw() },
                jvalue {
                    j: adapter as jlong,
                },
                jvalue { l: op.as_raw() },
                jvalue {
                    l: cookie_out.as_raw(),
                },
                jvalue { j: error as jlong },
            ],
        )?
        .j()?;
        if v == 0 {
            return Ok(Some((0, None)));
        }
        let c = env.get_object_array_element(&cookie_out, 0)?;
        let c = if c.is_null() {
            None
        } else {
            Some(env.new_global_ref(c)?)
        };
        Ok(Some((v, c)))
    });
    match result {
        Some((0, _)) => RORB_SYSTEM_EXCEPTION,
        Some((h, c)) => {
            unsafe {
                *servant = ptr(h);
                *cookie = c.map_or(std::ptr::null_mut(), |c| {
                    Box::into_raw(Box::new(c)) as *mut c_void
                });
            }
            RORB_OK
        }
        None => unknown(error),
    }
}

unsafe extern "C" fn postinvoke(
    this: *mut c_void,
    id: *const u8,
    id_len: usize,
    adapter: *mut rorb_poa,
    op: *const c_char,
    op_len: usize,
    cookie: *mut c_void,
    servant: *mut c_void,
) {
    let cookie = (!cookie.is_null()).then(|| *unsafe { Box::from_raw(cookie as *mut GlobalRef) });
    upcall((), |env| {
        let oid = object_id(env, id, id_len)?;
        let op = java_string(env, op, op_len)?;
        let servant = borrowed_servant(env, servant)?;
        let cookie = match &cookie {
            Some(c) => c.as_obj().as_raw(),
            None => std::ptr::null_mut(),
        };
        call_bridge(
            env,
            cache().postinvoke,
            VOID,
            &[
                jvalue {
                    l: Held::get(this).obj.as_obj().as_raw(),
                },
                jvalue { l: oid.as_raw() },
                jvalue {
                    j: adapter as jlong,
                },
                jvalue { l: op.as_raw() },
                jvalue { l: cookie },
                jvalue {
                    l: servant.as_raw(),
                },
            ],
        )?;
        Ok(())
    });
    drop(cookie);
}

static LOCATOR: rorb_servant_locator_vtable = rorb_servant_locator_vtable {
    preinvoke,
    postinvoke: Some(postinvoke),
    add_ref,
    remove_ref,
};

unsafe extern "C" fn unknown_adapter(
    this: *mut c_void,
    parent: *mut rorb_poa,
    name: *const c_char,
    len: usize,
) -> i32 {
    upcall(0, |env| {
        let name = java_string(env, name, len)?;
        let v = call_bridge(
            env,
            cache().unknown_adapter,
            BOOLEAN,
            &[
                jvalue {
                    l: Held::get(this).obj.as_obj().as_raw(),
                },
                jvalue { j: parent as jlong },
                jvalue { l: name.as_raw() },
            ],
        )?;
        Ok(v.z()? as i32)
    })
}

static ADAPTER_ACTIVATOR: rorb_adapter_activator_vtable = rorb_adapter_activator_vtable {
    unknown_adapter,
    add_ref,
    remove_ref,
};

/// Hands a new holder of obj to set, which adds its own reference.
fn hand_over(
    env: &mut JNIEnv,
    obj: &JObject,
    set: impl FnOnce(*mut c_void, *mut rorb_exception) -> rorb_status,
) -> JResult<()> {
    let held = Held::make(env, obj)?;
    let mut exc = Exc::new();
    let st = set(held, exc.ptr());
    unsafe { remove_ref(held) };
    exc.check(env, st)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaSetServantManager(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    manager: JObject,
    locator: jboolean,
) {
    run(&mut env, (), |env| {
        hand_over(env, &manager, |held, exc| unsafe {
            if locator != 0 {
                rorb_poa_set_servant_locator(ptr(poa), held, &LOCATOR, exc)
            } else {
                rorb_poa_set_servant_activator(ptr(poa), held, &ACTIVATOR, exc)
            }
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaGetServantManager(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
) -> jobject {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut kind = 0;
        let mut this = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe { rorb_poa_get_servant_manager(ptr(poa), &mut kind, &mut this, exc.ptr()) };
        exc.check(env, st)?;
        Ok(take(env, this)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaSetTheActivator(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    activator: JObject,
) {
    run(&mut env, (), |env| {
        if activator.is_null() {
            let mut exc = Exc::new();
            let st = unsafe {
                rorb_poa_set_the_activator(
                    ptr(poa),
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    exc.ptr(),
                )
            };
            return exc.check(env, st);
        }
        hand_over(env, &activator, |held, exc| unsafe {
            rorb_poa_set_the_activator(ptr(poa), held, &ADAPTER_ACTIVATOR, exc)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaTheActivator(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
) -> jobject {
    run(&mut env, std::ptr::null_mut(), |env| {
        let this = unsafe { rorb_poa_the_activator(ptr(poa)) };
        Ok(take(env, this)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_managerErrorForward(
    _: JNIEnv,
    _: JClass,
    error: jlong,
    obj: jlong,
) {
    unsafe { rorb_manager_error_forward(ptr(error), ptr(obj)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_managerErrorSystem(
    mut env: JNIEnv,
    _: JClass,
    error: jlong,
    id: JString,
    minor: jint,
    completed: jint,
) {
    run(&mut env, (), |env| {
        let id = crate::rust_string(env, &id)?;
        unsafe {
            rorb_manager_error_system(
                ptr(error),
                id.as_ptr() as *const c_char,
                id.len(),
                minor as u32,
                completed as u32,
            )
        };
        Ok(())
    })
}
