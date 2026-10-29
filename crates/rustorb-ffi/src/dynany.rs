//! `rorb_dynany`: the core's [`DynAny`] for the C++ and Java bindings.
//!
//! A handle is one reference to a DynAny; clones of the handle are the
//! same DynAny. Values go in and out as anys: `rorb_dynany_insert` takes
//! the value of an any made with the caller's own conversions (its type
//! is not looked at), and `rorb_dynany_get` makes an any of the type of
//! the kind (`string` for `tk_string`, `Object` for `tk_objref`, the type
//! of the instance for `tk_value`), for the caller to take apart.

use std::ffi::c_char;

use rustorb_core::{Any, DynAny, DynAnyError, DynAnyFactory, TCKind, TypeCode, Value};

use crate::{
    Bytes, Exception, OK, Status, bad_param, guard, into_handle, local, release_handle, set_local,
    set_system, str_arg,
};

type D = DynAny;

unsafe fn dyn_error(exc: *mut Exception, e: DynAnyError) -> Status {
    let code = match e {
        DynAnyError::System(e) => return unsafe { set_system(exc, &e) },
        DynAnyError::InvalidValue => local::DYN_INVALID_VALUE,
        DynAnyError::TypeMismatch => local::DYN_TYPE_MISMATCH,
        DynAnyError::InconsistentTypeCode => local::DYN_INCONSISTENT_TYPECODE,
        DynAnyError::MustTruncate => local::DYN_MUST_TRUNCATE,
    };
    unsafe { set_local(exc, code, 0) }
}

/// Runs `f`, storing its result in `*out`.
unsafe fn put<T>(
    exc: *mut Exception,
    out: *mut T,
    f: impl FnOnce() -> Result<T, DynAnyError>,
) -> Status {
    guard(exc, || match f() {
        Ok(v) => {
            if !out.is_null() {
                unsafe { out.write(v) };
            }
            OK
        }
        Err(e) => unsafe { dyn_error(exc, e) },
    })
}

unsafe fn run(exc: *mut Exception, f: impl FnOnce() -> Result<(), DynAnyError>) -> Status {
    unsafe { put(exc, std::ptr::null_mut::<()>(), f) }
}

unsafe fn arg<'a, T>(p: *const T) -> Result<&'a T, DynAnyError> {
    unsafe { p.as_ref() }.ok_or_else(|| DynAnyError::System(bad_param()))
}

fn kind_arg(kind: u32) -> Result<TCKind, DynAnyError> {
    TCKind::from_u32(kind).ok_or_else(|| DynAnyError::System(bad_param()))
}

fn handle(d: DynAny) -> *mut D {
    into_handle(d)
}

fn bytes(s: String) -> Bytes {
    Bytes::new(s.into_bytes())
}

/// The DynAny the insert and get operations work on: the current
/// component, or the DynAny itself.
fn target(d: &DynAny) -> DynAny {
    match d.current_component() {
        Ok(Some(c)) => c,
        _ => d.clone(),
    }
}

// --- the factory and the handles ---------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_create(
    any: *const Any,
    truncate: i32,
    out: *mut *mut D,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put(exc, out, || {
            let any = arg(any)?;
            let d = if truncate != 0 {
                DynAnyFactory.create_dyn_any(any)
            } else {
                DynAnyFactory.create_dyn_any_without_truncation(any)
            };
            d.map(handle)
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_from_typecode(
    tc: *const TypeCode,
    out: *mut *mut D,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put(exc, out, || {
            DynAnyFactory
                .create_dyn_any_from_type_code(arg(tc)?)
                .map(handle)
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_clone(d: *const D) -> *mut D {
    match unsafe { d.as_ref() } {
        Some(d) => handle(d.clone()),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_release(d: *mut D) {
    unsafe { release_handle(d) };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_kind(d: *const D) -> u32 {
    unsafe { &*d }.kind().as_u32()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_same(a: *const D, b: *const D) -> i32 {
    match unsafe { (a.as_ref(), b.as_ref()) } {
        (Some(a), Some(b)) => i32::from(a.ptr_eq(b)),
        _ => 0,
    }
}

// --- DynAny ------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_type(
    d: *const D,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.type_code().map(into_handle)) }
}

/// The type of the DynAny the insert and get operations work on.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_target_type(
    d: *const D,
    out: *mut *mut TypeCode,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || target(arg(d)?).type_code().map(into_handle)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_assign(
    d: *const D,
    other: *const D,
    exc: *mut Exception,
) -> Status {
    unsafe { run(exc, || arg(d)?.assign(arg(other)?)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_from_any(
    d: *const D,
    any: *const Any,
    exc: *mut Exception,
) -> Status {
    unsafe { run(exc, || arg(d)?.from_any(arg(any)?)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_to_any(
    d: *const D,
    out: *mut *mut Any,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.to_any().map(into_handle)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_equal(
    d: *const D,
    other: *const D,
    out: *mut i32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.equal(arg(other)?).map(i32::from)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_destroy(d: *const D, exc: *mut Exception) -> Status {
    unsafe { run(exc, || arg(d)?.destroy()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_copy(
    d: *const D,
    out: *mut *mut D,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.copy().map(handle)) }
}

/// The type of the any `rorb_dynany_get` makes for a value of `kind`.
fn type_for(d: &DynAny, kind: TCKind, value: &Value) -> Result<TypeCode, DynAnyError> {
    Ok(match (kind, value) {
        (TCKind::String, _) => TypeCode::string(0),
        (TCKind::WString, _) => TypeCode::wstring(0),
        (TCKind::Objref, _) => TypeCode::object(),
        (TCKind::Value, Value::Valuetype(Some(r))) => r.typecode().clone(),
        (TCKind::Value, _) => target(d).type_code()?,
        (k, _) => TypeCode::simple(k).map_err(|_| DynAnyError::System(bad_param()))?,
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_insert(
    d: *const D,
    kind: u32,
    value: *const Any,
    exc: *mut Exception,
) -> Status {
    unsafe {
        run(exc, || {
            arg(d)?.insert_basic(kind_arg(kind)?, arg(value)?.value().clone())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_get(
    d: *const D,
    kind: u32,
    out: *mut *mut Any,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put(exc, out, || {
            let d = arg(d)?;
            let kind = kind_arg(kind)?;
            let value = d.get_basic(kind)?;
            let tc = type_for(d, kind, &value)?;
            Ok(into_handle(Any::new(tc, value)))
        })
    }
}

/// `insert_<type>_seq`: the elements of `value`, an any of a sequence of
/// elements of `kind`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_insert_seq(
    d: *const D,
    kind: u32,
    value: *const Any,
    exc: *mut Exception,
) -> Status {
    unsafe {
        run(exc, || {
            let items = match arg(value)?.value() {
                Value::List(items) => items.clone(),
                Value::Octets(b) => b.iter().map(|&b| Value::Octet(b)).collect(),
                Value::Chars(s) => s.chars().map(Value::Char).collect(),
                _ => return Err(DynAnyError::System(bad_param())),
            };
            arg(d)?.insert_seq(kind_arg(kind)?, items)
        })
    }
}

/// `get_<type>_seq`: an any of an unbounded sequence of `kind`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_get_seq(
    d: *const D,
    kind: u32,
    out: *mut *mut Any,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put(exc, out, || {
            let kind = kind_arg(kind)?;
            let items = arg(d)?.get_seq(kind)?;
            let element = TypeCode::simple(kind).map_err(|_| DynAnyError::System(bad_param()))?;
            let tc = TypeCode::sequence(&element, 0)
                .map_err(|_| DynAnyError::System(bad_param()))?;
            let value = match kind {
                TCKind::Octet => Value::Octets(
                    items
                        .iter()
                        .filter_map(|v| match v {
                            Value::Octet(b) => Some(*b),
                            _ => None,
                        })
                        .collect(),
                ),
                _ => Value::List(items),
            };
            Ok(into_handle(Any::new(tc, value)))
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_insert_dyn_any(
    d: *const D,
    value: *const D,
    exc: *mut Exception,
) -> Status {
    unsafe { run(exc, || arg(d)?.insert_dyn_any(arg(value)?)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_get_dyn_any(
    d: *const D,
    out: *mut *mut D,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.get_dyn_any().map(handle)) }
}

/// `insert_longdouble`, `insert_abstract` and their gets: `NO_IMPLEMENT`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_no_implement(d: *const D, exc: *mut Exception) -> Status {
    unsafe { run(exc, || arg(d)?.no_implement()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_seek(
    d: *const D,
    index: i32,
    out: *mut i32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.seek(index).map(i32::from)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_rewind(d: *const D, exc: *mut Exception) -> Status {
    unsafe { run(exc, || arg(d)?.rewind()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_next(
    d: *const D,
    out: *mut i32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.next().map(i32::from)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_component_count(
    d: *const D,
    out: *mut u32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.component_count()) }
}

/// The current component, NULL at position -1.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_current_component(
    d: *const D,
    out: *mut *mut D,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put(exc, out, || {
            Ok(arg(d)?
                .current_component()?
                .map_or(std::ptr::null_mut(), handle))
        })
    }
}

// --- DynFixed, DynEnum -----------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_get_fixed_value(
    d: *const D,
    out: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.get_fixed_value().map(bytes)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_fixed_value(
    d: *const D,
    value: *const c_char,
    len: usize,
    out: *mut i32,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put(exc, out, || {
            let s = str_arg(value, len)?;
            arg(d)?.set_fixed_value(s).map(i32::from)
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_get_as_string(
    d: *const D,
    out: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.get_enum_as_string().map(bytes)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_as_string(
    d: *const D,
    value: *const c_char,
    len: usize,
    exc: *mut Exception,
) -> Status {
    unsafe { run(exc, || arg(d)?.set_enum_as_string(str_arg(value, len)?)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_get_as_ulong(
    d: *const D,
    out: *mut u32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.get_enum_as_ulong()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_as_ulong(
    d: *const D,
    value: u32,
    exc: *mut Exception,
) -> Status {
    unsafe { run(exc, || arg(d)?.set_enum_as_ulong(value)) }
}

// --- DynStruct, DynValue -------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_current_member_name(
    d: *const D,
    out: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.current_member_name().map(bytes)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_current_member_kind(
    d: *const D,
    out: *mut u32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.current_member_kind().map(TCKind::as_u32)) }
}

/// The number of members (`get_members` fails as it would: `InvalidValue`
/// for a `null` valuetype).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_member_count(
    d: *const D,
    out: *mut u32,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put(exc, out, || {
            Ok(arg(d)?.get_members_as_dyn_any()?.len() as u32)
        })
    }
}

/// The member `index`: its name and its component.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_member_at(
    d: *const D,
    index: u32,
    name: *mut Bytes,
    out: *mut *mut D,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put(exc, out, || {
            let members = arg(d)?.get_members_as_dyn_any()?;
            let (n, m) = members
                .into_iter()
                .nth(index as usize)
                .ok_or_else(|| DynAnyError::System(bad_param()))?;
            if !name.is_null() {
                name.write(bytes(n));
            }
            Ok(handle(m))
        })
    }
}

/// `count` names and values, from C arrays.
unsafe fn name_list<T: Clone>(
    count: usize,
    names: *const *const c_char,
    name_lens: *const usize,
    values: *const *const T,
) -> Result<Vec<(String, T)>, DynAnyError> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let name = unsafe { str_arg(*names.add(i), *name_lens.add(i)) }?;
        let v = unsafe { arg(*values.add(i)) }?;
        out.push((name.to_owned(), v.clone()));
    }
    Ok(out)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_members(
    d: *const D,
    count: usize,
    names: *const *const c_char,
    name_lens: *const usize,
    values: *const *const Any,
    exc: *mut Exception,
) -> Status {
    unsafe {
        run(exc, || {
            let members = name_list(count, names, name_lens, values)?;
            arg(d)?.set_members(&members)
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_members_as_dyn_any(
    d: *const D,
    count: usize,
    names: *const *const c_char,
    name_lens: *const usize,
    values: *const *const D,
    exc: *mut Exception,
) -> Status {
    unsafe {
        run(exc, || {
            let members = name_list(count, names, name_lens, values)?;
            arg(d)?.set_members_as_dyn_any(&members)
        })
    }
}

// --- DynUnion ----------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_get_discriminator(
    d: *const D,
    out: *mut *mut D,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.get_discriminator().map(handle)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_discriminator(
    d: *const D,
    disc: *const D,
    exc: *mut Exception,
) -> Status {
    unsafe { run(exc, || arg(d)?.set_discriminator(arg(disc)?)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_to_default_member(
    d: *const D,
    exc: *mut Exception,
) -> Status {
    unsafe { run(exc, || arg(d)?.set_to_default_member()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_to_no_active_member(
    d: *const D,
    exc: *mut Exception,
) -> Status {
    unsafe { run(exc, || arg(d)?.set_to_no_active_member()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_has_no_active_member(
    d: *const D,
    out: *mut i32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.has_no_active_member().map(i32::from)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_discriminator_kind(
    d: *const D,
    out: *mut u32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.discriminator_kind().map(TCKind::as_u32)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_member(
    d: *const D,
    out: *mut *mut D,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.member().map(handle)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_member_name(
    d: *const D,
    out: *mut Bytes,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.member_name().map(bytes)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_member_kind(
    d: *const D,
    out: *mut u32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.member_kind().map(TCKind::as_u32)) }
}

// --- DynSequence, DynArray -----------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_get_length(
    d: *const D,
    out: *mut u32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.get_length()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_length(
    d: *const D,
    len: u32,
    exc: *mut Exception,
) -> Status {
    unsafe { run(exc, || arg(d)?.set_length(len)) }
}

/// The number of elements of a sequence or array.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_element_count(
    d: *const D,
    out: *mut u32,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put(exc, out, || {
            Ok(arg(d)?.get_elements_as_dyn_any()?.len() as u32)
        })
    }
}

/// The element `index`, as a component.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_element_at(
    d: *const D,
    index: u32,
    out: *mut *mut D,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put(exc, out, || {
            arg(d)?
                .get_elements_as_dyn_any()?
                .into_iter()
                .nth(index as usize)
                .map(handle)
                .ok_or_else(|| DynAnyError::System(bad_param()))
        })
    }
}

/// `count` handles from a C array.
unsafe fn handles<T: Clone>(count: usize, values: *const *const T) -> Result<Vec<T>, DynAnyError> {
    (0..count)
        .map(|i| unsafe { arg(*values.add(i)) }.cloned())
        .collect()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_elements(
    d: *const D,
    count: usize,
    values: *const *const Any,
    exc: *mut Exception,
) -> Status {
    unsafe { run(exc, || arg(d)?.set_elements(&handles(count, values)?)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_elements_as_dyn_any(
    d: *const D,
    count: usize,
    values: *const *const D,
    exc: *mut Exception,
) -> Status {
    unsafe {
        run(exc, || {
            arg(d)?.set_elements_as_dyn_any(&handles(count, values)?)
        })
    }
}

// --- DynValueCommon, DynValueBox -----------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_is_null(
    d: *const D,
    out: *mut i32,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.is_null().map(i32::from)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_to_null(d: *const D, exc: *mut Exception) -> Status {
    unsafe { run(exc, || arg(d)?.set_to_null()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_to_value(d: *const D, exc: *mut Exception) -> Status {
    unsafe { run(exc, || arg(d)?.set_to_value()) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_get_boxed_value(
    d: *const D,
    out: *mut *mut Any,
    exc: *mut Exception,
) -> Status {
    unsafe { put(exc, out, || arg(d)?.get_boxed_value().map(into_handle)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_boxed_value(
    d: *const D,
    value: *const Any,
    exc: *mut Exception,
) -> Status {
    unsafe { run(exc, || arg(d)?.set_boxed_value(arg(value)?)) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_get_boxed_value_as_dyn_any(
    d: *const D,
    out: *mut *mut D,
    exc: *mut Exception,
) -> Status {
    unsafe {
        put(exc, out, || {
            arg(d)?.get_boxed_value_as_dyn_any().map(handle)
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_dynany_set_boxed_value_as_dyn_any(
    d: *const D,
    value: *const D,
    exc: *mut Exception,
) -> Status {
    unsafe { run(exc, || arg(d)?.set_boxed_value_as_dyn_any(arg(value)?)) }
}
