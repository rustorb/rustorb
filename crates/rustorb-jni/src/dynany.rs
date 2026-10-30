//! `org.rustorb.Native.dyn*`: DynamicAny, on the `rorb_dynany` functions
//! of the C ABI. Values cross as `rorb_any` handles (org.rustorb.AnyImpl),
//! DynAnys as `rorb_dynany` handles (org.rustorb.DynAnyImpl).

use jni::JNIEnv;
use jni::objects::{JClass, JLongArray, JObjectArray, JString};
use jni::sys::{jboolean, jint, jlong, jstring};

use crate::sys::*;
use crate::{Exc, JResult, handle, ptr, run, rust_string, take_bytes};

/// Runs a C call that reports through `exc`.
fn call(env: &mut JNIEnv, f: impl FnOnce(*mut rorb_exception) -> rorb_status) -> JResult<()> {
    let mut exc = Exc::new();
    let st = f(exc.ptr());
    exc.check(env, st)
}

/// Runs a C call that fills `*out`.
fn out<T: Default>(
    env: &mut JNIEnv,
    f: impl FnOnce(*mut T, *mut rorb_exception) -> rorb_status,
) -> JResult<T> {
    let mut v = T::default();
    call(env, |e| f(&mut v, e))?;
    Ok(v)
}

/// Runs a C call that fills `*out` with a handle.
fn out_handle<T>(
    env: &mut JNIEnv,
    f: impl FnOnce(*mut *mut T, *mut rorb_exception) -> rorb_status,
) -> JResult<jlong> {
    let mut p: *mut T = std::ptr::null_mut();
    call(env, |e| f(&mut p, e))?;
    Ok(handle(p))
}

/// A UTF-8 string of the C ABI as a Java string.
fn out_string(
    env: &mut JNIEnv,
    f: impl FnOnce(*mut rorb_bytes, *mut rorb_exception) -> rorb_status,
) -> JResult<jstring> {
    let b = out::<rorb_bytes>(env, f)?;
    let bytes = take_bytes(env, b)?;
    let bytes = env.convert_byte_array(&bytes)?;
    Ok(env.new_string(String::from_utf8_lossy(&bytes))?.into_raw())
}

fn handles<T>(env: &mut JNIEnv, array: &JLongArray) -> JResult<Vec<*const T>> {
    if array.is_null() {
        return crate::throw_system(env, "IDL:omg.org/CORBA/BAD_PARAM:1.0", 0, 1);
    }
    let len = env.get_array_length(array)? as usize;
    let mut v = vec![0; len];
    env.get_long_array_region(array, 0, &mut v)?;
    Ok(v.into_iter().map(|h| h as *const T).collect())
}

fn strings(env: &mut JNIEnv, array: &JObjectArray) -> JResult<Vec<String>> {
    if array.is_null() {
        return crate::throw_system(env, "IDL:omg.org/CORBA/BAD_PARAM:1.0", 0, 1);
    }
    let len = env.get_array_length(array)?;
    let mut v = Vec::with_capacity(len as usize);
    for i in 0..len {
        let s = JString::from(env.get_object_array_element(array, i)?);
        v.push(rust_string(env, &s)?);
    }
    Ok(v)
}

/// `name(d) -> handle`: a C function that fills a handle.
macro_rules! get_handle {
    ($($name:ident => $f:ident;)*) => {$(
        #[unsafe(no_mangle)]
        pub extern "system" fn $name(mut env: JNIEnv, _: JClass, d: jlong) -> jlong {
            run(&mut env, 0, |env| out_handle(env, |o, e| unsafe { $f(ptr(d), o, e) }))
        }
    )*};
}

/// `name(d) -> int`: a C function that fills a uint32_t.
macro_rules! get_int {
    ($($name:ident => $f:ident;)*) => {$(
        #[unsafe(no_mangle)]
        pub extern "system" fn $name(mut env: JNIEnv, _: JClass, d: jlong) -> jint {
            run(&mut env, 0, |env| out::<u32>(env, |o, e| unsafe { $f(ptr(d), o, e) }))
                as jint
        }
    )*};
}

/// `name(d) -> boolean`: a C function that fills an int32_t.
macro_rules! get_bool {
    ($($name:ident => $f:ident;)*) => {$(
        #[unsafe(no_mangle)]
        pub extern "system" fn $name(mut env: JNIEnv, _: JClass, d: jlong) -> jboolean {
            run(&mut env, 0, |env| out::<i32>(env, |o, e| unsafe { $f(ptr(d), o, e) }))
                as jboolean
        }
    )*};
}

/// `name(d) -> String`: a C function that fills an rorb_bytes.
macro_rules! get_string {
    ($($name:ident => $f:ident;)*) => {$(
        #[unsafe(no_mangle)]
        pub extern "system" fn $name(mut env: JNIEnv, _: JClass, d: jlong) -> jstring {
            run(&mut env, std::ptr::null_mut(), |env| {
                out_string(env, |o, e| unsafe { $f(ptr(d), o, e) })
            })
        }
    )*};
}

/// `name(d)`: a C function without results.
macro_rules! run0 {
    ($($name:ident => $f:ident;)*) => {$(
        #[unsafe(no_mangle)]
        pub extern "system" fn $name(mut env: JNIEnv, _: JClass, d: jlong) {
            run(&mut env, (), |env| call(env, |e| unsafe { $f(ptr(d), e) }))
        }
    )*};
}

/// `name(d, h)`: a C function that takes a handle.
macro_rules! with_handle {
    ($($name:ident => $f:ident;)*) => {$(
        #[unsafe(no_mangle)]
        pub extern "system" fn $name(mut env: JNIEnv, _: JClass, d: jlong, h: jlong) {
            run(&mut env, (), |env| call(env, |e| unsafe { $f(ptr(d), ptr(h), e) }))
        }
    )*};
}

/// `name(d, kind, any)`: insert and insert_seq.
macro_rules! with_kind_any {
    ($($name:ident => $f:ident;)*) => {$(
        #[unsafe(no_mangle)]
        pub extern "system" fn $name(mut env: JNIEnv, _: JClass, d: jlong, kind: jint, any: jlong) {
            run(&mut env, (), |env| {
                call(env, |e| unsafe { $f(ptr(d), kind as u32, ptr(any), e) })
            })
        }
    )*};
}

/// `name(d, kind) -> any`: get and get_seq.
macro_rules! kind_to_any {
    ($($name:ident => $f:ident;)*) => {$(
        #[unsafe(no_mangle)]
        pub extern "system" fn $name(mut env: JNIEnv, _: JClass, d: jlong, kind: jint) -> jlong {
            run(&mut env, 0, |env| {
                out_handle(env, |o, e| unsafe { $f(ptr(d), kind as u32, o, e) })
            })
        }
    )*};
}

/// `name(d, v)`: a C function that takes a uint32_t.
macro_rules! with_int {
    ($($name:ident => $f:ident;)*) => {$(
        #[unsafe(no_mangle)]
        pub extern "system" fn $name(mut env: JNIEnv, _: JClass, d: jlong, v: jint) {
            run(&mut env, (), |env| call(env, |e| unsafe { $f(ptr(d), v as u32, e) }))
        }
    )*};
}

get_handle! {
    Java_org_rustorb_Native_dynType => rorb_dynany_type;
    Java_org_rustorb_Native_dynTargetType => rorb_dynany_target_type;
    Java_org_rustorb_Native_dynToAny => rorb_dynany_to_any;
    Java_org_rustorb_Native_dynCopy => rorb_dynany_copy;
    Java_org_rustorb_Native_dynGetDynAny => rorb_dynany_get_dyn_any;
    Java_org_rustorb_Native_dynCurrentComponent => rorb_dynany_current_component;
    Java_org_rustorb_Native_dynGetDiscriminator => rorb_dynany_get_discriminator;
    Java_org_rustorb_Native_dynMember => rorb_dynany_member;
    Java_org_rustorb_Native_dynGetBoxedValue => rorb_dynany_get_boxed_value;
    Java_org_rustorb_Native_dynGetBoxedValueAsDynAny => rorb_dynany_get_boxed_value_as_dyn_any;
}

get_int! {
    Java_org_rustorb_Native_dynComponentCount => rorb_dynany_component_count;
    Java_org_rustorb_Native_dynGetAsUlong => rorb_dynany_get_as_ulong;
    Java_org_rustorb_Native_dynCurrentMemberKind => rorb_dynany_current_member_kind;
    Java_org_rustorb_Native_dynMemberCount => rorb_dynany_member_count;
    Java_org_rustorb_Native_dynDiscriminatorKind => rorb_dynany_discriminator_kind;
    Java_org_rustorb_Native_dynMemberKind => rorb_dynany_member_kind;
    Java_org_rustorb_Native_dynGetLength => rorb_dynany_get_length;
    Java_org_rustorb_Native_dynElementCount => rorb_dynany_element_count;
}

get_bool! {
    Java_org_rustorb_Native_dynNext => rorb_dynany_next;
    Java_org_rustorb_Native_dynHasNoActiveMember => rorb_dynany_has_no_active_member;
    Java_org_rustorb_Native_dynIsNull => rorb_dynany_is_null;
}

get_string! {
    Java_org_rustorb_Native_dynGetFixedValue => rorb_dynany_get_fixed_value;
    Java_org_rustorb_Native_dynGetAsString => rorb_dynany_get_as_string;
    Java_org_rustorb_Native_dynCurrentMemberName => rorb_dynany_current_member_name;
    Java_org_rustorb_Native_dynMemberName => rorb_dynany_member_name;
}

run0! {
    Java_org_rustorb_Native_dynDestroy => rorb_dynany_destroy;
    Java_org_rustorb_Native_dynRewind => rorb_dynany_rewind;
    Java_org_rustorb_Native_dynNoImplement => rorb_dynany_no_implement;
    Java_org_rustorb_Native_dynSetToDefaultMember => rorb_dynany_set_to_default_member;
    Java_org_rustorb_Native_dynSetToNoActiveMember => rorb_dynany_set_to_no_active_member;
    Java_org_rustorb_Native_dynSetToNull => rorb_dynany_set_to_null;
    Java_org_rustorb_Native_dynSetToValue => rorb_dynany_set_to_value;
}

with_handle! {
    Java_org_rustorb_Native_dynAssign => rorb_dynany_assign;
    Java_org_rustorb_Native_dynFromAny => rorb_dynany_from_any;
    Java_org_rustorb_Native_dynInsertDynAny => rorb_dynany_insert_dyn_any;
    Java_org_rustorb_Native_dynSetDiscriminator => rorb_dynany_set_discriminator;
    Java_org_rustorb_Native_dynSetBoxedValue => rorb_dynany_set_boxed_value;
    Java_org_rustorb_Native_dynSetBoxedValueAsDynAny => rorb_dynany_set_boxed_value_as_dyn_any;
}

with_kind_any! {
    Java_org_rustorb_Native_dynInsert => rorb_dynany_insert;
    Java_org_rustorb_Native_dynInsertSeq => rorb_dynany_insert_seq;
}

kind_to_any! {
    Java_org_rustorb_Native_dynGet => rorb_dynany_get;
    Java_org_rustorb_Native_dynGetSeq => rorb_dynany_get_seq;
}

with_int! {
    Java_org_rustorb_Native_dynSetAsUlong => rorb_dynany_set_as_ulong;
    Java_org_rustorb_Native_dynSetLength => rorb_dynany_set_length;
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynCreate(
    mut env: JNIEnv,
    _: JClass,
    any: jlong,
    truncate: jboolean,
) -> jlong {
    run(&mut env, 0, |env| {
        out_handle(env, |o, e| unsafe {
            rorb_dynany_create(ptr(any), truncate as i32, o, e)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynFromTypeCode(
    mut env: JNIEnv,
    _: JClass,
    tc: jlong,
) -> jlong {
    run(&mut env, 0, |env| {
        out_handle(env, |o, e| unsafe {
            rorb_dynany_from_typecode(ptr(tc), o, e)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynClone(_: JNIEnv, _: JClass, d: jlong) -> jlong {
    handle(unsafe { rorb_dynany_clone(ptr::<rorb_dynany>(d)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynRelease(_: JNIEnv, _: JClass, d: jlong) {
    unsafe { rorb_dynany_release(ptr(d)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynKind(_: JNIEnv, _: JClass, d: jlong) -> jint {
    unsafe { rorb_dynany_kind(ptr::<rorb_dynany>(d)) as jint }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynSame(
    _: JNIEnv,
    _: JClass,
    a: jlong,
    b: jlong,
) -> jboolean {
    unsafe { rorb_dynany_same(ptr::<rorb_dynany>(a), ptr::<rorb_dynany>(b)) as jboolean }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynEqual(
    mut env: JNIEnv,
    _: JClass,
    d: jlong,
    other: jlong,
) -> jboolean {
    run(&mut env, 0, |env| {
        out::<i32>(env, |o, e| unsafe {
            rorb_dynany_equal(ptr(d), ptr(other), o, e)
        })
    }) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynSeek(
    mut env: JNIEnv,
    _: JClass,
    d: jlong,
    index: jint,
) -> jboolean {
    run(&mut env, 0, |env| {
        out::<i32>(env, |o, e| unsafe { rorb_dynany_seek(ptr(d), index, o, e) })
    }) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynSetFixedValue(
    mut env: JNIEnv,
    _: JClass,
    d: jlong,
    value: JString,
) -> jboolean {
    run(&mut env, 0, |env| {
        let v = rust_string(env, &value)?;
        out::<i32>(env, |o, e| unsafe {
            rorb_dynany_set_fixed_value(ptr(d), v.as_ptr().cast(), v.len(), o, e)
        })
    }) as jboolean
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynSetAsString(
    mut env: JNIEnv,
    _: JClass,
    d: jlong,
    value: JString,
) {
    run(&mut env, (), |env| {
        let v = rust_string(env, &value)?;
        call(env, |e| unsafe {
            rorb_dynany_set_as_string(ptr(d), v.as_ptr().cast(), v.len(), e)
        })
    })
}

/// `dynMemberAt(d, index, name)`: the member's DynAny; its name goes in
/// `name[0]`.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynMemberAt(
    mut env: JNIEnv,
    _: JClass,
    d: jlong,
    index: jint,
    name: JObjectArray,
) -> jlong {
    run(&mut env, 0, |env| {
        let mut n = rorb_bytes::default();
        let h = out_handle(env, |o, e| unsafe {
            rorb_dynany_member_at(ptr(d), index as u32, &mut n, o, e)
        })?;
        let bytes = take_bytes(env, n)?;
        let bytes = env.convert_byte_array(&bytes)?;
        let s = env.new_string(String::from_utf8_lossy(&bytes))?;
        env.set_object_array_element(&name, 0, s)?;
        Ok(h)
    })
}

/// `dynSetMembers(d, names, values, asDynAny)`: set_members (values are
/// anys) or set_members_as_dyn_any (DynAnys).
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynSetMembers(
    mut env: JNIEnv,
    _: JClass,
    d: jlong,
    names: JObjectArray,
    values: JLongArray,
    as_dyn_any: jboolean,
) {
    run(&mut env, (), |env| {
        let names = strings(env, &names)?;
        let ptrs: Vec<*const std::ffi::c_char> = names.iter().map(|n| n.as_ptr().cast()).collect();
        let lens: Vec<usize> = names.iter().map(String::len).collect();
        if as_dyn_any != 0 {
            let v = handles::<rorb_dynany>(env, &values)?;
            call(env, |e| unsafe {
                rorb_dynany_set_members_as_dyn_any(
                    ptr(d),
                    v.len().min(ptrs.len()),
                    ptrs.as_ptr(),
                    lens.as_ptr(),
                    v.as_ptr(),
                    e,
                )
            })
        } else {
            let v = handles::<rorb_any>(env, &values)?;
            call(env, |e| unsafe {
                rorb_dynany_set_members(
                    ptr(d),
                    v.len().min(ptrs.len()),
                    ptrs.as_ptr(),
                    lens.as_ptr(),
                    v.as_ptr(),
                    e,
                )
            })
        }
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynElementAt(
    mut env: JNIEnv,
    _: JClass,
    d: jlong,
    index: jint,
) -> jlong {
    run(&mut env, 0, |env| {
        out_handle(env, |o, e| unsafe {
            rorb_dynany_element_at(ptr(d), index as u32, o, e)
        })
    })
}

/// `dynSetElements(d, values, asDynAny)`: set_elements (anys) or
/// set_elements_as_dyn_any (DynAnys).
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_dynSetElements(
    mut env: JNIEnv,
    _: JClass,
    d: jlong,
    values: JLongArray,
    as_dyn_any: jboolean,
) {
    run(&mut env, (), |env| {
        if as_dyn_any != 0 {
            let v = handles::<rorb_dynany>(env, &values)?;
            call(env, |e| unsafe {
                rorb_dynany_set_elements_as_dyn_any(ptr(d), v.len(), v.as_ptr(), e)
            })
        } else {
            let v = handles::<rorb_any>(env, &values)?;
            call(env, |e| unsafe {
                rorb_dynany_set_elements(ptr(d), v.len(), v.as_ptr(), e)
            })
        }
    })
}
