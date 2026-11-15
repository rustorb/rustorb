//! Valuetypes on the `rorb_out` and `rorb_in` streams.
//!
//! The caller marshals the state of its own instances; these functions
//! write and read what surrounds it (the value tag, the repository ids,
//! chunks, end tags) and keep what sharing needs: an instance written
//! twice in a message is sent as an indirection the second time (the
//! caller names instances by a key), and the instances read are kept by
//! slot, so that an indirection to one gives the same instance back.

use std::ffi::{c_char, c_void};

use std::sync::Arc;

use rustorb_cdr::ValueHeader;
use rustorb_core::TypeCode;

use crate::host::{Host, Owned, Written, host_of};
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
    /// The caller's key of the instance (`rorb_in_set_host_value`).
    pub(crate) key: Option<u64>,
    /// The core's reference, for an instance a core instance stands for.
    _host: Option<Arc<Host>>,
}

impl Held {
    /// The pointer and its release function, taken over from the stream.
    pub(crate) fn take(&mut self) -> (*mut c_void, ReleaseFn) {
        (self.ptr, self.release.take())
    }
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
    unsafe { begin_value(&mut *out, key, None, count, ids, id_lens, chunked) }
}

/// Whether the stream keeps the caller's instances written
/// (`rorb_out_begin_host_value`): 1 for the stream of `rorb_any_insert`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_keeps_hosts(out: *mut RorbOut) -> i32 {
    i32::from(unsafe { &*out }.written.is_some())
}

/// Like `rorb_out_begin_value`, and takes `host`, a reference to the
/// caller's instance (released with `release` when not kept): an any
/// whose value is read back from the stream keeps it, so that extracting
/// gives the same instance, and inserting the instance (the same key)
/// again reuses the core's instance.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_out_begin_host_value(
    out: *mut RorbOut,
    key: u64,
    host: *mut c_void,
    release: ReleaseFn,
    count: usize,
    ids: *const *const c_char,
    id_lens: *const usize,
    chunked: i32,
) -> i32 {
    let host = Owned::new(host, release);
    unsafe { begin_value(&mut *out, key, Some(host), count, ids, id_lens, chunked) }
}

unsafe fn begin_value(
    out: &mut RorbOut,
    key: u64,
    host: Option<Owned>,
    count: usize,
    ids: *const *const c_char,
    id_lens: *const usize,
    chunked: i32,
) -> i32 {
    let mut state = false;
    let mut slot = 0;
    out.with(|s| {
        let mut list = Vec::with_capacity(count);
        for i in 0..count {
            list.push(unsafe { str_arg(*ids.add(i), *id_lens.add(i)) }?);
        }
        slot = s.value_count();
        state = s.begin_value(key, &list, chunked != 0)?;
        Ok(())
    });
    let state = state && out.error.is_none();
    if state {
        if let Some(written) = &mut out.written {
            written.push(Written { slot, key, host });
        }
    }
    i32::from(state)
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
            // To a value skipped inside one given back (below).
            input.hold_host(n);
            unsafe { *slot = n };
            VALUE_INDIRECTION
        }
        Some(ValueHeader::Value { slot: n, ids, .. }) => {
            unsafe { *slot = n };
            if input.hold_host(n) {
                // The caller's instance is given back: its state (always
                // chunked in this stream) is skipped.
                let e = input.marshal();
                input.read(|s| s.end_value().map_err(|_| e));
                return VALUE_INDIRECTION;
            }
            unsafe { *id_count = ids.len() };
            input.value_ids = ids;
            VALUE_NEW
        }
    }
}

impl RorbIn {
    /// For the stream of `rorb_any_extract`: keeps for `slot` the
    /// caller's instance the core instance written there stands for, if
    /// it has one and none is kept yet; whether one is kept now.
    fn hold_host(&mut self, slot: usize) -> bool {
        if matches!(self.values.get(slot), Some(Some(_))) {
            return false;
        }
        let Some(host) = self
            .bound
            .as_ref()
            .and_then(|b| b.get(slot))
            .and_then(host_of)
        else {
            return false;
        };
        if self.values.len() <= slot {
            self.values.resize_with(slot + 1, || None);
        }
        self.values[slot] = Some(Held {
            ptr: host.ptr,
            release: None,
            key: None,
            _host: Some(host),
        });
        true
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
    unsafe { set_value(&mut *input, slot, None, ptr, release) }
}

/// Like `rorb_in_set_value`, with the caller's key of the instance (as it
/// writes it with `rorb_out_begin_host_value`): the stream of
/// `rorb_any_extract` then gives `ptr` to the any's core instance, which
/// keeps it instead of the stream.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_set_host_value(
    input: *mut RorbIn,
    slot: usize,
    key: u64,
    ptr: *mut c_void,
    release: ReleaseFn,
) {
    unsafe { set_value(&mut *input, slot, Some(key), ptr, release) }
}

/// Whether the stream gives the caller's instances it reads to the core
/// (`rorb_in_set_host_value`): 1 for the stream of `rorb_any_extract`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rorb_in_keeps_hosts(input: *mut RorbIn) -> i32 {
    i32::from(unsafe { &*input }.bound.is_some())
}

unsafe fn set_value(
    input: &mut RorbIn,
    slot: usize,
    key: Option<u64>,
    ptr: *mut c_void,
    release: ReleaseFn,
) {
    if input.values.len() <= slot {
        input.values.resize_with(slot + 1, || None);
    }
    input.values[slot] = Some(Held {
        ptr,
        release,
        key,
        _host: None,
    });
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
