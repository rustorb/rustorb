/*
 * rustorb.h: the stable C ABI of rustorb (crates/rustorb-ffi).
 *
 * The C++ binding (cpp/include/rustorb) is written on top of this header,
 * and so will the Java one be. It is not meant to be used directly by
 * applications.
 *
 * Conventions
 *
 * - Handles (rorb_orb, rorb_object, rorb_poa, rorb_poa_manager,
 *   rorb_servant, rorb_typecode, rorb_any) are reference counted on the Rust side. A function that
 *   returns a handle gives the caller one reference, which it gives back
 *   with the matching *_release function. *_clone adds a reference.
 *   Handles may be used from any thread.
 * - Fallible functions return an rorb_status and fill the rorb_exception
 *   the caller passes (never NULL). After RORB_SYSTEM_EXCEPTION or
 *   RORB_LOCAL_EXCEPTION the caller frees it with rorb_exception_clear.
 * - Strings are UTF-8 and passed as a pointer and a length, without a
 *   terminating NUL. Wide strings are UTF-16 code units.
 * - Memory returned in an rorb_bytes is freed with rorb_bytes_free.
 * - Streams (rorb_out, rorb_in) and server replies (rorb_server_reply) are
 *   only valid during the callback that receives them. Errors on a stream
 *   are sticky: after the first one, writes do nothing and reads return
 *   zeros, and the error is reported when the callback returns.
 * - Callbacks must not unwind (throw a C++ exception) into Rust.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
 */

#ifndef RUSTORB_RUSTORB_H
#define RUSTORB_RUSTORB_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Version of this ABI; rorb_abi_version() returns the library's. */
#define RORB_ABI_VERSION 1

typedef struct rorb_orb rorb_orb;
typedef struct rorb_object rorb_object;
typedef struct rorb_poa rorb_poa;
typedef struct rorb_poa_manager rorb_poa_manager;
typedef struct rorb_servant rorb_servant;
typedef struct rorb_out rorb_out;
typedef struct rorb_in rorb_in;
typedef struct rorb_server_reply rorb_server_reply;
typedef struct rorb_typecode rorb_typecode;
typedef struct rorb_any rorb_any;

/* --- status and exceptions ------------------------------------------- */

typedef int32_t rorb_status;

/* Success. */
#define RORB_OK 0
/* A CORBA system exception, described in the rorb_exception. */
#define RORB_SYSTEM_EXCEPTION 1
/* A user exception, taken by the read_user_exception callback. */
#define RORB_USER_EXCEPTION 2
/* An exception of a local interface (POA, POAManager, Current, ORB),
   named by rorb_exception.local. */
#define RORB_LOCAL_EXCEPTION 3
/* A callback failed and keeps its own error (a C++ exception, say). */
#define RORB_CALLBACK_FAILED 4

/* CORBA::CompletionStatus */
#define RORB_COMPLETED_YES 0
#define RORB_COMPLETED_NO 1
#define RORB_COMPLETED_MAYBE 2

/* rorb_exception.local after RORB_LOCAL_EXCEPTION */
#define RORB_ADAPTER_ALREADY_EXISTS 1 /* POA::AdapterAlreadyExists */
#define RORB_ADAPTER_NON_EXISTENT 2   /* POA::AdapterNonExistent */
#define RORB_INVALID_POLICY 3         /* POA::InvalidPolicy (index in .index) */
#define RORB_SERVANT_ALREADY_ACTIVE 4 /* POA::ServantAlreadyActive */
#define RORB_OBJECT_ALREADY_ACTIVE 5  /* POA::ObjectAlreadyActive */
#define RORB_SERVANT_NOT_ACTIVE 6     /* POA::ServantNotActive */
#define RORB_OBJECT_NOT_ACTIVE 7      /* POA::ObjectNotActive */
#define RORB_WRONG_POLICY 8           /* POA::WrongPolicy */
#define RORB_WRONG_ADAPTER 9          /* POA::WrongAdapter */
#define RORB_ADAPTER_INACTIVE 10      /* POAManager::AdapterInactive */
#define RORB_NO_SERVANT 11            /* POA::NoServant */
#define RORB_NO_CONTEXT 12            /* Current::NoContext */
#define RORB_INVALID_NAME 13          /* ORB::InvalidName */
#define RORB_TC_BAD_KIND 14           /* TypeCode::BadKind */
#define RORB_TC_BOUNDS 15             /* TypeCode::Bounds */
#define RORB_POLICY_ERROR 16          /* CORBA::PolicyError (reason in .index) */
#define RORB_DYN_INVALID_VALUE 17     /* DynamicAny::DynAny::InvalidValue */
#define RORB_DYN_TYPE_MISMATCH 18     /* DynamicAny::DynAny::TypeMismatch */
#define RORB_DYN_INCONSISTENT_TYPECODE 19 /* DynamicAny::DynAnyFactory::InconsistentTypeCode */
#define RORB_DYN_MUST_TRUNCATE 20     /* DynamicAny::MustTruncate */

typedef struct rorb_exception {
  /* RORB_SYSTEM_EXCEPTION: the repository id, such as
     "IDL:omg.org/CORBA/TRANSIENT:1.0" (owned, NUL terminated). */
  char* repository_id;
  size_t repository_id_len;
  uint32_t minor;
  uint32_t completed;
  /* RORB_LOCAL_EXCEPTION: which one, and the policy index of
     RORB_INVALID_POLICY or the reason of RORB_POLICY_ERROR. */
  uint32_t local;
  uint32_t index;
} rorb_exception;

/* Frees what an rorb_exception holds and zeroes it. */
void rorb_exception_clear(rorb_exception* exc);

typedef struct rorb_bytes {
  uint8_t* data;
  size_t len;
} rorb_bytes;

void rorb_bytes_free(rorb_bytes bytes);

uint32_t rorb_abi_version(void);

/* --- TypeCodes and anys ------------------------------------------------ */

/* CORBA::TCKind values (tk_null = 0 ... tk_home = 36). */
#define RORB_TK_NULL 0
#define RORB_TK_VOID 1
#define RORB_TK_SHORT 2
#define RORB_TK_LONG 3
#define RORB_TK_USHORT 4
#define RORB_TK_ULONG 5
#define RORB_TK_FLOAT 6
#define RORB_TK_DOUBLE 7
#define RORB_TK_BOOLEAN 8
#define RORB_TK_CHAR 9
#define RORB_TK_OCTET 10
#define RORB_TK_ANY 11
#define RORB_TK_TYPECODE 12
#define RORB_TK_PRINCIPAL 13
#define RORB_TK_OBJREF 14
#define RORB_TK_STRUCT 15
#define RORB_TK_UNION 16
#define RORB_TK_ENUM 17
#define RORB_TK_STRING 18
#define RORB_TK_SEQUENCE 19
#define RORB_TK_ARRAY 20
#define RORB_TK_ALIAS 21
#define RORB_TK_EXCEPT 22
#define RORB_TK_LONGLONG 23
#define RORB_TK_ULONGLONG 24
#define RORB_TK_LONGDOUBLE 25
#define RORB_TK_WCHAR 26
#define RORB_TK_WSTRING 27
#define RORB_TK_FIXED 28
#define RORB_TK_VALUE 29
#define RORB_TK_VALUE_BOX 30
#define RORB_TK_NATIVE 31
#define RORB_TK_ABSTRACT_INTERFACE 32
#define RORB_TK_LOCAL_INTERFACE 33
#define RORB_TK_COMPONENT 34
#define RORB_TK_HOME 35
#define RORB_TK_EVENT 36

/* TypeCodes are immutable values. The constructors fail with BAD_PARAM
   for parameters that make no valid type (an invalid name, duplicate
   member names or labels, a member type that cannot be one...). The
   queries fail with RORB_TC_BAD_KIND or RORB_TC_BOUNDS. */
rorb_typecode* rorb_tc_clone(const rorb_typecode* tc);
void rorb_tc_release(rorb_typecode* tc);

/* The TypeCode of a kind without parameters (tk_string and tk_wstring
   are unbounded). */
rorb_status rorb_tc_simple(uint32_t kind, rorb_typecode** out, rorb_exception* exc);
/* string (wide 0) or wstring (wide 1) with a bound (0 for none). */
rorb_status rorb_tc_string(int32_t wide, uint32_t bound, rorb_typecode** out,
                           rorb_exception* exc);
rorb_status rorb_tc_fixed(uint16_t digits, int16_t scale, rorb_typecode** out,
                          rorb_exception* exc);
rorb_status rorb_tc_sequence(const rorb_typecode* content, uint32_t bound, rorb_typecode** out,
                             rorb_exception* exc);
rorb_status rorb_tc_array(const rorb_typecode* content, uint32_t length, rorb_typecode** out,
                          rorb_exception* exc);
/* kind is RORB_TK_ALIAS or RORB_TK_VALUE_BOX. */
rorb_status rorb_tc_alias(uint32_t kind, const char* id, size_t id_len, const char* name,
                          size_t name_len, const rorb_typecode* content, rorb_typecode** out,
                          rorb_exception* exc);
/* kind is RORB_TK_OBJREF, RORB_TK_ABSTRACT_INTERFACE,
   RORB_TK_LOCAL_INTERFACE, RORB_TK_NATIVE, RORB_TK_COMPONENT or
   RORB_TK_HOME. */
rorb_status rorb_tc_interface(uint32_t kind, const char* id, size_t id_len, const char* name,
                              size_t name_len, rorb_typecode** out, rorb_exception* exc);
/* kind is RORB_TK_STRUCT or RORB_TK_EXCEPT; count members, each a name
   (names[i], name_lens[i]) and a type. */
rorb_status rorb_tc_struct(uint32_t kind, const char* id, size_t id_len, const char* name,
                           size_t name_len, size_t count, const char* const* names,
                           const size_t* name_lens, const rorb_typecode* const* types,
                           rorb_typecode** out, rorb_exception* exc);
/* One entry per label: labels[i] is the value of the discriminator as a
   number (boolean 0 or 1, char and wchar their code, an enumerator its
   position; signed values as two's complement), except for the entry
   default_index (-1 for none), the default member. A member with several
   labels has several consecutive entries with the same name. */
rorb_status rorb_tc_union(const char* id, size_t id_len, const char* name, size_t name_len,
                          const rorb_typecode* discriminator, size_t count,
                          const uint64_t* labels, int32_t default_index,
                          const char* const* names, const size_t* name_lens,
                          const rorb_typecode* const* types, rorb_typecode** out,
                          rorb_exception* exc);
rorb_status rorb_tc_enum(const char* id, size_t id_len, const char* name, size_t name_len,
                         size_t count, const char* const* names, const size_t* name_lens,
                         rorb_typecode** out, rorb_exception* exc);
/* A valuetype (kind RORB_TK_VALUE) or eventtype (RORB_TK_EVENT).
   modifier is CORBA::ValueModifier (VM_NONE 0, VM_CUSTOM 1, VM_ABSTRACT 2,
   VM_TRUNCATABLE 3), base the concrete base type or NULL, and each member
   has a visibility (PRIVATE_MEMBER 0, PUBLIC_MEMBER 1). */
rorb_status rorb_tc_value(uint32_t kind, const char* id, size_t id_len, const char* name,
                          size_t name_len, int16_t modifier, const rorb_typecode* base,
                          size_t count, const char* const* names, const size_t* name_lens,
                          const rorb_typecode* const* types, const int16_t* visibilities,
                          rorb_typecode** out, rorb_exception* exc);
/* ORB::create_recursive_tc: a placeholder for the struct, union or
   valuetype with this repository id, replaced when a TypeCode of that
   type is made with it in its members. */
rorb_status rorb_tc_recursive(const char* id, size_t id_len, rorb_typecode** out,
                              rorb_exception* exc);

uint32_t rorb_tc_kind(const rorb_typecode* tc);
/* Strings valid while tc is. */
rorb_status rorb_tc_id(const rorb_typecode* tc, const char** s, size_t* len,
                       rorb_exception* exc);
rorb_status rorb_tc_name(const rorb_typecode* tc, const char** s, size_t* len,
                         rorb_exception* exc);
rorb_status rorb_tc_member_count(const rorb_typecode* tc, uint32_t* count, rorb_exception* exc);
rorb_status rorb_tc_member_name(const rorb_typecode* tc, uint32_t index, const char** s,
                                size_t* len, rorb_exception* exc);
rorb_status rorb_tc_member_type(const rorb_typecode* tc, uint32_t index, rorb_typecode** out,
                                rorb_exception* exc);
/* An any holding the label (the octet 0 for the default member). */
rorb_status rorb_tc_member_label(const rorb_typecode* tc, uint32_t index, rorb_any** out,
                                 rorb_exception* exc);
rorb_status rorb_tc_discriminator_type(const rorb_typecode* tc, rorb_typecode** out,
                                       rorb_exception* exc);
rorb_status rorb_tc_default_index(const rorb_typecode* tc, int32_t* index, rorb_exception* exc);
rorb_status rorb_tc_length(const rorb_typecode* tc, uint32_t* length, rorb_exception* exc);
rorb_status rorb_tc_content_type(const rorb_typecode* tc, rorb_typecode** out,
                                 rorb_exception* exc);
rorb_status rorb_tc_fixed_digits(const rorb_typecode* tc, uint16_t* digits, rorb_exception* exc);
rorb_status rorb_tc_fixed_scale(const rorb_typecode* tc, int16_t* scale, rorb_exception* exc);
rorb_status rorb_tc_member_visibility(const rorb_typecode* tc, uint32_t index, int16_t* v,
                                      rorb_exception* exc);
rorb_status rorb_tc_type_modifier(const rorb_typecode* tc, int16_t* v, rorb_exception* exc);
/* *out is NULL for a valuetype without one. */
rorb_status rorb_tc_concrete_base_type(const rorb_typecode* tc, rorb_typecode** out,
                                       rorb_exception* exc);
int32_t rorb_tc_equal(const rorb_typecode* a, const rorb_typecode* b);
int32_t rorb_tc_equivalent(const rorb_typecode* a, const rorb_typecode* b);
/* TypeCode::get_compact_typecode */
rorb_typecode* rorb_tc_compact(const rorb_typecode* tc);
/* The TypeCode with its aliases removed. */
rorb_typecode* rorb_tc_unaliased(const rorb_typecode* tc);

/* An any: a TypeCode and a value of that type. rorb_any_new makes an
   empty one (tk_null). */
rorb_any* rorb_any_new(void);
rorb_any* rorb_any_clone(const rorb_any* any);
void rorb_any_release(rorb_any* any);
rorb_typecode* rorb_any_type(const rorb_any* any);
/* Replaces the TypeCode with an equivalent one (an alias, say), else
   BAD_TYPECODE. */
rorb_status rorb_any_set_type(rorb_any* any, const rorb_typecode* tc, rorb_exception* exc);
/* Whether the TypeCodes are equal and the values the same (floats
   compared exactly, object references with is_equivalent). */
int32_t rorb_any_equal(const rorb_any* a, const rorb_any* b);
/* If the any holds a value of a discriminator type (an integer, boolean,
   char, wchar or enum), stores it as a union label (see rorb_tc_union) and
   returns 1; else returns 0. */
int32_t rorb_any_label(const rorb_any* any, uint64_t* label);
/* Makes an any of type tc holding the value write(context, out) writes,
   as it writes an argument of that type (an exception without its
   repository id). A value that does not fit the type is BAD_PARAM.
   write returns RORB_OK or RORB_CALLBACK_FAILED. */
rorb_status rorb_any_insert(const rorb_typecode* tc,
                            rorb_status (*write)(void* context, rorb_out* out), void* context,
                            rorb_any** out, rorb_exception* exc);
/* Calls read(context, in) to read the value, as an argument of the type
   of the any; the caller has checked that type. A stream error is
   MARSHAL. Object references read belong to the ORB of the first one in
   the value, else to the first ORB initialised; without either, they
   are BAD_INV_ORDER. */
rorb_status rorb_any_extract(const rorb_any* any,
                             rorb_status (*read)(void* context, rorb_in* in), void* context,
                             rorb_exception* exc);

/* --- DynamicAny --------------------------------------------------------------

   An rorb_dynany handle is one reference to a DynAny of the core; clones
   are the same DynAny, released one by one. The operations of every
   derived interface (DynStruct, DynUnion...) take any handle: on a DynAny
   of another kind they are BAD_OPERATION. Failures are RORB_LOCAL_EXCEPTION
   with RORB_DYN_* or system exceptions (OBJECT_NOT_EXIST after destroy).
   Values go in and out as anys (see docs/dynany.md). Strings returned in
   rorb_bytes are UTF-8. */

typedef struct rorb_dynany rorb_dynany;

/* DynAnyFactory: create_dyn_any (truncate 1) or
   create_dyn_any_without_truncation (0), create_dyn_any_from_type_code. */
rorb_status rorb_dynany_create(const rorb_any* any, int32_t truncate, rorb_dynany** out,
                               rorb_exception* exc);
rorb_status rorb_dynany_from_typecode(const rorb_typecode* tc, rorb_dynany** out,
                                      rorb_exception* exc);
rorb_dynany* rorb_dynany_clone(const rorb_dynany* d);
void rorb_dynany_release(rorb_dynany* d);
/* The kind of the unaliased type, which tells the derived interface. */
uint32_t rorb_dynany_kind(const rorb_dynany* d);
/* 1 if both handles are the same DynAny. */
int32_t rorb_dynany_same(const rorb_dynany* a, const rorb_dynany* b);

rorb_status rorb_dynany_type(const rorb_dynany* d, rorb_typecode** out, rorb_exception* exc);
/* The type of the DynAny insert and get work on: the current component,
   else the DynAny itself. */
rorb_status rorb_dynany_target_type(const rorb_dynany* d, rorb_typecode** out,
                                    rorb_exception* exc);
rorb_status rorb_dynany_assign(const rorb_dynany* d, const rorb_dynany* other,
                               rorb_exception* exc);
rorb_status rorb_dynany_from_any(const rorb_dynany* d, const rorb_any* any, rorb_exception* exc);
rorb_status rorb_dynany_to_any(const rorb_dynany* d, rorb_any** out, rorb_exception* exc);
rorb_status rorb_dynany_equal(const rorb_dynany* d, const rorb_dynany* other, int32_t* out,
                              rorb_exception* exc);
rorb_status rorb_dynany_destroy(const rorb_dynany* d, rorb_exception* exc);
rorb_status rorb_dynany_copy(const rorb_dynany* d, rorb_dynany** out, rorb_exception* exc);
/* insert_<type>: the value of the any (its type is not looked at) as a
   value of the kind (RORB_TK_OBJREF for insert_reference, RORB_TK_VALUE for
   insert_val). get_<type>: an any of the kind's type (string, wstring,
   Object, the instance's type for RORB_TK_VALUE). */
rorb_status rorb_dynany_insert(const rorb_dynany* d, uint32_t kind, const rorb_any* value,
                               rorb_exception* exc);
rorb_status rorb_dynany_get(const rorb_dynany* d, uint32_t kind, rorb_any** out,
                            rorb_exception* exc);
/* insert_<type>_seq: the elements of an any of a sequence of the kind;
   get_<type>_seq: an any of an unbounded sequence of the kind. */
rorb_status rorb_dynany_insert_seq(const rorb_dynany* d, uint32_t kind, const rorb_any* value,
                                   rorb_exception* exc);
rorb_status rorb_dynany_get_seq(const rorb_dynany* d, uint32_t kind, rorb_any** out,
                                rorb_exception* exc);
rorb_status rorb_dynany_insert_dyn_any(const rorb_dynany* d, const rorb_dynany* value,
                                       rorb_exception* exc);
rorb_status rorb_dynany_get_dyn_any(const rorb_dynany* d, rorb_dynany** out,
                                    rorb_exception* exc);
/* NO_IMPLEMENT (long double, abstract interfaces), or OBJECT_NOT_EXIST. */
rorb_status rorb_dynany_no_implement(const rorb_dynany* d, rorb_exception* exc);
rorb_status rorb_dynany_seek(const rorb_dynany* d, int32_t index, int32_t* out,
                             rorb_exception* exc);
rorb_status rorb_dynany_rewind(const rorb_dynany* d, rorb_exception* exc);
rorb_status rorb_dynany_next(const rorb_dynany* d, int32_t* out, rorb_exception* exc);
rorb_status rorb_dynany_component_count(const rorb_dynany* d, uint32_t* out,
                                        rorb_exception* exc);
/* NULL in *out at position -1. */
rorb_status rorb_dynany_current_component(const rorb_dynany* d, rorb_dynany** out,
                                          rorb_exception* exc);

/* DynFixed, DynEnum */
rorb_status rorb_dynany_get_fixed_value(const rorb_dynany* d, rorb_bytes* out,
                                        rorb_exception* exc);
rorb_status rorb_dynany_set_fixed_value(const rorb_dynany* d, const char* value, size_t len,
                                        int32_t* out, rorb_exception* exc);
rorb_status rorb_dynany_get_as_string(const rorb_dynany* d, rorb_bytes* out,
                                      rorb_exception* exc);
rorb_status rorb_dynany_set_as_string(const rorb_dynany* d, const char* value, size_t len,
                                      rorb_exception* exc);
rorb_status rorb_dynany_get_as_ulong(const rorb_dynany* d, uint32_t* out, rorb_exception* exc);
rorb_status rorb_dynany_set_as_ulong(const rorb_dynany* d, uint32_t value, rorb_exception* exc);

/* DynStruct, DynValue. get_members is member_count (failing as
   get_members does) and member_at for each. */
rorb_status rorb_dynany_current_member_name(const rorb_dynany* d, rorb_bytes* out,
                                            rorb_exception* exc);
rorb_status rorb_dynany_current_member_kind(const rorb_dynany* d, uint32_t* out,
                                            rorb_exception* exc);
rorb_status rorb_dynany_member_count(const rorb_dynany* d, uint32_t* out, rorb_exception* exc);
rorb_status rorb_dynany_member_at(const rorb_dynany* d, uint32_t index, rorb_bytes* name,
                                  rorb_dynany** out, rorb_exception* exc);
rorb_status rorb_dynany_set_members(const rorb_dynany* d, size_t count,
                                    const char* const* names, const size_t* name_lens,
                                    const rorb_any* const* values, rorb_exception* exc);
rorb_status rorb_dynany_set_members_as_dyn_any(const rorb_dynany* d, size_t count,
                                               const char* const* names,
                                               const size_t* name_lens,
                                               const rorb_dynany* const* values,
                                               rorb_exception* exc);

/* DynUnion */
rorb_status rorb_dynany_get_discriminator(const rorb_dynany* d, rorb_dynany** out,
                                          rorb_exception* exc);
rorb_status rorb_dynany_set_discriminator(const rorb_dynany* d, const rorb_dynany* disc,
                                          rorb_exception* exc);
rorb_status rorb_dynany_set_to_default_member(const rorb_dynany* d, rorb_exception* exc);
rorb_status rorb_dynany_set_to_no_active_member(const rorb_dynany* d, rorb_exception* exc);
rorb_status rorb_dynany_has_no_active_member(const rorb_dynany* d, int32_t* out,
                                             rorb_exception* exc);
rorb_status rorb_dynany_discriminator_kind(const rorb_dynany* d, uint32_t* out,
                                           rorb_exception* exc);
rorb_status rorb_dynany_member(const rorb_dynany* d, rorb_dynany** out, rorb_exception* exc);
rorb_status rorb_dynany_member_name(const rorb_dynany* d, rorb_bytes* out, rorb_exception* exc);
rorb_status rorb_dynany_member_kind(const rorb_dynany* d, uint32_t* out, rorb_exception* exc);

/* DynSequence, DynArray. get_elements is element_count and element_at. */
rorb_status rorb_dynany_get_length(const rorb_dynany* d, uint32_t* out, rorb_exception* exc);
rorb_status rorb_dynany_set_length(const rorb_dynany* d, uint32_t len, rorb_exception* exc);
rorb_status rorb_dynany_element_count(const rorb_dynany* d, uint32_t* out,
                                      rorb_exception* exc);
rorb_status rorb_dynany_element_at(const rorb_dynany* d, uint32_t index, rorb_dynany** out,
                                   rorb_exception* exc);
rorb_status rorb_dynany_set_elements(const rorb_dynany* d, size_t count,
                                     const rorb_any* const* values, rorb_exception* exc);
rorb_status rorb_dynany_set_elements_as_dyn_any(const rorb_dynany* d, size_t count,
                                                const rorb_dynany* const* values,
                                                rorb_exception* exc);

/* DynValueCommon, DynValueBox */
rorb_status rorb_dynany_is_null(const rorb_dynany* d, int32_t* out, rorb_exception* exc);
rorb_status rorb_dynany_set_to_null(const rorb_dynany* d, rorb_exception* exc);
rorb_status rorb_dynany_set_to_value(const rorb_dynany* d, rorb_exception* exc);
rorb_status rorb_dynany_get_boxed_value(const rorb_dynany* d, rorb_any** out,
                                        rorb_exception* exc);
rorb_status rorb_dynany_set_boxed_value(const rorb_dynany* d, const rorb_any* value,
                                        rorb_exception* exc);
rorb_status rorb_dynany_get_boxed_value_as_dyn_any(const rorb_dynany* d, rorb_dynany** out,
                                                   rorb_exception* exc);
rorb_status rorb_dynany_set_boxed_value_as_dyn_any(const rorb_dynany* d,
                                                   const rorb_dynany* value,
                                                   rorb_exception* exc);

/* --- ORB --------------------------------------------------------------- */

/* CORBA::ORB_init: removes the ORB options (-ORB...) from argv and
   *argc; argv[0] is kept. */
rorb_status rorb_orb_init(int* argc, char** argv, rorb_orb** orb, rorb_exception* exc);
rorb_orb* rorb_orb_clone(rorb_orb* orb);
void rorb_orb_release(rorb_orb* orb);

/* *obj is NULL for a nil reference. */
rorb_status rorb_orb_string_to_object(rorb_orb* orb, const char* s, size_t len,
                                      rorb_object** obj, rorb_exception* exc);
/* obj may be NULL (a nil reference). */
rorb_status rorb_orb_object_to_string(rorb_orb* orb, rorb_object* obj, rorb_bytes* s,
                                      rorb_exception* exc);
/* Not for "RootPOA" and "POACurrent", which are local objects: see
   rorb_orb_root_poa. Unknown names give RORB_INVALID_NAME. */
rorb_status rorb_orb_resolve_initial_references(rorb_orb* orb, const char* name, size_t len,
                                                rorb_object** obj, rorb_exception* exc);
/* Calls each(context, name, len) for each initial service. */
void rorb_orb_list_initial_services(rorb_orb* orb, void* context,
                                    void (*each)(void* context, const char* name, size_t len));
rorb_status rorb_orb_root_poa(rorb_orb* orb, rorb_poa** poa, rorb_exception* exc);
/* Blocks until the ORB is shut down, running MAIN_THREAD_MODEL requests. */
void rorb_orb_run(rorb_orb* orb);
rorb_status rorb_orb_work_pending(rorb_orb* orb, int32_t* pending, rorb_exception* exc);
rorb_status rorb_orb_perform_work(rorb_orb* orb, rorb_exception* exc);
rorb_status rorb_orb_shutdown(rorb_orb* orb, int32_t wait_for_completion, rorb_exception* exc);
rorb_status rorb_orb_destroy(rorb_orb* orb, rorb_exception* exc);
/* Client timeouts in microseconds. For the ORB, 0 is no limit. For the
   calling thread (every ORB) and for an object reference (and its
   clones), negative unsets the timeout so that the next one applies, and
   0 is no limit; a call uses the thread's, else the reference's, else the
   ORB's. The getters return the same encoding. */
void rorb_orb_set_call_timeout(rorb_orb* orb, int64_t us);
int64_t rorb_orb_call_timeout(rorb_orb* orb);
void rorb_orb_set_connect_timeout(rorb_orb* orb, int64_t us);
int64_t rorb_orb_connect_timeout(rorb_orb* orb);
void rorb_set_thread_call_timeout(int64_t us);
int64_t rorb_thread_call_timeout(void);
/* The RootPOA of the first ORB initialised and not yet destroyed (the
   default POA of servants). */
rorb_status rorb_default_root_poa(rorb_poa** poa, rorb_exception* exc);
/* A new handle of the first ORB initialised and not yet destroyed; NULL if
   there is none. */
rorb_orb* rorb_default_orb(void);

/* --- policies ---------------------------------------------------------- */

/* A policy crosses the ABI as its type and a value: the enumerator of a
   POA policy (types 16 to 22), the relative_expiry of a
   Messaging::RelativeRoundtripTimeoutPolicy (type 32, units of 100 ns). */
#define RORB_RELATIVE_RT_TIMEOUT_POLICY_TYPE 32
/* CORBA::SetOverrideType */
#define RORB_SET_OVERRIDE 0
#define RORB_ADD_OVERRIDE 1

/* Called with each policy of a list. */
typedef void (*rorb_each_policy)(void* context, uint32_t type, uint64_t value);

/* ORB::create_policy: the value of the policy of `type` in the any.
   RORB_POLICY_ERROR with the reason (BAD_POLICY 0 for an unknown type,
   BAD_POLICY_TYPE 2 for an any of the wrong type, BAD_POLICY_VALUE 3). */
rorb_status rorb_orb_create_policy(rorb_orb* orb, uint32_t type, const rorb_any* value,
                                   uint64_t* raw_value, rorb_exception* exc);
/* ORB::register_initial_reference. RORB_INVALID_NAME for an empty name or
   one that is already an initial reference of the core (-ORBInitRef or
   registered); BAD_PARAM (minor 27) for a NULL obj. */
rorb_status rorb_orb_register_initial_reference(rorb_orb* orb, const char* name, size_t len,
                                                rorb_object* obj, rorb_exception* exc);
/* register_initial_reference for an object of the binding that has no
   rorb_object (a local object): the ORB keeps `object` until it is
   destroyed, then calls release(object) (at once if registering fails).
   Errors as above. resolve calls
   each(context, object) and returns 1 if `name` is such an object, else
   0; the object is only guaranteed to live during the call. */
rorb_status rorb_orb_register_local_initial_reference(rorb_orb* orb, const char* name,
                                                      size_t len, void* object,
                                                      void (*release)(void*),
                                                      rorb_exception* exc);
int32_t rorb_orb_resolve_local_initial_reference(rorb_orb* orb, const char* name, size_t len,
                                                 void* context,
                                                 void (*each)(void* context, void* object));
/* The overrides of the ORB (its PolicyManager), of the calling thread
   (PolicyCurrent) and of an object reference. Setting them raises
   NO_PERMISSION for a policy that is not a client side override (the POA
   policies) and BAD_PARAM (minor 30) for two policies of one type. The
   getters call `each` with the overrides of the `count` types (all types
   if count is 0). The timeout overrides are the call timeouts of
   rorb_orb_set_call_timeout and friends. */
rorb_status rorb_orb_set_policy_overrides(rorb_orb* orb, const uint32_t* types,
                                          const uint64_t* values, size_t count,
                                          uint32_t set_add, rorb_exception* exc);
void rorb_orb_get_policy_overrides(rorb_orb* orb, const uint32_t* types, size_t count,
                                   void* context, rorb_each_policy each);
rorb_status rorb_set_thread_policy_overrides(const uint32_t* types, const uint64_t* values,
                                             size_t count, uint32_t set_add,
                                             rorb_exception* exc);
void rorb_thread_policy_overrides(const uint32_t* types, size_t count, void* context,
                                  rorb_each_policy each);
/* Object::set_policy_overrides: a new reference in *result. */
rorb_status rorb_object_set_policy_overrides(rorb_object* obj, const uint32_t* types,
                                             const uint64_t* values, size_t count,
                                             uint32_t set_add, rorb_object** result,
                                             rorb_exception* exc);
void rorb_object_get_policy_overrides(rorb_object* obj, const uint32_t* types, size_t count,
                                      void* context, rorb_each_policy each);
/* Object::get_client_policy (and get_policy): 1 and the value if the
   reference, the thread or the ORB has a policy of `type`, else 0. */
int32_t rorb_object_get_client_policy(rorb_object* obj, uint32_t type, uint64_t* raw_value);
/* Object::validate_connection: binds with a LocateRequest. 1 on success;
   failures are system exceptions (OBJECT_NOT_EXIST when the server does
   not have the object). */
rorb_status rorb_object_validate_connection(rorb_object* obj, int32_t* result,
                                            rorb_exception* exc);

/* --- object references -------------------------------------------------- */

rorb_object* rorb_object_clone(rorb_object* obj);
void rorb_object_release(rorb_object* obj);
rorb_orb* rorb_object_orb(rorb_object* obj);
/* The repository id in the IOR (may be empty), valid while obj lives. */
const char* rorb_object_type_id(rorb_object* obj, size_t* len);
rorb_status rorb_object_is_a(rorb_object* obj, const char* id, size_t len, int32_t* result,
                             rorb_exception* exc);
rorb_status rorb_object_non_existent(rorb_object* obj, int32_t* result, rorb_exception* exc);
int32_t rorb_object_is_equivalent(rorb_object* a, rorb_object* b);
/* The same value for equivalent references, below maximum. */
uint32_t rorb_object_hash(rorb_object* obj, uint32_t maximum);
/* See rorb_orb_set_call_timeout. */
void rorb_object_set_call_timeout(rorb_object* obj, int64_t us);
int64_t rorb_object_call_timeout(rorb_object* obj);

/* How to make one invocation.

   write_args writes the in and inout arguments; it may run more than once
   (when the request is forwarded or sent again). read_reply reads the
   result and the inout and out arguments. read_user_exception is called
   for a user exception with its repository id; it returns
   RORB_USER_EXCEPTION when it took the exception, RORB_SYSTEM_EXCEPTION
   when the operation does not declare it (the invocation then fails with
   UNKNOWN), or RORB_CALLBACK_FAILED. The other callbacks return RORB_OK
   or RORB_CALLBACK_FAILED. Any callback may be NULL. */
typedef struct rorb_invocation {
  const char* operation;
  size_t operation_len;
  int32_t oneway;
  /* Reply timeout in microseconds; negative uses the timeout of the
     thread, the reference or the ORB (rorb_orb_set_call_timeout), and 0
     waits forever. */
  int64_t timeout_us;
  void* context;
  rorb_status (*write_args)(void* context, rorb_out* out);
  rorb_status (*read_reply)(void* context, rorb_in* in);
  rorb_status (*read_user_exception)(void* context, const char* id, size_t id_len, rorb_in* in);
} rorb_invocation;

/* Returns RORB_OK, RORB_SYSTEM_EXCEPTION, RORB_USER_EXCEPTION or
   RORB_CALLBACK_FAILED. A stream error in a read callback is MARSHAL. */
rorb_status rorb_object_invoke(rorb_object* obj, const rorb_invocation* inv, rorb_exception* exc);

/* --- streams --------------------------------------------------------------- */

/* Element kinds of rorb_out_write_array / rorb_in_read_array. */
#define RORB_PRIM_OCTET 0 /* also char */
#define RORB_PRIM_SHORT 1
#define RORB_PRIM_USHORT 2
#define RORB_PRIM_LONG 3
#define RORB_PRIM_ULONG 4
#define RORB_PRIM_LONGLONG 5
#define RORB_PRIM_ULONGLONG 6
#define RORB_PRIM_FLOAT 7
#define RORB_PRIM_DOUBLE 8

void rorb_out_write_boolean(rorb_out* out, int32_t v);
void rorb_out_write_octet(rorb_out* out, uint8_t v);
void rorb_out_write_char(rorb_out* out, char v);
void rorb_out_write_wchar(rorb_out* out, uint16_t v);
void rorb_out_write_short(rorb_out* out, int16_t v);
void rorb_out_write_ushort(rorb_out* out, uint16_t v);
void rorb_out_write_long(rorb_out* out, int32_t v);
void rorb_out_write_ulong(rorb_out* out, uint32_t v);
void rorb_out_write_longlong(rorb_out* out, int64_t v);
void rorb_out_write_ulonglong(rorb_out* out, uint64_t v);
void rorb_out_write_float(rorb_out* out, float v);
void rorb_out_write_double(rorb_out* out, double v);
/* A string in the negotiated code set; s must be UTF-8. */
void rorb_out_write_string(rorb_out* out, const char* s, size_t len);
void rorb_out_write_wstring(rorb_out* out, const uint16_t* s, size_t len);
/* obj may be NULL (a nil reference). */
void rorb_out_write_object(rorb_out* out, rorb_object* obj);
/* A TypeCode with an unresolved recursive placeholder is BAD_TYPECODE. */
void rorb_out_write_typecode(rorb_out* out, const rorb_typecode* tc);
void rorb_out_write_any(rorb_out* out, const rorb_any* any);
/* A fixed of count digits (0 to 9, most significant first) as packed
   decimal; the scale is not on the wire. More than 31 digits is BAD_PARAM. */
void rorb_out_write_fixed(rorb_out* out, const uint8_t* digits, size_t count, int32_t negative);
/* count elements of kind, in the host's representation. */
void rorb_out_write_array(rorb_out* out, int32_t kind, const void* data, size_t count);
/* Whether an error happened on the stream. */
int32_t rorb_out_failed(rorb_out* out);

int32_t rorb_in_read_boolean(rorb_in* in);
uint8_t rorb_in_read_octet(rorb_in* in);
char rorb_in_read_char(rorb_in* in);
uint16_t rorb_in_read_wchar(rorb_in* in);
int16_t rorb_in_read_short(rorb_in* in);
uint16_t rorb_in_read_ushort(rorb_in* in);
int32_t rorb_in_read_long(rorb_in* in);
uint32_t rorb_in_read_ulong(rorb_in* in);
int64_t rorb_in_read_longlong(rorb_in* in);
uint64_t rorb_in_read_ulonglong(rorb_in* in);
float rorb_in_read_float(rorb_in* in);
double rorb_in_read_double(rorb_in* in);
/* A string as UTF-8, valid until the next call on the stream. */
const char* rorb_in_read_string(rorb_in* in, size_t* len);
/* A wstring as UTF-16, valid until the next call on the stream. */
const uint16_t* rorb_in_read_wstring(rorb_in* in, size_t* len);
/* NULL for a nil reference (or an error). */
rorb_object* rorb_in_read_object(rorb_in* in);
/* NULL after an error. */
rorb_typecode* rorb_in_read_typecode(rorb_in* in);
rorb_any* rorb_in_read_any(rorb_in* in);
void rorb_in_read_array(rorb_in* in, int32_t kind, void* data, size_t count);
/* A fixed of count digits into digits[count]; returns 1 if it is negative. */
int32_t rorb_in_read_fixed(rorb_in* in, uint8_t* digits, size_t count);
/* A sequence length, checked against the bytes left for elements of at
   least min_element_size bytes. */
uint32_t rorb_in_read_length(rorb_in* in, size_t min_element_size);
/* Marks the stream as failed (MARSHAL), for a value the caller rejects. */
void rorb_in_fail(rorb_in* in);
int32_t rorb_in_failed(rorb_in* in);

/* --- valuetypes ------------------------------------------------------------

   The caller writes and reads the state of its instances; these functions
   do the rest (CORBA 3.4 part 2, 9.3.4). On output, key names the
   instance: an instance written again in the same message (the same key)
   is sent as an indirection. The repository ids are the type's own, then
   those of the bases it is truncatable to; chunked says to chunk the state
   (for truncatable types; a value inside a chunked one is chunked too). */

void rorb_out_write_null_value(rorb_out* out);
/* Returns 1 if the caller now writes the state, then calls
   rorb_out_end_value; 0 if an indirection was written (or the stream
   failed). */
int32_t rorb_out_begin_value(rorb_out* out, uint64_t key, size_t count, const char* const* ids,
                             const size_t* id_lens, int32_t chunked);
void rorb_out_end_value(rorb_out* out);

/* Results of rorb_in_read_value_header. */
#define RORB_VALUE_NULL 0
#define RORB_VALUE_INDIRECTION 1
#define RORB_VALUE_NEW 2

/* What stands where a value is expected: null (also after an error), an
   indirection to the value in *slot, or a new value with slot *slot and
   *id_count repository ids (most derived first; none leaves the type to
   the receiver). After RORB_VALUE_NEW the caller makes its instance, keeps
   it with rorb_in_set_value (before reading the state, for cycles), reads
   the state it knows and calls rorb_in_end_value, which skips the rest. */
int32_t rorb_in_read_value_header(rorb_in* in, size_t* slot, size_t* id_count);
/* Repository id index of the last header, valid until the next one. */
const char* rorb_in_value_id(rorb_in* in, size_t index, size_t* len);
void rorb_in_end_value(rorb_in* in);
/* Keeps ptr for slot until the stream ends, then calls release(ptr)
   (unless release is NULL). */
void rorb_in_set_value(rorb_in* in, size_t slot, void* ptr, void (*release)(void* ptr));
/* The pointer kept for slot, or NULL. */
void* rorb_in_get_value(rorb_in* in, size_t slot);

/* Makes the valuetype tc known to the core, so that values of it in anys
   are read as that type where a base type is expected. */
void rorb_register_value_type(const rorb_typecode* tc);
/* Asks resolve(context, id, len) for the valuetypes the core reads (in
   anys) that are not registered: it returns a new TypeCode (which the core
   takes and registers) or NULL. It may be called from any thread. NULL
   removes the resolver. */
void rorb_set_value_type_resolver(rorb_typecode* (*resolve)(void* context, const char* id,
                                                            size_t len),
                                  void* context);

/* --- servants --------------------------------------------------------------- */

/* The callbacks of a servant. self is the pointer given to
   rorb_servant_get. dispatch reads the arguments from in and writes the
   result to reply; it returns RORB_OK (also after
   rorb_server_reply_user_exception) or RORB_SYSTEM_EXCEPTION (after
   rorb_server_reply_system_exception). _is_a, _non_existent, _interface
   and _repository_id are answered by the ORB and do not reach dispatch. */
typedef struct rorb_servant_vtable {
  rorb_status (*dispatch)(void* self, const char* op, size_t op_len, rorb_in* in,
                          rorb_server_reply* reply);
  int32_t (*is_a)(void* self, const char* id, size_t len);
  /* The Rust side took a reference to self... */
  void (*add_ref)(void* self);
  /* ...and gives it back. */
  void (*remove_ref)(void* self);
} rorb_servant_vtable;

/* The Rust side of the servant self, made the first time (then
   vtable->add_ref(self) is called) and the same while the POA or a handle
   holds it. The vtable must outlive the servant. */
rorb_servant* rorb_servant_get(void* self, const char* repository_id, size_t len,
                               const rorb_servant_vtable* vtable);
void rorb_servant_release(rorb_servant* servant);
/* If the calling thread runs a request for servant, *obj is the target
   object and the result is 1; otherwise 0. */
int32_t rorb_servant_current_reference(rorb_servant* servant, rorb_object** obj);

rorb_out* rorb_server_reply_output(rorb_server_reply* reply);
/* Replaces the reply with the user exception id; its members are written
   to the stream returned. */
rorb_out* rorb_server_reply_user_exception(rorb_server_reply* reply, const char* id,
                                           size_t len);
void rorb_server_reply_system_exception(rorb_server_reply* reply, const char* id, size_t len,
                                        uint32_t minor, uint32_t completed);

/* --- POA ------------------------------------------------------------------- */

/* Policy types (CORBA::PolicyType) and values (the IDL enumerators in
   order, as in PortableServer.idl). */
#define RORB_THREAD_POLICY_ID 16
#define RORB_LIFESPAN_POLICY_ID 17
#define RORB_ID_UNIQUENESS_POLICY_ID 18
#define RORB_ID_ASSIGNMENT_POLICY_ID 19
#define RORB_IMPLICIT_ACTIVATION_POLICY_ID 20
#define RORB_SERVANT_RETENTION_POLICY_ID 21
#define RORB_REQUEST_PROCESSING_POLICY_ID 22

typedef struct rorb_policy {
  uint32_t type;
  uint32_t value;
} rorb_policy;

rorb_poa* rorb_poa_clone(rorb_poa* poa);
void rorb_poa_release(rorb_poa* poa);
/* Valid while poa lives. */
const char* rorb_poa_name(rorb_poa* poa, size_t* len);
/* NULL for the RootPOA. */
rorb_poa* rorb_poa_parent(rorb_poa* poa);
void rorb_poa_children(rorb_poa* poa, void* context, void (*each)(void* context, rorb_poa* child));
rorb_poa_manager* rorb_poa_the_manager(rorb_poa* poa);
/* The seven policies of the POA, in the order of the policy ids. */
void rorb_poa_policies(rorb_poa* poa, rorb_policy policies[7]);
/* manager may be NULL for a new one. */
rorb_status rorb_poa_create_poa(rorb_poa* poa, const char* name, size_t len,
                                rorb_poa_manager* manager, const rorb_policy* policies,
                                size_t count, rorb_poa** child, rorb_exception* exc);
rorb_status rorb_poa_find_poa(rorb_poa* poa, const char* name, size_t len, int32_t activate_it,
                              rorb_poa** child, rorb_exception* exc);
rorb_status rorb_poa_destroy(rorb_poa* poa, int32_t etherealize, int32_t wait_for_completion,
                             rorb_exception* exc);
rorb_status rorb_poa_activate_object(rorb_poa* poa, rorb_servant* servant, rorb_bytes* id,
                                     rorb_exception* exc);
rorb_status rorb_poa_activate_object_with_id(rorb_poa* poa, const uint8_t* id, size_t len,
                                             rorb_servant* servant, rorb_exception* exc);
rorb_status rorb_poa_deactivate_object(rorb_poa* poa, const uint8_t* id, size_t len,
                                       rorb_exception* exc);
rorb_status rorb_poa_servant_to_id(rorb_poa* poa, rorb_servant* servant, rorb_bytes* id,
                                   rorb_exception* exc);
rorb_status rorb_poa_servant_to_reference(rorb_poa* poa, rorb_servant* servant,
                                          rorb_object** obj, rorb_exception* exc);
/* *self is a servant made with rorb_servant_get, with a reference added
   for the caller (vtable->add_ref). A servant of another kind is
   OBJ_ADAPTER. */
rorb_status rorb_poa_id_to_servant(rorb_poa* poa, const uint8_t* id, size_t len, void** self,
                                   rorb_exception* exc);
rorb_status rorb_poa_id_to_reference(rorb_poa* poa, const uint8_t* id, size_t len,
                                     rorb_object** obj, rorb_exception* exc);
rorb_status rorb_poa_reference_to_id(rorb_poa* poa, rorb_object* obj, rorb_bytes* id,
                                     rorb_exception* exc);
rorb_status rorb_poa_reference_to_servant(rorb_poa* poa, rorb_object* obj, void** self,
                                          rorb_exception* exc);
rorb_status rorb_poa_create_reference(rorb_poa* poa, const char* intf, size_t len,
                                      rorb_object** obj, rorb_exception* exc);
rorb_status rorb_poa_create_reference_with_id(rorb_poa* poa, const uint8_t* id, size_t id_len,
                                              const char* intf, size_t len, rorb_object** obj,
                                              rorb_exception* exc);
/* The default servant (USE_DEFAULT_SERVANT). */
rorb_status rorb_poa_get_servant(rorb_poa* poa, void** self, rorb_exception* exc);
rorb_status rorb_poa_set_servant(rorb_poa* poa, rorb_servant* servant, rorb_exception* exc);

/* --- servant managers and adapter activators ------------------------------ */

/* What incarnate or preinvoke raised instead of providing a servant; only
   valid during the callback that receives it. */
typedef struct rorb_manager_error rorb_manager_error;

/* PortableServer::ForwardRequest: the client is sent to obj (cloned). */
void rorb_manager_error_forward(rorb_manager_error* error, rorb_object* obj);
/* A system exception, returned to the client. */
void rorb_manager_error_system(rorb_manager_error* error, const char* id, size_t len,
                               uint32_t minor, uint32_t completed);

/* The callbacks of the objects below. self is the pointer given when
   setting it; like a servant, the POA calls add_ref when it takes self
   and remove_ref when it lets it go. They are called on upcall threads.

   incarnate and preinvoke return RORB_OK with *servant set to a handle
   from rorb_servant_get (whose reference the POA takes over), or another
   status after rorb_manager_error_* (UNKNOWN when nothing was set). A
   NULL servant is OBJ_ADAPTER. The adapter handles are only valid during
   the callback (rorb_poa_clone keeps one). The servant given to
   etherealize and postinvoke is the self of a servant made with
   rorb_servant_get, valid during the callback without a reference added
   (NULL for a servant of another kind). etherealize and postinvoke may be
   NULL. */
typedef struct rorb_servant_activator_vtable {
  rorb_status (*incarnate)(void* self, const uint8_t* id, size_t id_len, rorb_poa* adapter,
                           rorb_servant** servant, rorb_manager_error* error);
  void (*etherealize)(void* self, const uint8_t* id, size_t id_len, rorb_poa* adapter,
                      void* servant, int32_t cleanup_in_progress,
                      int32_t remaining_activations);
  void (*add_ref)(void* self);
  void (*remove_ref)(void* self);
} rorb_servant_activator_vtable;

typedef struct rorb_servant_locator_vtable {
  /* *cookie is passed to postinvoke as it is. */
  rorb_status (*preinvoke)(void* self, const uint8_t* id, size_t id_len, rorb_poa* adapter,
                           const char* op, size_t op_len, rorb_servant** servant, void** cookie,
                           rorb_manager_error* error);
  void (*postinvoke)(void* self, const uint8_t* id, size_t id_len, rorb_poa* adapter,
                     const char* op, size_t op_len, void* cookie, void* servant);
  void (*add_ref)(void* self);
  void (*remove_ref)(void* self);
} rorb_servant_locator_vtable;

typedef struct rorb_adapter_activator_vtable {
  /* Creates the child name of parent and returns 1, or returns 0. */
  int32_t (*unknown_adapter)(void* self, rorb_poa* parent, const char* name, size_t len);
  void (*add_ref)(void* self);
  void (*remove_ref)(void* self);
} rorb_adapter_activator_vtable;

/* rorb_poa_get_servant_manager's *kind */
#define RORB_NO_SERVANT_MANAGER 0
#define RORB_SERVANT_ACTIVATOR 1
#define RORB_SERVANT_LOCATOR 2

/* set_servant_manager (USE_SERVANT_MANAGER): an activator with RETAIN, a
   locator with NON_RETAIN. The vtables must outlive the POA. */
rorb_status rorb_poa_set_servant_activator(rorb_poa* poa, void* self,
                                           const rorb_servant_activator_vtable* vtable,
                                           rorb_exception* exc);
rorb_status rorb_poa_set_servant_locator(rorb_poa* poa, void* self,
                                         const rorb_servant_locator_vtable* vtable,
                                         rorb_exception* exc);
/* *self is the manager given to set, with a reference added for the
   caller (add_ref), or NULL. */
rorb_status rorb_poa_get_servant_manager(rorb_poa* poa, int32_t* kind, void** self,
                                         rorb_exception* exc);
/* the_activator; self NULL removes it. */
rorb_status rorb_poa_set_the_activator(rorb_poa* poa, void* self,
                                       const rorb_adapter_activator_vtable* vtable,
                                       rorb_exception* exc);
/* The adapter activator given to set, with a reference added for the
   caller, or NULL. */
void* rorb_poa_the_activator(rorb_poa* poa);

/* PortableServer::Current; outside a request RORB_NO_CONTEXT. */
rorb_status rorb_current_get_poa(rorb_poa** poa, rorb_exception* exc);
rorb_status rorb_current_get_object_id(rorb_bytes* id, rorb_exception* exc);
rorb_status rorb_current_get_reference(rorb_object** obj, rorb_exception* exc);
rorb_status rorb_current_get_servant(void** self, rorb_exception* exc);

/* POAManager::State */
#define RORB_POA_MANAGER_HOLDING 0
#define RORB_POA_MANAGER_ACTIVE 1
#define RORB_POA_MANAGER_DISCARDING 2
#define RORB_POA_MANAGER_INACTIVE 3

rorb_poa_manager* rorb_poa_manager_clone(rorb_poa_manager* manager);
void rorb_poa_manager_release(rorb_poa_manager* manager);
uint32_t rorb_poa_manager_get_state(rorb_poa_manager* manager);
rorb_status rorb_poa_manager_activate(rorb_poa_manager* manager, rorb_exception* exc);
rorb_status rorb_poa_manager_hold_requests(rorb_poa_manager* manager,
                                           int32_t wait_for_completion, rorb_exception* exc);
rorb_status rorb_poa_manager_discard_requests(rorb_poa_manager* manager,
                                              int32_t wait_for_completion, rorb_exception* exc);
rorb_status rorb_poa_manager_deactivate(rorb_poa_manager* manager, int32_t etherealize,
                                        int32_t wait_for_completion, rorb_exception* exc);

#ifdef __cplusplus
}
#endif

#endif
