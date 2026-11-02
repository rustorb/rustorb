//! Input streams the caller owns, valid until `rorb_in_release`: the reply
//! of `rorb_object_invoke_stream`, what a callback writes
//! (`rorb_in_new`) and the value of an any (`rorb_any_input`). They are
//! the streams of the portable stubs and of the streams Java's
//! `create_input_stream` returns.

use std::ffi::{c_char, c_void};
use std::mem::ManuallyDrop;

use rustorb_cdr::{ByteOrder, Decoder, Encoding};
use rustorb_core::{
    Any, InStream, ObjectRef, Orb, OutStream, Reply, SystemException, UserException,
};

use crate::any::{default_orb, encoder, first_object};
use crate::object::{Invocation, Sent, send};
use crate::stream::{In, Out, RorbIn, RorbOut};
use crate::{CALLBACK_FAILED, Exception, OK, Status, USER_EXCEPTION, guard, set_system};

/// What an owned stream reads.
enum Source {
    /// Bytes of a standalone stream, with the ORB of the object references
    /// read from them.
    Bytes {
        data: Vec<u8>,
        orb: Option<Orb>,
    },
    Reply(Reply),
    User(UserException),
}

/// An owned `rorb_in`: a pointer to it is one to its [`In`].
#[repr(C)]
struct OwnedIn {
    input: ManuallyDrop<In<'static, 'static>>,
    /// Borrowed by `input`.
    stream: *mut InStream<'static>,
    /// Borrowed by `stream`.
    source: *mut Source,
    /// The repository id of the user exception of a reply.
    exception_id: Option<String>,
}

impl Drop for OwnedIn {
    fn drop(&mut self) {
        // In the reverse order of the borrows.
        unsafe {
            ManuallyDrop::drop(&mut self.input);
            drop(Box::from_raw(self.stream));
            drop(Box::from_raw(self.source));
        }
    }
}

/// A new owned stream over `source`.
fn make(source: Source, exception_id: Option<String>) -> Result<*mut RorbIn, SystemException> {
    let source = Box::into_raw(Box::new(source));
    // The source stays where it is until the stream is released.
    let src: &'static Source = unsafe { &*source };
    let stream = match src {
        Source::Bytes { data, orb } => Ok(InStream::standalone(
            Decoder::new(data, ByteOrder::Big, Encoding::Giop1_2),
            orb.as_ref(),
        )),
        Source::Reply(r) => Ok(r.input()),
        Source::User(u) => u.input(),
    };
    let stream = match stream {
        Ok(s) => s,
        Err(e) => {
            drop(unsafe { Box::from_raw(source) });
            return Err(e);
        }
    };
    let completed = stream.completion_status();
    let stream = Box::into_raw(Box::new(stream));
    let mut input = In::new(unsafe { &mut *stream }, completed);
    input.owned = true;
    let owned = Box::new(OwnedIn {
        input: ManuallyDrop::new(input),
        stream,
        source,
        exception_id,
    });
    Ok(Box::into_raw(owned).cast())
}

unsafe fn put(
    result: Result<*mut RorbIn, SystemException>,
    out: *mut *mut RorbIn,
    exc: *mut Exception,
    status: Status,
) -> Status {
    match result {
        Ok(p) => {
            unsafe { *out = p };
            status
        }
        Err(e) => unsafe { set_system(exc, &e) },
    }
}

/// The owned stream of `input`, if it is one.
unsafe fn owned<'a>(input: *mut RorbIn) -> Option<&'a mut OwnedIn> {
    let input = unsafe { input.as_mut() }?;
    input
        .owned
        .then(|| unsafe { &mut *(input as *mut RorbIn).cast::<OwnedIn>() })
}

/// Releases an owned stream; other streams are left alone.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_release(input: *mut RorbIn) {
    if let Some(o) = unsafe { owned(input) } {
        drop(unsafe { Box::from_raw(o as *mut OwnedIn) });
    }
}

/// The repository id of the user exception an owned reply stream holds;
/// NULL for a normal reply.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_exception_id(
    input: *mut RorbIn,
    len: *mut usize,
) -> *const c_char {
    match unsafe { owned(input) }.and_then(|o| o.exception_id.as_deref()) {
        Some(id) => {
            unsafe { *len = id.len() };
            id.as_ptr() as *const c_char
        }
        None => {
            unsafe { *len = 0 };
            std::ptr::null()
        }
    }
}

/// The error of a stream: RORB_OK, or RORB_SYSTEM_EXCEPTION with it in
/// `exc`. The error stays.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_error(input: *mut RorbIn, exc: *mut Exception) -> Status {
    match &unsafe { &*input }.error {
        Some(e) => unsafe { set_system(exc, e) },
        None => OK,
    }
}

type WriteFn = Option<extern "C" fn(*mut c_void, *mut RorbOut) -> Status>;

/// An owned stream over what `write` writes on a standalone stream (the
/// native code sets). Object references read from it belong to `orb`, else
/// to the ORB of the first one written, else to the default ORB.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_new(
    write: WriteFn,
    context: *mut c_void,
    orb: *mut Orb,
    input: *mut *mut RorbIn,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let mut enc = encoder();
        let written: Option<Orb>;
        {
            let mut stream = OutStream::standalone(&mut enc);
            let mut out = Out::standalone(&mut stream);
            if let Some(write) = write {
                if write(context, out.as_ptr()) != OK {
                    return CALLBACK_FAILED;
                }
            }
            if let Some(e) = out.error.take() {
                return unsafe { set_system(exc, &e) };
            }
            written = out.objects.first().map(|o| o.orb().clone());
        }
        let orb = unsafe { orb.as_ref() }
            .cloned()
            .or(written)
            .or_else(default_orb);
        let source = Source::Bytes {
            data: enc.into_bytes(),
            orb,
        };
        unsafe { put(make(source, None), input, exc, OK) }
    })
}

/// An owned stream over the value of `any` (without its TypeCode), as it
/// would be marshalled.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_any_input(
    any: *const Any,
    input: *mut *mut RorbIn,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let any = unsafe { &*any };
        let mut enc = encoder();
        if let Err(e) = OutStream::standalone(&mut enc).write_value(any.typecode(), any.value()) {
            return unsafe { set_system(exc, &e) };
        }
        let orb = first_object(any.value())
            .map(|o| o.orb().clone())
            .or_else(default_orb);
        let source = Source::Bytes {
            data: enc.into_bytes(),
            orb,
        };
        unsafe { put(make(source, None), input, exc, OK) }
    })
}

/// Invokes like `rorb_object_invoke`, with the arguments written by
/// `write_args` (`read_reply` and `read_user_exception` are not used), and
/// gives the reply as an owned stream: RORB_OK with the reply (NULL for a
/// oneway call), or RORB_USER_EXCEPTION with the members of the exception
/// (`rorb_in_exception_id` names it).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_object_invoke_stream(
    obj: *mut ObjectRef,
    inv: *const Invocation,
    reply: *mut *mut RorbIn,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let obj = unsafe { &*obj };
        let inv = unsafe { &*inv };
        unsafe { *reply = std::ptr::null_mut() };
        match unsafe { send(obj, inv, exc) } {
            Err(status) => status,
            Ok(Sent::Oneway) => OK,
            Ok(Sent::Reply(r)) => unsafe { put(make(Source::Reply(r), None), reply, exc, OK) },
            Ok(Sent::User(u)) => {
                let id = u.repository_id().to_owned();
                unsafe { put(make(Source::User(u), Some(id)), reply, exc, USER_EXCEPTION) }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{
        rorb_in_read_long, rorb_in_read_string, rorb_out_write_long, rorb_out_write_string,
    };
    use rustorb_core::{TCKind, TypeCode, Value};

    extern "C" fn write(_: *mut c_void, out: *mut RorbOut) -> Status {
        unsafe {
            rorb_out_write_long(out, 7);
            rorb_out_write_string(out, "seven".as_ptr() as *const c_char, 5);
        }
        OK
    }

    fn exc() -> Exception {
        Exception {
            repository_id: std::ptr::null_mut(),
            repository_id_len: 0,
            minor: 0,
            completed: 0,
            local: 0,
            index: 0,
        }
    }

    unsafe fn string(input: *mut RorbIn) -> String {
        let mut len = 0;
        let p = unsafe { rorb_in_read_string(input, &mut len) };
        let bytes = unsafe { std::slice::from_raw_parts(p as *const u8, len) };
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[test]
    fn reads_what_a_callback_wrote_after_it_returned() {
        let mut e = exc();
        let mut input = std::ptr::null_mut();
        let st = unsafe {
            rorb_in_new(
                Some(write),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut input,
                &mut e,
            )
        };
        assert_eq!(st, OK);
        unsafe {
            assert_eq!(rorb_in_read_long(input), 7);
            assert_eq!(string(input), "seven");
            assert_eq!(rorb_in_error(input, &mut e), OK);
            let mut len = 1;
            assert!(rorb_in_exception_id(input, &mut len).is_null());
            assert_eq!(len, 0);
            // Past the end: MARSHAL, completed no, and it stays.
            assert_eq!(rorb_in_read_long(input), 0);
            assert_eq!(rorb_in_error(input, &mut e), crate::SYSTEM_EXCEPTION);
            assert_eq!(e.completed, 1);
            crate::rorb_exception_clear(&mut e);
            rorb_in_release(input);
        }
    }

    #[test]
    fn reads_the_value_of_an_any() {
        let any = Any::new(TypeCode::simple(TCKind::Long).unwrap(), Value::Long(-3));
        let mut e = exc();
        let mut input = std::ptr::null_mut();
        assert_eq!(unsafe { rorb_any_input(&any, &mut input, &mut e) }, OK);
        unsafe {
            assert_eq!(rorb_in_read_long(input), -3);
            rorb_in_release(input);
        }
    }
}
