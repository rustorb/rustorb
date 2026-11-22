/* Local interfaces in -bc (LocalTest.idl, the one of the .NET tests): an
 * implementation struct with POA_<L>__init, local objects from
 * <L>__create, the stubs calling it without the ORB (attributes, out
 * parameters, user exceptions, inherited operations), is_a and the
 * reference operations, and a local object as a parameter and result.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "LocalTest.h"

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

/* --- Loud (and so Greeter) ------------------------------------------------- */

typedef struct LoudImpl {
  POA_LocalTest_Loud base;
  CORBA_long count;
  int shouts;
} LoudImpl;

static CORBA_char* greet(PortableServer_Servant s, const CORBA_char* who, CORBA_Environment* ev) {
  char buf[64];
  (void)s;
  if (strcmp(who, "nobody") == 0) {
    LocalTest_Greeter_Busy* b = LocalTest_Greeter_Busy__alloc();
    b->why = CORBA_string_dup("no one there");
    CORBA_exception_set(ev, CORBA_USER_EXCEPTION, ex_LocalTest_Greeter_Busy, b);
    return NULL;
  }
  snprintf(buf, sizeof buf, "hello, %s", who);
  return CORBA_string_dup(buf);
}

static CORBA_long get_count(PortableServer_Servant s, CORBA_Environment* ev) {
  (void)ev;
  return ((LoudImpl*)s)->count;
}

static void set_count(PortableServer_Servant s, CORBA_long v, CORBA_Environment* ev) {
  (void)ev;
  ((LoudImpl*)s)->count = v;
}

static LocalTest_Remote get_target(PortableServer_Servant s, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return CORBA_OBJECT_NIL;
}

static LocalTest_Greeter_Msg* make(PortableServer_Servant s, CORBA_long* n, CORBA_Environment* ev) {
  LocalTest_Greeter_Msg* m = LocalTest_Greeter_Msg__alloc();
  (void)ev;
  *n = ((LoudImpl*)s)->count * 2;
  m->text = CORBA_string_dup("made");
  return m;
}

static void shout(PortableServer_Servant s, CORBA_Environment* ev) {
  (void)ev;
  ((LoudImpl*)s)->shouts++;
}

static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static POA_LocalTest_Greeter__epv greeter_epv = {NULL, greet, get_count, set_count, get_target, make};
static POA_LocalTest_Loud__epv loud_epv = {NULL, shout};
static POA_LocalTest_Loud__vepv loud_vepv = {&base_epv, &greeter_epv, &loud_epv};

/* --- Factory -------------------------------------------------------------- */

typedef struct FactoryImpl {
  POA_LocalTest_Factory base;
} FactoryImpl;

static LocalTest_Greeter make_greeter(PortableServer_Servant s, LocalTest_Loud like,
                                      CORBA_Environment* ev) {
  (void)s;
  return CORBA_Object_duplicate(like, ev);
}

static POA_LocalTest_Factory__epv factory_epv = {NULL, make_greeter};
static POA_LocalTest_Factory__vepv factory_vepv = {&base_epv, &factory_epv};

int main(void) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  LoudImpl impl;
  FactoryImpl fimpl;
  LocalTest_Loud loud, dup;
  LocalTest_Factory factory;
  LocalTest_Greeter g;
  LocalTest_Greeter_Msg* m;
  CORBA_char* s;
  CORBA_long n = 0;

  memset(&impl, 0, sizeof impl);
  impl.base.vepv = &loud_vepv;
  /* Not initialised yet. */
  CHECK(LocalTest_Loud__create(&impl, &ev) == CORBA_OBJECT_NIL);
  CHECK(ev._major == CORBA_SYSTEM_EXCEPTION);
  CORBA_exception_free(&ev);
  POA_LocalTest_Loud__init(&impl, &ev);
  OK(&ev);
  loud = LocalTest_Loud__create(&impl, &ev);
  CHECK(OK(&ev) && loud);

  s = LocalTest_Loud_greet(loud, "world", &ev);
  CHECK(OK(&ev) && s && strcmp(s, "hello, world") == 0);
  CORBA_free(s);
  s = LocalTest_Greeter_greet(loud, "nobody", &ev);
  CHECK(s == NULL && ev._major == CORBA_USER_EXCEPTION &&
        strcmp(CORBA_exception_id(&ev), ex_LocalTest_Greeter_Busy) == 0 &&
        strcmp(((LocalTest_Greeter_Busy*)CORBA_exception_value(&ev))->why, "no one there") == 0);
  CORBA_exception_free(&ev);

  LocalTest_Loud__set_count(loud, 21, &ev);
  CHECK(OK(&ev) && LocalTest_Greeter__get_count(loud, &ev) == 21 && impl.count == 21);
  CHECK(LocalTest_Loud__get_target(loud, &ev) == CORBA_OBJECT_NIL && OK(&ev));
  m = LocalTest_Loud_make(loud, &n, &ev);
  CHECK(OK(&ev) && m && strcmp(m->text, "made") == 0 && n == 42);
  CORBA_free(m);
  LocalTest_Loud_shout(loud, &ev);
  CHECK(OK(&ev) && impl.shouts == 1);

  /* The reference operations. */
  CHECK(CORBA_Object_is_a(loud, "IDL:LocalTest/Loud:1.0", &ev));
  CHECK(CORBA_Object_is_a(loud, "IDL:LocalTest/Greeter:1.0", &ev));
  CHECK(CORBA_Object_is_a(loud, "IDL:omg.org/CORBA/LocalObject:1.0", &ev));
  CHECK(CORBA_Object_is_a(loud, "IDL:omg.org/CORBA/Object:1.0", &ev));
  CHECK(!CORBA_Object_is_a(loud, "IDL:LocalTest/Factory:1.0", &ev));
  s = CORBA_Object__repository_id(loud, &ev);
  CHECK(strcmp(s, "IDL:LocalTest/Loud:1.0") == 0);
  CORBA_free(s);
  dup = CORBA_Object_duplicate(loud, &ev);
  CHECK(dup && CORBA_Object_is_equivalent(dup, loud, &ev) && !CORBA_Object_non_existent(dup, &ev));
  CORBA_Object_release(dup, &ev);
  {
    CORBA_TypeCode tc = TC_LocalTest_Greeter;
    CHECK(CORBA_TypeCode_kind(tc, &ev) == CORBA_tk_local_interface);
  }

  /* A local object in and out of a local operation. */
  memset(&fimpl, 0, sizeof fimpl);
  fimpl.base.vepv = &factory_vepv;
  POA_LocalTest_Factory__init(&fimpl, &ev);
  factory = LocalTest_Factory__create(&fimpl, &ev);
  g = LocalTest_Factory_make_greeter(factory, loud, &ev);
  CHECK(OK(&ev) && g && CORBA_Object_is_equivalent(g, loud, &ev));
  s = LocalTest_Greeter_greet(g, "again", &ev);
  CHECK(OK(&ev) && strcmp(s, "hello, again") == 0);
  CORBA_free(s);
  CORBA_Object_release(g, &ev);

  /* A local object cannot be sent: MARSHAL minor 4. */
  {
    CORBA_any a;
    memset(&a, 0, sizeof a);
    a._type = TC_LocalTest_Greeter;
    a._value = &loud;
    CHECK(!CORBA_any_equal(&a, &a));
  }

  CORBA_Object_release(factory, &ev);
  CORBA_Object_release(loud, &ev);
  POA_LocalTest_Factory__fini(&fimpl, &ev);
  POA_LocalTest_Loud__fini(&impl, &ev);
  if (failures) {
    fprintf(stderr, "%d failure(s)\n", failures);
    return 1;
  }
  printf("local: ok\n");
  return 0;
}
