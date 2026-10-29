//! `org.rustorb.Native.orb*`: ORB_init and the ORB operations.

use std::ffi::{CString, c_char, c_int, c_void};

use jni::JNIEnv;
use jni::objects::{JClass, JObjectArray, JString};
use jni::sys::{jboolean, jlong, jobjectArray, jstring};

use crate::sys::*;
use crate::{Exc, handle, java_string, ptr, run, rust_string, string_array, throw_system};

/// `orbInit(String[] args)`: the ORB options are the -ORB... arguments;
/// the others are ignored.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbInit(
    mut env: JNIEnv,
    _: JClass,
    args: JObjectArray,
) -> jlong {
    run(&mut env, 0, |env| {
        let mut owned = vec![CString::new("java").unwrap()];
        let n = if args.is_null() {
            0
        } else {
            env.get_array_length(&args)?
        };
        for i in 0..n {
            let a = env.get_object_array_element(&args, i)?;
            if a.is_null() {
                continue;
            }
            let s = rust_string(env, &JString::from(a))?;
            match CString::new(s) {
                Ok(c) => owned.push(c),
                Err(_) => return throw_system(env, "IDL:omg.org/CORBA/BAD_PARAM:1.0", 0, 1),
            }
        }
        let mut argv: Vec<*mut c_char> = owned.iter().map(|c| c.as_ptr() as *mut c_char).collect();
        argv.push(std::ptr::null_mut());
        let mut argc = owned.len() as c_int;
        let mut orb = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe { rorb_orb_init(&mut argc, argv.as_mut_ptr(), &mut orb, exc.ptr()) };
        exc.check(env, st)?;
        Ok(handle(orb))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbRelease(_: JNIEnv, _: JClass, orb: jlong) {
    unsafe { rorb_orb_release(ptr(orb)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbStringToObject(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
    s: JString,
) -> jlong {
    run(&mut env, 0, |env| {
        let s = rust_string(env, &s)?;
        let mut obj = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_orb_string_to_object(
                ptr(orb),
                s.as_ptr() as *const c_char,
                s.len(),
                &mut obj,
                exc.ptr(),
            )
        };
        exc.check(env, st)?;
        Ok(handle(obj))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbObjectToString(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
    obj: jlong,
) -> jstring {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut bytes = rorb_bytes::default();
        let mut exc = Exc::new();
        let st = unsafe { rorb_orb_object_to_string(ptr(orb), ptr(obj), &mut bytes, exc.ptr()) };
        exc.check(env, st)?;
        let s = java_string(env, bytes.data as *const c_char, bytes.len);
        unsafe { rorb_bytes_free(bytes) };
        Ok(s?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbResolveInitialReferences(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
    name: JString,
) -> jlong {
    run(&mut env, 0, |env| {
        let name = rust_string(env, &name)?;
        let mut obj = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_orb_resolve_initial_references(
                ptr(orb),
                name.as_ptr() as *const c_char,
                name.len(),
                &mut obj,
                exc.ptr(),
            )
        };
        exc.check(env, st)?;
        Ok(handle(obj))
    })
}

unsafe extern "C" fn push_name(context: *mut c_void, name: *const c_char, len: usize) {
    let names = unsafe { &mut *(context as *mut Vec<String>) };
    let bytes = unsafe { std::slice::from_raw_parts(name as *const u8, len) };
    names.push(String::from_utf8_lossy(bytes).into_owned());
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbListInitialServices(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
) -> jobjectArray {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut names: Vec<String> = Vec::new();
        unsafe {
            rorb_orb_list_initial_services(
                ptr(orb),
                &mut names as *mut Vec<String> as *mut c_void,
                push_name,
            )
        };
        Ok(string_array(env, &names)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbRootPoa(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
) -> jlong {
    run(&mut env, 0, |env| {
        let mut poa = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe { rorb_orb_root_poa(ptr(orb), &mut poa, exc.ptr()) };
        exc.check(env, st)?;
        Ok(handle(poa))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_defaultRootPoa(mut env: JNIEnv, _: JClass) -> jlong {
    run(&mut env, 0, |env| {
        let mut poa = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe { rorb_default_root_poa(&mut poa, exc.ptr()) };
        exc.check(env, st)?;
        Ok(handle(poa))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbRun(_: JNIEnv, _: JClass, orb: jlong) {
    unsafe { rorb_orb_run(ptr(orb)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbWorkPending(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
) -> jboolean {
    run(&mut env, 0, |env| {
        let mut pending = 0;
        let mut exc = Exc::new();
        let st = unsafe { rorb_orb_work_pending(ptr(orb), &mut pending, exc.ptr()) };
        exc.check(env, st)?;
        Ok((pending != 0) as jboolean)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbPerformWork(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
) {
    run(&mut env, (), |env| {
        let mut exc = Exc::new();
        let st = unsafe { rorb_orb_perform_work(ptr(orb), exc.ptr()) };
        exc.check(env, st)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbShutdown(
    mut env: JNIEnv,
    _: JClass,
    orb: jlong,
    wait: jboolean,
) {
    run(&mut env, (), |env| {
        let mut exc = Exc::new();
        let st = unsafe { rorb_orb_shutdown(ptr(orb), wait as i32, exc.ptr()) };
        exc.check(env, st)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbDestroy(mut env: JNIEnv, _: JClass, orb: jlong) {
    run(&mut env, (), |env| {
        let mut exc = Exc::new();
        let st = unsafe { rorb_orb_destroy(ptr(orb), exc.ptr()) };
        exc.check(env, st)
    })
}

/// `orbSetCallTimeout(orb, us)` and the other timeout settings: the
/// microseconds of the C ABI (`rorb_orb_set_call_timeout`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbSetCallTimeout(
    _: JNIEnv,
    _: JClass,
    orb: jlong,
    us: jlong,
) {
    unsafe { rorb_orb_set_call_timeout(ptr(orb), us) }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbCallTimeout(
    _: JNIEnv,
    _: JClass,
    orb: jlong,
) -> jlong {
    unsafe { rorb_orb_call_timeout(ptr(orb)) }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbSetConnectTimeout(
    _: JNIEnv,
    _: JClass,
    orb: jlong,
    us: jlong,
) {
    unsafe { rorb_orb_set_connect_timeout(ptr(orb), us) }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_orbConnectTimeout(
    _: JNIEnv,
    _: JClass,
    orb: jlong,
) -> jlong {
    unsafe { rorb_orb_connect_timeout(ptr(orb)) }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_setThreadCallTimeout(
    _: JNIEnv,
    _: JClass,
    us: jlong,
) {
    unsafe { rorb_set_thread_call_timeout(us) }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_threadCallTimeout(_: JNIEnv, _: JClass) -> jlong {
    unsafe { rorb_thread_call_timeout() }
}
