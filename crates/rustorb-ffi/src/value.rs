//! Valuetypes on the `rorb_out` and `rorb_in` streams.
//!
//! The caller marshals the state of its own instances; these functions
//! write and read what surrounds it (the value tag, the repository ids,
//! chunks, end tags) and keep what sharing needs: an instance written
//! twice in a message is sent as an indirection the second time (the
//! caller names instances by a key), and the instances read are kept by
//! slot, so that an indirection to one gives the same instance back.

use std::ffi::{c_char, c_void};

use rustorb_cdr::ValueHeader;
use rustorb_core::TypeCode;

use crate::str_arg;
use crate::stream::{RorbIn, RorbOut};

/// `RORB_VALUE_NULL`, `RORB_VALUE_INDIRECTION`, `RORB_VALUE_NEW`.
pub const VALUE_NULL: i32 = 0;
pub const VALUE_INDIRECTION: i32 = 1;
pub const VALUE_NEW: i32 = 2;

/// Releases an instance stored with `rorb_in_set_value`.
pub type ReleaseFn = Option<unsafe extern "C" fn(*mut c_void)>;

/// An instance of the caller kept by an input stream.
#[derive(Debug)]
pub struct Held {
    ptr: *mut c_void,
    release: ReleaseFn,
}

impl Drop for Held {
    fn drop(&mut self) {
        if let Some(release) = self.release {
            unsafe { release(self.ptr) };
        }
    }
}

// --- output ---------------------------------------------------------------------

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_write_null_value(out: *mut RorbOut) {
    unsafe { &mut *out }.with(|s| {
        s.write_null_value();
        Ok(())
    });
}

/// Starts a value: returns 1 when the caller writes its state (then
/// calls `rorb_out_end_value`), 0 when an indirection was written or the
/// stream failed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_begin_value(
    out: *mut RorbOut,
    key: u64,
    count: usize,
    ids: *const *const c_char,
    id_lens: *const usize,
    chunked: i32,
) -> i32 {
    let out = unsafe { &mut *out };
    let mut state = false;
    out.with(|s| {
        let mut list = Vec::with_capacity(count);
        for i in 0..count {
            list.push(unsafe { str_arg(*ids.add(i), *id_lens.add(i)) }?);
        }
        state = s.begin_value(key, &list, chunked != 0)?;
        Ok(())
    });
    i32::from(state && out.error.is_none())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_end_value(out: *mut RorbOut) {
    unsafe { &mut *out }.with(|s| Ok(s.end_value()?));
}

// --- input ----------------------------------------------------------------------

/// Reads what stands where a value is expected. For an indirection,
/// `*slot` is the slot of the value it refers to; for a new value, its
/// own slot and `*id_count` the number of repository ids sent (read with
/// `rorb_in_value_id`). After an error, returns `RORB_VALUE_NULL`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_read_value_header(
    input: *mut RorbIn,
    slot: *mut usize,
    id_count: *mut usize,
) -> i32 {
    let input = unsafe { &mut *input };
    let e = input.marshal();
    let header = input.read(|s| s.read_value_header().map(Some).map_err(|_| e));
    unsafe {
        *slot = 0;
        *id_count = 0;
    }
    match header {
        None | Some(ValueHeader::Null) => VALUE_NULL,
        Some(ValueHeader::Indirection(n)) => {
            unsafe { *slot = n };
            VALUE_INDIRECTION
        }
        Some(ValueHeader::Value { slot: n, ids, .. }) => {
            unsafe {
                *slot = n;
                *id_count = ids.len();
            }
            input.value_ids = ids;
            VALUE_NEW
        }
    }
}

/// The repository id `index` of the last value header read (UTF-8,
/// valid until the next value header).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_value_id(
    input: *mut RorbIn,
    index: usize,
    len: *mut usize,
) -> *const c_char {
    let input = unsafe { &*input };
    match input.value_ids.get(index) {
        Some(id) => {
            unsafe { *len = id.len() };
            id.as_ptr().cast()
        }
        None => {
            unsafe { *len = 0 };
            c"".as_ptr()
        }
    }
}

/// Ends the value whose header was read last, skipping the state not
/// read (of a truncated derived type).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_end_value(input: *mut RorbIn) {
    let input = unsafe { &mut *input };
    let e = input.marshal();
    input.read(|s| s.end_value().map_err(|_| e));
}

/// Keeps `ptr`, the caller's instance of the value at `slot`, until the
/// stream ends; `release(ptr)` is then called (unless it is NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_set_value(
    input: *mut RorbIn,
    slot: usize,
    ptr: *mut c_void,
    release: ReleaseFn,
) {
    let input = unsafe { &mut *input };
    if input.values.len() <= slot {
        input.values.resize_with(slot + 1, || None);
    }
    input.values[slot] = Some(Held { ptr, release });
}

/// The instance kept for `slot`, or NULL (an indirection to a value the
/// caller did not read, say).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_get_value(input: *mut RorbIn, slot: usize) -> *mut c_void {
    match unsafe { &*input }.values.get(slot) {
        Some(Some(held)) => held.ptr,
        _ => std::ptr::null_mut(),
    }
}

// --- the registry of the core ---------------------------------------------------

/// Makes the valuetype `tc` known to the core, which reads the values in
/// anys (see `rustorb_core::register_value_type`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_register_value_type(tc: *const TypeCode) {
    if let Some(tc) = unsafe { tc.as_ref() } {
        rustorb_core::register_value_type(tc);
    }
}

/// Finds the valuetype of a repository id: a new TypeCode handle (taken
/// over) or NULL.
pub type ResolveFn =
    Option<unsafe extern "C" fn(*mut c_void, *const c_char, usize) -> *mut TypeCode>;

struct Resolver {
    resolve: unsafe extern "C" fn(*mut c_void, *const c_char, usize) -> *mut TypeCode,
    context: *mut c_void,
}

// The caller makes `resolve` callable from any thread with `context`.
unsafe impl Send for Resolver {}
unsafe impl Sync for Resolver {}

impl Resolver {
    fn find(&self, id: &str) -> Option<TypeCode> {
        let tc = unsafe { (self.resolve)(self.context, id.as_ptr().cast(), id.len()) };
        if tc.is_null() {
            None
        } else {
            Some(*unsafe { Box::from_raw(tc) })
        }
    }
}

/// Asks `resolve(context, id, len)` for the valuetypes the core reads
/// that are not registered (NULL removes it).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_set_value_type_resolver(resolve: ResolveFn, context: *mut c_void) {
    let resolver = resolve.map(|resolve| {
        let r = Resolver { resolve, context };
        std::sync::Arc::new(move |id: &str| r.find(id))
            as std::sync::Arc<rustorb_core::ValueTypeResolver>
    });
    rustorb_core::set_value_type_resolver(resolver);
}
