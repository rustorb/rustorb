/* See ami_cases.h.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#define _POSIX_C_SOURCE 200809L

#include "ami_cases.h"

#include <errno.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define WAIT_S 10

/* --- the lines the handler puts ---------------------------------------------------- */

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t cond = PTHREAD_COND_INITIALIZER;
static char* queue[16];
static int queued;

static void put(const char* fmt, ...) {
  char buf[256];
  va_list ap;
  va_start(ap, fmt);
  vsnprintf(buf, sizeof buf, fmt, ap);
  va_end(ap);
  pthread_mutex_lock(&lock);
  if (queued < 16) queue[queued++] = CORBA_string_dup(buf);
  pthread_cond_signal(&cond);
  pthread_mutex_unlock(&lock);
}

/* The next line, or NULL after WAIT_S seconds. */
static char* take(void) {
  struct timespec until;
  char* line = NULL;
  int r = 0;
  clock_gettime(CLOCK_REALTIME, &until);
  until.tv_sec += WAIT_S;
  pthread_mutex_lock(&lock);
  while (queued == 0 && r != ETIMEDOUT) r = pthread_cond_timedwait(&cond, &lock, &until);
  if (queued > 0) {
    line = queue[0];
    memmove(queue, queue + 1, (size_t)(queued - 1) * sizeof queue[0]);
    queued--;
  }
  pthread_mutex_unlock(&lock);
  return line;
}

/* What raise_exception sets, as a probe line. */
static void describe(const char* op, const Messaging_ExceptionHolder* h) {
  static const char* const completed[] = {"COMPLETED_YES", "COMPLETED_NO", "COMPLETED_MAYBE"};
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  if (!h) {
    put("%s: nil holder", op);
    return;
  }
  Messaging_ExceptionHolder_raise_exception(h, &ev);
  if (ev._major == CORBA_NO_EXCEPTION) {
    put("%s: nothing raised", op);
  } else if (ev._major == CORBA_SYSTEM_EXCEPTION) {
    const CORBA_SystemException* s = (const CORBA_SystemException*)CORBA_exception_value(&ev);
    const char* id = CORBA_exception_id(&ev);
    const char* name = strrchr(id, '/');
    char buf[64];
    size_t n;
    name = name ? name + 1 : id;
    n = strcspn(name, ":");
    if (n >= sizeof buf) n = sizeof buf - 1;
    memcpy(buf, name, n);
    buf[n] = 0;
    put("%s: %s %lu %s", op, buf, s ? (unsigned long)s->minor : 0UL,
        s && s->completed <= CORBA_COMPLETED_MAYBE ? completed[s->completed] : "?");
  } else if (strcmp(CORBA_exception_id(&ev), ex_InteropTest_Oops) == 0) {
    const InteropTest_Oops* o = (const InteropTest_Oops*)CORBA_exception_value(&ev);
    put("%s: Oops %ld %s", op, (long)o->code, o->reason);
  } else if (strcmp(CORBA_exception_id(&ev), ex_InteropTest_DivideByZero) == 0) {
    put("%s: DivideByZero", op);
  } else {
    put("%s: unexpected %s", op, CORBA_exception_id(&ev));
  }
  CORBA_exception_free(&ev);
}

/* --- the handler ----------------------------------------------------------------------- */

static void h_echo_long(PortableServer_Servant s, CORBA_long v, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  put("echo_long: %ld", (long)v);
}

static void h_echo_string(PortableServer_Servant s, const CORBA_char* v, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  put("echo_string: %s", v);
}

static void h_swap(PortableServer_Servant s, CORBA_long r, CORBA_long a, CORBA_long b,
                   CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  put("swap: %ld %ld %ld", (long)r, (long)a, (long)b);
}

static void h_divide(PortableServer_Servant s, CORBA_long r, CORBA_long rem, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  put("divide: %ld %ld", (long)r, (long)rem);
}

#define EXCEP(op)                                                                          \
  static void h_##op##_excep(PortableServer_Servant s, const Messaging_ExceptionHolder* h, \
                             CORBA_Environment* ev) {                                      \
    (void)s;                                                                               \
    (void)ev;                                                                              \
    describe(#op "_excep", h);                                                             \
  }
EXCEP(divide)
EXCEP(fail)
EXCEP(fail_system)
EXCEP(trigger)

static void h_set_counter(PortableServer_Servant s, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  put("set_counter");
}

static void h_get_counter(PortableServer_Servant s, CORBA_long v, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  put("get_counter: %ld", (long)v);
}

static void h_get_name(PortableServer_Servant s, const CORBA_char* v, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  put("get_name: %s", v);
}

static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static POA_Messaging_ReplyHandler__epv reply_handler_epv = {NULL};
static POA_InteropTest_AMI_EchoHandler__epv handler_epv;
static POA_InteropTest_AMI_EchoHandler__vepv handler_vepv = {&base_epv, &reply_handler_epv,
                                                             &handler_epv};
static POA_InteropTest_AMI_EchoHandler handler_servant;

/* --- the slow callback ------------------------------------------------------------------- */

static CORBA_char* slow_notify(PortableServer_Servant s, const CORBA_char* message,
                               CORBA_Environment* ev) {
  struct timespec t = {2, 0};
  char buf[256];
  (void)s;
  (void)ev;
  nanosleep(&t, NULL);
  snprintf(buf, sizeof buf, "ack:%s", message);
  return CORBA_string_dup(buf);
}

static POA_InteropTest_Callback__epv callback_epv = {NULL, slow_notify};
static POA_InteropTest_Callback__vepv callback_vepv = {&base_epv, &callback_epv};
static POA_InteropTest_Callback callback_servant;

/* --- the cases ----------------------------------------------------------------------------- */

static int status;

static void show(void (*emit)(const char*), const char* what) {
  char* line = take();
  char buf[128];
  if (line) {
    emit(line);
    CORBA_free(line);
  } else {
    snprintf(buf, sizeof buf, "%s: no reply", what);
    emit(buf);
    status = 1;
  }
}

static void check(CORBA_Environment* ev, const char* what) {
  if (ev->_major == CORBA_NO_EXCEPTION) return;
  fprintf(stderr, "%s: %s\n", what, CORBA_exception_id(ev));
  CORBA_exception_free(ev);
  status = 1;
}

static void deactivate(PortableServer_POA poa, PortableServer_Servant servant) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  PortableServer_ObjectId* oid = PortableServer_POA_servant_to_id(poa, servant, &ev);
  if (ev._major == CORBA_NO_EXCEPTION) PortableServer_POA_deactivate_object(poa, oid, &ev);
  CORBA_exception_free(&ev);
  CORBA_free(oid);
  PortableServer_ServantBase__fini(servant, &ev);
  CORBA_exception_free(&ev);
}

int ami_cases(PortableServer_POA poa, InteropTest_Echo echo, void (*emit)(const char* line)) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  InteropTest_AMI_EchoHandler h;
  InteropTest_Callback cb;
  struct timespec tick = {0, 10000000L};
  char buf[64];
  int i;

  status = 0;
  handler_epv.echo_long = h_echo_long;
  handler_epv.echo_string = h_echo_string;
  handler_epv.swap = h_swap;
  handler_epv.divide = h_divide;
  handler_epv.divide_excep = h_divide_excep;
  handler_epv.fail_excep = h_fail_excep;
  handler_epv.fail_system_excep = h_fail_system_excep;
  handler_epv.set_counter = h_set_counter;
  handler_epv.get_counter = h_get_counter;
  handler_epv.get_name = h_get_name;
  handler_epv.trigger_excep = h_trigger_excep;
  handler_servant.vepv = &handler_vepv;
  POA_InteropTest_AMI_EchoHandler__init(&handler_servant, &ev);
  check(&ev, "handler init");
  h = PortableServer_POA_servant_to_reference(poa, &handler_servant, &ev);
  check(&ev, "handler reference");

  InteropTest_Echo_sendc_echo_long(echo, h, 42, &ev);
  check(&ev, "sendc_echo_long");
  show(emit, "echo_long");
  InteropTest_Echo_sendc_echo_string(echo, h, "hello AMI", &ev);
  check(&ev, "sendc_echo_string");
  show(emit, "echo_string");
  InteropTest_Echo_sendc_swap(echo, h, 1, 2, &ev);
  check(&ev, "sendc_swap");
  show(emit, "swap");
  InteropTest_Echo_sendc_divide(echo, h, 7, 2, &ev);
  check(&ev, "sendc_divide");
  show(emit, "divide");
  InteropTest_Echo_sendc_divide(echo, h, 1, 0, &ev);
  check(&ev, "sendc_divide");
  show(emit, "divide_excep");
  InteropTest_Echo_sendc_fail(echo, h, 7, "bad", &ev);
  check(&ev, "sendc_fail");
  show(emit, "fail_excep");
  InteropTest_Echo_sendc_fail_system(echo, h, &ev);
  check(&ev, "sendc_fail_system");
  show(emit, "fail_system_excep");
  InteropTest_Echo_sendc_set_counter(echo, h, 5, &ev);
  check(&ev, "sendc_set_counter");
  show(emit, "set_counter");
  InteropTest_Echo_sendc_get_counter(echo, h, &ev);
  check(&ev, "sendc_get_counter");
  show(emit, "get_counter");
  InteropTest_Echo_sendc_get_name(echo, h, &ev);
  check(&ev, "sendc_get_name");
  show(emit, "get_name");

  /* A nil handler: the request is sent, the reply dropped. */
  InteropTest_Echo_sendc_set_counter(echo, CORBA_OBJECT_NIL, 9, &ev);
  check(&ev, "sendc_set_counter");
  for (i = 0; i < WAIT_S * 100; i++) {
    CORBA_long c = InteropTest_Echo__get_counter(echo, &ev);
    check(&ev, "counter");
    if (c == 9) break;
    nanosleep(&tick, NULL);
  }
  snprintf(buf, sizeof buf, "nil handler: counter %ld", (long)InteropTest_Echo__get_counter(echo, &ev));
  check(&ev, "counter");
  emit(buf);

  /* The server calls the slow callback; the call times out first. */
  callback_servant.vepv = &callback_vepv;
  POA_InteropTest_Callback__init(&callback_servant, &ev);
  check(&ev, "callback init");
  cb = PortableServer_POA_servant_to_reference(poa, &callback_servant, &ev);
  check(&ev, "callback reference");
  InteropTest_Echo_register_callback(echo, cb, &ev);
  check(&ev, "register_callback");
  rustorb_Object_set_call_timeout(echo, 500000);
  InteropTest_Echo_sendc_trigger(echo, h, "late", &ev);
  rustorb_Object_set_call_timeout(echo, 0);
  check(&ev, "sendc_trigger");
  show(emit, "trigger_excep");
  /* Let the server's call of the callback finish before going away. */
  {
    struct timespec t = {2, 0};
    nanosleep(&t, NULL);
  }
  CORBA_Object_release(cb, &ev);
  CORBA_Object_release(h, &ev);
  deactivate(poa, &callback_servant);
  deactivate(poa, &handler_servant);
  return status;
}
