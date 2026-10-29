//! `ORB.create_policy`, `ORB.register_initial_reference` and the policy
//! overrides of references, of the ORB and of the calling thread. Lists
//! of policies cross as an `int[]` of types and a `long[]` of values (see
//! `rorb_each_policy`); the getters return the pairs flattened in a
//! `long[]`.

use std::ffi::{c_char, c_void};

use jni::JNIEnv;
use jni::objects::{GlobalRef, JClass, JIntArray, JLongArray, JObject, JString};
use jni::sys::{jboolean, jint, jlong, jlongArray, jobject};

use crate::sys::*;
use crate::{Exc, JResult, handle, ptr, run, rust_string};

fn ints(env: &mut JNIEnv, a: &JIntArray) -> JResult<Vec<u32>> {
    let n = env.get_array_length(a)? as usize;
    let mut v = vec![0; n];
    env.get_int_array_region(a, 0, &mut v)?;
    Ok(v.into_iter().map(|t| t as u32).collect())
}

fn longs(env: &mut JNIEnv, a: &JLongArray) -> JResult<Vec<u64>> {
    let n = env.get_array_length(a)? as usize;
    let mut v = vec![0; n];
    env.get_long_array_region(a, 0, &mut v)?;
    Ok(v.into_iter().map(|t| t as u64).collect())
}

unsafe extern "C" fn push_policy(context: *mut c_void, policy_type: u32, value: u64) {
    let list = unsafe { &mut *(context as *mut Vec<jlong>) };
    list.push(jlong::from(policy_type));
    list.push(value as jlong);
}

/// Collects the policies `f` reports into a flattened `long[]`.
fn policy_array(
    env: &mut JNIEnv,
    f: impl FnOnce(*mut c_void, unsafe extern "C" fn(*mut c_void, u32, u64)),
) -> JResult<jlongArray> {
    let mut list: Vec<jlong> = Vec::new();
    f(&mut list as *mut Vec<jlong> as *mut c_void, push_policy);
    let a = env.new_long_array(list.len() as i32)?;
    env.set_long_array_region(&a, 0, &list)?;
    Ok(a.into_raw())
}

/// The types and values of a policy list; the longer array is cut to the
/// length of the other.
fn policy_list(
    env: &mut JNIEnv,
    types: &JIntArray,
    values: &JLongArray,
) -> JResult<(Vec<u32>, Vec<u64>)> {
    let mut t = ints(env, types)?;
    let mut v = longs(env, values)?;
    let n = t.len().min(v.len());
    t.truncate(n);
    v.truncate(n);
    Ok((t, v))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbCreatePolicy(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
    policy_type: jint,
    any: jlong,
) -> jlong {
    run(&mut env, 0, |env| {
        let mut value = 0u64;
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_orb_create_policy(
                ptr(orb),
                policy_type as u32,
                ptr(any),
                &mut value,
                exc.ptr(),
            )
        };
        exc.check(env, st)?;
        Ok(value as jlong)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbRegisterInitialReference(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
    name: JString,
    obj: jlong,
) {
    run(&mut env, (), |env| {
        let name = rust_string(env, &name)?;
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_orb_register_initial_reference(
                ptr(orb),
                name.as_ptr() as *const c_char,
                name.len(),
                ptr(obj),
                exc.ptr(),
            )
        };
        exc.check(env, st)
    })
}

unsafe extern "C" fn release_global(object: *mut c_void) {
    // Dropping the GlobalRef attaches the thread if it has to.
    drop(unsafe { Box::from_raw(object as *mut GlobalRef) });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbRegisterLocalInitialReference(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
    name: JString,
    obj: JObject,
) {
    run(&mut env, (), |env| {
        let name = rust_string(env, &name)?;
        let global = Box::new(env.new_global_ref(obj)?);
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_orb_register_local_initial_reference(
                ptr(orb),
                name.as_ptr() as *const c_char,
                name.len(),
                Box::into_raw(global) as *mut c_void,
                release_global,
                exc.ptr(),
            )
        };
        exc.check(env, st)
    })
}

unsafe extern "C" fn copy_global(context: *mut c_void, object: *mut c_void) {
    let out = unsafe { &mut *(context as *mut Option<GlobalRef>) };
    *out = Some(unsafe { &*(object as *const GlobalRef) }.clone());
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbResolveLocalInitialReference(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
    name: JString,
) -> jobject {
    run(&mut env, std::ptr::null_mut(), |env| {
        let name = rust_string(env, &name)?;
        let mut found: Option<GlobalRef> = None;
        unsafe {
            rorb_orb_resolve_local_initial_reference(
                ptr(orb),
                name.as_ptr() as *const c_char,
                name.len(),
                &mut found as *mut Option<GlobalRef> as *mut c_void,
                copy_global,
            )
        };
        match found {
            Some(g) => Ok(env.new_local_ref(g.as_obj())?.into_raw()),
            None => Ok(std::ptr::null_mut()),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbSetPolicyOverrides(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
    types: JIntArray,
    values: JLongArray,
    add: jboolean,
) {
    run(&mut env, (), |env| {
        let (t, v) = policy_list(env, &types, &values)?;
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_orb_set_policy_overrides(
                ptr(orb),
                t.as_ptr(),
                v.as_ptr(),
                t.len(),
                u32::from(add != 0),
                exc.ptr(),
            )
        };
        exc.check(env, st)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbPolicyOverrides(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
    types: JIntArray,
) -> jlongArray {
    run(&mut env, std::ptr::null_mut(), |env| {
        let t = ints(env, &types)?;
        policy_array(env, |ctx, each| unsafe {
            rorb_orb_get_policy_overrides(ptr(orb), t.as_ptr(), t.len(), ctx, each)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_setThreadPolicyOverrides(
    mut env: JNIEnv,
    _: JClass,
    types: JIntArray,
    values: JLongArray,
    add: jboolean,
) {
    run(&mut env, (), |env| {
        let (t, v) = policy_list(env, &types, &values)?;
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_set_thread_policy_overrides(
                t.as_ptr(),
                v.as_ptr(),
                t.len(),
                u32::from(add != 0),
                exc.ptr(),
            )
        };
        exc.check(env, st)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_threadPolicyOverrides(
    mut env: JNIEnv,
    _: JClass,
    types: JIntArray,
) -> jlongArray {
    run(&mut env, std::ptr::null_mut(), |env| {
        let t = ints(env, &types)?;
        policy_array(env, |ctx, each| unsafe {
            rorb_thread_policy_overrides(t.as_ptr(), t.len(), ctx, each)
        })
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectSetPolicyOverrides(
    mut env: JNIEnv,
    _: JClass,
    obj: jlong,
    types: JIntArray,
    values: JLongArray,
    add: jboolean,
) -> jlong {
    run(&mut env, 0, |env| {
        let (t, v) = policy_list(env, &types, &values)?;
        let mut result = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_object_set_policy_overrides(
                ptr(obj),
                t.as_ptr(),
                v.as_ptr(),
                t.len(),
                u32::from(add != 0),
                &mut result,
                exc.ptr(),
            )
        };
        exc.check(env, st)?;
        Ok(handle(result))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectPolicyOverrides(
    mut env: JNIEnv,
    _: JClass,
    obj: jlong,
    types: JIntArray,
) -> jlongArray {
    run(&mut env, std::ptr::null_mut(), |env| {
        let t = ints(env, &types)?;
        policy_array(env, |ctx, each| unsafe {
            rorb_object_get_policy_overrides(ptr(obj), t.as_ptr(), t.len(), ctx, each)
        })
    })
}

/// The value of the client policy in `value[0]`; false if there is none.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectClientPolicy(
    mut env: JNIEnv,
    _: JClass,
    obj: jlong,
    policy_type: jint,
    value: JLongArray,
) -> jboolean {
    run(&mut env, 0, |env| {
        let mut v = 0u64;
        let found = unsafe { rorb_object_get_client_policy(ptr(obj), policy_type as u32, &mut v) };
        if found != 0 {
            env.set_long_array_region(&value, 0, &[v as jlong])?;
        }
        Ok(jboolean::from(found != 0))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_objectValidateConnection(
    mut env: JNIEnv,
    _: JClass,
    obj: jlong,
) -> jboolean {
    run(&mut env, 0, |env| {
        let mut result = 0;
        let mut exc = Exc::new();
        let st = unsafe { rorb_object_validate_connection(ptr(obj), &mut result, exc.ptr()) };
        exc.check(env, st)?;
        Ok(jboolean::from(result != 0))
    })
}
