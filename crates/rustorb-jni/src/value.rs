//! `org.rustorb.Native.*Value*`: valuetypes on the CDR streams of
//! callbacks. The Java side keeps the instances it reads by slot, for
//! indirections; the keys it writes with name its instances.

use std::ffi::{c_char, c_void};

use jni::JNIEnv;
use jni::objects::{JClass, JLongArray, JObjectArray, JString};
use jni::sys::{jboolean, jint, jlong, jstring, jvalue};

use crate::servant::thread_env;
use crate::sys::*;
use crate::{LONG, cache, call_bridge, java_string, ptr, run, rust_string};

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_outNullValue(_: JNIEnv, _: JClass, out: jlong) {
    unsafe { rorb_out_write_null_value(ptr(out)) };
}

/// Whether the caller writes the state (else an indirection was written).
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_outBeginValue(
    mut env: JNIEnv,
    _: JClass,
    out: jlong,
    key: jlong,
    ids: JObjectArray,
    chunked: jboolean,
) -> jboolean {
    run(&mut env, 0, |env| {
        let n = env.get_array_length(&ids)?;
        let mut list = Vec::with_capacity(n as usize);
        for i in 0..n {
            let s = JString::from(env.get_object_array_element(&ids, i)?);
            list.push(rust_string(env, &s)?);
        }
        let ptrs: Vec<*const c_char> = list.iter().map(|s| s.as_ptr() as *const c_char).collect();
        let lens: Vec<usize> = list.iter().map(|s| s.len()).collect();
        let r = unsafe {
            rorb_out_begin_value(
                ptr(out),
                key as u64,
                list.len(),
                ptrs.as_ptr(),
                lens.as_ptr(),
                i32::from(chunked != 0),
            )
        };
        Ok(jboolean::from(r != 0))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_outEndValue(_: JNIEnv, _: JClass, out: jlong) {
    unsafe { rorb_out_end_value(ptr(out)) };
}

/// `RORB_VALUE_NULL`, `RORB_VALUE_INDIRECTION` or `RORB_VALUE_NEW`, with
/// the slot and the number of repository ids in `slot_count[0..2]`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inValueHeader(
    mut env: JNIEnv,
    _: JClass,
    input: jlong,
    slot_count: JLongArray,
) -> jint {
    run(&mut env, 0, |env| {
        let mut slot = 0usize;
        let mut count = 0usize;
        let kind = unsafe { rorb_in_read_value_header(ptr(input), &mut slot, &mut count) };
        env.set_long_array_region(&slot_count, 0, &[slot as jlong, count as jlong])?;
        Ok(kind)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inValueId(
    mut env: JNIEnv,
    _: JClass,
    input: jlong,
    index: jint,
) -> jstring {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut len = 0usize;
        let p = unsafe { rorb_in_value_id(ptr(input), index as usize, &mut len) };
        Ok(java_string(env, p, len)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inEndValue(_: JNIEnv, _: JClass, input: jlong) {
    unsafe { rorb_in_end_value(ptr(input)) };
}

/// Makes the core ask `Bridge.resolveValueType` for the valuetypes it
/// does not know (those of Helpers not loaded yet).
pub(crate) fn install_resolver() {
    unsafe { rorb_set_value_type_resolver(Some(resolve), std::ptr::null_mut()) };
}

unsafe extern "C" fn resolve(_: *mut c_void, id: *const c_char, len: usize) -> *mut rorb_typecode {
    let Some(mut env) = thread_env() else {
        return std::ptr::null_mut();
    };
    let found = env.with_local_frame(8, |env| {
        let id = java_string(env, id, len)?;
        let v = call_bridge(
            env,
            cache().resolve_value_type,
            LONG,
            &[jvalue { l: id.as_raw() }],
        )?;
        Ok::<_, crate::Thrown>(v.j()?)
    });
    match found {
        Ok(tc) => ptr(tc),
        Err(_) => {
            let _ = env.exception_clear();
            std::ptr::null_mut()
        }
    }
}
