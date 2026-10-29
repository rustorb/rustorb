//! `rorb_out` and `rorb_in`: the CDR streams given to callbacks.
//!
//! Both keep the first error and ignore the calls after it (writes do
//! nothing, reads return zeros), so that generated code can marshal a
//! whole value and check once, when the callback returns.

use std::ffi::{c_char, c_void};

use rustorb_core::{
    CompletionStatus, InStream, ObjectRef, OutStream, ServerReply, SystemException,
    SystemExceptionKind,
};

use crate::{into_handle, str_arg, sys};

// --- output ---------------------------------------------------------------------

#[derive(Debug)]
enum Target<'a, 'b> {
    Stream(&'a mut OutStream<'b>),
    /// The result stream of a server reply, taken again for each write
    /// (each one continues where the last stopped).
    Reply(*mut ServerReply),
}

/// An output stream for C.
#[derive(Debug)]
pub struct Out<'a, 'b> {
    target: Target<'a, 'b>,
    pub(crate) error: Option<SystemException>,
    /// The object references written, kept only by a standalone stream
    /// (`rorb_any_insert` reads them back with their ORB).
    pub(crate) objects: Vec<ObjectRef>,
    standalone: bool,
}

/// What a `rorb_out*` points to; the lifetimes are those of the callback.
pub type RorbOut = Out<'static, 'static>;

impl<'a, 'b> Out<'a, 'b> {
    pub(crate) fn new(stream: &'a mut OutStream<'b>) -> Self {
        Out {
            target: Target::Stream(stream),
            error: None,
            objects: Vec::new(),
            standalone: false,
        }
    }

    /// A stream that keeps the object references written.
    pub(crate) fn standalone(stream: &'a mut OutStream<'b>) -> Self {
        Out {
            standalone: true,
            ..Out::new(stream)
        }
    }

    pub(crate) fn reply(reply: *mut ServerReply) -> Self {
        Out {
            target: Target::Reply(reply),
            error: None,
            objects: Vec::new(),
            standalone: false,
        }
    }

    /// The server reply of a stream made by [`Out::reply`].
    pub(crate) fn server_reply(&self) -> *mut ServerReply {
        match self.target {
            Target::Reply(r) => r,
            Target::Stream(_) => unreachable!("not the stream of a server reply"),
        }
    }

    /// The pointer given to C, valid while `self` is borrowed.
    pub(crate) fn as_ptr(&mut self) -> *mut RorbOut {
        (self as *mut Self).cast()
    }

    pub(crate) fn with(
        &mut self,
        f: impl FnOnce(&mut OutStream<'_>) -> Result<(), SystemException>,
    ) {
        if self.error.is_some() {
            return;
        }
        let result = match &mut self.target {
            Target::Stream(s) => f(s),
            Target::Reply(r) => {
                let mut out = unsafe { &mut **r }.output();
                f(&mut out)
            }
        };
        if let Err(e) = result {
            self.error = Some(e);
        }
    }
}

macro_rules! write_fns {
    ($($name:ident($ty:ty) => $method:ident;)*) => {
        $(
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $name(out: *mut RorbOut, v: $ty) {
                unsafe { &mut *out }.with(|s| {
                    s.$method(v);
                    Ok(())
                });
            }
        )*
    };
}

write_fns! {
    rorb_out_write_octet(u8) => write_octet;
    rorb_out_write_short(i16) => write_i16;
    rorb_out_write_ushort(u16) => write_u16;
    rorb_out_write_long(i32) => write_i32;
    rorb_out_write_ulong(u32) => write_u32;
    rorb_out_write_longlong(i64) => write_i64;
    rorb_out_write_ulonglong(u64) => write_u64;
    rorb_out_write_float(f32) => write_f32;
    rorb_out_write_double(f64) => write_f64;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_write_boolean(out: *mut RorbOut, v: i32) {
    unsafe { &mut *out }.with(|s| {
        s.write_bool(v != 0);
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_write_char(out: *mut RorbOut, v: c_char) {
    unsafe { &mut *out }.with(|s| {
        s.write_char(v as u8);
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_write_wchar(out: *mut RorbOut, v: u16) {
    unsafe { &mut *out }.with(|s| s.write_wchar(v));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_write_string(out: *mut RorbOut, p: *const c_char, len: usize) {
    unsafe { &mut *out }.with(|s| {
        let text = unsafe { str_arg(p, len) }?;
        s.write_string(text)
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_write_wstring(out: *mut RorbOut, p: *const u16, len: usize) {
    unsafe { &mut *out }.with(|s| {
        let units = if p.is_null() || len == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(p, len) }
        };
        let text = String::from_utf16(units).map_err(|_| {
            sys(
                SystemExceptionKind::DATA_CONVERSION,
                0,
                CompletionStatus::No,
            )
        })?;
        s.write_wstring(&text)
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_write_object(out: *mut RorbOut, obj: *mut ObjectRef) {
    let out = unsafe { &mut *out };
    let obj = unsafe { obj.as_ref() };
    if out.standalone && out.error.is_none() {
        out.objects.extend(obj.cloned());
    }
    out.with(|s| s.write_object(obj));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_write_fixed(
    out: *mut RorbOut,
    digits: *const u8,
    count: usize,
    negative: i32,
) {
    unsafe { &mut *out }
        .with(|s| s.write_fixed(unsafe { slice::<u8>(digits.cast(), count) }, negative != 0));
}

/// Element kinds of the array functions (`RORB_PRIM_*`).
mod prim {
    pub const OCTET: i32 = 0;
    pub const SHORT: i32 = 1;
    pub const USHORT: i32 = 2;
    pub const LONG: i32 = 3;
    pub const ULONG: i32 = 4;
    pub const LONGLONG: i32 = 5;
    pub const ULONGLONG: i32 = 6;
    pub const FLOAT: i32 = 7;
    pub const DOUBLE: i32 = 8;
}

unsafe fn slice<'s, T>(data: *const c_void, count: usize) -> &'s [T] {
    if data.is_null() || count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data as *const T, count) }
    }
}

unsafe fn slice_mut<'s, T>(data: *mut c_void, count: usize) -> &'s mut [T] {
    if data.is_null() || count == 0 {
        &mut []
    } else {
        unsafe { std::slice::from_raw_parts_mut(data as *mut T, count) }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_write_array(
    out: *mut RorbOut,
    kind: i32,
    data: *const c_void,
    count: usize,
) {
    unsafe { &mut *out }.with(|s| {
        macro_rules! each {
            ($ty:ty, $method:ident) => {
                for v in unsafe { slice::<$ty>(data, count) } {
                    s.$method(*v);
                }
            };
        }
        match kind {
            prim::OCTET => s.write_octets(unsafe { slice::<u8>(data, count) }),
            prim::SHORT => each!(i16, write_i16),
            prim::USHORT => each!(u16, write_u16),
            prim::LONG => each!(i32, write_i32),
            prim::ULONG => each!(u32, write_u32),
            prim::LONGLONG => each!(i64, write_i64),
            prim::ULONGLONG => each!(u64, write_u64),
            prim::FLOAT => each!(f32, write_f32),
            prim::DOUBLE => each!(f64, write_f64),
            _ => return Err(crate::bad_param()),
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_failed(out: *mut RorbOut) -> i32 {
    i32::from(unsafe { &*out }.error.is_some())
}

// --- input ----------------------------------------------------------------------

/// An input stream for C.
#[derive(Debug)]
pub struct In<'a, 'b> {
    stream: &'a mut InStream<'b>,
    completed: CompletionStatus,
    pub(crate) error: Option<SystemException>,
    /// The last string read, until the next call.
    text: String,
    wide: Vec<u16>,
}

/// What a `rorb_in*` points to; the lifetimes are those of the callback.
pub type RorbIn = In<'static, 'static>;

impl<'a, 'b> In<'a, 'b> {
    /// A stream whose errors are `MARSHAL` with `completed`.
    pub(crate) fn new(stream: &'a mut InStream<'b>, completed: CompletionStatus) -> Self {
        In {
            stream,
            completed,
            error: None,
            text: String::new(),
            wide: Vec::new(),
        }
    }

    pub(crate) fn as_ptr(&mut self) -> *mut RorbIn {
        (self as *mut Self).cast()
    }

    fn marshal(&self) -> SystemException {
        sys(SystemExceptionKind::MARSHAL, 0, self.completed)
    }

    pub(crate) fn read<T: Default>(
        &mut self,
        f: impl FnOnce(&mut InStream<'b>) -> Result<T, SystemException>,
    ) -> T {
        if self.error.is_some() {
            return T::default();
        }
        match f(self.stream) {
            Ok(v) => v,
            Err(e) => {
                self.error = Some(e);
                T::default()
            }
        }
    }
}

macro_rules! read_fns {
    ($($name:ident -> $ty:ty;)*) => {
        $(
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $name(input: *mut RorbIn) -> $ty {
                unsafe { &mut *input }.read(|s| s.read::<$ty>())
            }
        )*
    };
}

read_fns! {
    rorb_in_read_short -> i16;
    rorb_in_read_ushort -> u16;
    rorb_in_read_long -> i32;
    rorb_in_read_ulong -> u32;
    rorb_in_read_longlong -> i64;
    rorb_in_read_ulonglong -> u64;
    rorb_in_read_float -> f32;
    rorb_in_read_double -> f64;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_octet(input: *mut RorbIn) -> u8 {
    let input = unsafe { &mut *input };
    let e = input.marshal();
    input.read(|s| s.read_octet().map_err(|_| e))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_boolean(input: *mut RorbIn) -> i32 {
    i32::from(unsafe { &mut *input }.read(|s| s.read::<bool>()))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_char(input: *mut RorbIn) -> c_char {
    let input = unsafe { &mut *input };
    let e = input.marshal();
    input.read(|s| s.read_char().map_err(|_| e)) as c_char
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_wchar(input: *mut RorbIn) -> u16 {
    unsafe { &mut *input }.read(|s| s.read_wchar())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_string(input: *mut RorbIn, len: *mut usize) -> *const c_char {
    let input = unsafe { &mut *input };
    input.text = input.read(|s| s.read_string());
    unsafe { *len = input.text.len() };
    input.text.as_ptr() as *const c_char
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_wstring(input: *mut RorbIn, len: *mut usize) -> *const u16 {
    let input = unsafe { &mut *input };
    let text = input.read(|s| s.read_wstring());
    input.wide.clear();
    input.wide.extend(text.encode_utf16());
    unsafe { *len = input.wide.len() };
    input.wide.as_ptr()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_object(input: *mut RorbIn) -> *mut ObjectRef {
    match unsafe { &mut *input }.read(|s| s.read_object()) {
        Some(obj) => into_handle(obj),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_array(
    input: *mut RorbIn,
    kind: i32,
    data: *mut c_void,
    count: usize,
) {
    let input = unsafe { &mut *input };
    let e = input.marshal();
    input.read(|s| {
        macro_rules! each {
            ($ty:ty) => {
                for v in unsafe { slice_mut::<$ty>(data, count) } {
                    *v = s.read::<$ty>()?;
                }
            };
        }
        match kind {
            prim::OCTET => {
                let bytes = s.read_octets(count).map_err(|_| e)?;
                unsafe { slice_mut::<u8>(data, count) }.copy_from_slice(bytes);
            }
            prim::SHORT => each!(i16),
            prim::USHORT => each!(u16),
            prim::LONG => each!(i32),
            prim::ULONG => each!(u32),
            prim::LONGLONG => each!(i64),
            prim::ULONGLONG => each!(u64),
            prim::FLOAT => each!(f32),
            prim::DOUBLE => each!(f64),
            _ => return Err(crate::bad_param()),
        }
        Ok(())
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_fixed(
    input: *mut RorbIn,
    digits: *mut u8,
    count: usize,
) -> i32 {
    let input = unsafe { &mut *input };
    let (read, negative) = input.read(|s| s.read_fixed(count));
    let out = unsafe { slice_mut::<u8>(digits.cast(), count) };
    if read.len() == count {
        out.copy_from_slice(&read);
    } else {
        out.fill(0);
    }
    i32::from(negative)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_length(input: *mut RorbIn, min_element_size: usize) -> u32 {
    let input = unsafe { &mut *input };
    let e = input.marshal();
    input.read(|s| {
        s.read_len(min_element_size.max(1))
            .map(|n| n as u32)
            .map_err(|_| e)
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_fail(input: *mut RorbIn) {
    let input = unsafe { &mut *input };
    if input.error.is_none() {
        input.error = Some(input.marshal());
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_failed(input: *mut RorbIn) -> i32 {
    i32::from(unsafe { &*input }.error.is_some())
}
