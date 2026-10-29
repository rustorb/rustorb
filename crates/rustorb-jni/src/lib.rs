//! JNI library of the rustorb Java binding (`librustorb_jni`).
//!
//! The native methods of `org.rustorb.Native` (java/src/main/java), each a
//! thin wrapper over one function of the C ABI of `rustorb-ffi` (declared
//! in [`sys`] as `include/rustorb/rustorb.h` does). Handles cross into Java
//! as `long`s; the Java classes that hold them release them.
//!
//! - Errors: a failed C call throws the Java exception that matches its
//!   `rorb_exception` (made by `org.rustorb.Bridge`), and the native method
//!   returns a dummy value.
//! - Callbacks (writing the arguments, reading the reply, servant dispatch)
//!   call static methods of `org.rustorb.Bridge`, which give Java the
//!   stream as a `long` and keep any Java exception for the caller.
//! - Servants: each Java servant has a peer (see [`servant`]) whose address
//!   is the `self` of the C ABI. Servant managers and adapter activators
//!   are held by a counted global reference (see [`manager`]).
//!
//! Threads of the ORB's upcall pool attach to the JVM as daemons the first
//! time they call Java, and detach when they exit.

// Every native method takes handles from Java; what they must be is the
// contract of org.rustorb.Native.
#![allow(clippy::missing_safety_doc, clippy::not_unsafe_ptr_arg_deref)]

mod any;
mod manager;
mod object;
mod orb;
mod poa;
mod servant;
mod stream;
mod sys;
mod value;

use std::ffi::{c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;

use jni::objects::{GlobalRef, JClass, JObject, JStaticMethodID, JString, JValueOwned};
use jni::signature::{Primitive, ReturnType};
use jni::sys::{JNI_VERSION_1_8, jint, jlong, jvalue};
use jni::{JNIEnv, JavaVM};

use sys::*;

// rustorb-ffi provides the functions of sys.
extern crate rustorb_ffi;

/// A Java exception is pending; the native method returns to throw it.
#[derive(Debug)]
pub(crate) struct Thrown;

impl From<jni::errors::Error> for Thrown {
    fn from(_: jni::errors::Error) -> Self {
        Thrown
    }
}

pub(crate) type JResult<T> = Result<T, Thrown>;

/// The class org.rustorb.Bridge and the methods the library calls.
struct Cache {
    bridge: GlobalRef,
    system_exception: JStaticMethodID,
    local_exception: JStaticMethodID,
    call_write: JStaticMethodID,
    call_read: JStaticMethodID,
    call_read_exception: JStaticMethodID,
    dispatch: JStaticMethodID,
    is_a: JStaticMethodID,
    incarnate: JStaticMethodID,
    etherealize: JStaticMethodID,
    preinvoke: JStaticMethodID,
    postinvoke: JStaticMethodID,
    unknown_adapter: JStaticMethodID,
    resolve_value_type: JStaticMethodID,
}

static VM: OnceLock<JavaVM> = OnceLock::new();
static CACHE: OnceLock<Cache> = OnceLock::new();

#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _reserved: *mut c_void) -> jint {
    let _ = VM.set(vm);
    JNI_VERSION_1_8
}

fn cache() -> &'static Cache {
    CACHE.get().expect("org.rustorb.Native.init was not called")
}

fn bridge_class() -> &'static JClass<'static> {
    <&JClass>::from(cache().bridge.as_obj())
}

/// `Native.init(Class bridge)`: called once, when Native is loaded.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_rustorb_Native_init(
    mut env: JNIEnv,
    _: JClass,
    bridge: JClass,
) -> jint {
    run(&mut env, 0, |env| {
        let id =
            |env: &mut JNIEnv, name: &str, sig: &str| env.get_static_method_id(&bridge, name, sig);
        let c = Cache {
            system_exception: id(
                env,
                "systemException",
                "(Ljava/lang/String;II)Ljava/lang/RuntimeException;",
            )?,
            local_exception: id(env, "localException", "(II)Ljava/lang/Exception;")?,
            call_write: id(env, "callWrite", "(Lorg/rustorb/Call;J)Z")?,
            call_read: id(env, "callRead", "(Lorg/rustorb/Call;J)Z")?,
            call_read_exception: id(
                env,
                "callReadException",
                "(Lorg/rustorb/Call;Ljava/lang/String;J)I",
            )?,
            dispatch: id(env, "dispatch", "(Ljava/lang/Object;Ljava/lang/String;JJ)I")?,
            is_a: id(env, "isA", "(Ljava/lang/Object;Ljava/lang/String;)Z")?,
            incarnate: id(env, "incarnate", "(Ljava/lang/Object;[BJJ)J")?,
            etherealize: id(
                env,
                "etherealize",
                "(Ljava/lang/Object;[BJLjava/lang/Object;ZZ)V",
            )?,
            preinvoke: id(
                env,
                "preinvoke",
                "(Ljava/lang/Object;[BJLjava/lang/String;[Ljava/lang/Object;J)J",
            )?,
            postinvoke: id(
                env,
                "postinvoke",
                "(Ljava/lang/Object;[BJLjava/lang/String;Ljava/lang/Object;Ljava/lang/Object;)V",
            )?,
            unknown_adapter: id(
                env,
                "unknownAdapter",
                "(Ljava/lang/Object;JLjava/lang/String;)Z",
            )?,
            resolve_value_type: id(env, "resolveValueType", "(Ljava/lang/String;)J")?,
            bridge: env.new_global_ref(&bridge)?,
        };
        let _ = CACHE.set(c);
        value::install_resolver();
        Ok(unsafe { rorb_abi_version() } as jint)
    })
}

/// Runs the body of a native method: a pending Java exception or a panic
/// (thrown as `INTERNAL`) returns `default`.
pub(crate) fn run<'local, T>(
    env: &mut JNIEnv<'local>,
    default: T,
    f: impl FnOnce(&mut JNIEnv<'local>) -> JResult<T>,
) -> T {
    match catch_unwind(AssertUnwindSafe(|| f(env))) {
        Ok(Ok(v)) => v,
        Ok(Err(Thrown)) => default,
        Err(_) => {
            if !env.exception_check().unwrap_or(true) {
                let _ = throw_system::<()>(env, "IDL:omg.org/CORBA/INTERNAL:1.0", 0, 2);
            }
            default
        }
    }
}

/// Calls a static method of Bridge.
pub(crate) fn call_bridge<'local>(
    env: &mut JNIEnv<'local>,
    method: JStaticMethodID,
    ret: ReturnType,
    args: &[jvalue],
) -> JResult<JValueOwned<'local>> {
    Ok(unsafe { env.call_static_method_unchecked(bridge_class(), method, ret, args) }?)
}

pub(crate) fn throw_system<T>(
    env: &mut JNIEnv,
    id: &str,
    minor: u32,
    completed: u32,
) -> JResult<T> {
    let id = env.new_string(id)?;
    let e = call_bridge(
        env,
        cache().system_exception,
        ReturnType::Object,
        &[
            jvalue { l: id.as_raw() },
            jvalue { i: minor as jint },
            jvalue {
                i: completed as jint,
            },
        ],
    )?
    .l()?;
    env.throw(jni::objects::JThrowable::from(e))?;
    Err(Thrown)
}

fn throw_local<T>(env: &mut JNIEnv, code: u32, index: u32) -> JResult<T> {
    let e = call_bridge(
        env,
        cache().local_exception,
        ReturnType::Object,
        &[jvalue { i: code as jint }, jvalue { i: index as jint }],
    )?
    .l()?;
    env.throw(jni::objects::JThrowable::from(e))?;
    Err(Thrown)
}

/// An `rorb_exception` for one call, cleared when dropped.
pub(crate) struct Exc(pub rorb_exception);

impl Exc {
    pub(crate) fn new() -> Exc {
        Exc(rorb_exception::default())
    }

    pub(crate) fn ptr(&mut self) -> *mut rorb_exception {
        &mut self.0
    }

    /// Ok for `RORB_OK`; otherwise throws the exception the status names
    /// (nothing for `RORB_CALLBACK_FAILED`, whose error Java keeps).
    pub(crate) fn check(&mut self, env: &mut JNIEnv, status: rorb_status) -> JResult<()> {
        match status {
            RORB_OK => Ok(()),
            RORB_SYSTEM_EXCEPTION => {
                let id = if self.0.repository_id.is_null() {
                    String::from("IDL:omg.org/CORBA/UNKNOWN:1.0")
                } else {
                    let bytes = unsafe {
                        std::slice::from_raw_parts(
                            self.0.repository_id as *const u8,
                            self.0.repository_id_len,
                        )
                    };
                    String::from_utf8_lossy(bytes).into_owned()
                };
                throw_system(env, &id, self.0.minor, self.0.completed)
            }
            RORB_LOCAL_EXCEPTION => throw_local(env, self.0.local, self.0.index),
            _ => Err(Thrown),
        }
    }
}

impl Drop for Exc {
    fn drop(&mut self) {
        unsafe { rorb_exception_clear(&mut self.0) };
    }
}

/// A Java string as UTF-8.
pub(crate) fn rust_string(env: &mut JNIEnv, s: &JString) -> JResult<String> {
    if s.is_null() {
        return throw_system(env, "IDL:omg.org/CORBA/BAD_PARAM:1.0", 0, 1);
    }
    Ok(env.get_string(s)?.into())
}

/// A string the C ABI returned (pointer and length) as a Java string.
pub(crate) fn java_string<'local>(
    env: &mut JNIEnv<'local>,
    p: *const c_char,
    len: usize,
) -> JResult<JString<'local>> {
    let bytes = if p.is_null() || len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(p as *const u8, len) }
    };
    Ok(env.new_string(String::from_utf8_lossy(bytes))?)
}

/// The bytes of an `rorb_bytes` as a Java byte array; frees them.
pub(crate) fn take_bytes<'local>(
    env: &mut JNIEnv<'local>,
    b: rorb_bytes,
) -> JResult<jni::objects::JByteArray<'local>> {
    let slice = if b.data.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(b.data, b.len) }
    };
    let array = env.byte_array_from_slice(slice);
    unsafe { rorb_bytes_free(b) };
    Ok(array?)
}

/// A Java object array of strings.
pub(crate) fn string_array<'local>(
    env: &mut JNIEnv<'local>,
    items: &[String],
) -> JResult<jni::objects::JObjectArray<'local>> {
    let array = env.new_object_array(items.len() as jint, "java/lang/String", JObject::null())?;
    for (i, s) in items.iter().enumerate() {
        let js = env.new_string(s)?;
        env.set_object_array_element(&array, i as jint, js)?;
    }
    Ok(array)
}

pub(crate) fn handle<T>(p: *mut T) -> jlong {
    p as jlong
}

pub(crate) fn ptr<T>(h: jlong) -> *mut T {
    h as *mut T
}

pub(crate) const BOOLEAN: ReturnType = ReturnType::Primitive(Primitive::Boolean);
pub(crate) const INT: ReturnType = ReturnType::Primitive(Primitive::Int);
pub(crate) const LONG: ReturnType = ReturnType::Primitive(Primitive::Long);
pub(crate) const VOID: ReturnType = ReturnType::Primitive(Primitive::Void);
