/* DynamicAny (CORBA 3.x, chapter "Dynamic Management of Any Values") on the
 * OMG C mapping: the local interfaces of the DynamicAny module, with the
 * names the C mapping gives their operations (DynamicAny_DynAny_to_any,
 * DynamicAny_DynStruct_get_members...). Every DynAny is a CORBA_Object;
 * the operations of DynAny take any of the derived interfaces too, and an
 * operation of a derived interface on a DynAny of another kind raises
 * BAD_OPERATION, as in the other bindings (docs/dynany.md).
 *
 * The factory is resolve_initial_references("DynAnyFactory"). A DynAny is
 * released with CORBA_Object_release; destroy destroys the DynAny and its
 * components (after which they raise OBJECT_NOT_EXIST), and the handle is
 * still released. Values returned are the caller's, freed with CORBA_free
 * (strings, anys, sequences) or CORBA_Object_release (DynAnys, TypeCodes,
 * references).
 *
 * The C mapping has no valuetypes, so insert_val / get_val are left out;
 * DynValue and DynValueBox work on the values TypeCodes describe (from
 * CORBA_ORB_create_value_tc or an any read from the wire), their members
 * as anys or DynAnys.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#ifndef RUSTORB_C_DYNANY_H
#define RUSTORB_C_DYNANY_H

#include <rustorb/orb.h>
#include <rustorb/runtime.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef CORBA_Object DynamicAny_DynAny;
typedef CORBA_Object DynamicAny_DynFixed;
typedef CORBA_Object DynamicAny_DynEnum;
typedef CORBA_Object DynamicAny_DynStruct;
typedef CORBA_Object DynamicAny_DynUnion;
typedef CORBA_Object DynamicAny_DynSequence;
typedef CORBA_Object DynamicAny_DynArray;
typedef CORBA_Object DynamicAny_DynAnyFactory;
typedef CORBA_Object DynamicAny_DynValueCommon;
typedef CORBA_Object DynamicAny_DynValue;
typedef CORBA_Object DynamicAny_DynValueBox;

/* The sequences of the CORBA module that insert_<type>_seq and
   get_<type>_seq take (CORBA_LongSeq is CORBA_sequence_long...), guarded
   as generated code declares them. */
#ifndef _CORBA_sequence_boolean_defined
#define _CORBA_sequence_boolean_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_boolean, CORBA_boolean);
#define CORBA_sequence_boolean__alloc() ((CORBA_sequence_boolean*)rustorb_c_sequence_alloc())
#define CORBA_sequence_boolean_allocbuf(n) ((CORBA_boolean*)rustorb_c_alloc(&CORBA_boolean__desc, (n)))
#endif
#ifndef _CORBA_sequence_char_defined
#define _CORBA_sequence_char_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_char, CORBA_char);
#define CORBA_sequence_char__alloc() ((CORBA_sequence_char*)rustorb_c_sequence_alloc())
#define CORBA_sequence_char_allocbuf(n) ((CORBA_char*)rustorb_c_alloc(&CORBA_char__desc, (n)))
#endif
#ifndef _CORBA_sequence_wchar_defined
#define _CORBA_sequence_wchar_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_wchar, CORBA_wchar);
#define CORBA_sequence_wchar__alloc() ((CORBA_sequence_wchar*)rustorb_c_sequence_alloc())
#define CORBA_sequence_wchar_allocbuf(n) ((CORBA_wchar*)rustorb_c_alloc(&CORBA_wchar__desc, (n)))
#endif
#ifndef _CORBA_sequence_short_defined
#define _CORBA_sequence_short_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_short, CORBA_short);
#define CORBA_sequence_short__alloc() ((CORBA_sequence_short*)rustorb_c_sequence_alloc())
#define CORBA_sequence_short_allocbuf(n) ((CORBA_short*)rustorb_c_alloc(&CORBA_short__desc, (n)))
#endif
#ifndef _CORBA_sequence_unsigned_short_defined
#define _CORBA_sequence_unsigned_short_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_unsigned_short, CORBA_unsigned_short);
#define CORBA_sequence_unsigned_short__alloc() ((CORBA_sequence_unsigned_short*)rustorb_c_sequence_alloc())
#define CORBA_sequence_unsigned_short_allocbuf(n) ((CORBA_unsigned_short*)rustorb_c_alloc(&CORBA_unsigned_short__desc, (n)))
#endif
#ifndef _CORBA_sequence_long_defined
#define _CORBA_sequence_long_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_long, CORBA_long);
#define CORBA_sequence_long__alloc() ((CORBA_sequence_long*)rustorb_c_sequence_alloc())
#define CORBA_sequence_long_allocbuf(n) ((CORBA_long*)rustorb_c_alloc(&CORBA_long__desc, (n)))
#endif
#ifndef _CORBA_sequence_unsigned_long_defined
#define _CORBA_sequence_unsigned_long_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_unsigned_long, CORBA_unsigned_long);
#define CORBA_sequence_unsigned_long__alloc() ((CORBA_sequence_unsigned_long*)rustorb_c_sequence_alloc())
#define CORBA_sequence_unsigned_long_allocbuf(n) ((CORBA_unsigned_long*)rustorb_c_alloc(&CORBA_unsigned_long__desc, (n)))
#endif
#ifndef _CORBA_sequence_long_long_defined
#define _CORBA_sequence_long_long_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_long_long, CORBA_long_long);
#define CORBA_sequence_long_long__alloc() ((CORBA_sequence_long_long*)rustorb_c_sequence_alloc())
#define CORBA_sequence_long_long_allocbuf(n) ((CORBA_long_long*)rustorb_c_alloc(&CORBA_long_long__desc, (n)))
#endif
#ifndef _CORBA_sequence_unsigned_long_long_defined
#define _CORBA_sequence_unsigned_long_long_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_unsigned_long_long, CORBA_unsigned_long_long);
#define CORBA_sequence_unsigned_long_long__alloc() ((CORBA_sequence_unsigned_long_long*)rustorb_c_sequence_alloc())
#define CORBA_sequence_unsigned_long_long_allocbuf(n) ((CORBA_unsigned_long_long*)rustorb_c_alloc(&CORBA_unsigned_long_long__desc, (n)))
#endif
#ifndef _CORBA_sequence_float_defined
#define _CORBA_sequence_float_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_float, CORBA_float);
#define CORBA_sequence_float__alloc() ((CORBA_sequence_float*)rustorb_c_sequence_alloc())
#define CORBA_sequence_float_allocbuf(n) ((CORBA_float*)rustorb_c_alloc(&CORBA_float__desc, (n)))
#endif
#ifndef _CORBA_sequence_double_defined
#define _CORBA_sequence_double_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_double, CORBA_double);
#define CORBA_sequence_double__alloc() ((CORBA_sequence_double*)rustorb_c_sequence_alloc())
#define CORBA_sequence_double_allocbuf(n) ((CORBA_double*)rustorb_c_alloc(&CORBA_double__desc, (n)))
#endif
#ifndef _CORBA_sequence_long_double_defined
#define _CORBA_sequence_long_double_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_long_double, CORBA_long_double);
#define CORBA_sequence_long_double__alloc() ((CORBA_sequence_long_double*)rustorb_c_sequence_alloc())
#define CORBA_sequence_long_double_allocbuf(n) ((CORBA_long_double*)rustorb_c_alloc(&CORBA_long_double__desc, (n)))
#endif
typedef CORBA_sequence_octet CORBA_OctetSeq;
typedef CORBA_sequence_boolean CORBA_BooleanSeq;
typedef CORBA_sequence_char CORBA_CharSeq;
typedef CORBA_sequence_wchar CORBA_WCharSeq;
typedef CORBA_sequence_short CORBA_ShortSeq;
typedef CORBA_sequence_unsigned_short CORBA_UShortSeq;
typedef CORBA_sequence_long CORBA_LongSeq;
typedef CORBA_sequence_unsigned_long CORBA_ULongSeq;
typedef CORBA_sequence_long_long CORBA_LongLongSeq;
typedef CORBA_sequence_unsigned_long_long CORBA_ULongLongSeq;
typedef CORBA_sequence_float CORBA_FloatSeq;
typedef CORBA_sequence_double CORBA_DoubleSeq;
typedef CORBA_sequence_long_double CORBA_LongDoubleSeq;


typedef CORBA_char* DynamicAny_FieldName;

typedef struct DynamicAny_NameValuePair {
  DynamicAny_FieldName id;
  CORBA_any value;
} DynamicAny_NameValuePair;
typedef struct DynamicAny_NameDynAnyPair {
  DynamicAny_FieldName id;
  DynamicAny_DynAny value;
} DynamicAny_NameDynAnyPair;

#ifndef _CORBA_sequence_any_defined
#define _CORBA_sequence_any_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_any, CORBA_any);
#endif
RUSTORB_C_SEQUENCE(DynamicAny_NameValuePairSeq, DynamicAny_NameValuePair);
RUSTORB_C_SEQUENCE(DynamicAny_NameDynAnyPairSeq, DynamicAny_NameDynAnyPair);
RUSTORB_C_SEQUENCE(DynamicAny_DynAnySeq, DynamicAny_DynAny);
typedef CORBA_sequence_any DynamicAny_AnySeq;

/* Allocation, freed with CORBA_free with what they hold. */
DynamicAny_NameValuePairSeq* DynamicAny_NameValuePairSeq__alloc(void);
DynamicAny_NameValuePair* DynamicAny_NameValuePairSeq_allocbuf(CORBA_unsigned_long len);
DynamicAny_NameDynAnyPairSeq* DynamicAny_NameDynAnyPairSeq__alloc(void);
DynamicAny_NameDynAnyPair* DynamicAny_NameDynAnyPairSeq_allocbuf(CORBA_unsigned_long len);
DynamicAny_DynAnySeq* DynamicAny_DynAnySeq__alloc(void);
DynamicAny_DynAny* DynamicAny_DynAnySeq_allocbuf(CORBA_unsigned_long len);
DynamicAny_AnySeq* DynamicAny_AnySeq__alloc(void);
CORBA_any* DynamicAny_AnySeq_allocbuf(CORBA_unsigned_long len);

/* The user exceptions (no members). */
#define ex_DynamicAny_DynAny_InvalidValue "IDL:omg.org/DynamicAny/DynAny/InvalidValue:1.0"
#define ex_DynamicAny_DynAny_TypeMismatch "IDL:omg.org/DynamicAny/DynAny/TypeMismatch:1.0"
#define ex_DynamicAny_DynAnyFactory_InconsistentTypeCode \
  "IDL:omg.org/DynamicAny/DynAnyFactory/InconsistentTypeCode:1.0"
#define ex_DynamicAny_MustTruncate "IDL:omg.org/DynamicAny/MustTruncate:1.0"

/* --- DynAnyFactory ------------------------------------------------------------ */

DynamicAny_DynAny DynamicAny_DynAnyFactory_create_dyn_any(DynamicAny_DynAnyFactory _obj,
                                                          const CORBA_any* value,
                                                          CORBA_Environment* ev);
DynamicAny_DynAny DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(
    DynamicAny_DynAnyFactory _obj, CORBA_TypeCode type, CORBA_Environment* ev);
DynamicAny_DynAny DynamicAny_DynAnyFactory_create_dyn_any_without_truncation(
    DynamicAny_DynAnyFactory _obj, const CORBA_any* value, CORBA_Environment* ev);
DynamicAny_DynAnySeq* DynamicAny_DynAnyFactory_create_multiple_dyn_anys(
    DynamicAny_DynAnyFactory _obj, const DynamicAny_AnySeq* values, CORBA_boolean allow_truncate,
    CORBA_Environment* ev);
DynamicAny_AnySeq* DynamicAny_DynAnyFactory_create_multiple_anys(DynamicAny_DynAnyFactory _obj,
                                                                 const DynamicAny_DynAnySeq* values,
                                                                 CORBA_Environment* ev);

/* --- DynAny ----------------------------------------------------------------------- */

CORBA_TypeCode DynamicAny_DynAny_type(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_assign(DynamicAny_DynAny _obj, DynamicAny_DynAny dyn_any,
                              CORBA_Environment* ev);
void DynamicAny_DynAny_from_any(DynamicAny_DynAny _obj, const CORBA_any* value,
                                CORBA_Environment* ev);
CORBA_any* DynamicAny_DynAny_to_any(DynamicAny_DynAny _obj, CORBA_Environment* ev);
CORBA_boolean DynamicAny_DynAny_equal(DynamicAny_DynAny _obj, DynamicAny_DynAny dyn_any,
                                      CORBA_Environment* ev);
void DynamicAny_DynAny_destroy(DynamicAny_DynAny _obj, CORBA_Environment* ev);
DynamicAny_DynAny DynamicAny_DynAny_copy(DynamicAny_DynAny _obj, CORBA_Environment* ev);

#define RUSTORB_C_DYN_BASIC(name, type)                                                  \
  void DynamicAny_DynAny_insert_##name(DynamicAny_DynAny _obj, type value,              \
                                       CORBA_Environment* ev);                          \
  type DynamicAny_DynAny_get_##name(DynamicAny_DynAny _obj, CORBA_Environment* ev);
RUSTORB_C_DYN_BASIC(boolean, CORBA_boolean)
RUSTORB_C_DYN_BASIC(octet, CORBA_octet)
RUSTORB_C_DYN_BASIC(char, CORBA_char)
RUSTORB_C_DYN_BASIC(short, CORBA_short)
RUSTORB_C_DYN_BASIC(ushort, CORBA_unsigned_short)
RUSTORB_C_DYN_BASIC(long, CORBA_long)
RUSTORB_C_DYN_BASIC(ulong, CORBA_unsigned_long)
RUSTORB_C_DYN_BASIC(float, CORBA_float)
RUSTORB_C_DYN_BASIC(double, CORBA_double)
RUSTORB_C_DYN_BASIC(longlong, CORBA_long_long)
RUSTORB_C_DYN_BASIC(ulonglong, CORBA_unsigned_long_long)
RUSTORB_C_DYN_BASIC(longdouble, CORBA_long_double)
RUSTORB_C_DYN_BASIC(wchar, CORBA_wchar)
#undef RUSTORB_C_DYN_BASIC

void DynamicAny_DynAny_insert_string(DynamicAny_DynAny _obj, const CORBA_char* value,
                                     CORBA_Environment* ev);
CORBA_char* DynamicAny_DynAny_get_string(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_wstring(DynamicAny_DynAny _obj, const CORBA_wchar* value,
                                      CORBA_Environment* ev);
CORBA_wchar* DynamicAny_DynAny_get_wstring(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_reference(DynamicAny_DynAny _obj, CORBA_Object value,
                                        CORBA_Environment* ev);
CORBA_Object DynamicAny_DynAny_get_reference(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_typecode(DynamicAny_DynAny _obj, CORBA_TypeCode value,
                                       CORBA_Environment* ev);
CORBA_TypeCode DynamicAny_DynAny_get_typecode(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_any(DynamicAny_DynAny _obj, const CORBA_any* value,
                                  CORBA_Environment* ev);
CORBA_any* DynamicAny_DynAny_get_any(DynamicAny_DynAny _obj, CORBA_Environment* ev);

/* insert_<type>_seq / get_<type>_seq (the result is the caller's). */
void DynamicAny_DynAny_insert_boolean_seq(DynamicAny_DynAny _obj, const CORBA_BooleanSeq* value,
                                     CORBA_Environment* ev);
CORBA_BooleanSeq* DynamicAny_DynAny_get_boolean_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_octet_seq(DynamicAny_DynAny _obj, const CORBA_OctetSeq* value,
                                     CORBA_Environment* ev);
CORBA_OctetSeq* DynamicAny_DynAny_get_octet_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_char_seq(DynamicAny_DynAny _obj, const CORBA_CharSeq* value,
                                     CORBA_Environment* ev);
CORBA_CharSeq* DynamicAny_DynAny_get_char_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_wchar_seq(DynamicAny_DynAny _obj, const CORBA_WCharSeq* value,
                                     CORBA_Environment* ev);
CORBA_WCharSeq* DynamicAny_DynAny_get_wchar_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_short_seq(DynamicAny_DynAny _obj, const CORBA_ShortSeq* value,
                                     CORBA_Environment* ev);
CORBA_ShortSeq* DynamicAny_DynAny_get_short_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_ushort_seq(DynamicAny_DynAny _obj, const CORBA_UShortSeq* value,
                                     CORBA_Environment* ev);
CORBA_UShortSeq* DynamicAny_DynAny_get_ushort_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_long_seq(DynamicAny_DynAny _obj, const CORBA_LongSeq* value,
                                     CORBA_Environment* ev);
CORBA_LongSeq* DynamicAny_DynAny_get_long_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_ulong_seq(DynamicAny_DynAny _obj, const CORBA_ULongSeq* value,
                                     CORBA_Environment* ev);
CORBA_ULongSeq* DynamicAny_DynAny_get_ulong_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_longlong_seq(DynamicAny_DynAny _obj, const CORBA_LongLongSeq* value,
                                     CORBA_Environment* ev);
CORBA_LongLongSeq* DynamicAny_DynAny_get_longlong_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_ulonglong_seq(DynamicAny_DynAny _obj, const CORBA_ULongLongSeq* value,
                                     CORBA_Environment* ev);
CORBA_ULongLongSeq* DynamicAny_DynAny_get_ulonglong_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_float_seq(DynamicAny_DynAny _obj, const CORBA_FloatSeq* value,
                                     CORBA_Environment* ev);
CORBA_FloatSeq* DynamicAny_DynAny_get_float_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_double_seq(DynamicAny_DynAny _obj, const CORBA_DoubleSeq* value,
                                     CORBA_Environment* ev);
CORBA_DoubleSeq* DynamicAny_DynAny_get_double_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_longdouble_seq(DynamicAny_DynAny _obj, const CORBA_LongDoubleSeq* value,
                                     CORBA_Environment* ev);
CORBA_LongDoubleSeq* DynamicAny_DynAny_get_longdouble_seq(DynamicAny_DynAny _obj, CORBA_Environment* ev);
void DynamicAny_DynAny_insert_dyn_any(DynamicAny_DynAny _obj, DynamicAny_DynAny value,
                                      CORBA_Environment* ev);
DynamicAny_DynAny DynamicAny_DynAny_get_dyn_any(DynamicAny_DynAny _obj, CORBA_Environment* ev);

CORBA_boolean DynamicAny_DynAny_seek(DynamicAny_DynAny _obj, CORBA_long index,
                                     CORBA_Environment* ev);
void DynamicAny_DynAny_rewind(DynamicAny_DynAny _obj, CORBA_Environment* ev);
CORBA_boolean DynamicAny_DynAny_next(DynamicAny_DynAny _obj, CORBA_Environment* ev);
CORBA_unsigned_long DynamicAny_DynAny_component_count(DynamicAny_DynAny _obj,
                                                      CORBA_Environment* ev);
/* NIL at position -1. */
DynamicAny_DynAny DynamicAny_DynAny_current_component(DynamicAny_DynAny _obj,
                                                      CORBA_Environment* ev);

/* --- DynFixed, DynEnum ---------------------------------------------------------- */

CORBA_char* DynamicAny_DynFixed_get_value(DynamicAny_DynFixed _obj, CORBA_Environment* ev);
CORBA_boolean DynamicAny_DynFixed_set_value(DynamicAny_DynFixed _obj, const CORBA_char* val,
                                            CORBA_Environment* ev);

CORBA_char* DynamicAny_DynEnum_get_as_string(DynamicAny_DynEnum _obj, CORBA_Environment* ev);
void DynamicAny_DynEnum_set_as_string(DynamicAny_DynEnum _obj, const CORBA_char* value,
                                      CORBA_Environment* ev);
CORBA_unsigned_long DynamicAny_DynEnum_get_as_ulong(DynamicAny_DynEnum _obj,
                                                    CORBA_Environment* ev);
void DynamicAny_DynEnum_set_as_ulong(DynamicAny_DynEnum _obj, CORBA_unsigned_long value,
                                     CORBA_Environment* ev);

/* --- DynStruct (and exceptions) ------------------------------------------------- */

DynamicAny_FieldName DynamicAny_DynStruct_current_member_name(DynamicAny_DynStruct _obj,
                                                              CORBA_Environment* ev);
CORBA_TCKind DynamicAny_DynStruct_current_member_kind(DynamicAny_DynStruct _obj,
                                                      CORBA_Environment* ev);
DynamicAny_NameValuePairSeq* DynamicAny_DynStruct_get_members(DynamicAny_DynStruct _obj,
                                                              CORBA_Environment* ev);
void DynamicAny_DynStruct_set_members(DynamicAny_DynStruct _obj,
                                      const DynamicAny_NameValuePairSeq* value,
                                      CORBA_Environment* ev);
DynamicAny_NameDynAnyPairSeq* DynamicAny_DynStruct_get_members_as_dyn_any(
    DynamicAny_DynStruct _obj, CORBA_Environment* ev);
void DynamicAny_DynStruct_set_members_as_dyn_any(DynamicAny_DynStruct _obj,
                                                 const DynamicAny_NameDynAnyPairSeq* value,
                                                 CORBA_Environment* ev);

/* --- DynUnion -------------------------------------------------------------------- */

DynamicAny_DynAny DynamicAny_DynUnion_get_discriminator(DynamicAny_DynUnion _obj,
                                                        CORBA_Environment* ev);
void DynamicAny_DynUnion_set_discriminator(DynamicAny_DynUnion _obj, DynamicAny_DynAny d,
                                           CORBA_Environment* ev);
void DynamicAny_DynUnion_set_to_default_member(DynamicAny_DynUnion _obj, CORBA_Environment* ev);
void DynamicAny_DynUnion_set_to_no_active_member(DynamicAny_DynUnion _obj,
                                                 CORBA_Environment* ev);
CORBA_boolean DynamicAny_DynUnion_has_no_active_member(DynamicAny_DynUnion _obj,
                                                       CORBA_Environment* ev);
CORBA_TCKind DynamicAny_DynUnion_discriminator_kind(DynamicAny_DynUnion _obj,
                                                    CORBA_Environment* ev);
DynamicAny_DynAny DynamicAny_DynUnion_member(DynamicAny_DynUnion _obj, CORBA_Environment* ev);
DynamicAny_FieldName DynamicAny_DynUnion_member_name(DynamicAny_DynUnion _obj,
                                                     CORBA_Environment* ev);
CORBA_TCKind DynamicAny_DynUnion_member_kind(DynamicAny_DynUnion _obj, CORBA_Environment* ev);

/* --- DynSequence, DynArray --------------------------------------------------------- */

CORBA_unsigned_long DynamicAny_DynSequence_get_length(DynamicAny_DynSequence _obj,
                                                      CORBA_Environment* ev);
void DynamicAny_DynSequence_set_length(DynamicAny_DynSequence _obj, CORBA_unsigned_long len,
                                       CORBA_Environment* ev);
DynamicAny_AnySeq* DynamicAny_DynSequence_get_elements(DynamicAny_DynSequence _obj,
                                                       CORBA_Environment* ev);
void DynamicAny_DynSequence_set_elements(DynamicAny_DynSequence _obj,
                                         const DynamicAny_AnySeq* value, CORBA_Environment* ev);
DynamicAny_DynAnySeq* DynamicAny_DynSequence_get_elements_as_dyn_any(
    DynamicAny_DynSequence _obj, CORBA_Environment* ev);
void DynamicAny_DynSequence_set_elements_as_dyn_any(DynamicAny_DynSequence _obj,
                                                    const DynamicAny_DynAnySeq* value,
                                                    CORBA_Environment* ev);

DynamicAny_AnySeq* DynamicAny_DynArray_get_elements(DynamicAny_DynArray _obj,
                                                    CORBA_Environment* ev);
void DynamicAny_DynArray_set_elements(DynamicAny_DynArray _obj, const DynamicAny_AnySeq* value,
                                      CORBA_Environment* ev);
DynamicAny_DynAnySeq* DynamicAny_DynArray_get_elements_as_dyn_any(DynamicAny_DynArray _obj,
                                                                  CORBA_Environment* ev);
void DynamicAny_DynArray_set_elements_as_dyn_any(DynamicAny_DynArray _obj,
                                                 const DynamicAny_DynAnySeq* value,
                                                 CORBA_Environment* ev);

/* DynValueCommon, DynValue, DynValueBox. The operations of DynValueCommon
   have the names of the derived interfaces too. */
CORBA_boolean DynamicAny_DynValueCommon_is_null(DynamicAny_DynValueCommon _obj,
                                                CORBA_Environment* ev);
void DynamicAny_DynValueCommon_set_to_null(DynamicAny_DynValueCommon _obj, CORBA_Environment* ev);
void DynamicAny_DynValueCommon_set_to_value(DynamicAny_DynValueCommon _obj,
                                            CORBA_Environment* ev);
#define DynamicAny_DynValue_is_null DynamicAny_DynValueCommon_is_null
#define DynamicAny_DynValue_set_to_null DynamicAny_DynValueCommon_set_to_null
#define DynamicAny_DynValue_set_to_value DynamicAny_DynValueCommon_set_to_value
#define DynamicAny_DynValueBox_is_null DynamicAny_DynValueCommon_is_null
#define DynamicAny_DynValueBox_set_to_null DynamicAny_DynValueCommon_set_to_null
#define DynamicAny_DynValueBox_set_to_value DynamicAny_DynValueCommon_set_to_value

DynamicAny_FieldName DynamicAny_DynValue_current_member_name(DynamicAny_DynValue _obj,
                                                             CORBA_Environment* ev);
CORBA_TCKind DynamicAny_DynValue_current_member_kind(DynamicAny_DynValue _obj,
                                                     CORBA_Environment* ev);
DynamicAny_NameValuePairSeq* DynamicAny_DynValue_get_members(DynamicAny_DynValue _obj,
                                                             CORBA_Environment* ev);
void DynamicAny_DynValue_set_members(DynamicAny_DynValue _obj,
                                     const DynamicAny_NameValuePairSeq* value,
                                     CORBA_Environment* ev);
DynamicAny_NameDynAnyPairSeq* DynamicAny_DynValue_get_members_as_dyn_any(
    DynamicAny_DynValue _obj, CORBA_Environment* ev);
void DynamicAny_DynValue_set_members_as_dyn_any(DynamicAny_DynValue _obj,
                                                const DynamicAny_NameDynAnyPairSeq* value,
                                                CORBA_Environment* ev);

CORBA_any* DynamicAny_DynValueBox_get_boxed_value(DynamicAny_DynValueBox _obj,
                                                  CORBA_Environment* ev);
void DynamicAny_DynValueBox_set_boxed_value(DynamicAny_DynValueBox _obj, const CORBA_any* boxed,
                                            CORBA_Environment* ev);
DynamicAny_DynAny DynamicAny_DynValueBox_get_boxed_value_as_dyn_any(DynamicAny_DynValueBox _obj,
                                                                   CORBA_Environment* ev);
void DynamicAny_DynValueBox_set_boxed_value_as_dyn_any(DynamicAny_DynValueBox _obj,
                                                       DynamicAny_DynAny boxed,
                                                       CORBA_Environment* ev);

#ifdef __cplusplus
}
#endif

#endif
