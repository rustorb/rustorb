//! `rorb_typecode` and `rorb_any`: TypeCodes and anys.
//!
//! A TypeCode handle is a core [`TypeCode`], an any handle a core [`Any`]
//! (a TypeCode and a [`rustorb_core::Value`]). The caller puts its own
//! values in an any and takes them out through callbacks that write and
//! read them on a standalone CDR stream, as it marshals arguments: the
//! value is converted to and from the core representation on the way.

use std::ffi::{c_char, c_void};

use rustorb_cdr::{ByteOrder, CdrVersion, Decoder, Encoder};
use rustorb_core::{
    Any, CompletionStatus, InStream, ObjectRef, Orb, OutStream, SystemExceptionKind, TCKind,
    TypeCode, TypeCodeError,
};

use crate::stream::{In, Out, RorbIn, RorbOut};
use crate::{
    CALLBACK_FAILED, Exception, OK, Status, bad_param, guard, into_handle, local, release_handle,
    set_local, set_system, str_arg, sys,
};

// --- TypeCodes ------------------------------------------------------------------

unsafe fn tc_error(exc: *mut Exception, e: TypeCodeError) -> Status {
    match e {
        TypeCodeError::BadKind => unsafe { set_local(exc, local::TC_BAD_KIND, 0) },
        TypeCodeError::Bounds => unsafe { set_local(exc, local::TC_BOUNDS, 0) },
        TypeCodeError::BadParam => unsafe { set_system(exc, &bad_param()) },
    }
}

/// Stores the result of a TypeCode operation in `*out`.
unsafe fn put<T>(
    exc: *mut Exception,
    out: *mut T,
    f: impl FnOnce() -> Result<T, TypeCodeError>,
) -> Status {
    guard(exc, || match f() {
        Ok(v) => {
            unsafe { out.write(v) };
            OK
        }
        Err(e) => unsafe { tc_error(exc, e) },
    })
}

/// Stores a new TypeCode handle in `*out`.
unsafe fn put_tc(
    exc: *mut Exception,
    out: *mut *mut TypeCode,
    f: impl FnOnce() -> Result<TypeCode, TypeCodeError>,
) -> Status {
    unsafe { put(exc, out, || f().map(into_handle)) }
}

/// A string argument of a TypeCode constructor; invalid UTF-8 is
/// `BadParam`.
unsafe fn name_arg<'a>(p: *const c_char, len: usize) -> Result<&'a str, TypeCodeError> {
    unsafe { str_arg(p, len) }.map_err(|_| TypeCodeError::BadParam)
}

unsafe fn tc_arg<'a>(tc: *const TypeCode) -> Result<&'a TypeCode, TypeCodeError> {
    unsafe { tc.as_ref() }.ok_or(TypeCodeError::BadParam)
}

/// `count` names and types from C arrays.
unsafe fn members<'a>(
    count: usize,
    names: *const *const c_char,
    name_lens: *const usize,
    types: *const *const TypeCode,
) -> Result<Vec<(&'a str, TypeCode)>, TypeCodeError> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let name = unsafe { name_arg(*names.add(i), *name_lens.add(i)) }?;
        let tc = unsafe { tc_arg(*types.add(i)) }?;
        out.push((name, tc.clone()));
    }
    Ok(out)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_clone(tc: *const TypeCode) -> *mut TypeCode {
    match unsafe { tc.as_ref() } {
        Some(tc) => into_handle(tc.clone()),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_release(tc: *mut TypeCode) {
    unsafe { release_handle(tc) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_simple(
    kind: u32,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put_tc(exc, out, || {
            let kind = TCKind::from_u32(kind).ok_or(TypeCodeError::BadParam)?;
            match kind {
                TCKind::String => Ok(TypeCode::string(0)),
                TCKind::WString => Ok(TypeCode::wstring(0)),
                _ => TypeCode::simple(kind),
            }
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_string(
    wide: i32,
    bound: u32,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put_tc(exc, out, || {
            Ok(if wide != 0 {
                TypeCode::wstring(bound)
            } else {
                TypeCode::string(bound)
            })
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_fixed(
    digits: u16,
    scale: i16,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe { put_tc(exc, out, || TypeCode::fixed(digits, scale)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_sequence(
    content: *const TypeCode,
    bound: u32,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe { put_tc(exc, out, || TypeCode::sequence(tc_arg(content)?, bound)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_array(
    content: *const TypeCode,
    length: u32,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe { put_tc(exc, out, || TypeCode::array(tc_arg(content)?, length)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_alias(
    kind: u32,
    id: *const c_char,
    id_len: usize,
    name: *const c_char,
    name_len: usize,
    content: *const TypeCode,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put_tc(exc, out, || {
            let id = name_arg(id, id_len)?;
            let name = name_arg(name, name_len)?;
            let content = tc_arg(content)?;
            match TCKind::from_u32(kind) {
                Some(TCKind::Alias) => TypeCode::alias(id, name, content),
                Some(TCKind::ValueBox) => TypeCode::value_box(id, name, content),
                _ => Err(TypeCodeError::BadParam),
            }
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_interface(
    kind: u32,
    id: *const c_char,
    id_len: usize,
    name: *const c_char,
    name_len: usize,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put_tc(exc, out, || {
            let kind = TCKind::from_u32(kind).ok_or(TypeCodeError::BadParam)?;
            TypeCode::interface(kind, name_arg(id, id_len)?, name_arg(name, name_len)?)
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_struct(
    kind: u32,
    id: *const c_char,
    id_len: usize,
    name: *const c_char,
    name_len: usize,
    count: usize,
    names: *const *const c_char,
    name_lens: *const usize,
    types: *const *const TypeCode,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put_tc(exc, out, || {
            let id = name_arg(id, id_len)?;
            let name = name_arg(name, name_len)?;
            let members = members(count, names, name_lens, types)?;
            match TCKind::from_u32(kind) {
                Some(TCKind::Struct) => TypeCode::struct_tc(id, name, &members),
                Some(TCKind::Except) => TypeCode::exception(id, name, &members),
                _ => Err(TypeCodeError::BadParam),
            }
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_union(
    id: *const c_char,
    id_len: usize,
    name: *const c_char,
    name_len: usize,
    discriminator: *const TypeCode,
    count: usize,
    labels: *const u64,
    default_index: i32,
    names: *const *const c_char,
    name_lens: *const usize,
    types: *const *const TypeCode,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put_tc(exc, out, || {
            let id = name_arg(id, id_len)?;
            let name = name_arg(name, name_len)?;
            let disc = tc_arg(discriminator)?;
            let unsigned = disc.unaliased().kind() == TCKind::ULongLong;
            let members = members(count, names, name_lens, types)?
                .into_iter()
                .enumerate()
                .map(|(i, (n, tc))| {
                    let bits = *labels.add(i);
                    let label = if i as i32 == default_index {
                        None
                    } else if unsigned {
                        Some(i128::from(bits))
                    } else {
                        Some(i128::from(bits as i64))
                    };
                    (label, n, tc)
                })
                .collect::<Vec<_>>();
            TypeCode::union_tc(id, name, disc, &members)
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_enum(
    id: *const c_char,
    id_len: usize,
    name: *const c_char,
    name_len: usize,
    count: usize,
    names: *const *const c_char,
    name_lens: *const usize,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put_tc(exc, out, || {
            let id = name_arg(id, id_len)?;
            let name = name_arg(name, name_len)?;
            let names = (0..count)
                .map(|i| name_arg(*names.add(i), *name_lens.add(i)))
                .collect::<Result<Vec<_>, _>>()?;
            TypeCode::enum_tc(id, name, &names)
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_recursive(
    id: *const c_char,
    id_len: usize,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe { put_tc(exc, out, || Ok(TypeCode::recursive(name_arg(id, id_len)?))) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_kind(tc: *const TypeCode) -> u32 {
    unsafe { &*tc }.kind().as_u32()
}

/// Stores a string of the TypeCode, valid while it lives.
unsafe fn put_str(
    exc: *mut Exception,
    p: *mut *const c_char,
    len: *mut usize,
    f: impl FnOnce() -> Result<*const str, TypeCodeError>,
) -> Status {
    let mut s: *const str = "";
    let status = unsafe { put(exc, &mut s, f) };
    if status == OK {
        let s = unsafe { &*s };
        unsafe {
            p.write(s.as_ptr() as *const c_char);
            len.write(s.len());
        }
    }
    status
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_id(
    tc: *const TypeCode,
    p: *mut *const c_char,
    len: *mut usize,
    exc: *mut Exception,
) -> Status {
    unsafe { put_str(exc, p, len, || (*tc).id().map(|s| s as *const str)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_name(
    tc: *const TypeCode,
    p: *mut *const c_char,
    len: *mut usize,
    exc: *mut Exception,
) -> Status {
    unsafe { put_str(exc, p, len, || (*tc).name().map(|s| s as *const str)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_member_count(
    tc: *const TypeCode,
    out: *mut u32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || (*tc).member_count()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_member_name(
    tc: *const TypeCode,
    index: u32,
    p: *mut *const c_char,
    len: *mut usize,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put_str(exc, p, len, || {
            (*tc).member_name(index).map(|s| s as *const str)
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_member_type(
    tc: *const TypeCode,
    index: u32,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe { put_tc(exc, out, || (*tc).member_type(index)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_member_label(
    tc: *const TypeCode,
    index: u32,
    out: *mut *mut Any,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || (*tc).member_label(index).map(into_handle)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_discriminator_type(
    tc: *const TypeCode,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe { put_tc(exc, out, || (*tc).discriminator_type()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_default_index(
    tc: *const TypeCode,
    out: *mut i32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || (*tc).default_index()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_length(
    tc: *const TypeCode,
    out: *mut u32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || (*tc).length()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_content_type(
    tc: *const TypeCode,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe { put_tc(exc, out, || (*tc).content_type()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_fixed_digits(
    tc: *const TypeCode,
    out: *mut u16,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || (*tc).fixed_digits()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_fixed_scale(
    tc: *const TypeCode,
    out: *mut i16,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || (*tc).fixed_scale()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_member_visibility(
    tc: *const TypeCode,
    index: u32,
    out: *mut i16,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || (*tc).member_visibility(index)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_type_modifier(
    tc: *const TypeCode,
    out: *mut i16,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || (*tc).type_modifier()) }
}

/// The concrete base type, or NULL for none.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_concrete_base_type(
    tc: *const TypeCode,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put(exc, out, || {
            (*tc)
                .concrete_base_type()
                .map(|b| b.map_or(std::ptr::null_mut(), into_handle))
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_equal(a: *const TypeCode, b: *const TypeCode) -> i32 {
    i32::from(unsafe { (*a).equal(&*b) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_equivalent(a: *const TypeCode, b: *const TypeCode) -> i32 {
    i32::from(unsafe { (*a).equivalent(&*b) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_compact(tc: *const TypeCode) -> *mut TypeCode {
    into_handle(unsafe { &*tc }.compact())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_tc_unaliased(tc: *const TypeCode) -> *mut TypeCode {
    into_handle(unsafe { &*tc }.unaliased())
}

// --- anys -----------------------------------------------------------------------

/// Writes the value of an any (`rorb_any_insert`).
pub type WriteFn = Option<unsafe extern "C" fn(*mut c_void, *mut RorbOut) -> Status>;
/// Reads the value of an any (`rorb_any_extract`).
pub type ReadFn = Option<unsafe extern "C" fn(*mut c_void, *mut RorbIn) -> Status>;

#[unsafe(no_mangle)]
pub extern "C" fn rorb_any_new() -> *mut Any {
    into_handle(Any::default())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_any_clone(any: *const Any) -> *mut Any {
    match unsafe { any.as_ref() } {
        Some(any) => into_handle(any.clone()),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_any_release(any: *mut Any) {
    unsafe { release_handle(any) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_any_type(any: *const Any) -> *mut TypeCode {
    into_handle(unsafe { &*any }.typecode().clone())
}

/// Replaces the TypeCode of an any with an equivalent one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_any_set_type(
    any: *mut Any,
    tc: *const TypeCode,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let any = unsafe { &mut *any };
        let tc = unsafe { &*tc };
        if !any.typecode().equivalent(tc) {
            let e = sys(SystemExceptionKind::BAD_TYPECODE, 0, CompletionStatus::No);
            return unsafe { set_system(exc, &e) };
        }
        let value = std::mem::take(any).into_parts().1;
        *any = Any::new(tc.clone(), value);
        OK
    })
}

/// Whether two anys have equal TypeCodes and the same value (floats
/// compared exactly, object references with `is_equivalent`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_any_equal(a: *const Any, b: *const Any) -> i32 {
    let (a, b) = unsafe { (&*a, &*b) };
    i32::from(a.typecode().equal(b.typecode()) && a.value() == b.value())
}

/// The value of an any of a discriminator type as a union label number
/// (two's complement for negative values); 0 if it is not one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_any_label(any: *const Any, label: *mut u64) -> i32 {
    match unsafe { &*any }.value().label_key() {
        Some(v) => {
            unsafe { label.write(v as u64) };
            1
        }
        None => 0,
    }
}

/// The ORB of the object references read back from a standalone stream:
/// the first one in a value, else the first ORB initialised.
fn default_orb() -> Option<Orb> {
    crate::orb::default_orb()
}

fn encoder() -> Encoder {
    Encoder::new(ByteOrder::Big, CdrVersion::Giop1_2)
}

/// Whether values of type `tc` start with a repository id when marshalled
/// in an any (exceptions), which the caller's codecs do not write.
fn exception_id(tc: &TypeCode) -> Option<String> {
    let tc = tc.unaliased();
    (tc.kind() == TCKind::Except).then(|| tc.id().unwrap_or_default().to_owned())
}

/// Makes an any of type `tc` from the value `write` writes, as it would
/// marshal it as an argument.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_any_insert(
    tc: *const TypeCode,
    write: WriteFn,
    context: *mut c_void,
    out: *mut *mut Any,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let tc = unsafe { &*tc };
        let mut enc = encoder();
        let mut orb = None;
        {
            let mut stream = OutStream::standalone(&mut enc);
            if let Some(id) = exception_id(tc) {
                if let Err(e) = stream.write_string(&id) {
                    return unsafe { set_system(exc, &e) };
                }
            }
            let mut out = Out::standalone(&mut stream);
            if let Some(write) = write {
                let status = unsafe { write(context, out.as_ptr()) };
                if status != OK {
                    return CALLBACK_FAILED;
                }
            }
            if let Some(e) = out.error.take() {
                return unsafe { set_system(exc, &e) };
            }
            if let Some(o) = out.objects.first() {
                orb = Some(o.orb().clone());
            }
        }
        let orb = orb.or_else(default_orb);
        let bytes = enc.into_bytes();
        let mut input = InStream::standalone(
            Decoder::new(&bytes, ByteOrder::Big, CdrVersion::Giop1_2),
            orb.as_ref(),
        );
        match input.read_value(tc) {
            Ok(v) => {
                unsafe { out.write(into_handle(Any::new(tc.clone(), v))) };
                OK
            }
            Err(e) => unsafe { set_system(exc, &e) },
        }
    })
}

/// Reads the value of an any with `read`, as it would read an argument
/// of its type. The caller has checked the type.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_any_extract(
    any: *const Any,
    read: ReadFn,
    context: *mut c_void,
    exc: *mut Exception,
) -> Status {
    guard(exc, || {
        let any = unsafe { &*any };
        let mut enc = encoder();
        {
            let mut stream = OutStream::standalone(&mut enc);
            if let Err(e) = stream.write_value(any.typecode(), any.value()) {
                return unsafe { set_system(exc, &e) };
            }
        }
        let orb = first_object(any.value())
            .map(|o| o.orb().clone())
            .or_else(default_orb);
        let bytes = enc.into_bytes();
        let mut stream = InStream::standalone(
            Decoder::new(&bytes, ByteOrder::Big, CdrVersion::Giop1_2),
            orb.as_ref(),
        );
        if exception_id(any.typecode()).is_some() {
            if let Err(e) = stream.read_string() {
                return unsafe { set_system(exc, &e) };
            }
        }
        let mut input = In::new(&mut stream, CompletionStatus::No);
        if let Some(read) = read {
            let status = unsafe { read(context, input.as_ptr()) };
            if status != OK {
                return CALLBACK_FAILED;
            }
        }
        match input.error.take() {
            Some(e) => unsafe { set_system(exc, &e) },
            None => OK,
        }
    })
}

/// The first object reference in a value.
fn first_object(v: &rustorb_core::Value) -> Option<&ObjectRef> {
    use rustorb_core::Value;
    match v {
        Value::Objref(Some(o)) => Some(o),
        Value::List(l) => l.iter().find_map(first_object),
        Value::Union {
            member: Some((_, m)),
            ..
        } => first_object(m),
        Value::Any(a) => first_object(a.value()),
        _ => None,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_write_typecode(out: *mut RorbOut, tc: *const TypeCode) {
    unsafe { &mut *out }.with(|s| s.write_typecode(unsafe { &*tc }));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_write_any(out: *mut RorbOut, any: *const Any) {
    unsafe { &mut *out }.with(|s| s.write_any(unsafe { &*any }));
}

/// NULL after an error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_typecode(input: *mut RorbIn) -> *mut TypeCode {
    match unsafe { &mut *input }.read(|s| s.read_typecode().map(Some)) {
        Some(tc) => into_handle(tc),
        None => std::ptr::null_mut(),
    }
}

/// NULL after an error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_any(input: *mut RorbIn) -> *mut Any {
    match unsafe { &mut *input }.read(|s| s.read_any().map(Some)) {
        Some(any) => into_handle(any),
        None => std::ptr::null_mut(),
    }
}
