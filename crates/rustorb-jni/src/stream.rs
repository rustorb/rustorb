//! `org.rustorb.Native.out*` / `in*`: the CDR streams of callbacks.
//!
//! The stream pointers are only valid during the callback that got them;
//! org.rustorb.NativeOutputStream / NativeInputStream forget them when the
//! callback returns. Errors are sticky, as in the C ABI, and reported
//! when the callback returns.

use std::ffi::{c_char, c_void};

use jni::JNIEnv;
use jni::objects::{
    JByteArray, JClass, JDoubleArray, JFloatArray, JIntArray, JLongArray, JShortArray, JString,
};
use jni::sys::{jboolean, jbyte, jchar, jdouble, jfloat, jint, jlong, jshort, jsize, jstring};

use crate::sys::*;
use crate::{Thrown, handle, ptr, run};

macro_rules! out_fns {
    ($($java:ident($jty:ty) => $c:ident as $cty:ty;)*) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "system" fn $java(_: JNIEnv, _: JClass, out: jlong, v: $jty) {
                unsafe { $c(ptr(out), v as $cty) };
            }
        )*
    };
}

out_fns! {
    Java_org_rustorb_Native_outBoolean(jboolean) => rorb_out_write_boolean as i32;
    Java_org_rustorb_Native_outOctet(jbyte) => rorb_out_write_octet as u8;
    Java_org_rustorb_Native_outWchar(jchar) => rorb_out_write_wchar as u16;
    Java_org_rustorb_Native_outShort(jshort) => rorb_out_write_short as i16;
    Java_org_rustorb_Native_outUshort(jshort) => rorb_out_write_ushort as u16;
    Java_org_rustorb_Native_outLong(jint) => rorb_out_write_long as i32;
    Java_org_rustorb_Native_outUlong(jint) => rorb_out_write_ulong as u32;
    Java_org_rustorb_Native_outLonglong(jlong) => rorb_out_write_longlong as i64;
    Java_org_rustorb_Native_outUlonglong(jlong) => rorb_out_write_ulonglong as u64;
    Java_org_rustorb_Native_outFloat(jfloat) => rorb_out_write_float as f32;
    Java_org_rustorb_Native_outDouble(jdouble) => rorb_out_write_double as f64;
}

/// A double, as a long double.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_outLongdouble(
    _: JNIEnv,
    _: JClass,
    out: jlong,
    v: jdouble,
) {
    let (mut hi, mut lo) = (0, 0);
    unsafe {
        rorb_longdouble_from_double(v, &mut hi, &mut lo);
        rorb_out_write_longdouble(ptr(out), hi, lo);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_outChar(_: JNIEnv, _: JClass, out: jlong, v: jchar) {
    unsafe { rorb_out_write_char(ptr(out), v as u8 as c_char) };
}

/// The UTF-16 units of a Java string.
pub(crate) fn utf16(env: &JNIEnv, s: &JString) -> Vec<u16> {
    let raw = env.get_raw();
    unsafe {
        let len = ((**raw).GetStringLength.unwrap())(raw, s.as_raw());
        let mut units = vec![0u16; len as usize];
        ((**raw).GetStringRegion.unwrap())(raw, s.as_raw(), 0, len, units.as_mut_ptr());
        units
    }
}

/// A Java string of UTF-16 units.
pub(crate) fn from_utf16<'local>(env: &JNIEnv<'local>, units: &[u16]) -> JResult16<'local> {
    let raw = env.get_raw();
    let s = unsafe { ((**raw).NewString.unwrap())(raw, units.as_ptr(), units.len() as jsize) };
    if s.is_null() {
        Err(Thrown)
    } else {
        Ok(unsafe { JString::from_raw(s) })
    }
}

type JResult16<'local> = Result<JString<'local>, Thrown>;

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_outString(
    mut env: JNIEnv,
    _: JClass,
    out: jlong,
    s: JString,
) {
    run(&mut env, (), |env| {
        let s = crate::rust_string(env, &s)?;
        unsafe { rorb_out_write_string(ptr(out), s.as_ptr() as *const c_char, s.len()) };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_outWstring(
    mut env: JNIEnv,
    _: JClass,
    out: jlong,
    s: JString,
) {
    run(&mut env, (), |env| {
        if s.is_null() {
            return crate::throw_system(env, "IDL:omg.org/CORBA/BAD_PARAM:1.0", 0, 1);
        }
        let units = utf16(env, &s);
        unsafe { rorb_out_write_wstring(ptr(out), units.as_ptr(), units.len()) };
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_outObject(
    _: JNIEnv,
    _: JClass,
    out: jlong,
    obj: jlong,
) {
    unsafe { rorb_out_write_object(ptr(out), ptr(obj)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_outTypeCode(
    _: JNIEnv,
    _: JClass,
    out: jlong,
    tc: jlong,
) {
    unsafe { rorb_out_write_typecode(ptr(out), ptr(tc)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_outAny(
    _: JNIEnv,
    _: JClass,
    out: jlong,
    any: jlong,
) {
    unsafe { rorb_out_write_any(ptr(out), ptr(any)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_outFailed(
    _: JNIEnv,
    _: JClass,
    out: jlong,
) -> jboolean {
    (unsafe { rorb_out_failed(ptr(out)) } != 0) as jboolean
}

macro_rules! out_array_fns {
    ($($java:ident($arr:ident, $elem:ty, $get:ident);)*) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "system" fn $java(
                mut env: JNIEnv,
                _: JClass,
                out: jlong,
                kind: jint,
                v: $arr,
                off: jint,
                len: jint,
            ) {
                run(&mut env, (), |env| {
                    let mut buf: Vec<$elem> = vec![Default::default(); len.max(0) as usize];
                    env.$get(&v, off, &mut buf)?;
                    unsafe {
                        rorb_out_write_array(ptr(out), kind, buf.as_ptr() as *const c_void, buf.len())
                    };
                    Ok(())
                })
            }
        )*
    };
}

out_array_fns! {
    Java_org_rustorb_Native_outArrayByte(JByteArray, jbyte, get_byte_array_region);
    Java_org_rustorb_Native_outArrayShort(JShortArray, jshort, get_short_array_region);
    Java_org_rustorb_Native_outArrayInt(JIntArray, jint, get_int_array_region);
    Java_org_rustorb_Native_outArrayLong(JLongArray, jlong, get_long_array_region);
    Java_org_rustorb_Native_outArrayFloat(JFloatArray, jfloat, get_float_array_region);
    Java_org_rustorb_Native_outArrayDouble(JDoubleArray, jdouble, get_double_array_region);
}

// --- input ---------------------------------------------------------------------

macro_rules! in_fns {
    ($($java:ident -> $jty:ty = $c:ident;)*) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "system" fn $java(_: JNIEnv, _: JClass, r#in: jlong) -> $jty {
                unsafe { $c(ptr(r#in)) as $jty }
            }
        )*
    };
}

in_fns! {
    Java_org_rustorb_Native_inOctet -> jbyte = rorb_in_read_octet;
    Java_org_rustorb_Native_inWchar -> jchar = rorb_in_read_wchar;
    Java_org_rustorb_Native_inShort -> jshort = rorb_in_read_short;
    Java_org_rustorb_Native_inUshort -> jshort = rorb_in_read_ushort;
    Java_org_rustorb_Native_inLong -> jint = rorb_in_read_long;
    Java_org_rustorb_Native_inUlong -> jint = rorb_in_read_ulong;
    Java_org_rustorb_Native_inLonglong -> jlong = rorb_in_read_longlong;
    Java_org_rustorb_Native_inUlonglong -> jlong = rorb_in_read_ulonglong;
    Java_org_rustorb_Native_inFloat -> jfloat = rorb_in_read_float;
    Java_org_rustorb_Native_inDouble -> jdouble = rorb_in_read_double;
}

/// A long double, as the nearest double.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inLongdouble(
    _: JNIEnv,
    _: JClass,
    r#in: jlong,
) -> jdouble {
    let (mut hi, mut lo) = (0, 0);
    unsafe {
        rorb_in_read_longdouble(ptr(r#in), &mut hi, &mut lo);
        rorb_longdouble_to_double(hi, lo)
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inChar(_: JNIEnv, _: JClass, r#in: jlong) -> jchar {
    unsafe { rorb_in_read_char(ptr(r#in)) as u8 as jchar }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inBoolean(
    _: JNIEnv,
    _: JClass,
    r#in: jlong,
) -> jboolean {
    (unsafe { rorb_in_read_boolean(ptr(r#in)) } != 0) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inString(
    mut env: JNIEnv,
    _: JClass,
    r#in: jlong,
) -> jstring {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut len = 0;
        let p = unsafe { rorb_in_read_string(ptr(r#in), &mut len) };
        Ok(crate::java_string(env, p, len)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inWstring(
    mut env: JNIEnv,
    _: JClass,
    r#in: jlong,
) -> jstring {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut len = 0;
        let p = unsafe { rorb_in_read_wstring(ptr(r#in), &mut len) };
        let units = if p.is_null() || len == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(p, len) }
        };
        Ok(from_utf16(env, units)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inObject(
    _: JNIEnv,
    _: JClass,
    r#in: jlong,
) -> jlong {
    handle(unsafe { rorb_in_read_object(ptr(r#in)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inTypeCode(
    _: JNIEnv,
    _: JClass,
    r#in: jlong,
) -> jlong {
    handle(unsafe { rorb_in_read_typecode(ptr(r#in)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inAny(_: JNIEnv, _: JClass, r#in: jlong) -> jlong {
    handle(unsafe { rorb_in_read_any(ptr(r#in)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inLength(
    _: JNIEnv,
    _: JClass,
    r#in: jlong,
    min_element_size: jint,
) -> jint {
    let n = unsafe { rorb_in_read_length(ptr(r#in), min_element_size.max(0) as usize) };
    // Java arrays hold at most i32::MAX elements.
    if n > i32::MAX as u32 {
        unsafe { rorb_in_fail(ptr(r#in)) };
        return 0;
    }
    n as jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inFail(_: JNIEnv, _: JClass, r#in: jlong) {
    unsafe { rorb_in_fail(ptr(r#in)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_inFailed(
    _: JNIEnv,
    _: JClass,
    r#in: jlong,
) -> jboolean {
    (unsafe { rorb_in_failed(ptr(r#in)) } != 0) as jboolean
}

macro_rules! in_array_fns {
    ($($java:ident($arr:ident, $elem:ty, $set:ident);)*) => {
        $(
            #[unsafe(no_mangle)]
            pub extern "system" fn $java(
                mut env: JNIEnv,
                _: JClass,
                r#in: jlong,
                kind: jint,
                v: $arr,
                off: jint,
                len: jint,
            ) {
                run(&mut env, (), |env| {
                    let mut buf: Vec<$elem> = vec![Default::default(); len.max(0) as usize];
                    unsafe {
                        rorb_in_read_array(ptr(r#in), kind, buf.as_mut_ptr() as *mut c_void, buf.len())
                    };
                    env.$set(&v, off, &buf)?;
                    Ok(())
                })
            }
        )*
    };
}

in_array_fns! {
    Java_org_rustorb_Native_inArrayByte(JByteArray, jbyte, set_byte_array_region);
    Java_org_rustorb_Native_inArrayShort(JShortArray, jshort, set_short_array_region);
    Java_org_rustorb_Native_inArrayInt(JIntArray, jint, set_int_array_region);
    Java_org_rustorb_Native_inArrayLong(JLongArray, jlong, set_long_array_region);
    Java_org_rustorb_Native_inArrayFloat(JFloatArray, jfloat, set_float_array_region);
    Java_org_rustorb_Native_inArrayDouble(JDoubleArray, jdouble, set_double_array_region);
}
