//! `org.rustorb.Native` for the owned input streams of
//! org.rustorb.OwnedInputStream (the replies of portable stubs and the
//! streams of create_input_stream), and the values of anys on streams.

use std::ffi::c_char;

use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jlong, jstring};

use crate::object::{CallContext, call_write};
use crate::sys::*;
use crate::{Exc, handle, java_string, ptr, run, rust_string};

/// `objectInvokeStream(obj, op, oneway, timeoutUs, call)`: the reply
/// stream (0 for a oneway call, or when call.write failed and keeps the
/// Java exception); a user exception is a stream with an exception id.
/// System exceptions are thrown.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectInvokeStream(
    mut env: JNIEnv,
    _: JClass,
    obj: jlong,
    op: JString,
    oneway: jboolean,
    timeout_us: jlong,
    call: JObject,
) -> jlong {
    run(&mut env, 0, |env| {
        let op = rust_string(env, &op)?;
        let mut ctx = CallContext::new(env, &call);
        let inv = rorb_invocation {
            operation: op.as_ptr() as *const c_char,
            operation_len: op.len(),
            oneway: oneway as i32,
            timeout_us,
            context: ctx.as_ptr(),
            write_args: Some(call_write),
            read_reply: None,
            read_user_exception: None,
        };
        let mut reply = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe { rorb_object_invoke_stream(ptr(obj), &inv, &mut reply, exc.ptr()) };
        match st {
            RORB_USER_EXCEPTION => Ok(handle(reply)),
            RORB_CALLBACK_FAILED => Ok(0),
            _ => {
                exc.check(env, st)?;
                Ok(handle(reply))
            }
        }
    })
}

/// `inExceptionId(in)`: the repository id of the user exception of a
/// reply stream, or null.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inExceptionId(
    mut env: JNIEnv,
    _: JClass,
    r#in: jlong,
) -> jstring {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut len = 0;
        let p = unsafe { rorb_in_exception_id(ptr(r#in), &mut len) };
        if p.is_null() {
            return Ok(std::ptr::null_mut());
        }
        Ok(java_string(env, p, len)?.into_raw())
    })
}

/// `inNew(orb, call)`: a stream over what call.write writes (0 when it
/// failed and keeps the Java exception).
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inNew(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
    call: JObject,
) -> jlong {
    run(&mut env, 0, |env| {
        let mut ctx = CallContext::new(env, &call);
        let mut input = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_in_new(
                Some(call_write),
                ctx.as_ptr(),
                ptr(orb),
                &mut input,
                exc.ptr(),
            )
        };
        if st == RORB_CALLBACK_FAILED {
            return Ok(0);
        }
        exc.check(env, st)?;
        Ok(handle(input))
    })
}

/// `anyInput(any)`: a stream over the value of the any.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_anyInput(
    mut env: JNIEnv,
    _: JClass,
    any: jlong,
) -> jlong {
    run(&mut env, 0, |env| {
        let mut input = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe { rorb_any_input(ptr::<rorb_any>(any), &mut input, exc.ptr()) };
        exc.check(env, st)?;
        Ok(handle(input))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inRelease(_: JNIEnv, _: JClass, r#in: jlong) {
    unsafe { rorb_in_release(ptr(r#in)) };
}

/// `inCheck(in)`: throws the error of the stream, if it has one.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inCheck(mut env: JNIEnv, _: JClass, r#in: jlong) {
    run(&mut env, (), |env| {
        let mut exc = Exc::new();
        let st = unsafe { rorb_in_error(ptr(r#in), exc.ptr()) };
        exc.check(env, st)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_outAnyValue(
    _: JNIEnv,
    _: JClass,
    out: jlong,
    any: jlong,
) {
    unsafe { rorb_out_write_any_value(ptr(out), ptr::<rorb_any>(any)) };
}

/// `inAnyValue(in, tc)`: an any of a value of type tc read from the
/// stream; 0 after an error (reported by the stream).
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inAnyValue(
    _: JNIEnv,
    _: JClass,
    r#in: jlong,
    tc: jlong,
) -> jlong {
    handle(unsafe { rorb_in_read_any_value(ptr(r#in), ptr::<rorb_typecode>(tc)) })
}
