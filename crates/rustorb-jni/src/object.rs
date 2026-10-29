//! `org.rustorb.Native.object*`: object references and invocations.

use std::ffi::{c_char, c_void};

use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jint, jlong, jobject, jstring, jvalue};

use crate::sys::*;
use crate::{BOOLEAN, Exc, INT, cache, call_bridge, handle, java_string, ptr, run, rust_string};

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectClone(
    _: JNIEnv,
    _: JClass,
    obj: jlong,
) -> jlong {
    handle(unsafe { rorb_object_clone(ptr(obj)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectRelease(_: JNIEnv, _: JClass, obj: jlong) {
    unsafe { rorb_object_release(ptr(obj)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectOrb(
    _: JNIEnv,
    _: JClass,
    obj: jlong,
) -> jlong {
    handle(unsafe { rorb_object_orb(ptr(obj)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectTypeId(
    mut env: JNIEnv,
    _: JClass,
    obj: jlong,
) -> jstring {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut len = 0;
        let p = unsafe { rorb_object_type_id(ptr(obj), &mut len) };
        Ok(java_string(env, p, len)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectIsA(
    mut env: JNIEnv,
    _: JClass,
    obj: jlong,
    id: JString,
) -> jboolean {
    run(&mut env, 0, |env| {
        let id = rust_string(env, &id)?;
        let mut result = 0;
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_object_is_a(
                ptr(obj),
                id.as_ptr() as *const c_char,
                id.len(),
                &mut result,
                exc.ptr(),
            )
        };
        exc.check(env, st)?;
        Ok((result != 0) as jboolean)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectNonExistent(
    mut env: JNIEnv,
    _: JClass,
    obj: jlong,
) -> jboolean {
    run(&mut env, 0, |env| {
        let mut result = 0;
        let mut exc = Exc::new();
        let st = unsafe { rorb_object_non_existent(ptr(obj), &mut result, exc.ptr()) };
        exc.check(env, st)?;
        Ok((result != 0) as jboolean)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectIsEquivalent(
    _: JNIEnv,
    _: JClass,
    a: jlong,
    b: jlong,
) -> jboolean {
    (unsafe { rorb_object_is_equivalent(ptr(a), ptr(b)) } != 0) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectHash(
    _: JNIEnv,
    _: JClass,
    obj: jlong,
    maximum: jint,
) -> jint {
    let maximum = if maximum <= 0 {
        u32::MAX
    } else {
        maximum as u32
    };
    unsafe { rorb_object_hash(ptr(obj), maximum) as jint }
}

/// The context of the callbacks of one call: the thread's JNIEnv and the
/// org.rustorb.Call object.
pub(crate) struct CallContext {
    pub(crate) env: *mut jni::sys::JNIEnv,
    pub(crate) call: jobject,
}

impl CallContext {
    pub(crate) fn new(env: &JNIEnv, call: &JObject) -> CallContext {
        CallContext {
            env: env.get_raw(),
            call: call.as_raw(),
        }
    }

    pub(crate) fn as_ptr(&mut self) -> *mut c_void {
        self as *mut CallContext as *mut c_void
    }
}

fn status_of(ok: Result<bool, crate::Thrown>) -> rorb_status {
    match ok {
        Ok(true) => RORB_OK,
        _ => RORB_CALLBACK_FAILED,
    }
}

/// `Bridge.callWrite(call, out)`.
pub(crate) unsafe extern "C" fn call_write(
    context: *mut c_void,
    out: *mut rorb_out,
) -> rorb_status {
    let ctx = unsafe { &*(context as *const CallContext) };
    let Ok(mut env) = (unsafe { JNIEnv::from_raw(ctx.env) }) else {
        return RORB_CALLBACK_FAILED;
    };
    status_of(
        call_bridge(
            &mut env,
            cache().call_write,
            BOOLEAN,
            &[jvalue { l: ctx.call }, jvalue { j: out as jlong }],
        )
        .and_then(|v| Ok(v.z()?)),
    )
}

/// `Bridge.callRead(call, in)`.
pub(crate) unsafe extern "C" fn call_read(context: *mut c_void, r#in: *mut rorb_in) -> rorb_status {
    let ctx = unsafe { &*(context as *const CallContext) };
    let Ok(mut env) = (unsafe { JNIEnv::from_raw(ctx.env) }) else {
        return RORB_CALLBACK_FAILED;
    };
    status_of(
        call_bridge(
            &mut env,
            cache().call_read,
            BOOLEAN,
            &[jvalue { l: ctx.call }, jvalue { j: r#in as jlong }],
        )
        .and_then(|v| Ok(v.z()?)),
    )
}

/// `Bridge.callReadException(call, id, in)`: RORB_USER_EXCEPTION,
/// RORB_SYSTEM_EXCEPTION (not declared) or RORB_CALLBACK_FAILED.
unsafe extern "C" fn call_read_exception(
    context: *mut c_void,
    id: *const c_char,
    id_len: usize,
    r#in: *mut rorb_in,
) -> rorb_status {
    let ctx = unsafe { &*(context as *const CallContext) };
    let Ok(mut env) = (unsafe { JNIEnv::from_raw(ctx.env) }) else {
        return RORB_CALLBACK_FAILED;
    };
    let result = (|| {
        let id = java_string(&mut env, id, id_len)?;
        let v = call_bridge(
            &mut env,
            cache().call_read_exception,
            INT,
            &[
                jvalue { l: ctx.call },
                jvalue { l: id.as_raw() },
                jvalue { j: r#in as jlong },
            ],
        )?;
        Ok::<_, crate::Thrown>(v.i()?)
    })();
    result.unwrap_or(RORB_CALLBACK_FAILED)
}

/// `objectInvoke(obj, op, oneway, timeoutUs, call)`: RORB_OK,
/// RORB_USER_EXCEPTION (the call took it) or RORB_CALLBACK_FAILED (the
/// call keeps the Java exception); system exceptions are thrown.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectInvoke(
    mut env: JNIEnv,
    _: JClass,
    obj: jlong,
    op: JString,
    oneway: jboolean,
    timeout_us: jlong,
    call: JObject,
) -> jint {
    run(&mut env, RORB_CALLBACK_FAILED, |env| {
        let op = rust_string(env, &op)?;
        let mut ctx = CallContext::new(env, &call);
        let inv = rorb_invocation {
            operation: op.as_ptr() as *const c_char,
            operation_len: op.len(),
            oneway: oneway as i32,
            timeout_us,
            context: ctx.as_ptr(),
            write_args: Some(call_write),
            read_reply: Some(call_read),
            read_user_exception: Some(call_read_exception),
        };
        let mut exc = Exc::new();
        let st = unsafe { rorb_object_invoke(ptr(obj), &inv, exc.ptr()) };
        match st {
            RORB_USER_EXCEPTION | RORB_CALLBACK_FAILED => Ok(st),
            _ => {
                exc.check(env, st)?;
                Ok(st)
            }
        }
    })
}
