/* The DII and the DSI on the C mapping (<rustorb/dii.h>), in one process:
 * NVLists, requests to the Echo servant of interop/impls/rustorbc
 * (CORBA_IN_COPY_VALUE, storage the ORB allocates, send / poll /
 * get_response, oneway, user exceptions in and out of the list) and a
 * PortableServer_DynamicImpl written by hand.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <rustorb/dii.h>

#include "InteropTest.h"
#include "echo_impl.h"

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

static void test_nvlist(CORBA_ORB orb) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_NVList list;
  CORBA_long n = -1, v = 7;
  CORBA_NamedValue* nv;
  CORBA_ORB_create_list(orb, 2, &list, &ev);
  OK(&ev);
  CORBA_NVList_add_item(list, "a", TC_long, &v, sizeof v, CORBA_ARG_IN | CORBA_IN_COPY_VALUE,
                        &ev);
  CORBA_NVList_add_item(list, "b", TC_string, NULL, 0, CORBA_ARG_OUT, &ev);
  OK(&ev);
  CORBA_NVList_get_count(list, &n, &ev);
  CHECK(n == 2);
  v = 8;
  nv = CORBA_NVList_item(list, 0, &ev);
  /* A copy, the list's. */
  CHECK(OK(&ev) && strcmp(nv->name, "a") == 0 && nv->argument._value != &v &&
        *(CORBA_long*)nv->argument._value == 7 && nv->argument._release &&
        nv->arg_modes == CORBA_ARG_IN);
  CHECK(CORBA_NVList_item(list, 2, &ev) == NULL);
  RAISED(&ev, ex_CORBA_BAD_PARAM);
  CORBA_NVList_add_item(list, "c", TC_long, &v, 0, 0, &ev);
  RAISED(&ev, ex_CORBA_BAD_PARAM);
  CORBA_NVList_free_memory(list, &ev);
  CHECK(CORBA_NVList_item(list, 0, &ev)->argument._value == NULL);
  CORBA_NVList_free(list, &ev);
  OK(&ev);
}

static void test_requests(CORBA_ORB orb, CORBA_Object echo) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_Request req;
  CORBA_NamedValue result;
  CORBA_NVList list;
  CORBA_long a = 7, b = 2, rem = 0, quotient = 0;
  CORBA_char* s = (CORBA_char*)"hello";
  int polls;

  /* Arguments from a list; the result into the caller's storage. */
  CORBA_ORB_create_list(orb, 3, &list, &ev);
  CORBA_NVList_add_item(list, "a", TC_long, &a, 0, CORBA_ARG_IN, &ev);
  CORBA_NVList_add_item(list, "b", TC_long, &b, 0, CORBA_ARG_IN, &ev);
  CORBA_NVList_add_item(list, "remainder", TC_long, &rem, 0, CORBA_ARG_OUT, &ev);
  memset(&result, 0, sizeof result);
  result.argument._type = TC_long;
  result.argument._value = &quotient;
  CORBA_Object_create_request(echo, NULL, "divide", list, &result, &req, 0, &ev);
  OK(&ev);
  CORBA_Request_add_exception(req, TC_InteropTest_DivideByZero, &ev);
  CORBA_Request_invoke(req, 0, &ev);
  CHECK(OK(&ev) && quotient == 3 && rem == 1);
  /* Again, raising DivideByZero (in the list). */
  b = 0;
  CORBA_Request_invoke(req, 0, &ev);
  CHECK(ev._major == CORBA_USER_EXCEPTION);
  RAISED(&ev, ex_InteropTest_DivideByZero);
  CORBA_Request_delete(req, &ev);

  /* add_arg; a string result the ORB allocates; send and get_response. */
  CORBA_Object_create_request(echo, NULL, "echo_string", NULL, NULL, &req, 0, &ev);
  CORBA_Request_add_arg(req, "v", TC_string, &s, 0, CORBA_ARG_IN, &ev);
  CORBA_Request_result(req, &ev)->argument._type = TC_string;
  CORBA_Request_get_response(req, 0, &ev);
  RAISED(&ev, ex_CORBA_BAD_INV_ORDER);
  CORBA_Request_send(req, 0, &ev);
  OK(&ev);
  CORBA_Request_send(req, 0, &ev);
  RAISED(&ev, ex_CORBA_BAD_INV_ORDER);
  for (polls = 0; polls < 100000 && !CORBA_Request_poll_response(req, &ev); polls++) OK(&ev);
  CORBA_Request_get_response(req, CORBA_RESP_NO_WAIT, &ev);
  {
    CORBA_NamedValue* r = CORBA_Request_result(req, &ev);
    CHECK(OK(&ev) && r->argument._value && r->argument._release &&
          strcmp(*(CORBA_char**)r->argument._value, "hello") == 0);
  }
  CORBA_Request_poll_response(req, &ev);
  RAISED(&ev, ex_CORBA_BAD_INV_ORDER);
  CORBA_Request_delete(req, &ev);

  /* inout into the caller's storage. */
  a = 1;
  b = 2;
  CORBA_Object_create_request(echo, NULL, "swap", NULL, NULL, &req, 0, &ev);
  CORBA_Request_add_arg(req, "a", TC_long, &a, 0, CORBA_ARG_INOUT, &ev);
  CORBA_Request_add_arg(req, "b", TC_long, &b, 0, CORBA_ARG_INOUT, &ev);
  CORBA_Request_result(req, &ev)->argument._type = TC_long;
  CORBA_Request_invoke(req, 0, &ev);
  CHECK(OK(&ev) && a == 2 && b == 1 &&
        *(CORBA_long*)CORBA_Request_result(req, &ev)->argument._value == 3);
  CORBA_Request_delete(req, &ev);

  /* A user exception not in the list. */
  CORBA_Object_create_request(echo, NULL, "fail", NULL, NULL, &req, 0, &ev);
  a = 5;
  CORBA_Request_add_arg(req, "code", TC_long, &a, 0, CORBA_ARG_IN, &ev);
  CORBA_Request_add_arg(req, "reason", TC_string, &s, 0, CORBA_ARG_IN, &ev);
  CORBA_Request_invoke(req, 0, &ev);
  RAISED(&ev, ex_CORBA_UNKNOWN);
  /* In the list: the value as the generated struct. */
  CORBA_Request_add_exception(req, TC_InteropTest_Oops, &ev);
  CORBA_Request_invoke(req, 0, &ev);
  {
    InteropTest_Oops* o = (InteropTest_Oops*)CORBA_exception_value(&ev);
    CHECK(ev._major == CORBA_USER_EXCEPTION && o && o->code == 5 &&
          strcmp(o->reason, "hello") == 0);
  }
  RAISED(&ev, ex_InteropTest_Oops);
  CORBA_Request_delete(req, &ev);

  /* A system exception; an unknown operation. */
  CORBA_Object_create_request(echo, NULL, "fail_system", NULL, NULL, &req, 0, &ev);
  CORBA_Request_invoke(req, 0, &ev);
  CHECK(ev._major == CORBA_SYSTEM_EXCEPTION &&
        ((CORBA_SystemException*)ev._params)->minor == 42);
  RAISED(&ev, ex_CORBA_BAD_PARAM);
  CORBA_Request_delete(req, &ev);
  CORBA_Object_create_request(echo, NULL, "no_such_op", NULL, NULL, &req, 0, &ev);
  CORBA_Request_invoke(req, 0, &ev);
  RAISED(&ev, ex_CORBA_BAD_OPERATION);
  CORBA_Request_delete(req, &ev);

  /* An in value missing; a context; a nil target. */
  CORBA_Object_create_request(echo, NULL, "echo_long", NULL, NULL, &req, 0, &ev);
  CORBA_Request_add_arg(req, "v", TC_long, NULL, 0, CORBA_ARG_IN, &ev);
  CORBA_Request_invoke(req, 0, &ev);
  RAISED(&ev, ex_CORBA_BAD_PARAM);
  CORBA_Request_delete(req, &ev);
  CORBA_Object_create_request(echo, (CORBA_Context)echo, "echo_long", NULL, NULL, &req, 0, &ev);
  RAISED(&ev, ex_CORBA_NO_IMPLEMENT);
  CORBA_Object_create_request(CORBA_OBJECT_NIL, NULL, "echo_long", NULL, NULL, &req, 0, &ev);
  RAISED(&ev, ex_CORBA_INV_OBJREF);

  /* Oneway. */
  CORBA_Object_create_request(echo, NULL, "post", NULL, NULL, &req, 0, &ev);
  CORBA_Request_add_arg(req, "message", TC_string, &s, 0, CORBA_ARG_IN | CORBA_IN_COPY_VALUE,
                        &ev);
  CORBA_Request_send(req, CORBA_INV_NO_RESPONSE, &ev);
  OK(&ev);
  CORBA_Request_delete(req, &ev);
}

/* --- a DynamicImpl: "add" (long, long -> long), "split" (in string, out
   string, inout long), "boom" (Oops), anything else BAD_OPERATION. --- */

typedef struct Calc {
  POA_PortableServer_DynamicImpl base;
  int calls;
} Calc;

static CORBA_RepositoryId calc_primary(PortableServer_Servant servant,
                                       PortableServer_ObjectId* oid, PortableServer_POA poa,
                                       CORBA_Environment* ev) {
  (void)servant;
  (void)oid;
  (void)poa;
  (void)ev;
  return CORBA_string_dup("IDL:rustorb.org/Test/Calc:1.0");
}

static void calc_invoke(PortableServer_Servant servant, CORBA_ServerRequest sr) {
  Calc* c = (Calc*)servant;
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  const char* op = CORBA_ServerRequest_operation(sr, &ev);
  CORBA_NVList list;
  c->calls++;
  CORBA_ORB_create_list(CORBA_OBJECT_NIL, 2, &list, &ev);
  if (strcmp(op, "add") == 0) {
    CORBA_long r;
    CORBA_any a;
    CORBA_NVList_add_item(list, "a", TC_long, NULL, 0, CORBA_ARG_IN, &ev);
    CORBA_NVList_add_item(list, "b", TC_long, NULL, 0, CORBA_ARG_IN, &ev);
    CORBA_ServerRequest_arguments(sr, list, &ev);
    if (ev._major != CORBA_NO_EXCEPTION) return;
    r = *(CORBA_long*)CORBA_NVList_item(list, 0, &ev)->argument._value +
        *(CORBA_long*)CORBA_NVList_item(list, 1, &ev)->argument._value;
    memset(&a, 0, sizeof a);
    a._type = TC_long;
    a._value = &r;
    CORBA_ServerRequest_set_result(sr, &a, &ev);
  } else if (strcmp(op, "split") == 0) {
    CORBA_NamedValue* in;
    CORBA_NamedValue* out;
    CORBA_NamedValue* n;
    CORBA_NVList_add_item(list, "s", TC_string, NULL, 0, CORBA_ARG_IN, &ev);
    CORBA_NVList_add_item(list, "rest", TC_string, NULL, 0, CORBA_ARG_OUT, &ev);
    CORBA_NVList_add_item(list, "n", TC_long, NULL, 0, CORBA_ARG_INOUT, &ev);
    CORBA_ServerRequest_arguments(sr, list, &ev);
    if (ev._major != CORBA_NO_EXCEPTION) return;
    in = CORBA_NVList_item(list, 0, &ev);
    out = CORBA_NVList_item(list, 1, &ev);
    n = CORBA_NVList_item(list, 2, &ev);
    /* The ORB made zeroed storage for the out value. */
    *(CORBA_char**)out->argument._value = CORBA_string_dup(*(CORBA_char**)in->argument._value + 1);
    *(CORBA_long*)n->argument._value *= 10;
  } else if (strcmp(op, "boom") == 0) {
    InteropTest_Oops o;
    CORBA_any a;
    CORBA_ServerRequest_arguments(sr, list, &ev);
    o.code = 9;
    o.reason = (CORBA_char*)"boom";
    memset(&a, 0, sizeof a);
    a._type = TC_InteropTest_Oops;
    a._value = &o;
    CORBA_ServerRequest_set_exception(sr, CORBA_USER_EXCEPTION, &a, &ev);
  } else {
    CORBA_NVList_free(list, &ev);
    CORBA_ServerRequest_set_system_exception(sr, ex_CORBA_BAD_OPERATION, 7, CORBA_COMPLETED_NO,
                                             &ev);
  }
}

static PortableServer_ServantBase__epv calc_base_epv = {NULL, NULL, NULL};
static PortableServer_DynamicImpl__epv calc_epv = {NULL, calc_invoke, calc_primary};
static POA_PortableServer_DynamicImpl__vepv calc_vepv = {&calc_base_epv, &calc_epv};

static void test_dsi(PortableServer_POA poa) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  Calc calc;
  PortableServer_ObjectId* oid;
  CORBA_Object ref;
  CORBA_Request req;
  CORBA_long a = 2, b = 40, n = 4;
  CORBA_char* s = (CORBA_char*)"xyz";
  memset(&calc, 0, sizeof calc);
  calc.base.vepv = &calc_vepv;
  PortableServer_DynamicImpl__init(&calc, &ev);
  OK(&ev);
  oid = PortableServer_POA_activate_object(poa, &calc, &ev);
  ref = PortableServer_POA_id_to_reference(poa, oid, &ev);
  OK(&ev);
  CHECK(CORBA_Object_is_a(ref, "IDL:rustorb.org/Test/Calc:1.0", &ev));
  CHECK(!CORBA_Object_is_a(ref, "IDL:rustorb.org/Test/Other:1.0", &ev));

  CORBA_Object_create_request(ref, NULL, "add", NULL, NULL, &req, 0, &ev);
  CORBA_Request_add_arg(req, "a", TC_long, &a, 0, CORBA_ARG_IN, &ev);
  CORBA_Request_add_arg(req, "b", TC_long, &b, 0, CORBA_ARG_IN, &ev);
  CORBA_Request_result(req, &ev)->argument._type = TC_long;
  CORBA_Request_invoke(req, 0, &ev);
  CHECK(OK(&ev) && *(CORBA_long*)CORBA_Request_result(req, &ev)->argument._value == 42);
  CORBA_Request_delete(req, &ev);

  CORBA_Object_create_request(ref, NULL, "split", NULL, NULL, &req, 0, &ev);
  CORBA_Request_add_arg(req, "s", TC_string, &s, 0, CORBA_ARG_IN, &ev);
  CORBA_Request_add_arg(req, "rest", TC_string, NULL, 0, CORBA_ARG_OUT, &ev);
  CORBA_Request_add_arg(req, "n", TC_long, &n, 0, CORBA_ARG_INOUT, &ev);
  CORBA_Request_invoke(req, 0, &ev);
  {
    CORBA_NamedValue* rest = CORBA_NVList_item(CORBA_Request_arguments(req, &ev), 1, &ev);
    CHECK(OK(&ev) && n == 40 && rest->argument._value &&
          strcmp(*(CORBA_char**)rest->argument._value, "yz") == 0);
  }
  CORBA_Request_delete(req, &ev);

  CORBA_Object_create_request(ref, NULL, "boom", NULL, NULL, &req, 0, &ev);
  CORBA_Request_add_exception(req, TC_InteropTest_Oops, &ev);
  CORBA_Request_invoke(req, 0, &ev);
  CHECK(ev._major == CORBA_USER_EXCEPTION &&
        ((InteropTest_Oops*)ev._params)->code == 9);
  RAISED(&ev, ex_InteropTest_Oops);
  CORBA_Request_delete(req, &ev);

  CORBA_Object_create_request(ref, NULL, "other", NULL, NULL, &req, 0, &ev);
  CORBA_Request_invoke(req, 0, &ev);
  CHECK(ev._major == CORBA_SYSTEM_EXCEPTION &&
        ((CORBA_SystemException*)ev._params)->minor == 7);
  RAISED(&ev, ex_CORBA_BAD_OPERATION);
  CORBA_Request_delete(req, &ev);

  /* The static stubs reach a DSI servant too. */
  CHECK(calc.calls == 4);

  CORBA_Object_release(ref, &ev);
  PortableServer_POA_deactivate_object(poa, oid, &ev);
  OK(&ev);
  CORBA_free(oid);
  PortableServer_DynamicImpl__fini(&calc, &ev);
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_ORB orb = CORBA_ORB_init(&argc, argv, "", &ev);
  PortableServer_POA poa;
  PortableServer_POAManager manager;
  PortableServer_Servant echo_servant;
  PortableServer_ObjectId* oid;
  CORBA_Object echo;
  if (!OK(&ev)) return 1;
  poa = CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
  manager = PortableServer_POA__get_the_POAManager(poa, &ev);
  PortableServer_POAManager_activate(manager, &ev);
  if (!OK(&ev)) return 1;
  echo_servant = echo_impl_new(orb, &ev);
  oid = PortableServer_POA_activate_object(poa, echo_servant, &ev);
  echo = PortableServer_POA_id_to_reference(poa, oid, &ev);
  if (!OK(&ev)) return 1;

  test_nvlist(orb);
  test_requests(orb, echo);
  test_dsi(poa);

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
  printf("dii: ok\n");
  return 0;
}
