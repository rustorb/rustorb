//! `org.rustorb.Native.poa*`, `pm*`, `current*`: the POA, its manager and
//! PortableServer::Current.

use std::ffi::{c_char, c_void};

use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JIntArray, JString};
use jni::sys::{jboolean, jbyteArray, jint, jintArray, jlong, jlongArray, jobject, jstring};

use crate::servant::servant_object;
use crate::sys::*;
use crate::{Exc, JResult, handle, java_string, ptr, run, rust_string, take_bytes};

fn bytes(env: &mut JNIEnv, a: &JByteArray) -> JResult<Vec<u8>> {
    if a.is_null() {
        return crate::throw_system(env, "IDL:omg.org/CORBA/BAD_PARAM:1.0", 0, 1);
    }
    Ok(env.convert_byte_array(a)?)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaClone(_: JNIEnv, _: JClass, poa: jlong) -> jlong {
    handle(unsafe { rorb_poa_clone(ptr(poa)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaRelease(_: JNIEnv, _: JClass, poa: jlong) {
    unsafe { rorb_poa_release(ptr(poa)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaName(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
) -> jstring {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut len = 0;
        let p = unsafe { rorb_poa_name(ptr(poa), &mut len) };
        Ok(java_string(env, p, len)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaParent(
    _: JNIEnv,
    _: JClass,
    poa: jlong,
) -> jlong {
    handle(unsafe { rorb_poa_parent(ptr(poa)) })
}

unsafe extern "C" fn push_poa(context: *mut c_void, child: *mut rorb_poa) {
    let v = unsafe { &mut *(context as *mut Vec<jlong>) };
    v.push(handle(unsafe { rorb_poa_clone(child) }));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaChildren(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
) -> jlongArray {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut children: Vec<jlong> = Vec::new();
        unsafe {
            rorb_poa_children(
                ptr(poa),
                &mut children as *mut Vec<jlong> as *mut c_void,
                push_poa,
            )
        };
        let a = env.new_long_array(children.len() as jint)?;
        env.set_long_array_region(&a, 0, &children)?;
        Ok(a.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaTheManager(
    _: JNIEnv,
    _: JClass,
    poa: jlong,
) -> jlong {
    handle(unsafe { rorb_poa_the_manager(ptr(poa)) })
}

/// The seven policy values, in the order of the policy ids.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaPolicies(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
) -> jintArray {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut p = [rorb_policy::default(); 7];
        unsafe { rorb_poa_policies(ptr(poa), p.as_mut_ptr()) };
        let values: Vec<jint> = p.iter().map(|p| p.value as jint).collect();
        let a = env.new_int_array(7)?;
        env.set_int_array_region(&a, 0, &values)?;
        Ok(a.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaCreatePoa(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    name: JString,
    manager: jlong,
    types: JIntArray,
    values: JIntArray,
) -> jlong {
    run(&mut env, 0, |env| {
        let name = rust_string(env, &name)?;
        let n = env.get_array_length(&types)? as usize;
        let mut t = vec![0; n];
        let mut v = vec![0; n];
        env.get_int_array_region(&types, 0, &mut t)?;
        env.get_int_array_region(&values, 0, &mut v)?;
        let policies: Vec<rorb_policy> = t
            .iter()
            .zip(&v)
            .map(|(t, v)| rorb_policy {
                r#type: *t as u32,
                value: *v as u32,
            })
            .collect();
        let mut child = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_poa_create_poa(
                ptr(poa),
                name.as_ptr() as *const c_char,
                name.len(),
                ptr(manager),
                policies.as_ptr(),
                policies.len(),
                &mut child,
                exc.ptr(),
            )
        };
        exc.check(env, st)?;
        Ok(handle(child))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaFindPoa(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    name: JString,
    activate_it: jboolean,
) -> jlong {
    run(&mut env, 0, |env| {
        let name = rust_string(env, &name)?;
        let mut child = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_poa_find_poa(
                ptr(poa),
                name.as_ptr() as *const c_char,
                name.len(),
                activate_it as i32,
                &mut child,
                exc.ptr(),
            )
        };
        exc.check(env, st)?;
        Ok(handle(child))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaDestroy(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    etherealize: jboolean,
    wait: jboolean,
) {
    run(&mut env, (), |env| {
        let mut exc = Exc::new();
        let st = unsafe { rorb_poa_destroy(ptr(poa), etherealize as i32, wait as i32, exc.ptr()) };
        exc.check(env, st)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaActivateObject(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    servant: jlong,
) -> jbyteArray {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut id = rorb_bytes::default();
        let mut exc = Exc::new();
        let st = unsafe { rorb_poa_activate_object(ptr(poa), ptr(servant), &mut id, exc.ptr()) };
        exc.check(env, st)?;
        Ok(take_bytes(env, id)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaActivateObjectWithId(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    id: JByteArray,
    servant: jlong,
) {
    run(&mut env, (), |env| {
        let id = bytes(env, &id)?;
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_poa_activate_object_with_id(
                ptr(poa),
                id.as_ptr(),
                id.len(),
                ptr(servant),
                exc.ptr(),
            )
        };
        exc.check(env, st)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaDeactivateObject(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    id: JByteArray,
) {
    run(&mut env, (), |env| {
        let id = bytes(env, &id)?;
        let mut exc = Exc::new();
        let st = unsafe { rorb_poa_deactivate_object(ptr(poa), id.as_ptr(), id.len(), exc.ptr()) };
        exc.check(env, st)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaServantToId(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    servant: jlong,
) -> jbyteArray {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut id = rorb_bytes::default();
        let mut exc = Exc::new();
        let st = unsafe { rorb_poa_servant_to_id(ptr(poa), ptr(servant), &mut id, exc.ptr()) };
        exc.check(env, st)?;
        Ok(take_bytes(env, id)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaServantToReference(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    servant: jlong,
) -> jlong {
    run(&mut env, 0, |env| {
        let mut obj = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st =
            unsafe { rorb_poa_servant_to_reference(ptr(poa), ptr(servant), &mut obj, exc.ptr()) };
        exc.check(env, st)?;
        Ok(handle(obj))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaIdToServant(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    id: JByteArray,
) -> jobject {
    run(&mut env, std::ptr::null_mut(), |env| {
        let id = bytes(env, &id)?;
        let mut this = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_poa_id_to_servant(ptr(poa), id.as_ptr(), id.len(), &mut this, exc.ptr())
        };
        exc.check(env, st)?;
        Ok(servant_object(env, this)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaIdToReference(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    id: JByteArray,
) -> jlong {
    run(&mut env, 0, |env| {
        let id = bytes(env, &id)?;
        let mut obj = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_poa_id_to_reference(ptr(poa), id.as_ptr(), id.len(), &mut obj, exc.ptr())
        };
        exc.check(env, st)?;
        Ok(handle(obj))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaReferenceToId(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    obj: jlong,
) -> jbyteArray {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut id = rorb_bytes::default();
        let mut exc = Exc::new();
        let st = unsafe { rorb_poa_reference_to_id(ptr(poa), ptr(obj), &mut id, exc.ptr()) };
        exc.check(env, st)?;
        Ok(take_bytes(env, id)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaReferenceToServant(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    obj: jlong,
) -> jobject {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut this = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe { rorb_poa_reference_to_servant(ptr(poa), ptr(obj), &mut this, exc.ptr()) };
        exc.check(env, st)?;
        Ok(servant_object(env, this)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaCreateReference(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    intf: JString,
) -> jlong {
    run(&mut env, 0, |env| {
        let intf = rust_string(env, &intf)?;
        let mut obj = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_poa_create_reference(
                ptr(poa),
                intf.as_ptr() as *const c_char,
                intf.len(),
                &mut obj,
                exc.ptr(),
            )
        };
        exc.check(env, st)?;
        Ok(handle(obj))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaCreateReferenceWithId(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    id: JByteArray,
    intf: JString,
) -> jlong {
    run(&mut env, 0, |env| {
        let id = bytes(env, &id)?;
        let intf = rust_string(env, &intf)?;
        let mut obj = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_poa_create_reference_with_id(
                ptr(poa),
                id.as_ptr(),
                id.len(),
                intf.as_ptr() as *const c_char,
                intf.len(),
                &mut obj,
                exc.ptr(),
            )
        };
        exc.check(env, st)?;
        Ok(handle(obj))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaGetServant(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
) -> jobject {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut this = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe { rorb_poa_get_servant(ptr(poa), &mut this, exc.ptr()) };
        exc.check(env, st)?;
        Ok(servant_object(env, this)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_poaSetServant(
    mut env: JNIEnv,
    _: JClass,
    poa: jlong,
    servant: jlong,
) {
    run(&mut env, (), |env| {
        let mut exc = Exc::new();
        let st = unsafe { rorb_poa_set_servant(ptr(poa), ptr(servant), exc.ptr()) };
        exc.check(env, st)
    })
}

// --- PortableServer::Current ---------------------------------------------------

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_currentGetPoa(mut env: JNIEnv, _: JClass) -> jlong {
    run(&mut env, 0, |env| {
        let mut poa = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe { rorb_current_get_poa(&mut poa, exc.ptr()) };
        exc.check(env, st)?;
        Ok(handle(poa))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_currentGetObjectId(
    mut env: JNIEnv,
    _: JClass,
) -> jbyteArray {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut id = rorb_bytes::default();
        let mut exc = Exc::new();
        let st = unsafe { rorb_current_get_object_id(&mut id, exc.ptr()) };
        exc.check(env, st)?;
        Ok(take_bytes(env, id)?.into_raw())
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_currentGetReference(
    mut env: JNIEnv,
    _: JClass,
) -> jlong {
    run(&mut env, 0, |env| {
        let mut obj = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe { rorb_current_get_reference(&mut obj, exc.ptr()) };
        exc.check(env, st)?;
        Ok(handle(obj))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_currentGetServant(
    mut env: JNIEnv,
    _: JClass,
) -> jobject {
    run(&mut env, std::ptr::null_mut(), |env| {
        let mut this = std::ptr::null_mut();
        let mut exc = Exc::new();
        let st = unsafe { rorb_current_get_servant(&mut this, exc.ptr()) };
        exc.check(env, st)?;
        Ok(servant_object(env, this)?.into_raw())
    })
}

// --- POAManager ---------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_pmClone(_: JNIEnv, _: JClass, m: jlong) -> jlong {
    handle(unsafe { rorb_poa_manager_clone(ptr(m)) })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_pmRelease(_: JNIEnv, _: JClass, m: jlong) {
    unsafe { rorb_poa_manager_release(ptr(m)) };
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_pmGetState(_: JNIEnv, _: JClass, m: jlong) -> jint {
    unsafe { rorb_poa_manager_get_state(ptr(m)) as jint }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_pmActivate(mut env: JNIEnv, _: JClass, m: jlong) {
    run(&mut env, (), |env| {
        let mut exc = Exc::new();
        let st = unsafe { rorb_poa_manager_activate(ptr(m), exc.ptr()) };
        exc.check(env, st)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_pmHoldRequests(
    mut env: JNIEnv,
    _: JClass,
    m: jlong,
    wait: jboolean,
) {
    run(&mut env, (), |env| {
        let mut exc = Exc::new();
        let st = unsafe { rorb_poa_manager_hold_requests(ptr(m), wait as i32, exc.ptr()) };
        exc.check(env, st)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_pmDiscardRequests(
    mut env: JNIEnv,
    _: JClass,
    m: jlong,
    wait: jboolean,
) {
    run(&mut env, (), |env| {
        let mut exc = Exc::new();
        let st = unsafe { rorb_poa_manager_discard_requests(ptr(m), wait as i32, exc.ptr()) };
        exc.check(env, st)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_pmDeactivate(
    mut env: JNIEnv,
    _: JClass,
    m: jlong,
    etherealize: jboolean,
    wait: jboolean,
) {
    run(&mut env, (), |env| {
        let mut exc = Exc::new();
        let st = unsafe {
            rorb_poa_manager_deactivate(ptr(m), etherealize as i32, wait as i32, exc.ptr())
        };
        exc.check(env, st)
    })
}
