/* DynamicAny on the C mapping (<rustorb/dynany.h>): DynAnys made from
 * values and from TypeCodes of interop/idl/AnyTest.idl, their components,
 * the derived interfaces, the exceptions and destroy.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <rustorb/dynany.h>

#include "AnyTest.h"

static int failures;

#define CHECK(cond)                                                    \
  do {                                                                 \
    if (!(cond)) {                                                     \
      failures++;                                                      \
      fprintf(stderr, "%s:%d: CHECK(%s) failed\n", __FILE__, __LINE__, #cond); \
    }                                                                  \
  } while (0)

static int ok(CORBA_Environment* ev, int line) {
  if (ev->_major == CORBA_NO_EXCEPTION) return 1;
  failures++;
  fprintf(stderr, "%s:%d: unexpected %s\n", __FILE__, line, CORBA_exception_id(ev));
  CORBA_exception_free(ev);
  return 0;
}
#define OK(ev) ok((ev), __LINE__)

/* ev holds the exception id (and is freed). */
static int raised(CORBA_Environment* ev, const char* id, int line) {
  int r = ev->_major != CORBA_NO_EXCEPTION && strcmp(CORBA_exception_id(ev), id) == 0;
  if (!r) {
    failures++;
    fprintf(stderr, "%s:%d: wanted %s, got %s\n", __FILE__, line, id,
            ev->_major == CORBA_NO_EXCEPTION ? "no exception" : CORBA_exception_id(ev));
  }
  CORBA_exception_free(ev);
  return r;
}
#define RAISED(ev, id) raised((ev), (id), __LINE__)

static CORBA_any any_of(CORBA_TypeCode tc, void* value) {
  CORBA_any a;
  memset(&a, 0, sizeof a);
  a._type = tc;
  a._value = value;
  return a;
}

static void test_basic(DynamicAny_DynAnyFactory f) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_long v = 42;
  CORBA_any a = any_of(TC_long, &v);
  DynamicAny_DynAny d = DynamicAny_DynAnyFactory_create_dyn_any(f, &a, &ev);
  CORBA_any* back;
  CORBA_char* s;
  if (!OK(&ev)) return;
  CHECK(CORBA_Object_is_a(d, "IDL:omg.org/DynamicAny/DynAny:1.0", &ev));
  CHECK(!CORBA_Object_is_a(d, "IDL:omg.org/DynamicAny/DynStruct:1.0", &ev));
  CHECK(DynamicAny_DynAny_get_long(d, &ev) == 42 && OK(&ev));
  CHECK(DynamicAny_DynAny_component_count(d, &ev) == 0);
  DynamicAny_DynAny_get_short(d, &ev);
  RAISED(&ev, ex_DynamicAny_DynAny_TypeMismatch);
  DynamicAny_DynAny_insert_long(d, -5, &ev);
  OK(&ev);
  back = DynamicAny_DynAny_to_any(d, &ev);
  CHECK(OK(&ev) && back && *(CORBA_long*)back->_value == -5);
  CORBA_free(back);
  CORBA_Object_release(d, &ev);

  /* From TypeCodes: strings, bounded strings, wstrings, any, TypeCode. */
  d = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, TC_AnyTest_Short5, &ev);
  DynamicAny_DynAny_insert_string(d, "abcdef", &ev);
  RAISED(&ev, ex_DynamicAny_DynAny_InvalidValue);
  DynamicAny_DynAny_insert_string(d, "abc", &ev);
  s = DynamicAny_DynAny_get_string(d, &ev);
  CHECK(OK(&ev) && strcmp(s, "abc") == 0);
  CORBA_free(s);
  CORBA_Object_release(d, &ev);

  d = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, TC_wstring, &ev);
  {
    CORBA_wchar w[] = {'h', 0x3042, 0};
    CORBA_wchar* got;
    DynamicAny_DynAny_insert_wstring(d, w, &ev);
    got = DynamicAny_DynAny_get_wstring(d, &ev);
    CHECK(OK(&ev) && got && got[0] == 'h' && got[1] == 0x3042 && got[2] == 0);
    CORBA_free(got);
  }
  CORBA_Object_release(d, &ev);

  d = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, TC_any, &ev);
  {
    CORBA_any* inner;
    DynamicAny_DynAny id;
    DynamicAny_DynAny_insert_any(d, &a, &ev);
    OK(&ev);
    /* get_dyn_any is a DynAny for the contained value. */
    id = DynamicAny_DynAny_get_dyn_any(d, &ev);
    CHECK(OK(&ev) && DynamicAny_DynAny_get_long(id, &ev) == 42 && OK(&ev));
    inner = DynamicAny_DynAny_get_any(d, &ev);
    CHECK(OK(&ev) && inner && *(CORBA_long*)inner->_value == 42);
    CORBA_free(inner);
    CORBA_Object_release(id, &ev);
  }
  CORBA_Object_release(d, &ev);

  d = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, TC_TypeCode, &ev);
  {
    CORBA_TypeCode t;
    DynamicAny_DynAny_insert_typecode(d, TC_AnyTest_Point, &ev);
    t = DynamicAny_DynAny_get_typecode(d, &ev);
    CHECK(OK(&ev) && CORBA_TypeCode_equal(t, TC_AnyTest_Point, &ev));
    CORBA_Object_release(t, &ev);
  }
  CORBA_Object_release(d, &ev);
}

static void test_struct(DynamicAny_DynAnyFactory f) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  DynamicAny_DynStruct d = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, TC_AnyTest_Point, &ev);
  DynamicAny_NameValuePairSeq* m;
  DynamicAny_NameDynAnyPairSeq* md;
  DynamicAny_DynAny c;
  CORBA_char* name;
  CORBA_any* back;
  if (!OK(&ev)) return;
  CHECK(CORBA_Object_is_a(d, "IDL:omg.org/DynamicAny/DynStruct:1.0", &ev));
  CHECK(DynamicAny_DynAny_component_count(d, &ev) == 2);
  /* insert_ does not move the current position; next does. */
  DynamicAny_DynAny_insert_long(d, 3, &ev);
  name = DynamicAny_DynStruct_current_member_name(d, &ev);
  CHECK(OK(&ev) && strcmp(name, "x") == 0);
  CORBA_free(name);
  CHECK(DynamicAny_DynAny_next(d, &ev) && OK(&ev));
  name = DynamicAny_DynStruct_current_member_name(d, &ev);
  CHECK(OK(&ev) && strcmp(name, "y") == 0);
  CORBA_free(name);
  CHECK(DynamicAny_DynStruct_current_member_kind(d, &ev) == CORBA_tk_double);
  DynamicAny_DynAny_insert_double(d, 2.5, &ev);
  OK(&ev);
  back = DynamicAny_DynAny_to_any(d, &ev);
  CHECK(OK(&ev) && back && ((AnyTest_Point*)back->_value)->x == 3 &&
        ((AnyTest_Point*)back->_value)->y == 2.5);
  CORBA_free(back);

  m = DynamicAny_DynStruct_get_members(d, &ev);
  CHECK(OK(&ev) && m && m->_length == 2 && strcmp(m->_buffer[0].id, "x") == 0 &&
        *(CORBA_long*)m->_buffer[0].value._value == 3);
  if (m && m->_length == 2) {
    *(CORBA_long*)m->_buffer[0].value._value = 8;
    DynamicAny_DynStruct_set_members(d, m, &ev);
    OK(&ev);
    CORBA_free(m->_buffer[0].id);
    m->_buffer[0].id = CORBA_string_dup("nope");
    DynamicAny_DynStruct_set_members(d, m, &ev);
    RAISED(&ev, ex_DynamicAny_DynAny_TypeMismatch);
  }
  CORBA_free(m);

  /* The components are shared with the DynAny. */
  md = DynamicAny_DynStruct_get_members_as_dyn_any(d, &ev);
  CHECK(OK(&ev) && md && md->_length == 2);
  if (md && md->_length == 2) DynamicAny_DynAny_insert_long(md->_buffer[0].value, 12, &ev);
  CORBA_free(md);
  DynamicAny_DynAny_rewind(d, &ev);
  c = DynamicAny_DynAny_current_component(d, &ev);
  CHECK(OK(&ev) && DynamicAny_DynAny_get_long(c, &ev) == 12);
  CORBA_Object_release(c, &ev);

  /* seek, next, -1. */
  CHECK(!DynamicAny_DynAny_seek(d, 5, &ev));
  CHECK(DynamicAny_DynAny_current_component(d, &ev) == CORBA_OBJECT_NIL && OK(&ev));
  /* From -1, next moves to the first component. */
  CHECK(DynamicAny_DynAny_next(d, &ev) == CORBA_TRUE);
  CHECK(DynamicAny_DynAny_seek(d, 1, &ev) && !DynamicAny_DynAny_next(d, &ev));

  /* A derived operation on another kind. */
  DynamicAny_DynEnum_get_as_ulong(d, &ev);
  RAISED(&ev, ex_CORBA_BAD_OPERATION);

  /* destroy: then OBJECT_NOT_EXIST, also for a component. */
  DynamicAny_DynAny_seek(d, 0, &ev);
  c = DynamicAny_DynAny_current_component(d, &ev);
  DynamicAny_DynAny_destroy(d, &ev);
  OK(&ev);
  DynamicAny_DynAny_get_long(c, &ev);
  RAISED(&ev, ex_CORBA_OBJECT_NOT_EXIST);
  DynamicAny_DynAny_component_count(d, &ev);
  RAISED(&ev, ex_CORBA_OBJECT_NOT_EXIST);
  CORBA_Object_release(c, &ev);
  CORBA_Object_release(d, &ev);
}

static void test_union_enum(DynamicAny_DynAnyFactory f) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  AnyTest_Choice ch;
  CORBA_any a;
  DynamicAny_DynUnion u;
  DynamicAny_DynAny disc, m;
  DynamicAny_DynEnum e;
  CORBA_char* s;
  memset(&ch, 0, sizeof ch);
  ch._d = 3;
  ch._u.s = (CORBA_char*)"three";
  a = any_of(TC_AnyTest_Choice, &ch);
  u = DynamicAny_DynAnyFactory_create_dyn_any(f, &a, &ev);
  if (!OK(&ev)) return;
  CHECK(CORBA_Object_is_a(u, "IDL:omg.org/DynamicAny/DynUnion:1.0", &ev));
  CHECK(DynamicAny_DynUnion_discriminator_kind(u, &ev) == CORBA_tk_long);
  s = DynamicAny_DynUnion_member_name(u, &ev);
  CHECK(OK(&ev) && strcmp(s, "s") == 0);
  CORBA_free(s);
  m = DynamicAny_DynUnion_member(u, &ev);
  s = DynamicAny_DynAny_get_string(m, &ev);
  CHECK(OK(&ev) && strcmp(s, "three") == 0);
  CORBA_free(s);
  CORBA_Object_release(m, &ev);
  /* A discriminator value of no case: the default member. */
  disc = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, TC_long, &ev);
  DynamicAny_DynAny_insert_long(disc, 9, &ev);
  DynamicAny_DynUnion_set_discriminator(u, disc, &ev);
  OK(&ev);
  CHECK(!DynamicAny_DynUnion_has_no_active_member(u, &ev));
  CHECK(DynamicAny_DynUnion_member_kind(u, &ev) == CORBA_tk_boolean);
  CORBA_Object_release(disc, &ev);
  DynamicAny_DynUnion_set_to_no_active_member(u, &ev);
  RAISED(&ev, ex_DynamicAny_DynAny_TypeMismatch);
  DynamicAny_DynAny_destroy(u, &ev);
  CORBA_Object_release(u, &ev);

  e = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, TC_AnyTest_Shade, &ev);
  CHECK(CORBA_Object_is_a(e, "IDL:omg.org/DynamicAny/DynEnum:1.0", &ev));
  DynamicAny_DynEnum_set_as_string(e, "DIM", &ev);
  CHECK(OK(&ev) && DynamicAny_DynEnum_get_as_ulong(e, &ev) == AnyTest_DIM);
  DynamicAny_DynEnum_set_as_ulong(e, 7, &ev);
  RAISED(&ev, ex_DynamicAny_DynAny_InvalidValue);
  DynamicAny_DynEnum_set_as_ulong(e, 1, &ev);
  s = DynamicAny_DynEnum_get_as_string(e, &ev);
  CHECK(OK(&ev) && strcmp(s, "DARK") == 0);
  CORBA_free(s);
  CORBA_Object_release(e, &ev);
}

static void test_sequence_array_fixed(DynamicAny_DynAnyFactory f) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  DynamicAny_DynSequence q = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, TC_AnyTest_LongSeq, &ev);
  DynamicAny_AnySeq* el;
  DynamicAny_DynAnySeq* del;
  DynamicAny_DynArray arr;
  DynamicAny_DynFixed fx;
  CORBA_any* back;
  CORBA_char* s;
  if (!OK(&ev)) return;
  DynamicAny_DynSequence_set_length(q, 3, &ev);
  CHECK(OK(&ev) && DynamicAny_DynAny_component_count(q, &ev) == 3);
  DynamicAny_DynAny_seek(q, 0, &ev);
  DynamicAny_DynAny_insert_long(q, 1, &ev);
  DynamicAny_DynAny_next(q, &ev);
  DynamicAny_DynAny_insert_long(q, 2, &ev);
  DynamicAny_DynAny_next(q, &ev);
  DynamicAny_DynAny_insert_long(q, 3, &ev);
  OK(&ev);
  el = DynamicAny_DynSequence_get_elements(q, &ev);
  CHECK(OK(&ev) && el && el->_length == 3 && *(CORBA_long*)el->_buffer[2]._value == 3);
  if (el && el->_length == 3) {
    el->_length = 2;
    DynamicAny_DynSequence_set_elements(q, el, &ev);
    CHECK(OK(&ev) && DynamicAny_DynSequence_get_length(q, &ev) == 2);
    el->_length = 3;
  }
  CORBA_free(el);
  back = DynamicAny_DynAny_to_any(q, &ev);
  CHECK(OK(&ev) && back && ((AnyTest_LongSeq*)back->_value)->_length == 2);
  CORBA_free(back);
  del = DynamicAny_DynSequence_get_elements_as_dyn_any(q, &ev);
  CHECK(OK(&ev) && del && del->_length == 2);
  if (del && del->_length == 2) {
    DynamicAny_DynSequence_set_elements_as_dyn_any(q, del, &ev);
    OK(&ev);
  }
  CORBA_free(del);
  CORBA_Object_release(q, &ev);

  arr = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, TC_AnyTest_ShortArray, &ev);
  CHECK(CORBA_Object_is_a(arr, "IDL:omg.org/DynamicAny/DynArray:1.0", &ev));
  CHECK(DynamicAny_DynAny_component_count(arr, &ev) == 3);
  el = DynamicAny_DynArray_get_elements(arr, &ev);
  CHECK(OK(&ev) && el && el->_length == 3);
  if (el && el->_length == 3) {
    el->_length = 2;
    DynamicAny_DynArray_set_elements(arr, el, &ev);
    RAISED(&ev, ex_DynamicAny_DynAny_InvalidValue);
    el->_length = 3;
  }
  CORBA_free(el);
  CORBA_Object_release(arr, &ev);

  fx = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, TC_AnyTest_Money, &ev);
  CHECK(DynamicAny_DynFixed_set_value(fx, "-123.45", &ev) && OK(&ev));
  s = DynamicAny_DynFixed_get_value(fx, &ev);
  CHECK(OK(&ev) && strcmp(s, "-123.45") == 0);
  CORBA_free(s);
  back = DynamicAny_DynAny_to_any(fx, &ev);
  CHECK(OK(&ev) && back && ((AnyTest_Money*)back->_value)->_value[2] == 0x5d);
  CORBA_free(back);
  DynamicAny_DynFixed_set_value(fx, "x", &ev);
  RAISED(&ev, ex_DynamicAny_DynAny_TypeMismatch);
  CORBA_Object_release(fx, &ev);
}

static void test_copy_equal_multiple(DynamicAny_DynAnyFactory f) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  AnyTest_Point p = {1, 2.5};
  CORBA_any a = any_of(TC_AnyTest_Point, &p);
  DynamicAny_DynAny d = DynamicAny_DynAnyFactory_create_dyn_any(f, &a, &ev);
  DynamicAny_DynAny c = DynamicAny_DynAny_copy(d, &ev);
  DynamicAny_AnySeq anys;
  DynamicAny_DynAnySeq* dyns;
  DynamicAny_AnySeq* back;
  CORBA_TypeCode t;
  CHECK(OK(&ev) && DynamicAny_DynAny_equal(d, c, &ev));
  DynamicAny_DynAny_insert_long(c, 2, &ev);
  CHECK(!DynamicAny_DynAny_equal(d, c, &ev));
  DynamicAny_DynAny_assign(d, c, &ev);
  CHECK(OK(&ev) && DynamicAny_DynAny_equal(d, c, &ev));
  t = DynamicAny_DynAny_type(d, &ev);
  CHECK(OK(&ev) && CORBA_TypeCode_equal(t, TC_AnyTest_Point, &ev));
  CORBA_Object_release(t, &ev);
  {
    CORBA_long l = 1;
    CORBA_any wrong = any_of(TC_long, &l);
    DynamicAny_DynAny_from_any(d, &wrong, &ev);
    RAISED(&ev, ex_DynamicAny_DynAny_TypeMismatch);
  }

  anys._maximum = anys._length = 1;
  anys._buffer = &a;
  anys._release = CORBA_FALSE;
  dyns = DynamicAny_DynAnyFactory_create_multiple_dyn_anys(f, &anys, CORBA_TRUE, &ev);
  CHECK(OK(&ev) && dyns && dyns->_length == 1);
  back = DynamicAny_DynAnyFactory_create_multiple_anys(f, dyns, &ev);
  CHECK(OK(&ev) && back && back->_length == 1 && CORBA_any_equal(&back->_buffer[0], &a));
  CORBA_free(back);
  CORBA_free(dyns);
  CORBA_Object_release(c, &ev);
  CORBA_Object_release(d, &ev);

  /* An unknown TypeCode kind. */
  d = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, rustorb_c_tc_simple(CORBA_tk_Principal), &ev);
  RAISED(&ev, ex_DynamicAny_DynAnyFactory_InconsistentTypeCode);
  CHECK(d == CORBA_OBJECT_NIL);
}

/* TypeCode factories, DynValue, DynValueBox and insert_<type>_seq, every
   result released (the probe checks the behaviour; this the memory). */
static void test_factories_values(CORBA_ORB orb, DynamicAny_DynAnyFactory f) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_long one = 1, three = 3;
  CORBA_octet zero = 0;
  CORBA_UnionMember um[2];
  CORBA_UnionMemberSeq useq = {2, 2, NULL, CORBA_FALSE};
  CORBA_StructMemberSeq* sseq = CORBA_StructMemberSeq__alloc();
  CORBA_ValueMember vm[1];
  CORBA_ValueMemberSeq vseq = {1, 1, NULL, CORBA_FALSE};
  CORBA_TypeCode u, st, v, box, lseq;
  DynamicAny_DynAny d, c;
  CORBA_LongSeq in = {2, 2, NULL, CORBA_FALSE};
  CORBA_LongSeq* got;
  CORBA_long vals[2] = {7, 8};
  DynamicAny_NameValuePairSeq* m;
  CORBA_any a, *back;

  memset(um, 0, sizeof um);
  um[0].name = "x";
  um[0].label._type = TC_long;
  um[0].label._value = &one;
  um[0].type = TC_long;
  um[1].name = "z";
  um[1].label._type = TC_octet;
  um[1].label._value = &zero;
  um[1].type = TC_string;
  useq._buffer = um;
  u = CORBA_ORB_create_union_tc(orb, "IDL:T/U:1.0", "U", TC_long, &useq, &ev);
  CHECK(OK(&ev) && CORBA_TypeCode_default_index(u, &ev) == 1);
  /* Two default labels. */
  um[0].label._type = TC_octet;
  CHECK(CORBA_ORB_create_union_tc(orb, "IDL:T/U:1.0", "U", TC_long, &useq, &ev) == NULL &&
        ev._major == CORBA_SYSTEM_EXCEPTION);
  CORBA_exception_free(&ev);

  /* A struct of the union, from an allocated member sequence. */
  sseq->_buffer = CORBA_StructMemberSeq_allocbuf(1);
  sseq->_length = sseq->_maximum = 1;
  sseq->_release = CORBA_TRUE;
  sseq->_buffer[0].name = CORBA_string_dup("u");
  sseq->_buffer[0].type = (CORBA_TypeCode)CORBA_Object_duplicate((CORBA_Object)u, &ev);
  st = CORBA_ORB_create_struct_tc(orb, "IDL:T/S:1.0", "S", sseq, &ev);
  CHECK(OK(&ev) && CORBA_TypeCode_member_count(st, &ev) == 1);
  CORBA_free(sseq);
  d = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, st, &ev);
  CHECK(OK(&ev) && DynamicAny_DynAny_component_count(d, &ev) == 1);
  CORBA_Object_release(d, &ev);

  memset(vm, 0, sizeof vm);
  vm[0].name = "n";
  vm[0].type = TC_long;
  vm[0].access = CORBA_PUBLIC_MEMBER;
  vseq._buffer = vm;
  v = CORBA_ORB_create_value_tc(orb, "IDL:T/V:1.0", "V", CORBA_VM_NONE, NULL, &vseq, &ev);
  CHECK(OK(&ev) && CORBA_TypeCode_kind(v, &ev) == CORBA_tk_value);
  d = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, v, &ev);
  CHECK(OK(&ev) && CORBA_Object_is_a(d, "IDL:omg.org/DynamicAny/DynValue:1.0", &ev) &&
        CORBA_Object_is_a(d, "IDL:omg.org/DynamicAny/DynValueCommon:1.0", &ev));
  CHECK(DynamicAny_DynValue_is_null(d, &ev) && OK(&ev));
  DynamicAny_DynValue_set_to_value(d, &ev);
  c = DynamicAny_DynAny_current_component(d, &ev);
  DynamicAny_DynAny_insert_long(c, 3, &ev);
  CORBA_Object_release(c, &ev);
  m = DynamicAny_DynValue_get_members(d, &ev);
  CHECK(OK(&ev) && m && m->_length == 1 && strcmp(m->_buffer[0].id, "n") == 0 &&
        *(CORBA_long*)m->_buffer[0].value._value == 3);
  CORBA_free(m);
  back = DynamicAny_DynAny_to_any(d, &ev);
  CHECK(OK(&ev) && back);
  CORBA_free(back);
  CORBA_Object_release(d, &ev);

  box = CORBA_ORB_create_value_box_tc(orb, "IDL:T/B:1.0", "B", TC_long, &ev);
  d = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, box, &ev);
  a._type = TC_long;
  a._value = &three;
  a._release = CORBA_FALSE;
  a._rustorb = NULL;
  DynamicAny_DynValueBox_set_boxed_value(d, &a, &ev);
  back = DynamicAny_DynValueBox_get_boxed_value(d, &ev);
  CHECK(OK(&ev) && back && *(CORBA_long*)back->_value == 3);
  CORBA_free(back);
  CORBA_Object_release(d, &ev);

  lseq = CORBA_ORB_create_sequence_tc(orb, 0, TC_long, &ev);
  d = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, lseq, &ev);
  in._buffer = vals;
  DynamicAny_DynAny_insert_long_seq(d, &in, &ev);
  got = DynamicAny_DynAny_get_long_seq(d, &ev);
  CHECK(OK(&ev) && got && got->_length == 2 && got->_buffer[0] == 7 && got->_buffer[1] == 8);
  CORBA_free(got);
  CORBA_Object_release(d, &ev);

  CORBA_Object_release((CORBA_Object)lseq, &ev);
  CORBA_Object_release((CORBA_Object)box, &ev);
  CORBA_Object_release((CORBA_Object)v, &ev);
  CORBA_Object_release((CORBA_Object)st, &ev);
  CORBA_Object_release((CORBA_Object)u, &ev);
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_ORB orb = CORBA_ORB_init(&argc, argv, "", &ev);
  DynamicAny_DynAnyFactory f;
  CORBA_ORB_ObjectIdList* services;
  CORBA_unsigned_long i;
  int listed = 0;
  if (!OK(&ev)) return 1;
  f = CORBA_ORB_resolve_initial_references(orb, "DynAnyFactory", &ev);
  if (!OK(&ev)) return 1;
  CHECK(CORBA_Object_is_a(f, "IDL:omg.org/DynamicAny/DynAnyFactory:1.0", &ev));
  services = CORBA_ORB_list_initial_services(orb, &ev);
  for (i = 0; services && i < services->_length; i++)
    if (strcmp(services->_buffer[i], "DynAnyFactory") == 0) listed = 1;
  CHECK(listed);
  CORBA_free(services);
  test_basic(f);
  test_struct(f);
  test_union_enum(f);
  test_sequence_array_fixed(f);
  test_copy_equal_multiple(f);
  test_factories_values(orb, f);
  CORBA_Object_release(f, &ev);
  CORBA_ORB_destroy(orb, &ev);
  CORBA_Object_release(orb, &ev);
  if (failures) {
    fprintf(stderr, "%d failure(s)\n", failures);
    return 1;
  }
  printf("dynany: ok\n");
  return 0;
}
