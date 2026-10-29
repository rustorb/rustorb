//! `org.rustorb.Native.tc*` / `any*`: TypeCodes and anys.

use std::ffi::c_char;

use jni::JNIEnv;
use jni::objects::{JClass, JLongArray, JObject, JObjectArray, JString};
use jni::sys::{jboolean, jint, jlong, jshort, jstring};

use crate::object::{CallContext, call_read, call_write};
use crate::sys::*;
use crate::{Exc, JResult, handle, java_string, ptr, run, rust_string};

type Tc = *mut rorb_typecode;

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcClone(_: JNIEnv, _: JClass, tc: jlong) -> jlong {
    handle(unsafe { rorb_tc_clone(ptr::<rorb_typecode>(tc)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcRelease(_: JNIEnv, _: JClass, tc: jlong) {
    unsafe { rorb_tc_release(ptr(tc)) };
}

/// Runs a constructor that fills `*out`, returning the handle.
fn make(
    env: &mut JNIEnv,
    f: impl FnOnce(*mut Tc, *mut rorb_exception) -> rorb_status,
) -> JResult<jlong> {
    let mut tc: Tc = std::ptr::null_mut();
    let mut exc = Exc::new();
    let st = f(&mut tc, exc.ptr());
    exc.check(env, st)?;
    Ok(handle(tc))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcSimple(
    mut env: JNIEnv,
    _: JClass,
    kind: jint,
) -> jlong {
    run(&mut env, 0, |env| {
        make(env, |o, e| unsafe { rorb_tc_simple(kind as u32, o, e) })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcString(
    mut env: JNIEnv,
    _: JClass,
    wide: jboolean,
    bound: jint,
) -> jlong {
    run(&mut env, 0, |env| {
        make(env, |o, e| unsafe {
            rorb_tc_string(wide as i32, bound as u32, o, e)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcFixed(
    mut env: JNIEnv,
    _: JClass,
    digits: jshort,
    scale: jshort,
) -> jlong {
    run(&mut env, 0, |env| {
        make(env, |o, e| unsafe {
            rorb_tc_fixed(digits as u16, scale, o, e)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcSequence(
    mut env: JNIEnv,
    _: JClass,
    content: jlong,
    bound: jint,
) -> jlong {
    run(&mut env, 0, |env| {
        make(env, |o, e| unsafe {
            rorb_tc_sequence(ptr(content), bound as u32, o, e)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcArray(
    mut env: JNIEnv,
    _: JClass,
    content: jlong,
    length: jint,
) -> jlong {
    run(&mut env, 0, |env| {
        make(env, |o, e| unsafe {
            rorb_tc_array(ptr(content), length as u32, o, e)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcAlias(
    mut env: JNIEnv,
    _: JClass,
    kind: jint,
    id: JString,
    name: JString,
    content: jlong,
) -> jlong {
    run(&mut env, 0, |env| {
        let id = rust_string(env, &id)?;
        let name = rust_string(env, &name)?;
        make(env, |o, e| unsafe {
            rorb_tc_alias(
                kind as u32,
                id.as_ptr() as *const c_char,
                id.len(),
                name.as_ptr() as *const c_char,
                name.len(),
                ptr(content),
                o,
                e,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcInterface(
    mut env: JNIEnv,
    _: JClass,
    kind: jint,
    id: JString,
    name: JString,
) -> jlong {
    run(&mut env, 0, |env| {
        let id = rust_string(env, &id)?;
        let name = rust_string(env, &name)?;
        make(env, |o, e| unsafe {
            rorb_tc_interface(
                kind as u32,
                id.as_ptr() as *const c_char,
                id.len(),
                name.as_ptr() as *const c_char,
                name.len(),
                o,
                e,
            )
        })
    })
}

fn strings(env: &mut JNIEnv, a: &JObjectArray) -> JResult<Vec<String>> {
    let n = env.get_array_length(a)?;
    let mut v = Vec::with_capacity(n as usize);
    for i in 0..n {
        let s = JString::from(env.get_object_array_element(a, i)?);
        v.push(rust_string(env, &s)?);
    }
    Ok(v)
}

fn longs(env: &mut JNIEnv, a: &JLongArray) -> JResult<Vec<jlong>> {
    let n = env.get_array_length(a)? as usize;
    let mut v = vec![0; n];
    env.get_long_array_region(a, 0, &mut v)?;
    Ok(v)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcStruct(
    mut env: JNIEnv,
    _: JClass,
    kind: jint,
    id: JString,
    name: JString,
    names: JObjectArray,
    types: JLongArray,
) -> jlong {
    run(&mut env, 0, |env| {
        let id = rust_string(env, &id)?;
        let name = rust_string(env, &name)?;
        let names = strings(env, &names)?;
        let types = longs(env, &types)?;
        let name_ptrs: Vec<*const c_char> =
            names.iter().map(|s| s.as_ptr() as *const c_char).collect();
        let name_lens: Vec<usize> = names.iter().map(|s| s.len()).collect();
        let type_ptrs: Vec<*const rorb_typecode> =
            types.iter().map(|t| ptr(*t) as *const _).collect();
        make(env, |o, e| unsafe {
            rorb_tc_struct(
                kind as u32,
                id.as_ptr() as *const c_char,
                id.len(),
                name.as_ptr() as *const c_char,
                name.len(),
                names.len().min(type_ptrs.len()),
                name_ptrs.as_ptr(),
                name_lens.as_ptr(),
                type_ptrs.as_ptr(),
                o,
                e,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcUnion(
    mut env: JNIEnv,
    _: JClass,
    id: JString,
    name: JString,
    discriminator: jlong,
    labels: JLongArray,
    default_index: jint,
    names: JObjectArray,
    types: JLongArray,
) -> jlong {
    run(&mut env, 0, |env| {
        let id = rust_string(env, &id)?;
        let name = rust_string(env, &name)?;
        let labels: Vec<u64> = longs(env, &labels)?.into_iter().map(|l| l as u64).collect();
        let names = strings(env, &names)?;
        let types = longs(env, &types)?;
        let name_ptrs: Vec<*const c_char> =
            names.iter().map(|s| s.as_ptr() as *const c_char).collect();
        let name_lens: Vec<usize> = names.iter().map(|s| s.len()).collect();
        let type_ptrs: Vec<*const rorb_typecode> =
            types.iter().map(|t| ptr(*t) as *const _).collect();
        let count = names.len().min(type_ptrs.len()).min(labels.len());
        make(env, |o, e| unsafe {
            rorb_tc_union(
                id.as_ptr() as *const c_char,
                id.len(),
                name.as_ptr() as *const c_char,
                name.len(),
                ptr(discriminator),
                count,
                labels.as_ptr(),
                default_index,
                name_ptrs.as_ptr(),
                name_lens.as_ptr(),
                type_ptrs.as_ptr(),
                o,
                e,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcEnum(
    mut env: JNIEnv,
    _: JClass,
    id: JString,
    name: JString,
    names: JObjectArray,
) -> jlong {
    run(&mut env, 0, |env| {
        let id = rust_string(env, &id)?;
        let name = rust_string(env, &name)?;
        let names = strings(env, &names)?;
        let name_ptrs: Vec<*const c_char> =
            names.iter().map(|s| s.as_ptr() as *const c_char).collect();
        let name_lens: Vec<usize> = names.iter().map(|s| s.len()).collect();
        make(env, |o, e| unsafe {
            rorb_tc_enum(
                id.as_ptr() as *const c_char,
                id.len(),
                name.as_ptr() as *const c_char,
                name.len(),
                names.len(),
                name_ptrs.as_ptr(),
                name_lens.as_ptr(),
                o,
                e,
            )
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcRecursive(
    mut env: JNIEnv,
    _: JClass,
    id: JString,
) -> jlong {
    run(&mut env, 0, |env| {
        let id = rust_string(env, &id)?;
        make(env, |o, e| unsafe {
            rorb_tc_recursive(id.as_ptr() as *const c_char, id.len(), o, e)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcKind(_: JNIEnv, _: JClass, tc: jlong) -> jint {
    unsafe { rorb_tc_kind(ptr(tc)) as jint }
}

type StrFn = unsafe extern "C" fn(
    *const rorb_typecode,
    *mut *const c_char,
    *mut usize,
    *mut rorb_exception,
) -> rorb_status;

fn tc_str(env: &mut JNIEnv, tc: jlong, f: StrFn) -> jstring {
    run(env, std::ptr::null_mut(), |env| {
        let mut p = std::ptr::null();
        let mut len = 0;
        let mut exc = Exc::new();
        let st = unsafe { f(ptr(tc), &mut p, &mut len, exc.ptr()) };
        exc.check(env, st)?;
        Ok(java_string(env, p, len)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcId(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jstring {
    tc_str(&mut env, tc, rorb_tc_id)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcName(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jstring {
    tc_str(&mut env, tc, rorb_tc_name)
}

/// Runs a query that fills `*out`.
fn query<T: Default>(
    env: &mut JNIEnv,
    f: impl FnOnce(*mut T, *mut rorb_exception) -> rorb_status,
) -> JResult<T> {
    let mut v = T::default();
    let mut exc = Exc::new();
    let st = f(&mut v, exc.ptr());
    exc.check(env, st)?;
    Ok(v)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcMemberCount(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jint {
    run(&mut env, 0, |env| {
        Ok(query(env, |o, e| unsafe { rorb_tc_member_count(ptr(tc), o, e) })? as jint)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcMemberName(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
    index: jint,
) -> jstring {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut p = std::ptr::null();
        let mut len = 0;
        let mut exc = Exc::new();
        let st = unsafe { rorb_tc_member_name(ptr(tc), index as u32, &mut p, &mut len, exc.ptr()) };
        exc.check(env, st)?;
        Ok(java_string(env, p, len)?.into_raw())
    })
}

/// A query that returns a TypeCode or an any handle.
fn query_handle<T>(
    env: &mut JNIEnv,
    f: impl FnOnce(*mut *mut T, *mut rorb_exception) -> rorb_status,
) -> JResult<jlong> {
    let mut v: *mut T = std::ptr::null_mut();
    let mut exc = Exc::new();
    let st = f(&mut v, exc.ptr());
    exc.check(env, st)?;
    Ok(handle(v))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcMemberType(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
    index: jint,
) -> jlong {
    run(&mut env, 0, |env| {
        query_handle(env, |o, e| unsafe {
            rorb_tc_member_type(ptr(tc), index as u32, o, e)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcMemberLabel(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
    index: jint,
) -> jlong {
    run(&mut env, 0, |env| {
        query_handle(env, |o, e| unsafe {
            rorb_tc_member_label(ptr(tc), index as u32, o, e)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcDiscriminatorType(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jlong {
    run(&mut env, 0, |env| {
        query_handle(env, |o, e| unsafe {
            rorb_tc_discriminator_type(ptr(tc), o, e)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcDefaultIndex(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jint {
    run(&mut env, 0, |env| {
        query(env, |o, e| unsafe { rorb_tc_default_index(ptr(tc), o, e) })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcLength(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jint {
    run(&mut env, 0, |env| {
        Ok(query(env, |o, e| unsafe { rorb_tc_length(ptr(tc), o, e) })? as jint)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcContentType(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jlong {
    run(&mut env, 0, |env| {
        query_handle(env, |o, e| unsafe { rorb_tc_content_type(ptr(tc), o, e) })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcFixedDigits(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jshort {
    run(&mut env, 0, |env| {
        Ok(query(env, |o, e| unsafe { rorb_tc_fixed_digits(ptr(tc), o, e) })? as jshort)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcFixedScale(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jshort {
    run(&mut env, 0, |env| {
        query(env, |o, e| unsafe { rorb_tc_fixed_scale(ptr(tc), o, e) })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcMemberVisibility(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
    index: jint,
) -> jshort {
    run(&mut env, 0, |env| {
        query(env, |o, e| unsafe {
            rorb_tc_member_visibility(ptr(tc), index as u32, o, e)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcTypeModifier(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jshort {
    run(&mut env, 0, |env| {
        query(env, |o, e| unsafe { rorb_tc_type_modifier(ptr(tc), o, e) })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcConcreteBaseType(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jlong {
    run(&mut env, 0, |env| {
        query_handle(env, |o, e| unsafe {
            rorb_tc_concrete_base_type(ptr(tc), o, e)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcEqual(
    _: JNIEnv,
    _: JClass,
    a: jlong,
    b: jlong,
) -> jboolean {
    (unsafe { rorb_tc_equal(ptr(a), ptr(b)) } != 0) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcEquivalent(
    _: JNIEnv,
    _: JClass,
    a: jlong,
    b: jlong,
) -> jboolean {
    (unsafe { rorb_tc_equivalent(ptr(a), ptr(b)) } != 0) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcCompact(_: JNIEnv, _: JClass, tc: jlong) -> jlong {
    handle(unsafe { rorb_tc_compact(ptr(tc)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_tcUnaliased(
    _: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jlong {
    handle(unsafe { rorb_tc_unaliased(ptr(tc)) })
}

// --- anys ------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_anyNew(_: JNIEnv, _: JClass) -> jlong {
    handle(unsafe { rorb_any_new() })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_anyClone(_: JNIEnv, _: JClass, any: jlong) -> jlong {
    handle(unsafe { rorb_any_clone(ptr::<rorb_any>(any)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_anyRelease(_: JNIEnv, _: JClass, any: jlong) {
    unsafe { rorb_any_release(ptr(any)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_anyType(_: JNIEnv, _: JClass, any: jlong) -> jlong {
    handle(unsafe { rorb_any_type(ptr::<rorb_any>(any)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_anySetType(
    mut env: JNIEnv,
    _: JClass,
    any: jlong,
    tc: jlong,
) {
    run(&mut env, (), |env| {
        let mut exc = Exc::new();
        let st = unsafe { rorb_any_set_type(ptr(any), ptr(tc), exc.ptr()) };
        exc.check(env, st)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_anyEqual(
    _: JNIEnv,
    _: JClass,
    a: jlong,
    b: jlong,
) -> jboolean {
    (unsafe { rorb_any_equal(ptr::<rorb_any>(a), ptr::<rorb_any>(b)) } != 0) as jboolean
}

/// The value of the any as a union label in label[0]; false if it is not
/// of a discriminator type.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_anyLabel(
    mut env: JNIEnv,
    _: JClass,
    any: jlong,
    label: JLongArray,
) -> jboolean {
    run(&mut env, 0, |env| {
        let mut v = 0u64;
        if unsafe { rorb_any_label(ptr::<rorb_any>(any), &mut v) } == 0 {
            return Ok(0);
        }
        env.set_long_array_region(&label, 0, &[v as jlong])?;
        Ok(1)
    })
}

/// `anyInsert(tc, call)`: an any of type tc holding what call.write writes.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_anyInsert(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
    call: JObject,
) -> jlong {
    run(&mut env, 0, |env| {
        let mut ctx = CallContext::new(env, &call);
        let mut any = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe { rorb_any_insert(ptr(tc), call_write, ctx.as_ptr(), &mut any, exc.ptr()) };
        exc.check(env, st)?;
        Ok(handle(any))
    })
}

/// `anyExtract(any, call)`: call.read reads the value of the any.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_anyExtract(
    mut env: JNIEnv,
    _: JClass,
    any: jlong,
    call: JObject,
) {
    run(&mut env, (), |env| {
        let mut ctx = CallContext::new(env, &call);
        let mut exc = Exc::new();
        let st =
            unsafe { rorb_any_extract(ptr::<rorb_any>(any), call_read, ctx.as_ptr(), exc.ptr()) };
        exc.check(env, st)
    })
}
