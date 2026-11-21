/*
 * rustorb/orb.h: the CORBA module of the OMG C Language Mapping 1.0 on
 * rustorb (librustorb_c, on the C ABI of crates/rustorb-ffi).
 *
 * The types follow the mapping (CORBA_long, CORBA_char*, sequences with
 * _maximum, _length and _buffer, CORBA_any with _type and _value,
 * CORBA_Environment...). Operations of the pseudo objects are named by the
 * mapping's rule <interface>_<operation>, with the object first and the
 * CORBA_Environment last (docs/c-binding.md).
 *
 * Memory: what the ORB allocates for the caller (strings, sequences, out
 * values, exceptions) is freed with CORBA_free, which frees what the value
 * holds too. Object references are released with CORBA_Object_release.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
 */

#ifndef RUSTORB_C_ORB_H
#define RUSTORB_C_ORB_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* --- basic types (1.7) ---------------------------------------------------- */

typedef int16_t CORBA_short;
typedef int32_t CORBA_long;
typedef int64_t CORBA_long_long;
typedef uint16_t CORBA_unsigned_short;
typedef uint32_t CORBA_unsigned_long;
typedef uint64_t CORBA_unsigned_long_long;
typedef float CORBA_float;
typedef double CORBA_double;
/* Sent as IEEE 754 binary128 through a double (rustorb). */
typedef long double CORBA_long_double;
typedef char CORBA_char;
/* A UTF-16 code unit (rustorb). */
typedef uint16_t CORBA_wchar;
typedef unsigned char CORBA_boolean;
typedef unsigned char CORBA_octet;
/* IDL 4 int8 (rustorb): sent and typed as an octet. */
typedef signed char CORBA_int8;
typedef CORBA_unsigned_long CORBA_enum;

#define CORBA_FALSE 0
#define CORBA_TRUE 1

/* --- object references --------------------------------------------------- */

/* Every object reference and pseudo object (ORB, TypeCode, POA,
   POAManager, Policy, Current) is a CORBA_Object. */
typedef struct CORBA_Object_struct* CORBA_Object;
#define CORBA_OBJECT_NIL ((CORBA_Object)0)

typedef CORBA_Object CORBA_ORB;
typedef CORBA_Object CORBA_TypeCode;
typedef CORBA_Object CORBA_Policy;
typedef CORBA_char* CORBA_ORBid;
typedef CORBA_char* CORBA_RepositoryId;
typedef CORBA_char* CORBA_Identifier;
typedef CORBA_unsigned_long CORBA_PolicyType;

/* --- exceptions (1.25) ------------------------------------------------------ */

typedef enum CORBA_exception_type {
  CORBA_NO_EXCEPTION = 0,
  CORBA_USER_EXCEPTION = 1,
  CORBA_SYSTEM_EXCEPTION = 2
} CORBA_exception_type;

typedef enum CORBA_completion_status {
  CORBA_COMPLETED_YES = 0,
  CORBA_COMPLETED_NO = 1,
  CORBA_COMPLETED_MAYBE = 2
} CORBA_completion_status;

/* Set by every operation: _major CORBA_NO_EXCEPTION, or an exception
   whose id and value CORBA_exception_id and CORBA_exception_value
   return. A zeroed Environment is a valid one without an exception; the
   members after _major are rustorb's. */
typedef struct CORBA_Environment {
  CORBA_exception_type _major;
  CORBA_char* _id;
  void* _params;
} CORBA_Environment;

/* The value of every system exception (rustorb). */
typedef struct CORBA_SystemException {
  CORBA_unsigned_long minor;
  CORBA_completion_status completed;
} CORBA_SystemException;

CORBA_char* CORBA_exception_id(CORBA_Environment* ev);
void* CORBA_exception_value(CORBA_Environment* ev);
void CORBA_exception_free(CORBA_Environment* ev);
/* Takes param (allocated with <Exception>__alloc, or NULL for an
   exception without members) and copies id. */
void CORBA_exception_set(CORBA_Environment* ev, CORBA_exception_type major,
                         const CORBA_char* id, void* param);
/* rustorb: a zeroed Environment (the same as = {0}). */
void CORBA_exception_init(CORBA_Environment* ev);
/* rustorb: CORBA_exception_set with a system exception. */
void CORBA_exception_set_system(CORBA_Environment* ev, const CORBA_char* id,
                                CORBA_unsigned_long minor, CORBA_completion_status completed);
CORBA_SystemException* CORBA_SystemException__alloc(void);

#define ex_CORBA_UNKNOWN "IDL:omg.org/CORBA/UNKNOWN:1.0"
#define ex_CORBA_BAD_PARAM "IDL:omg.org/CORBA/BAD_PARAM:1.0"
#define ex_CORBA_NO_MEMORY "IDL:omg.org/CORBA/NO_MEMORY:1.0"
#define ex_CORBA_IMP_LIMIT "IDL:omg.org/CORBA/IMP_LIMIT:1.0"
#define ex_CORBA_COMM_FAILURE "IDL:omg.org/CORBA/COMM_FAILURE:1.0"
#define ex_CORBA_INV_OBJREF "IDL:omg.org/CORBA/INV_OBJREF:1.0"
#define ex_CORBA_NO_PERMISSION "IDL:omg.org/CORBA/NO_PERMISSION:1.0"
#define ex_CORBA_INTERNAL "IDL:omg.org/CORBA/INTERNAL:1.0"
#define ex_CORBA_MARSHAL "IDL:omg.org/CORBA/MARSHAL:1.0"
#define ex_CORBA_INITIALIZE "IDL:omg.org/CORBA/INITIALIZE:1.0"
#define ex_CORBA_NO_IMPLEMENT "IDL:omg.org/CORBA/NO_IMPLEMENT:1.0"
#define ex_CORBA_BAD_TYPECODE "IDL:omg.org/CORBA/BAD_TYPECODE:1.0"
#define ex_CORBA_BAD_OPERATION "IDL:omg.org/CORBA/BAD_OPERATION:1.0"
#define ex_CORBA_NO_RESOURCES "IDL:omg.org/CORBA/NO_RESOURCES:1.0"
#define ex_CORBA_NO_RESPONSE "IDL:omg.org/CORBA/NO_RESPONSE:1.0"
#define ex_CORBA_PERSIST_STORE "IDL:omg.org/CORBA/PERSIST_STORE:1.0"
#define ex_CORBA_BAD_INV_ORDER "IDL:omg.org/CORBA/BAD_INV_ORDER:1.0"
#define ex_CORBA_TRANSIENT "IDL:omg.org/CORBA/TRANSIENT:1.0"
#define ex_CORBA_FREE_MEM "IDL:omg.org/CORBA/FREE_MEM:1.0"
#define ex_CORBA_INV_IDENT "IDL:omg.org/CORBA/INV_IDENT:1.0"
#define ex_CORBA_INV_FLAG "IDL:omg.org/CORBA/INV_FLAG:1.0"
#define ex_CORBA_INTF_REPOS "IDL:omg.org/CORBA/INTF_REPOS:1.0"
#define ex_CORBA_BAD_CONTEXT "IDL:omg.org/CORBA/BAD_CONTEXT:1.0"
#define ex_CORBA_OBJ_ADAPTER "IDL:omg.org/CORBA/OBJ_ADAPTER:1.0"
#define ex_CORBA_DATA_CONVERSION "IDL:omg.org/CORBA/DATA_CONVERSION:1.0"
#define ex_CORBA_OBJECT_NOT_EXIST "IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0"
#define ex_CORBA_TRANSACTION_REQUIRED "IDL:omg.org/CORBA/TRANSACTION_REQUIRED:1.0"
#define ex_CORBA_TRANSACTION_ROLLEDBACK "IDL:omg.org/CORBA/TRANSACTION_ROLLEDBACK:1.0"
#define ex_CORBA_INVALID_TRANSACTION "IDL:omg.org/CORBA/INVALID_TRANSACTION:1.0"
#define ex_CORBA_INV_POLICY "IDL:omg.org/CORBA/INV_POLICY:1.0"
#define ex_CORBA_CODESET_INCOMPATIBLE "IDL:omg.org/CORBA/CODESET_INCOMPATIBLE:1.0"
#define ex_CORBA_REBIND "IDL:omg.org/CORBA/REBIND:1.0"
#define ex_CORBA_TIMEOUT "IDL:omg.org/CORBA/TIMEOUT:1.0"
#define ex_CORBA_TRANSACTION_UNAVAILABLE "IDL:omg.org/CORBA/TRANSACTION_UNAVAILABLE:1.0"
#define ex_CORBA_TRANSACTION_MODE "IDL:omg.org/CORBA/TRANSACTION_MODE:1.0"
#define ex_CORBA_BAD_QOS "IDL:omg.org/CORBA/BAD_QOS:1.0"
#define ex_CORBA_INVALID_ACTIVITY "IDL:omg.org/CORBA/INVALID_ACTIVITY:1.0"
#define ex_CORBA_ACTIVITY_COMPLETED "IDL:omg.org/CORBA/ACTIVITY_COMPLETED:1.0"
#define ex_CORBA_ACTIVITY_REQUIRED "IDL:omg.org/CORBA/ACTIVITY_REQUIRED:1.0"
#define ex_CORBA_THREAD_CANCELLED "IDL:omg.org/CORBA/THREAD_CANCELLED:1.0"

/* User exceptions of the pseudo objects (no members). */
#define ex_CORBA_ORB_InvalidName "IDL:omg.org/CORBA/ORB/InvalidName:1.0"
#define ex_CORBA_TypeCode_BadKind "IDL:omg.org/CORBA/TypeCode/BadKind:1.0"
#define ex_CORBA_TypeCode_Bounds "IDL:omg.org/CORBA/TypeCode/Bounds:1.0"
/* CORBA::PolicyError: its reason is a CORBA_PolicyError. */
#define ex_CORBA_PolicyError "IDL:omg.org/CORBA/PolicyError:1.0"
typedef struct CORBA_PolicyError {
  CORBA_short reason;
} CORBA_PolicyError;

/* --- memory (1.21) ---------------------------------------------------------- */

/* Frees memory the ORB allocated (or *__alloc, CORBA_string_alloc...),
   and what the value holds: strings, sequence buffers with _release set,
   object references, anys. NULL does nothing. */
void CORBA_free(void* storage);
CORBA_char* CORBA_string_alloc(CORBA_unsigned_long len);
CORBA_wchar* CORBA_wstring_alloc(CORBA_unsigned_long len);
/* rustorb: copies of a NUL-terminated string, freed with CORBA_free. */
CORBA_char* CORBA_string_dup(const CORBA_char* s);
CORBA_wchar* CORBA_wstring_dup(const CORBA_wchar* s);
/* rustorb: the length of a NUL-terminated wstring. */
CORBA_unsigned_long CORBA_wstring_len(const CORBA_wchar* s);

/* --- sequences (1.11) ------------------------------------------------------- */

/* Each sequence type has this shape; _release (rustorb's member, set by
   CORBA_sequence_set_release) says whether freeing the sequence frees
   _buffer. */
#define RUSTORB_C_SEQUENCE(name, type)  \
  typedef struct name {                  \
    CORBA_unsigned_long _maximum;        \
    CORBA_unsigned_long _length;         \
    type* _buffer;                       \
    CORBA_boolean _release;              \
  } name

void CORBA_sequence_set_release(void* seq, CORBA_boolean release);
CORBA_boolean CORBA_sequence_get_release(void* seq);

#ifndef _CORBA_sequence_octet_defined
#define _CORBA_sequence_octet_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_octet, CORBA_octet);
#endif
#ifndef _CORBA_sequence_string_defined
#define _CORBA_sequence_string_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_string, CORBA_char*);
#endif
#ifndef _CORBA_sequence_CORBA_Policy_defined
#define _CORBA_sequence_CORBA_Policy_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_CORBA_Policy, CORBA_Policy);
#endif
typedef CORBA_sequence_CORBA_Policy CORBA_PolicyList;
typedef CORBA_sequence_string CORBA_ORB_ObjectIdList;

CORBA_sequence_octet* CORBA_sequence_octet__alloc(void);
CORBA_octet* CORBA_sequence_octet_allocbuf(CORBA_unsigned_long len);
CORBA_sequence_string* CORBA_sequence_string__alloc(void);
CORBA_char** CORBA_sequence_string_allocbuf(CORBA_unsigned_long len);
CORBA_PolicyList* CORBA_PolicyList__alloc(void);
CORBA_Policy* CORBA_sequence_CORBA_Policy_allocbuf(CORBA_unsigned_long len);

/* --- TypeCode (1.18) ---------------------------------------------------------- */

typedef enum CORBA_TCKind {
  CORBA_tk_null = 0, CORBA_tk_void, CORBA_tk_short, CORBA_tk_long, CORBA_tk_ushort,
  CORBA_tk_ulong, CORBA_tk_float, CORBA_tk_double, CORBA_tk_boolean, CORBA_tk_char,
  CORBA_tk_octet, CORBA_tk_any, CORBA_tk_TypeCode, CORBA_tk_Principal, CORBA_tk_objref,
  CORBA_tk_struct, CORBA_tk_union, CORBA_tk_enum, CORBA_tk_string, CORBA_tk_sequence,
  CORBA_tk_array, CORBA_tk_alias, CORBA_tk_except, CORBA_tk_longlong, CORBA_tk_ulonglong,
  CORBA_tk_longdouble, CORBA_tk_wchar, CORBA_tk_wstring, CORBA_tk_fixed, CORBA_tk_value,
  CORBA_tk_value_box, CORBA_tk_native, CORBA_tk_abstract_interface, CORBA_tk_local_interface,
  CORBA_tk_component, CORBA_tk_home, CORBA_tk_event
} CORBA_TCKind;

/* The TypeCode constants are borrowed: they need no release (releasing
   them does nothing). */
CORBA_TypeCode rustorb_c_tc_simple(CORBA_TCKind kind);
#define TC_null rustorb_c_tc_simple(CORBA_tk_null)
#define TC_void rustorb_c_tc_simple(CORBA_tk_void)
#define TC_short rustorb_c_tc_simple(CORBA_tk_short)
#define TC_long rustorb_c_tc_simple(CORBA_tk_long)
#define TC_longlong rustorb_c_tc_simple(CORBA_tk_longlong)
#define TC_ushort rustorb_c_tc_simple(CORBA_tk_ushort)
#define TC_ulong rustorb_c_tc_simple(CORBA_tk_ulong)
#define TC_ulonglong rustorb_c_tc_simple(CORBA_tk_ulonglong)
#define TC_float rustorb_c_tc_simple(CORBA_tk_float)
#define TC_double rustorb_c_tc_simple(CORBA_tk_double)
#define TC_longdouble rustorb_c_tc_simple(CORBA_tk_longdouble)
#define TC_boolean rustorb_c_tc_simple(CORBA_tk_boolean)
#define TC_char rustorb_c_tc_simple(CORBA_tk_char)
#define TC_wchar rustorb_c_tc_simple(CORBA_tk_wchar)
#define TC_octet rustorb_c_tc_simple(CORBA_tk_octet)
#define TC_any rustorb_c_tc_simple(CORBA_tk_any)
#define TC_TypeCode rustorb_c_tc_simple(CORBA_tk_TypeCode)
#define TC_string rustorb_c_tc_simple(CORBA_tk_string)
#define TC_wstring rustorb_c_tc_simple(CORBA_tk_wstring)
#define TC_Object rustorb_c_tc_simple(CORBA_tk_objref)

CORBA_TCKind CORBA_TypeCode_kind(CORBA_TypeCode tc, CORBA_Environment* ev);
CORBA_boolean CORBA_TypeCode_equal(CORBA_TypeCode tc, CORBA_TypeCode other, CORBA_Environment* ev);
CORBA_boolean CORBA_TypeCode_equivalent(CORBA_TypeCode tc, CORBA_TypeCode other,
                                        CORBA_Environment* ev);
CORBA_TypeCode CORBA_TypeCode_get_compact_typecode(CORBA_TypeCode tc, CORBA_Environment* ev);
/* Strings to CORBA_free. */
CORBA_RepositoryId CORBA_TypeCode_id(CORBA_TypeCode tc, CORBA_Environment* ev);
CORBA_Identifier CORBA_TypeCode_name(CORBA_TypeCode tc, CORBA_Environment* ev);
CORBA_unsigned_long CORBA_TypeCode_member_count(CORBA_TypeCode tc, CORBA_Environment* ev);
CORBA_Identifier CORBA_TypeCode_member_name(CORBA_TypeCode tc, CORBA_unsigned_long index,
                                            CORBA_Environment* ev);
CORBA_TypeCode CORBA_TypeCode_member_type(CORBA_TypeCode tc, CORBA_unsigned_long index,
                                          CORBA_Environment* ev);
CORBA_TypeCode CORBA_TypeCode_discriminator_type(CORBA_TypeCode tc, CORBA_Environment* ev);
CORBA_long CORBA_TypeCode_default_index(CORBA_TypeCode tc, CORBA_Environment* ev);
CORBA_unsigned_long CORBA_TypeCode_length(CORBA_TypeCode tc, CORBA_Environment* ev);
CORBA_TypeCode CORBA_TypeCode_content_type(CORBA_TypeCode tc, CORBA_Environment* ev);
CORBA_unsigned_short CORBA_TypeCode_fixed_digits(CORBA_TypeCode tc, CORBA_Environment* ev);
CORBA_short CORBA_TypeCode_fixed_scale(CORBA_TypeCode tc, CORBA_Environment* ev);

/* --- any (1.19) ---------------------------------------------------------------- */

/* _value points to a value of the type _type (as the mapping lays it
   out). An any read from the wire whose type the program does not know
   (no generated code for it was used) has _value NULL and is sent on as
   it came; _rustorb is rustorb's. */
typedef struct CORBA_any {
  CORBA_TypeCode _type;
  void* _value;
  CORBA_boolean _release;
  void* _rustorb;
} CORBA_any;

CORBA_any* CORBA_any__alloc(void);
#define CORBA_any_alloc CORBA_any__alloc

void CORBA_any_set_release(CORBA_any* any, CORBA_boolean release);
CORBA_boolean CORBA_any_get_release(CORBA_any* any);
/* rustorb: whether the anys hold equal values of equal types. */
CORBA_boolean CORBA_any_equal(const CORBA_any* a, const CORBA_any* b);

/* The exception as an any (NULL for none). The Environment keeps it: it
   stays valid until CORBA_exception_free, and the caller does not free
   it. A user exception of a type no generated code in the program
   describes gives NULL, unless it has no members. */
CORBA_any* CORBA_exception_as_any(CORBA_Environment* ev);

/* --- fixed (1.14) ------------------------------------------------------------- */

/* fixed<d, s> is CORBA_fixed_d_s: _value holds the d digits and the sign
   as packed decimal (the CDR form: two digits an octet, the last
   half-octet 0xC or 0xD). */
#define RUSTORB_C_FIXED(d, s)                         \
  typedef struct CORBA_fixed_##d##_##s {               \
    CORBA_unsigned_short _digits;                       \
    CORBA_short _scale;                                 \
    CORBA_octet _value[((d) + 2) / 2];                  \
  } CORBA_fixed_##d##_##s

/* Any fixed up to 31 digits (CORBA_fixed_alloc). */
typedef struct CORBA_fixed_d_s {
  CORBA_unsigned_short _digits;
  CORBA_short _scale;
  CORBA_octet _value[16];
} CORBA_fixed_d_s;

/* The functions take any CORBA_fixed_D_S as void*; _digits and _scale of
   every operand and of the result must be set. The exact result is made
   to fit *rp: the digits below its _scale are truncated toward zero and
   those above its _digits dropped. Division by zero gives 0. */
CORBA_long CORBA_fixed_integer_part(const void* fp);
/* The fraction as an integer of min(_scale, 9) digits, with the sign of
   the value: 12.345 (scale 3) gives 345, -1.5 gives -5. */
CORBA_long CORBA_fixed_fraction_part(const void* fp);
/* *rp = i + f / 10^min(_scale, 9). */
void CORBA_fixed_set(void* rp, const CORBA_long i, const CORBA_long f);
void CORBA_fixed_add(void* rp, const void* f1p, const void* f2p);
void CORBA_fixed_sub(void* rp, const void* f1p, const void* f2p);
void CORBA_fixed_mul(void* rp, const void* f1p, const void* f2p);
void CORBA_fixed_div(void* rp, const void* f1p, const void* f2p);
/* A fixed of d digits (31 at most) and scale 0, value 0, freed with
   CORBA_free; set _scale before use. */
CORBA_fixed_d_s* CORBA_fixed_alloc(CORBA_unsigned_short d);

/* --- Object (1.29) -------------------------------------------------------------- */

CORBA_Object CORBA_Object_duplicate(CORBA_Object obj, CORBA_Environment* ev);
void CORBA_Object_release(CORBA_Object obj, CORBA_Environment* ev);
CORBA_boolean CORBA_Object_is_nil(CORBA_Object obj, CORBA_Environment* ev);
CORBA_boolean CORBA_Object_is_a(CORBA_Object obj, const CORBA_char* logical_type_id,
                                CORBA_Environment* ev);
CORBA_boolean CORBA_Object_non_existent(CORBA_Object obj, CORBA_Environment* ev);
CORBA_boolean CORBA_Object_is_equivalent(CORBA_Object obj, CORBA_Object other,
                                         CORBA_Environment* ev);
CORBA_unsigned_long CORBA_Object_hash(CORBA_Object obj, CORBA_unsigned_long maximum,
                                      CORBA_Environment* ev);
CORBA_boolean CORBA_Object_validate_connection(CORBA_Object obj, CORBA_PolicyList** inconsistent,
                                               CORBA_Environment* ev);
/* rustorb: the repository id of the reference's type, to CORBA_free. */
CORBA_char* CORBA_Object__repository_id(CORBA_Object obj, CORBA_Environment* ev);

/* --- ORB (1.30) ----------------------------------------------------------------- */

/* argc and argv: the -ORB options are removed. orb_identifier may be
   NULL or "". */
CORBA_ORB CORBA_ORB_init(int* argc, char** argv, const CORBA_char* orb_identifier,
                         CORBA_Environment* ev);
CORBA_char* CORBA_ORB_object_to_string(CORBA_ORB orb, CORBA_Object obj, CORBA_Environment* ev);
CORBA_Object CORBA_ORB_string_to_object(CORBA_ORB orb, const CORBA_char* str,
                                        CORBA_Environment* ev);
/* "RootPOA" and "POACurrent" too. ex_CORBA_ORB_InvalidName for an unknown name. */
CORBA_Object CORBA_ORB_resolve_initial_references(CORBA_ORB orb, const CORBA_char* identifier,
                                                  CORBA_Environment* ev);
CORBA_ORB_ObjectIdList* CORBA_ORB_list_initial_services(CORBA_ORB orb, CORBA_Environment* ev);
void CORBA_ORB_register_initial_reference(CORBA_ORB orb, const CORBA_char* identifier,
                                          CORBA_Object obj, CORBA_Environment* ev);
CORBA_boolean CORBA_ORB_work_pending(CORBA_ORB orb, CORBA_Environment* ev);
void CORBA_ORB_perform_work(CORBA_ORB orb, CORBA_Environment* ev);
void CORBA_ORB_run(CORBA_ORB orb, CORBA_Environment* ev);
void CORBA_ORB_shutdown(CORBA_ORB orb, CORBA_boolean wait_for_completion, CORBA_Environment* ev);
void CORBA_ORB_destroy(CORBA_ORB orb, CORBA_Environment* ev);
CORBA_Policy CORBA_ORB_create_policy(CORBA_ORB orb, CORBA_PolicyType type, const CORBA_any* val,
                                     CORBA_Environment* ev);
/* TypeCode factories (to release with CORBA_Object_release). */
CORBA_TypeCode CORBA_ORB_create_string_tc(CORBA_ORB orb, CORBA_unsigned_long bound,
                                          CORBA_Environment* ev);
CORBA_TypeCode CORBA_ORB_create_wstring_tc(CORBA_ORB orb, CORBA_unsigned_long bound,
                                           CORBA_Environment* ev);
CORBA_TypeCode CORBA_ORB_create_sequence_tc(CORBA_ORB orb, CORBA_unsigned_long bound,
                                            CORBA_TypeCode element_type, CORBA_Environment* ev);
CORBA_TypeCode CORBA_ORB_create_array_tc(CORBA_ORB orb, CORBA_unsigned_long length,
                                         CORBA_TypeCode element_type, CORBA_Environment* ev);
CORBA_TypeCode CORBA_ORB_create_alias_tc(CORBA_ORB orb, const CORBA_char* id,
                                         const CORBA_char* name, CORBA_TypeCode original_type,
                                         CORBA_Environment* ev);
CORBA_TypeCode CORBA_ORB_create_interface_tc(CORBA_ORB orb, const CORBA_char* id,
                                             const CORBA_char* name, CORBA_Environment* ev);
CORBA_TypeCode CORBA_ORB_create_fixed_tc(CORBA_ORB orb, CORBA_unsigned_short digits,
                                         CORBA_short scale, CORBA_Environment* ev);

/* The members of the TypeCode factories (CORBA module). type_def (the
   interface repository) is not used. */
typedef CORBA_Object CORBA_IDLType;
typedef CORBA_char* CORBA_VersionSpec;
typedef struct CORBA_StructMember {
  CORBA_Identifier name;
  CORBA_TypeCode type;
  CORBA_IDLType type_def;
} CORBA_StructMember;
RUSTORB_C_SEQUENCE(CORBA_StructMemberSeq, CORBA_StructMember);
/* label: a value of the discriminator type; the octet 0 marks the default
   member. */
typedef struct CORBA_UnionMember {
  CORBA_Identifier name;
  CORBA_any label;
  CORBA_TypeCode type;
  CORBA_IDLType type_def;
} CORBA_UnionMember;
RUSTORB_C_SEQUENCE(CORBA_UnionMemberSeq, CORBA_UnionMember);
typedef CORBA_sequence_string CORBA_EnumMemberSeq;
typedef CORBA_short CORBA_ValueModifier;
#define CORBA_VM_NONE ((CORBA_ValueModifier)0)
#define CORBA_VM_CUSTOM ((CORBA_ValueModifier)1)
#define CORBA_VM_ABSTRACT ((CORBA_ValueModifier)2)
#define CORBA_VM_TRUNCATABLE ((CORBA_ValueModifier)3)
typedef CORBA_short CORBA_Visibility;
#define CORBA_PRIVATE_MEMBER ((CORBA_Visibility)0)
#define CORBA_PUBLIC_MEMBER ((CORBA_Visibility)1)
typedef struct CORBA_ValueMember {
  CORBA_Identifier name;
  CORBA_RepositoryId id;
  CORBA_RepositoryId defined_in;
  CORBA_VersionSpec version;
  CORBA_TypeCode type;
  CORBA_IDLType type_def;
  CORBA_Visibility access;
} CORBA_ValueMember;
RUSTORB_C_SEQUENCE(CORBA_ValueMemberSeq, CORBA_ValueMember);
CORBA_StructMemberSeq* CORBA_StructMemberSeq__alloc(void);
CORBA_StructMember* CORBA_StructMemberSeq_allocbuf(CORBA_unsigned_long len);
CORBA_UnionMemberSeq* CORBA_UnionMemberSeq__alloc(void);
CORBA_UnionMember* CORBA_UnionMemberSeq_allocbuf(CORBA_unsigned_long len);
CORBA_ValueMemberSeq* CORBA_ValueMemberSeq__alloc(void);
CORBA_ValueMember* CORBA_ValueMemberSeq_allocbuf(CORBA_unsigned_long len);

CORBA_TypeCode CORBA_ORB_create_struct_tc(CORBA_ORB orb, const CORBA_char* id,
                                          const CORBA_char* name,
                                          const CORBA_StructMemberSeq* members,
                                          CORBA_Environment* ev);
CORBA_TypeCode CORBA_ORB_create_exception_tc(CORBA_ORB orb, const CORBA_char* id,
                                             const CORBA_char* name,
                                             const CORBA_StructMemberSeq* members,
                                             CORBA_Environment* ev);
CORBA_TypeCode CORBA_ORB_create_union_tc(CORBA_ORB orb, const CORBA_char* id,
                                         const CORBA_char* name, CORBA_TypeCode discriminator_type,
                                         const CORBA_UnionMemberSeq* members,
                                         CORBA_Environment* ev);
CORBA_TypeCode CORBA_ORB_create_enum_tc(CORBA_ORB orb, const CORBA_char* id,
                                        const CORBA_char* name, const CORBA_EnumMemberSeq* members,
                                        CORBA_Environment* ev);
CORBA_TypeCode CORBA_ORB_create_value_tc(CORBA_ORB orb, const CORBA_char* id,
                                         const CORBA_char* name, CORBA_ValueModifier type_modifier,
                                         CORBA_TypeCode concrete_base,
                                         const CORBA_ValueMemberSeq* members,
                                         CORBA_Environment* ev);
CORBA_TypeCode CORBA_ORB_create_value_box_tc(CORBA_ORB orb, const CORBA_char* id,
                                             const CORBA_char* name, CORBA_TypeCode boxed_type,
                                             CORBA_Environment* ev);
CORBA_TypeCode CORBA_ORB_create_recursive_tc(CORBA_ORB orb, const CORBA_char* id,
                                             CORBA_Environment* ev);

/* rustorb: client call timeouts in microseconds (0: no limit). */
void rustorb_ORB_set_call_timeout(CORBA_ORB orb, CORBA_long_long us);
void rustorb_Object_set_call_timeout(CORBA_Object obj, CORBA_long_long us);

/* --- Policy -------------------------------------------------------------------- */

CORBA_PolicyType CORBA_Policy__get_policy_type(CORBA_Policy policy, CORBA_Environment* ev);
CORBA_Policy CORBA_Policy_copy(CORBA_Policy policy, CORBA_Environment* ev);
void CORBA_Policy_destroy(CORBA_Policy policy, CORBA_Environment* ev);

#ifdef __cplusplus
}
#endif

#endif /* RUSTORB_C_ORB_H */
