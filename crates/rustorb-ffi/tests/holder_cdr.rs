//! `rorb_cdr_encode` and `rorb_in_from_bytes`: the plain CDR of
//! `Messaging::ExceptionHolder::marshaled_exception` (`docs/ami.md`).

use std::ffi::{c_char, c_void};

use rustorb_ffi::owned::{rorb_cdr_encode, rorb_in_from_bytes, rorb_in_release};
use rustorb_ffi::stream::{
    RorbIn, RorbOut, rorb_in_failed, rorb_in_read_string, rorb_in_read_ulong, rorb_in_read_wstring,
    rorb_out_write_string, rorb_out_write_ulong, rorb_out_write_wstring,
};
use rustorb_ffi::{Bytes, CALLBACK_FAILED, Exception, OK, Status, rorb_bytes_free};

const ID: &str = "IDL:test/Oops:1.0";
const TEXT: &str = "héllo";

fn empty() -> Exception {
    Exception {
        repository_id: std::ptr::null_mut(),
        repository_id_len: 0,
        minor: 0,
        completed: 0,
        local: 0,
        index: 0,
    }
}

extern "C" fn write(_: *mut c_void, out: *mut RorbOut) -> Status {
    let wide: Vec<u16> = TEXT.encode_utf16().collect();
    unsafe {
        rorb_out_write_string(out, ID.as_ptr() as *const c_char, ID.len());
        rorb_out_write_ulong(out, 0x0102_0304);
        rorb_out_write_string(out, TEXT.as_ptr() as *const c_char, TEXT.len());
        rorb_out_write_wstring(out, wide.as_ptr(), wide.len());
    }
    OK
}

extern "C" fn refuse(_: *mut c_void, _: *mut RorbOut) -> Status {
    CALLBACK_FAILED
}

fn encode(little_endian: bool) -> Vec<u8> {
    let mut bytes = Bytes {
        data: std::ptr::null_mut(),
        len: 0,
    };
    let mut exc = empty();
    let status = unsafe {
        rorb_cdr_encode(
            Some(write),
            std::ptr::null_mut(),
            little_endian as i32,
            &mut bytes,
            &mut exc,
        )
    };
    assert_eq!(status, OK);
    let v = unsafe { std::slice::from_raw_parts(bytes.data, bytes.len) }.to_vec();
    unsafe { rorb_bytes_free(bytes) };
    v
}

fn decode(data: &[u8], little_endian: bool) -> (String, u32, String, String, bool) {
    let mut input: *mut RorbIn = std::ptr::null_mut();
    let mut exc = empty();
    let status = unsafe {
        rorb_in_from_bytes(
            data.as_ptr(),
            data.len(),
            little_endian as i32,
            std::ptr::null_mut(),
            &mut input,
            &mut exc,
        )
    };
    assert_eq!(status, OK);
    let string = |input| {
        let mut len = 0;
        let p = unsafe { rorb_in_read_string(input, &mut len) };
        if p.is_null() {
            return String::new();
        }
        String::from_utf8(unsafe { std::slice::from_raw_parts(p as *const u8, len) }.to_vec())
            .unwrap()
    };
    let id = string(input);
    let n = unsafe { rorb_in_read_ulong(input) };
    let text = string(input);
    let mut len = 0;
    let p = unsafe { rorb_in_read_wstring(input, &mut len) };
    let wide = if p.is_null() {
        String::new()
    } else {
        String::from_utf16(unsafe { std::slice::from_raw_parts(p, len) }).unwrap()
    };
    let failed = unsafe { rorb_in_failed(input) } != 0;
    unsafe { rorb_in_release(input) };
    (id, n, text, wide, failed)
}

#[test]
fn plain_cdr_from_offset_zero() {
    let big = encode(false);
    let little = encode(true);
    // The id's length, then the id with its NUL, then the ulong aligned to 4.
    assert_eq!(&big[..4], &[0, 0, 0, ID.len() as u8 + 1]);
    assert_eq!(&little[..4], &[ID.len() as u8 + 1, 0, 0, 0]);
    assert_eq!(&big[4..4 + ID.len()], ID.as_bytes());
    let at = (4 + ID.len() + 1 + 3) & !3;
    assert_eq!(&big[at..at + 4], &[1, 2, 3, 4]);
    assert_eq!(&little[at..at + 4], &[4, 3, 2, 1]);
    // UTF-8 strings: the bytes as they are.
    let s = at + 4;
    assert_eq!(&big[s + 4..s + 4 + TEXT.len()], TEXT.as_bytes());
}

#[test]
fn round_trip_in_both_orders() {
    for le in [false, true] {
        let (id, n, text, wide, failed) = decode(&encode(le), le);
        assert!(!failed);
        assert_eq!(id, ID);
        assert_eq!(n, 0x0102_0304);
        assert_eq!(text, TEXT);
        assert_eq!(wide, TEXT);
    }
}

#[test]
fn short_input_fails() {
    let data = encode(false);
    let (_, _, _, _, failed) = decode(&data[..10], false);
    assert!(failed);
}

#[test]
fn refused_write() {
    let mut bytes = Bytes {
        data: std::ptr::null_mut(),
        len: 0,
    };
    let mut exc = empty();
    let status =
        unsafe { rorb_cdr_encode(Some(refuse), std::ptr::null_mut(), 0, &mut bytes, &mut exc) };
    assert_eq!(status, CALLBACK_FAILED);
    assert!(bytes.data.is_null());
}
