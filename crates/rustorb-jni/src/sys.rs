//! The C ABI of `rustorb-ffi`, as `include/rustorb/rustorb.h` declares it.
//!
//! The Java binding uses only these functions, like the C++ binding, so
//! that the core has one stable boundary.

#![allow(non_camel_case_types, dead_code)]

use std::ffi::{c_char, c_int, c_void};

macro_rules! opaque {
    ($($name:ident),*) => {
        $(
            #[repr(C)]
            #[derive(Debug)]
            pub struct $name {
                _private: [u8; 0],
            }
        )*
    };
}

opaque!(
    rorb_orb,
    rorb_object,
    rorb_poa,
    rorb_poa_manager,
    rorb_servant,
    rorb_out,
    rorb_in,
    rorb_server_reply,
    rorb_typecode,
    rorb_any,
    rorb_manager_error
);

pub type rorb_status = i32;

pub const RORB_OK: rorb_status = 0;
pub const RORB_SYSTEM_EXCEPTION: rorb_status = 1;
pub const RORB_USER_EXCEPTION: rorb_status = 2;
pub const RORB_LOCAL_EXCEPTION: rorb_status = 3;
pub const RORB_CALLBACK_FAILED: rorb_status = 4;

#[repr(C)]
#[derive(Debug)]
pub struct rorb_exception {
    pub repository_id: *mut c_char,
    pub repository_id_len: usize,
    pub minor: u32,
    pub completed: u32,
    pub local: u32,
    pub index: u32,
}

impl Default for rorb_exception {
    fn default() -> Self {
        rorb_exception {
            repository_id: std::ptr::null_mut(),
            repository_id_len: 0,
            minor: 0,
            completed: 0,
            local: 0,
            index: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct rorb_bytes {
    pub data: *mut u8,
    pub len: usize,
}

impl Default for rorb_bytes {
    fn default() -> Self {
        rorb_bytes {
            data: std::ptr::null_mut(),
            len: 0,
        }
    }
}

pub type WriteFn = unsafe extern "C" fn(*mut c_void, *mut rorb_out) -> rorb_status;
pub type ReadFn = unsafe extern "C" fn(*mut c_void, *mut rorb_in) -> rorb_status;
pub type UserExceptionFn =
    unsafe extern "C" fn(*mut c_void, *const c_char, usize, *mut rorb_in) -> rorb_status;

#[repr(C)]
#[derive(Debug)]
pub struct rorb_invocation {
    pub operation: *const c_char,
    pub operation_len: usize,
    pub oneway: i32,
    pub timeout_us: i64,
    pub context: *mut c_void,
    pub write_args: Option<WriteFn>,
    pub read_reply: Option<ReadFn>,
    pub read_user_exception: Option<UserExceptionFn>,
}

#[repr(C)]
#[derive(Debug)]
pub struct rorb_servant_vtable {
    pub dispatch: unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        usize,
        *mut rorb_in,
        *mut rorb_server_reply,
    ) -> rorb_status,
    pub is_a: unsafe extern "C" fn(*mut c_void, *const c_char, usize) -> i32,
    pub add_ref: unsafe extern "C" fn(*mut c_void),
    pub remove_ref: unsafe extern "C" fn(*mut c_void),
}

#[repr(C)]
#[derive(Debug)]
pub struct rorb_servant_activator_vtable {
    pub incarnate: unsafe extern "C" fn(
        *mut c_void,
        *const u8,
        usize,
        *mut rorb_poa,
        *mut *mut rorb_servant,
        *mut rorb_manager_error,
    ) -> rorb_status,
    pub etherealize: Option<
        unsafe extern "C" fn(*mut c_void, *const u8, usize, *mut rorb_poa, *mut c_void, i32, i32),
    >,
    pub add_ref: unsafe extern "C" fn(*mut c_void),
    pub remove_ref: unsafe extern "C" fn(*mut c_void),
}

#[repr(C)]
#[derive(Debug)]
pub struct rorb_servant_locator_vtable {
    pub preinvoke: unsafe extern "C" fn(
        *mut c_void,
        *const u8,
        usize,
        *mut rorb_poa,
        *const c_char,
        usize,
        *mut *mut rorb_servant,
        *mut *mut c_void,
        *mut rorb_manager_error,
    ) -> rorb_status,
    pub postinvoke: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const u8,
            usize,
            *mut rorb_poa,
            *const c_char,
            usize,
            *mut c_void,
            *mut c_void,
        ),
    >,
    pub add_ref: unsafe extern "C" fn(*mut c_void),
    pub remove_ref: unsafe extern "C" fn(*mut c_void),
}

#[repr(C)]
#[derive(Debug)]
pub struct rorb_adapter_activator_vtable {
    pub unknown_adapter:
        unsafe extern "C" fn(*mut c_void, *mut rorb_poa, *const c_char, usize) -> i32,
    pub add_ref: unsafe extern "C" fn(*mut c_void),
    pub remove_ref: unsafe extern "C" fn(*mut c_void),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct rorb_policy {
    pub r#type: u32,
    pub value: u32,
}

pub const RORB_PRIM_OCTET: i32 = 0;

pub type Exc = *mut rorb_exception;

unsafe extern "C" {
    pub fn rorb_exception_clear(exc: Exc);
    pub fn rorb_bytes_free(bytes: rorb_bytes);
    pub fn rorb_abi_version() -> u32;

    // TypeCodes and anys
    pub fn rorb_tc_clone(tc: *const rorb_typecode) -> *mut rorb_typecode;
    pub fn rorb_tc_release(tc: *mut rorb_typecode);
    pub fn rorb_tc_simple(kind: u32, out: *mut *mut rorb_typecode, exc: Exc) -> rorb_status;
    pub fn rorb_tc_string(
        wide: i32,
        bound: u32,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_fixed(
        digits: u16,
        scale: i16,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_sequence(
        content: *const rorb_typecode,
        bound: u32,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_array(
        content: *const rorb_typecode,
        length: u32,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_alias(
        kind: u32,
        id: *const c_char,
        id_len: usize,
        name: *const c_char,
        name_len: usize,
        content: *const rorb_typecode,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_interface(
        kind: u32,
        id: *const c_char,
        id_len: usize,
        name: *const c_char,
        name_len: usize,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_struct(
        kind: u32,
        id: *const c_char,
        id_len: usize,
        name: *const c_char,
        name_len: usize,
        count: usize,
        names: *const *const c_char,
        name_lens: *const usize,
        types: *const *const rorb_typecode,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_union(
        id: *const c_char,
        id_len: usize,
        name: *const c_char,
        name_len: usize,
        discriminator: *const rorb_typecode,
        count: usize,
        labels: *const u64,
        default_index: i32,
        names: *const *const c_char,
        name_lens: *const usize,
        types: *const *const rorb_typecode,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_enum(
        id: *const c_char,
        id_len: usize,
        name: *const c_char,
        name_len: usize,
        count: usize,
        names: *const *const c_char,
        name_lens: *const usize,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_recursive(
        id: *const c_char,
        id_len: usize,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_kind(tc: *const rorb_typecode) -> u32;
    pub fn rorb_tc_id(
        tc: *const rorb_typecode,
        s: *mut *const c_char,
        len: *mut usize,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_name(
        tc: *const rorb_typecode,
        s: *mut *const c_char,
        len: *mut usize,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_member_count(tc: *const rorb_typecode, count: *mut u32, exc: Exc)
    -> rorb_status;
    pub fn rorb_tc_member_name(
        tc: *const rorb_typecode,
        index: u32,
        s: *mut *const c_char,
        len: *mut usize,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_member_type(
        tc: *const rorb_typecode,
        index: u32,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_member_label(
        tc: *const rorb_typecode,
        index: u32,
        out: *mut *mut rorb_any,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_discriminator_type(
        tc: *const rorb_typecode,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_default_index(
        tc: *const rorb_typecode,
        index: *mut i32,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_length(tc: *const rorb_typecode, length: *mut u32, exc: Exc) -> rorb_status;
    pub fn rorb_tc_content_type(
        tc: *const rorb_typecode,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_fixed_digits(
        tc: *const rorb_typecode,
        digits: *mut u16,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_fixed_scale(tc: *const rorb_typecode, scale: *mut i16, exc: Exc) -> rorb_status;
    pub fn rorb_tc_member_visibility(
        tc: *const rorb_typecode,
        index: u32,
        v: *mut i16,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_type_modifier(tc: *const rorb_typecode, v: *mut i16, exc: Exc) -> rorb_status;
    pub fn rorb_tc_concrete_base_type(
        tc: *const rorb_typecode,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_tc_equal(a: *const rorb_typecode, b: *const rorb_typecode) -> i32;
    pub fn rorb_tc_equivalent(a: *const rorb_typecode, b: *const rorb_typecode) -> i32;
    pub fn rorb_tc_compact(tc: *const rorb_typecode) -> *mut rorb_typecode;
    pub fn rorb_tc_unaliased(tc: *const rorb_typecode) -> *mut rorb_typecode;

    pub fn rorb_any_new() -> *mut rorb_any;
    pub fn rorb_any_clone(any: *const rorb_any) -> *mut rorb_any;
    pub fn rorb_any_release(any: *mut rorb_any);
    pub fn rorb_any_type(any: *const rorb_any) -> *mut rorb_typecode;
    pub fn rorb_any_set_type(any: *mut rorb_any, tc: *const rorb_typecode, exc: Exc)
    -> rorb_status;
    pub fn rorb_any_equal(a: *const rorb_any, b: *const rorb_any) -> i32;
    pub fn rorb_any_label(any: *const rorb_any, label: *mut u64) -> i32;
    pub fn rorb_any_insert(
        tc: *const rorb_typecode,
        write: WriteFn,
        context: *mut c_void,
        out: *mut *mut rorb_any,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_any_extract(
        any: *const rorb_any,
        read: ReadFn,
        context: *mut c_void,
        exc: Exc,
    ) -> rorb_status;

    // ORB
    pub fn rorb_orb_init(
        argc: *mut c_int,
        argv: *mut *mut c_char,
        orb: *mut *mut rorb_orb,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_orb_clone(orb: *mut rorb_orb) -> *mut rorb_orb;
    pub fn rorb_orb_release(orb: *mut rorb_orb);
    pub fn rorb_orb_string_to_object(
        orb: *mut rorb_orb,
        s: *const c_char,
        len: usize,
        obj: *mut *mut rorb_object,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_orb_object_to_string(
        orb: *mut rorb_orb,
        obj: *mut rorb_object,
        s: *mut rorb_bytes,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_orb_resolve_initial_references(
        orb: *mut rorb_orb,
        name: *const c_char,
        len: usize,
        obj: *mut *mut rorb_object,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_orb_list_initial_services(
        orb: *mut rorb_orb,
        context: *mut c_void,
        each: unsafe extern "C" fn(*mut c_void, *const c_char, usize),
    );
    pub fn rorb_orb_root_poa(orb: *mut rorb_orb, poa: *mut *mut rorb_poa, exc: Exc) -> rorb_status;
    pub fn rorb_orb_run(orb: *mut rorb_orb);
    pub fn rorb_orb_work_pending(orb: *mut rorb_orb, pending: *mut i32, exc: Exc) -> rorb_status;
    pub fn rorb_orb_perform_work(orb: *mut rorb_orb, exc: Exc) -> rorb_status;
    pub fn rorb_orb_shutdown(orb: *mut rorb_orb, wait: i32, exc: Exc) -> rorb_status;
    pub fn rorb_orb_destroy(orb: *mut rorb_orb, exc: Exc) -> rorb_status;
    pub fn rorb_default_root_poa(poa: *mut *mut rorb_poa, exc: Exc) -> rorb_status;
    pub fn rorb_default_orb() -> *mut rorb_orb;
    pub fn rorb_orb_set_call_timeout(orb: *mut rorb_orb, us: i64);
    pub fn rorb_orb_call_timeout(orb: *mut rorb_orb) -> i64;
    pub fn rorb_orb_set_connect_timeout(orb: *mut rorb_orb, us: i64);
    pub fn rorb_orb_connect_timeout(orb: *mut rorb_orb) -> i64;
    pub fn rorb_set_thread_call_timeout(us: i64);
    pub fn rorb_thread_call_timeout() -> i64;

    // object references
    pub fn rorb_object_clone(obj: *mut rorb_object) -> *mut rorb_object;
    pub fn rorb_object_release(obj: *mut rorb_object);
    pub fn rorb_object_orb(obj: *mut rorb_object) -> *mut rorb_orb;
    pub fn rorb_object_type_id(obj: *mut rorb_object, len: *mut usize) -> *const c_char;
    pub fn rorb_object_is_a(
        obj: *mut rorb_object,
        id: *const c_char,
        len: usize,
        result: *mut i32,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_object_non_existent(
        obj: *mut rorb_object,
        result: *mut i32,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_object_is_equivalent(a: *mut rorb_object, b: *mut rorb_object) -> i32;
    pub fn rorb_object_hash(obj: *mut rorb_object, maximum: u32) -> u32;
    pub fn rorb_object_set_call_timeout(obj: *mut rorb_object, us: i64);
    pub fn rorb_object_call_timeout(obj: *mut rorb_object) -> i64;
    pub fn rorb_object_invoke(
        obj: *mut rorb_object,
        inv: *const rorb_invocation,
        exc: Exc,
    ) -> rorb_status;

    // streams
    pub fn rorb_out_write_boolean(out: *mut rorb_out, v: i32);
    pub fn rorb_out_write_octet(out: *mut rorb_out, v: u8);
    pub fn rorb_out_write_char(out: *mut rorb_out, v: c_char);
    pub fn rorb_out_write_wchar(out: *mut rorb_out, v: u16);
    pub fn rorb_out_write_short(out: *mut rorb_out, v: i16);
    pub fn rorb_out_write_ushort(out: *mut rorb_out, v: u16);
    pub fn rorb_out_write_long(out: *mut rorb_out, v: i32);
    pub fn rorb_out_write_ulong(out: *mut rorb_out, v: u32);
    pub fn rorb_out_write_longlong(out: *mut rorb_out, v: i64);
    pub fn rorb_out_write_ulonglong(out: *mut rorb_out, v: u64);
    pub fn rorb_out_write_float(out: *mut rorb_out, v: f32);
    pub fn rorb_out_write_double(out: *mut rorb_out, v: f64);
    pub fn rorb_out_write_string(out: *mut rorb_out, s: *const c_char, len: usize);
    pub fn rorb_out_write_wstring(out: *mut rorb_out, s: *const u16, len: usize);
    pub fn rorb_out_write_object(out: *mut rorb_out, obj: *mut rorb_object);
    pub fn rorb_out_write_typecode(out: *mut rorb_out, tc: *const rorb_typecode);
    pub fn rorb_out_write_any(out: *mut rorb_out, any: *const rorb_any);
    pub fn rorb_out_write_array(out: *mut rorb_out, kind: i32, data: *const c_void, count: usize);
    pub fn rorb_out_failed(out: *mut rorb_out) -> i32;

    pub fn rorb_in_read_boolean(r#in: *mut rorb_in) -> i32;
    pub fn rorb_in_read_octet(r#in: *mut rorb_in) -> u8;
    pub fn rorb_in_read_char(r#in: *mut rorb_in) -> c_char;
    pub fn rorb_in_read_wchar(r#in: *mut rorb_in) -> u16;
    pub fn rorb_in_read_short(r#in: *mut rorb_in) -> i16;
    pub fn rorb_in_read_ushort(r#in: *mut rorb_in) -> u16;
    pub fn rorb_in_read_long(r#in: *mut rorb_in) -> i32;
    pub fn rorb_in_read_ulong(r#in: *mut rorb_in) -> u32;
    pub fn rorb_in_read_longlong(r#in: *mut rorb_in) -> i64;
    pub fn rorb_in_read_ulonglong(r#in: *mut rorb_in) -> u64;
    pub fn rorb_in_read_float(r#in: *mut rorb_in) -> f32;
    pub fn rorb_in_read_double(r#in: *mut rorb_in) -> f64;
    pub fn rorb_in_read_string(r#in: *mut rorb_in, len: *mut usize) -> *const c_char;
    pub fn rorb_in_read_wstring(r#in: *mut rorb_in, len: *mut usize) -> *const u16;
    pub fn rorb_in_read_object(r#in: *mut rorb_in) -> *mut rorb_object;
    pub fn rorb_in_read_typecode(r#in: *mut rorb_in) -> *mut rorb_typecode;
    pub fn rorb_in_read_any(r#in: *mut rorb_in) -> *mut rorb_any;
    pub fn rorb_in_read_array(r#in: *mut rorb_in, kind: i32, data: *mut c_void, count: usize);
    pub fn rorb_in_read_length(r#in: *mut rorb_in, min_element_size: usize) -> u32;
    pub fn rorb_in_fail(r#in: *mut rorb_in);
    pub fn rorb_in_failed(r#in: *mut rorb_in) -> i32;
    pub fn rorb_out_write_null_value(out: *mut rorb_out);
    pub fn rorb_out_begin_value(
        out: *mut rorb_out,
        key: u64,
        count: usize,
        ids: *const *const c_char,
        id_lens: *const usize,
        chunked: i32,
    ) -> i32;
    pub fn rorb_out_end_value(out: *mut rorb_out);
    pub fn rorb_in_read_value_header(
        r#in: *mut rorb_in,
        slot: *mut usize,
        id_count: *mut usize,
    ) -> i32;
    pub fn rorb_in_value_id(r#in: *mut rorb_in, index: usize, len: *mut usize) -> *const c_char;
    pub fn rorb_in_end_value(r#in: *mut rorb_in);
    pub fn rorb_register_value_type(tc: *const rorb_typecode);
    pub fn rorb_set_value_type_resolver(
        resolve: Option<
            unsafe extern "C" fn(*mut c_void, *const c_char, usize) -> *mut rorb_typecode,
        >,
        context: *mut c_void,
    );
    pub fn rorb_tc_value(
        kind: u32,
        id: *const c_char,
        id_len: usize,
        name: *const c_char,
        name_len: usize,
        modifier: i16,
        base: *const rorb_typecode,
        count: usize,
        names: *const *const c_char,
        name_lens: *const usize,
        types: *const *const rorb_typecode,
        visibilities: *const i16,
        out: *mut *mut rorb_typecode,
        exc: Exc,
    ) -> rorb_status;

    // servants
    pub fn rorb_servant_get(
        this: *mut c_void,
        repository_id: *const c_char,
        len: usize,
        vtable: *const rorb_servant_vtable,
    ) -> *mut rorb_servant;
    pub fn rorb_servant_release(servant: *mut rorb_servant);
    pub fn rorb_servant_current_reference(
        servant: *mut rorb_servant,
        obj: *mut *mut rorb_object,
    ) -> i32;
    pub fn rorb_server_reply_output(reply: *mut rorb_server_reply) -> *mut rorb_out;
    pub fn rorb_server_reply_user_exception(
        reply: *mut rorb_server_reply,
        id: *const c_char,
        len: usize,
    ) -> *mut rorb_out;
    pub fn rorb_server_reply_system_exception(
        reply: *mut rorb_server_reply,
        id: *const c_char,
        len: usize,
        minor: u32,
        completed: u32,
    );

    // POA
    pub fn rorb_poa_clone(poa: *mut rorb_poa) -> *mut rorb_poa;
    pub fn rorb_poa_release(poa: *mut rorb_poa);
    pub fn rorb_poa_name(poa: *mut rorb_poa, len: *mut usize) -> *const c_char;
    pub fn rorb_poa_parent(poa: *mut rorb_poa) -> *mut rorb_poa;
    pub fn rorb_poa_children(
        poa: *mut rorb_poa,
        context: *mut c_void,
        each: unsafe extern "C" fn(*mut c_void, *mut rorb_poa),
    );
    pub fn rorb_poa_the_manager(poa: *mut rorb_poa) -> *mut rorb_poa_manager;
    pub fn rorb_poa_policies(poa: *mut rorb_poa, policies: *mut rorb_policy);
    pub fn rorb_poa_create_poa(
        poa: *mut rorb_poa,
        name: *const c_char,
        len: usize,
        manager: *mut rorb_poa_manager,
        policies: *const rorb_policy,
        count: usize,
        child: *mut *mut rorb_poa,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_find_poa(
        poa: *mut rorb_poa,
        name: *const c_char,
        len: usize,
        activate_it: i32,
        child: *mut *mut rorb_poa,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_destroy(
        poa: *mut rorb_poa,
        etherealize: i32,
        wait_for_completion: i32,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_activate_object(
        poa: *mut rorb_poa,
        servant: *mut rorb_servant,
        id: *mut rorb_bytes,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_activate_object_with_id(
        poa: *mut rorb_poa,
        id: *const u8,
        len: usize,
        servant: *mut rorb_servant,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_deactivate_object(
        poa: *mut rorb_poa,
        id: *const u8,
        len: usize,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_servant_to_id(
        poa: *mut rorb_poa,
        servant: *mut rorb_servant,
        id: *mut rorb_bytes,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_servant_to_reference(
        poa: *mut rorb_poa,
        servant: *mut rorb_servant,
        obj: *mut *mut rorb_object,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_id_to_servant(
        poa: *mut rorb_poa,
        id: *const u8,
        len: usize,
        this: *mut *mut c_void,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_id_to_reference(
        poa: *mut rorb_poa,
        id: *const u8,
        len: usize,
        obj: *mut *mut rorb_object,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_reference_to_id(
        poa: *mut rorb_poa,
        obj: *mut rorb_object,
        id: *mut rorb_bytes,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_reference_to_servant(
        poa: *mut rorb_poa,
        obj: *mut rorb_object,
        this: *mut *mut c_void,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_create_reference(
        poa: *mut rorb_poa,
        intf: *const c_char,
        len: usize,
        obj: *mut *mut rorb_object,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_create_reference_with_id(
        poa: *mut rorb_poa,
        id: *const u8,
        id_len: usize,
        intf: *const c_char,
        len: usize,
        obj: *mut *mut rorb_object,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_get_servant(
        poa: *mut rorb_poa,
        this: *mut *mut c_void,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_set_servant(
        poa: *mut rorb_poa,
        servant: *mut rorb_servant,
        exc: Exc,
    ) -> rorb_status;

    pub fn rorb_manager_error_forward(error: *mut rorb_manager_error, obj: *mut rorb_object);
    pub fn rorb_manager_error_system(
        error: *mut rorb_manager_error,
        id: *const c_char,
        len: usize,
        minor: u32,
        completed: u32,
    );
    pub fn rorb_poa_set_servant_activator(
        poa: *mut rorb_poa,
        this: *mut c_void,
        vtable: *const rorb_servant_activator_vtable,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_set_servant_locator(
        poa: *mut rorb_poa,
        this: *mut c_void,
        vtable: *const rorb_servant_locator_vtable,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_get_servant_manager(
        poa: *mut rorb_poa,
        kind: *mut i32,
        this: *mut *mut c_void,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_set_the_activator(
        poa: *mut rorb_poa,
        this: *mut c_void,
        vtable: *const rorb_adapter_activator_vtable,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_the_activator(poa: *mut rorb_poa) -> *mut c_void;

    pub fn rorb_current_get_poa(poa: *mut *mut rorb_poa, exc: Exc) -> rorb_status;
    pub fn rorb_current_get_object_id(id: *mut rorb_bytes, exc: Exc) -> rorb_status;
    pub fn rorb_current_get_reference(obj: *mut *mut rorb_object, exc: Exc) -> rorb_status;
    pub fn rorb_current_get_servant(this: *mut *mut c_void, exc: Exc) -> rorb_status;

    pub fn rorb_poa_manager_clone(manager: *mut rorb_poa_manager) -> *mut rorb_poa_manager;
    pub fn rorb_poa_manager_release(manager: *mut rorb_poa_manager);
    pub fn rorb_poa_manager_get_state(manager: *mut rorb_poa_manager) -> u32;
    pub fn rorb_poa_manager_activate(manager: *mut rorb_poa_manager, exc: Exc) -> rorb_status;
    pub fn rorb_poa_manager_hold_requests(
        manager: *mut rorb_poa_manager,
        wait_for_completion: i32,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_manager_discard_requests(
        manager: *mut rorb_poa_manager,
        wait_for_completion: i32,
        exc: Exc,
    ) -> rorb_status;
    pub fn rorb_poa_manager_deactivate(
        manager: *mut rorb_poa_manager,
        etherealize: i32,
        wait_for_completion: i32,
        exc: Exc,
    ) -> rorb_status;
}
