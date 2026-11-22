/* CORBA_exception_as_any (C mapping 1.22): user exceptions of generated
 * code, system exceptions, exceptions of the pseudo objects, and the anys
 * through IOP::Codec.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <string.h>

#include <rustorb/pi.h>

#include "InteropTest.h"

static int failures;

#define CHECK(cond)                                                    \
  do {                                                                 \
    if (!(cond)) {                                                     \
      failures++;                                                      \
      fprintf(stderr, "%s:%d: CHECK(%s) failed\n", __FILE__, __LINE__, #cond); \
    }                                                                  \
  } while (0)

static IOP_Codec codec;

/* The any through the CDR encapsulation and back. */
static CORBA_any* round_trip(const CORBA_any* a) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_sequence_octet* data = IOP_Codec_encode(codec, a, &ev);
  CORBA_any* r;
  CHECK(ev._major == CORBA_NO_EXCEPTION && data);
  CORBA_exception_free(&ev);
  if (!data) return NULL;
  r = IOP_Codec_decode(codec, data, &ev);
  CHECK(ev._major == CORBA_NO_EXCEPTION && r);
  CORBA_exception_free(&ev);
  CORBA_free(data);
  return r;
}

static void check_id(const CORBA_any* a, CORBA_TCKind kind, const char* id) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_char* got;
  CHECK(CORBA_TypeCode_kind(a->_type, &ev) == kind);
  got = CORBA_TypeCode_id(a->_type, &ev);
  CHECK(got && strcmp(got, id) == 0);
  CORBA_free(got);
  CORBA_exception_free(&ev);
}

static void test_user(void) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_Oops* oops = InteropTest_Oops__alloc();
  CORBA_any *a, *r;
  CHECK(CORBA_exception_as_any(&ev) == NULL);
  oops->code = 42;
  oops->reason = CORBA_string_dup("bad");
  CORBA_exception_set(&ev, CORBA_USER_EXCEPTION, ex_InteropTest_Oops, oops);
  a = CORBA_exception_as_any(&ev);
  CHECK(a != NULL);
  if (!a) return;
  /* The same any until CORBA_exception_free. */
  CHECK(CORBA_exception_as_any(&ev) == a);
  check_id(a, CORBA_tk_except, ex_InteropTest_Oops);
  CHECK(a->_value == oops && !CORBA_any_get_release(a));
  r = round_trip(a);
  if (r) {
    InteropTest_Oops* v = (InteropTest_Oops*)r->_value;
    check_id(r, CORBA_tk_except, ex_InteropTest_Oops);
    CHECK(v && v->code == 42 && strcmp(v->reason, "bad") == 0);
    CHECK(CORBA_any_equal(a, r));
    CORBA_free(r);
  }
  CORBA_exception_free(&ev);
  CHECK(CORBA_exception_as_any(&ev) == NULL);

  /* Without members. */
  CORBA_exception_set(&ev, CORBA_USER_EXCEPTION, ex_InteropTest_DivideByZero, NULL);
  a = CORBA_exception_as_any(&ev);
  CHECK(a != NULL);
  if (a) {
    check_id(a, CORBA_tk_except, ex_InteropTest_DivideByZero);
    r = round_trip(a);
    if (r) check_id(r, CORBA_tk_except, ex_InteropTest_DivideByZero);
    CORBA_free(r);
  }
  CORBA_exception_free(&ev);

  /* An exception with members no generated code describes: no any. */
  CORBA_exception_set(&ev, CORBA_USER_EXCEPTION, "IDL:nowhere/Gone:1.0",
                      CORBA_SystemException__alloc());
  CHECK(CORBA_exception_as_any(&ev) == NULL);
  CORBA_exception_free(&ev);
}

static void test_system(void) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any *a, *r;
  CORBA_exception_set_system(&ev, ex_CORBA_BAD_PARAM, 0x4f4d0007, CORBA_COMPLETED_MAYBE);
  a = CORBA_exception_as_any(&ev);
  CHECK(a != NULL);
  if (!a) return;
  check_id(a, CORBA_tk_except, ex_CORBA_BAD_PARAM);
  {
    CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
    CORBA_char* name = CORBA_TypeCode_name(a->_type, &e);
    CORBA_TypeCode completed = CORBA_TypeCode_member_type(a->_type, 1, &e);
    CHECK(name && strcmp(name, "BAD_PARAM") == 0);
    CHECK(CORBA_TypeCode_member_count(a->_type, &e) == 2);
    CHECK(CORBA_TypeCode_kind(completed, &e) == CORBA_tk_enum);
    CHECK(e._major == CORBA_NO_EXCEPTION);
    CORBA_free(name);
    CORBA_Object_release((CORBA_Object)completed, &e);
    CORBA_exception_free(&e);
  }
  r = round_trip(a);
  if (r) {
    CORBA_SystemException* v = (CORBA_SystemException*)r->_value;
    check_id(r, CORBA_tk_except, ex_CORBA_BAD_PARAM);
    CHECK(v && v->minor == 0x4f4d0007 && v->completed == CORBA_COMPLETED_MAYBE);
    CORBA_free(r);
  }
  /* A new exception on the same Environment: a new any. */
  CORBA_exception_set_system(&ev, ex_CORBA_NO_MEMORY, 1, CORBA_COMPLETED_NO);
  a = CORBA_exception_as_any(&ev);
  CHECK(a != NULL);
  if (a) check_id(a, CORBA_tk_except, ex_CORBA_NO_MEMORY);
  CORBA_exception_free(&ev);
}

/* Exceptions of the pseudo objects. */
static void test_pseudo(CORBA_ORB orb) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a;
  CORBA_Object o = CORBA_ORB_resolve_initial_references(orb, "NoSuchService", &ev);
  CHECK(o == CORBA_OBJECT_NIL && ev._major == CORBA_USER_EXCEPTION);
  a = CORBA_exception_as_any(&ev);
  CHECK(a != NULL);
  if (a) {
    CORBA_any* r = round_trip(a);
    check_id(a, CORBA_tk_except, "IDL:omg.org/CORBA/ORB/InvalidName:1.0");
    if (r) check_id(r, CORBA_tk_except, "IDL:omg.org/CORBA/ORB/InvalidName:1.0");
    CORBA_free(r);
  }
  CORBA_exception_free(&ev);
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_ORB orb = CORBA_ORB_init(&argc, argv, "", &ev);
  IOP_CodecFactory factory;
  IOP_Encoding enc = {IOP_ENCODING_CDR_ENCAPS, 1, 2};
  if (ev._major != CORBA_NO_EXCEPTION) return 1;
  factory = (IOP_CodecFactory)CORBA_ORB_resolve_initial_references(orb, "CodecFactory", &ev);
  codec = IOP_CodecFactory_create_codec(factory, &enc, &ev);
  if (ev._major != CORBA_NO_EXCEPTION) return 1;
  /* The types of InteropTest.idl are known once the program uses them. */
  (void)TC_InteropTest_Oops;

  test_user();
  test_system();
  test_pseudo(orb);

  CORBA_Object_release(codec, &ev);
  CORBA_Object_release(factory, &ev);
  CORBA_ORB_destroy(orb, &ev);
  CORBA_Object_release(orb, &ev);
  if (failures) {
    fprintf(stderr, "%d failure(s)\n", failures);
    return 1;
  }
  return 0;
}
