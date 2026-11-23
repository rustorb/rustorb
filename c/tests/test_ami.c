/* Callback AMI (<rustorb/messaging.h>, rustorb-idl -bc -Wbami): the cases
 * of run-ami-probe.sh (interop/impls/rustorbc/src/ami_cases.c) against the
 * Echo servant in this process, whose lines must be those of
 * interop/expected/ami-probe.txt; and the ExceptionHolder functions.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ami_cases.h"
#include "echo_impl.h"

static int failures;

#define CHECK(cond)                                                    \
  do {                                                                 \
    if (!(cond)) {                                                     \
      failures++;                                                      \
      fprintf(stderr, "%s:%d: CHECK(%s) failed\n", __FILE__, __LINE__, #cond); \
    }                                                                  \
  } while (0)

static char output[4096];

static void emit(const char* line) {
  size_t n = strlen(output);
  snprintf(output + n, sizeof output - n, "%s\n", line);
}

static char* read_file(const char* path) {
  FILE* f = fopen(path, "rb");
  char* buf;
  long n;
  if (!f) return NULL;
  fseek(f, 0, SEEK_END);
  n = ftell(f);
  fseek(f, 0, SEEK_SET);
  buf = (char*)calloc(1, (size_t)n + 1);
  if (buf && fread(buf, 1, (size_t)n, f) != (size_t)n) buf[0] = 0;
  fclose(f);
  if (buf) { /* drop the CRs of a Windows checkout (core.autocrlf) */
    char* w = buf;
    const char* r;
    for (r = buf; *r; r++)
      if (*r != '\r') *w++ = *r;
    *w = 0;
  }
  return buf;
}

/* A holder that does not read, and one of an exception not known here. */
static void test_holders(void) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  Messaging_ExceptionHolder* h = Messaging_ExceptionHolder__alloc();
  static CORBA_octet junk[] = {1, 2};
  /* "IDL:x/Y:1.0" and nothing else, little endian. */
  static CORBA_octet unknown[] = {12, 0, 0, 0, 'I', 'D', 'L', ':', 'x', '/', 'Y', ':', '1', '.', '0', 0};
  CORBA_TypeCode list[1];
  h->byte_order = CORBA_TRUE;
  h->marshaled_exception._buffer = junk;
  h->marshaled_exception._length = h->marshaled_exception._maximum = 2;
  h->marshaled_exception._release = CORBA_FALSE;
  Messaging_ExceptionHolder_raise_exception(h, &ev);
  CHECK(ev._major == CORBA_SYSTEM_EXCEPTION && strcmp(ev._id, ex_CORBA_MARSHAL) == 0);
  CORBA_exception_free(&ev);
  h->marshaled_exception._buffer = unknown;
  h->marshaled_exception._length = h->marshaled_exception._maximum = sizeof unknown;
  Messaging_ExceptionHolder_raise_exception(h, &ev);
  CHECK(ev._major == CORBA_SYSTEM_EXCEPTION && strcmp(ev._id, ex_CORBA_UNKNOWN) == 0 &&
        ((CORBA_SystemException*)ev._params)->minor == 0x4f4d0001u);
  CORBA_exception_free(&ev);
  /* A known exception not in the list. */
  {
    /* The id as a little-endian CDR string; DivideByZero has no members. */
    const char* id = ex_InteropTest_DivideByZero;
    CORBA_octet buf[64] = {0};
    size_t n = strlen(id) + 1;
    buf[0] = (CORBA_octet)n;
    memcpy(buf + 4, id, n);
    h->marshaled_exception._buffer = buf;
    h->marshaled_exception._length = h->marshaled_exception._maximum = (CORBA_unsigned_long)(4 + n);
    Messaging_ExceptionHolder_raise_exception(h, &ev);
    CHECK(ev._major == CORBA_USER_EXCEPTION && strcmp(ev._id, ex_InteropTest_DivideByZero) == 0);
    CORBA_exception_free(&ev);
    list[0] = TC_InteropTest_Oops;
    Messaging_ExceptionHolder_raise_exception_with_list(h, list, 1, &ev);
    CHECK(ev._major == CORBA_SYSTEM_EXCEPTION && strcmp(ev._id, ex_CORBA_UNKNOWN) == 0);
    CORBA_exception_free(&ev);
    list[0] = TC_InteropTest_DivideByZero;
    Messaging_ExceptionHolder_raise_exception_with_list(h, list, 1, &ev);
    CHECK(ev._major == CORBA_USER_EXCEPTION);
    CORBA_exception_free(&ev);
  }
  h->marshaled_exception._buffer = NULL;
  h->marshaled_exception._length = h->marshaled_exception._maximum = 0;
  CORBA_free(h);
  CHECK(CORBA_TypeCode_kind(TC_Messaging_ExceptionHolder, &ev) == CORBA_tk_value);
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_ORB orb = CORBA_ORB_init(&argc, argv, "", &ev);
  PortableServer_POA poa;
  PortableServer_POAManager manager;
  PortableServer_Servant echo_servant;
  PortableServer_ObjectId* oid;
  CORBA_Object echo;
  char* expected;
  if (ev._major != CORBA_NO_EXCEPTION) return 1;
  poa = CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
  manager = PortableServer_POA__get_the_POAManager(poa, &ev);
  PortableServer_POAManager_activate(manager, &ev);
  echo_servant = echo_impl_new(orb, &ev);
  oid = PortableServer_POA_activate_object(poa, echo_servant, &ev);
  echo = PortableServer_POA_id_to_reference(poa, oid, &ev);
  if (ev._major != CORBA_NO_EXCEPTION) {
    fprintf(stderr, "setup: %s\n", CORBA_exception_id(&ev));
    return 1;
  }

  /* Not for a nil target, nor a local object. */
  InteropTest_Echo_sendc_echo_long(CORBA_OBJECT_NIL, CORBA_OBJECT_NIL, 1, &ev);
  CHECK(ev._major == CORBA_SYSTEM_EXCEPTION && strcmp(ev._id, ex_CORBA_INV_OBJREF) == 0);
  CORBA_exception_free(&ev);

  CHECK(ami_cases(poa, echo, emit) == 0);
  expected = read_file(AMI_EXPECTED);
  CHECK(expected != NULL);
  if (expected && strcmp(expected, output) != 0) {
    failures++;
    fprintf(stderr, "got:\n%sexpected:\n%s", output, expected);
  }
  free(expected);
  test_holders();

  CORBA_Object_release(echo, &ev);
  PortableServer_POA_deactivate_object(poa, oid, &ev);
  CORBA_free(oid);
  CORBA_Object_release(manager, &ev);
  CORBA_Object_release(poa, &ev);
  CORBA_ORB_destroy(orb, &ev);
  CORBA_Object_release(orb, &ev);
  echo_impl_free(echo_servant);
  if (failures) {
    fprintf(stderr, "%d failure(s)\n", failures);
    return 1;
  }
  printf("ami: ok\n");
  return 0;
}
