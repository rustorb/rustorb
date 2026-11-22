/* The rustorb C server for the POA and thread model probes (PoaProbe.idl):
 *
 *   poa_probe_server --host HOST --port N --ior-file FILE [ORB options]
 *
 * The same objects and POAs as impls/omniorbpy/poa_probe_server.py, on the
 * C mapping: servant activators, locators and an adapter activator from
 * <rustorb/poa.h>. See run-poa-probe.sh.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#define _POSIX_C_SOURCE 200809L

#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "PoaProbe.h"

static const char* const target_id = "IDL:rustorb.org/PoaProbe/Target:1.0";
static struct timespec start_time;
static pthread_t main_thread;
static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static PortableServer_Current current;

static double now(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (double)(t.tv_sec - start_time.tv_sec) + (double)(t.tv_nsec - start_time.tv_nsec) / 1e9;
}

static void sleep_ms(long ms) {
  struct timespec t;
  if (ms <= 0) return;
  t.tv_sec = ms / 1000;
  t.tv_nsec = (ms % 1000) * 1000000L;
  nanosleep(&t, NULL);
}

static void die_on(CORBA_Environment* ev, const char* what) {
  if (ev->_major == CORBA_NO_EXCEPTION) return;
  fprintf(stderr, "%s: %s\n", what, CORBA_exception_id(ev));
  exit(1);
}

/* --- thread labels and events -------------------------------------------------------- */

static pthread_t threads[256];
static int thread_count;
static char* events[1024];
static int event_count;

static CORBA_char* thread_label(void) {
  pthread_t self = pthread_self();
  char buf[16];
  int i;
  if (pthread_equal(self, main_thread)) return CORBA_string_dup("main");
  pthread_mutex_lock(&lock);
  for (i = 0; i < thread_count; i++)
    if (pthread_equal(threads[i], self)) break;
  if (i == thread_count && thread_count < 256) threads[thread_count++] = self;
  pthread_mutex_unlock(&lock);
  snprintf(buf, sizeof buf, "T%d", i + 1);
  return CORBA_string_dup(buf);
}

static void event(const char* fmt, ...) {
  char buf[256];
  va_list ap;
  va_start(ap, fmt);
  vsnprintf(buf, sizeof buf, fmt, ap);
  va_end(ap);
  pthread_mutex_lock(&lock);
  if (event_count < 1024) events[event_count++] = CORBA_string_dup(buf);
  pthread_mutex_unlock(&lock);
}

static CORBA_char* oid_string(const PortableServer_ObjectId* oid) {
  return PortableServer_ObjectId_to_string(oid);
}

/* "RootPOA", or the names from the RootPOA down ("persistent_user/nested"). */
static CORBA_char* poa_path(PortableServer_POA poa, CORBA_Environment* ev) {
  char path[512] = "";
  char tmp[512];
  PortableServer_POA p = CORBA_Object_duplicate(poa, ev);
  while (p) {
    PortableServer_POA parent = PortableServer_POA__get_the_parent(p, ev);
    CORBA_char* name;
    if (!parent) {
      CORBA_exception_free(ev);
      CORBA_Object_release(p, ev);
      break;
    }
    name = PortableServer_POA__get_the_name(p, ev);
    snprintf(tmp, sizeof tmp, "%s%s%s", name, path[0] ? "/" : "", path);
    memcpy(path, tmp, sizeof path);
    CORBA_free(name);
    CORBA_Object_release(p, ev);
    p = parent;
  }
  return CORBA_string_dup(path[0] ? path : "RootPOA");
}

/* --- Target ------------------------------------------------------------------------------ */

typedef struct TargetImpl {
  POA_PoaProbe_Target base;
} TargetImpl;

static PoaProbe_CallInfo* work(PortableServer_Servant s, CORBA_long ms, CORBA_Environment* ev) {
  PoaProbe_CallInfo* r = PoaProbe_CallInfo__alloc();
  PortableServer_ObjectId* oid;
  PortableServer_POA poa;
  (void)s;
  r->start = now();
  r->thread = thread_label();
  oid = PortableServer_Current_get_object_id(current, ev);
  r->object_id = oid ? oid_string(oid) : CORBA_string_dup("");
  CORBA_free(oid);
  poa = PortableServer_Current_get_POA(current, ev);
  r->poa = poa ? poa_path(poa, ev) : CORBA_string_dup("");
  CORBA_Object_release(poa, ev);
  sleep_ms(ms);
  r->end = now();
  return r;
}

static PoaProbe_CallInfo* relay(PortableServer_Servant s, PoaProbe_Target other, CORBA_long ms,
                                CORBA_char** outer_thread, CORBA_Environment* ev) {
  (void)s;
  *outer_thread = thread_label();
  return PoaProbe_Target_work(other, ms, ev);
}

static void post(PortableServer_Servant s, CORBA_long seq, CORBA_long ms, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  sleep_ms(ms);
  event("post %d", (int)seq);
}

static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static POA_PoaProbe_Target__epv target_epv = {NULL, work, relay, post};
static POA_PoaProbe_Target__vepv target_vepv = {&base_epv, &target_epv};

static PortableServer_Servant target_new(void) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  TargetImpl* t = (TargetImpl*)calloc(1, sizeof *t);
  t->base.vepv = &target_vepv;
  POA_PoaProbe_Target__init(t, &ev);
  die_on(&ev, "Target");
  return t;
}

/* --- servant managers ------------------------------------------------------------------ */

static PortableServer_Servant the_servant;
static CORBA_Object the_forward;

/* "bad": OBJECT_NOT_EXIST; "fwd": ForwardRequest to the_forward. */
static PortableServer_Servant provide(const char* name, CORBA_Environment* ev) {
  if (strcmp(name, "bad") == 0) {
    CORBA_exception_set_system(ev, ex_CORBA_OBJECT_NOT_EXIST, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  if (strcmp(name, "fwd") == 0) {
    PortableServer_ForwardRequest* f = PortableServer_ForwardRequest__alloc();
    f->forward_reference = CORBA_Object_duplicate(the_forward, ev);
    CORBA_exception_set(ev, CORBA_USER_EXCEPTION, ex_PortableServer_ForwardRequest, f);
    return NULL;
  }
  return the_servant;
}

static PortableServer_Servant incarnate(PortableServer_Servant self,
                                        const PortableServer_ObjectId* oid,
                                        PortableServer_POA adapter, CORBA_Environment* ev) {
  CORBA_char* name = oid_string(oid);
  PortableServer_Servant s;
  (void)self;
  (void)adapter;
  event("incarnate %s", name);
  s = provide(name, ev);
  CORBA_free(name);
  return s;
}

static void etherealize(PortableServer_Servant self, const PortableServer_ObjectId* oid,
                        PortableServer_POA adapter, PortableServer_Servant serv,
                        CORBA_boolean cleanup_in_progress, CORBA_boolean remaining_activations,
                        CORBA_Environment* ev) {
  CORBA_char* name = oid_string(oid);
  (void)self;
  (void)adapter;
  (void)serv;
  (void)cleanup_in_progress;
  (void)remaining_activations;
  (void)ev;
  event("etherealize %s", name);
  CORBA_free(name);
}

static PortableServer_Servant preinvoke(PortableServer_Servant self,
                                        const PortableServer_ObjectId* oid,
                                        PortableServer_POA adapter, const CORBA_char* operation,
                                        PortableServer_ServantLocator_Cookie* cookie,
                                        CORBA_Environment* ev) {
  CORBA_char* name = oid_string(oid);
  PortableServer_Servant s;
  (void)self;
  (void)adapter;
  (void)cookie;
  event("preinvoke %s %s", name, operation);
  s = provide(name, ev);
  CORBA_free(name);
  return s;
}

static void postinvoke(PortableServer_Servant self, const PortableServer_ObjectId* oid,
                       PortableServer_POA adapter, const CORBA_char* operation,
                       PortableServer_ServantLocator_Cookie cookie,
                       PortableServer_Servant serv, CORBA_Environment* ev) {
  CORBA_char* name = oid_string(oid);
  (void)self;
  (void)adapter;
  (void)cookie;
  (void)serv;
  (void)ev;
  event("postinvoke %s %s", name, operation);
  CORBA_free(name);
}

static POA_PortableServer_ServantManager__epv manager_epv = {NULL};
static POA_PortableServer_ServantActivator__epv activator_epv = {NULL, incarnate, etherealize};
static POA_PortableServer_ServantActivator__vepv activator_vepv = {&base_epv, &manager_epv,
                                                                   &activator_epv};
static POA_PortableServer_ServantActivator activator_impl = {NULL, &activator_vepv};
static POA_PortableServer_ServantLocator__epv locator_epv = {NULL, preinvoke, postinvoke};
static POA_PortableServer_ServantLocator__vepv locator_vepv = {&base_epv, &manager_epv,
                                                               &locator_epv};
static POA_PortableServer_ServantLocator locator_impl = {NULL, &locator_vepv};

/* Policies: PERSISTENT and USER_ID, then up to four more. */
static CORBA_Policy policies[8];
static CORBA_PolicyList plist;

static CORBA_PolicyList* persistent_user(PortableServer_POA root, int n, ...) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  va_list ap;
  int i;
  for (i = 0; i < (int)plist._length; i++) CORBA_Object_release(policies[i], &ev);
  policies[0] = PortableServer_POA_create_lifespan_policy(root, PortableServer_PERSISTENT, &ev);
  policies[1] = PortableServer_POA_create_id_assignment_policy(root, PortableServer_USER_ID, &ev);
  va_start(ap, n);
  for (i = 0; i < n; i++) policies[2 + i] = va_arg(ap, CORBA_Policy);
  va_end(ap);
  plist._maximum = plist._length = (CORBA_unsigned_long)(2 + n);
  plist._buffer = policies;
  plist._release = CORBA_FALSE;
  return &plist;
}

static CORBA_PolicyList* lifespan_ids(PortableServer_POA root,
                                      PortableServer_LifespanPolicyValue l,
                                      PortableServer_IdAssignmentPolicyValue a) {
  CORBA_PolicyList* pl = persistent_user(root, 0);
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_Object_release(policies[0], &ev);
  CORBA_Object_release(policies[1], &ev);
  policies[0] = PortableServer_POA_create_lifespan_policy(root, l, &ev);
  policies[1] = PortableServer_POA_create_id_assignment_policy(root, a, &ev);
  return pl;
}

/* Creates POAs named lazy* on demand, with a default servant. */
static CORBA_boolean unknown_adapter(PortableServer_Servant self, PortableServer_POA parent,
                                     const CORBA_char* name, CORBA_Environment* ev) {
  PortableServer_POAManager m;
  PortableServer_POA poa;
  (void)self;
  event("unknown_adapter %s", name);
  if (strncmp(name, "lazy", 4) != 0) return CORBA_FALSE;
  m = PortableServer_POA__get_the_POAManager(parent, ev);
  pthread_mutex_lock(&lock);
  poa = PortableServer_POA_create_POA(
      parent, name, m,
      persistent_user(
          parent, 3,
          PortableServer_POA_create_servant_retention_policy(parent, PortableServer_NON_RETAIN, ev),
          PortableServer_POA_create_request_processing_policy(parent,
                                                              PortableServer_USE_DEFAULT_SERVANT, ev),
          PortableServer_POA_create_id_uniqueness_policy(parent, PortableServer_MULTIPLE_ID, ev)),
      ev);
  pthread_mutex_unlock(&lock);
  CORBA_Object_release(m, ev);
  if (ev->_major != CORBA_NO_EXCEPTION) return CORBA_FALSE;
  PortableServer_POA_set_servant(poa, the_servant, ev);
  CORBA_Object_release(poa, ev);
  return ev->_major == CORBA_NO_EXCEPTION;
}

static POA_PortableServer_AdapterActivator__epv aa_epv = {NULL, unknown_adapter};
static POA_PortableServer_AdapterActivator__vepv aa_vepv = {&base_epv, &aa_epv};
static POA_PortableServer_AdapterActivator aa_impl = {NULL, &aa_vepv};

/* --- Control ------------------------------------------------------------------------------ */

static CORBA_ORB the_orb;
static PortableServer_POA the_root;
static PortableServer_POAManager the_managed;
static struct {
  const char* poa;
  CORBA_Object ref;
} system_refs[3];

static void failed(CORBA_Environment* ev, const char* reason) {
  PoaProbe_Failed* f = PoaProbe_Failed__alloc();
  f->reason = CORBA_string_dup(reason);
  CORBA_exception_set(ev, CORBA_USER_EXCEPTION, ex_PoaProbe_Failed, f);
}

/* The POA of a path from the RootPOA, or NULL with Failed. */
static PortableServer_POA find(const char* path, CORBA_Environment* ev) {
  PortableServer_POA poa = CORBA_Object_duplicate(the_root, ev);
  const char* p = path;
  while (*p) {
    const char* e = strchr(p, '/');
    size_t len = e ? (size_t)(e - p) : strlen(p);
    char name[128];
    snprintf(name, sizeof name, "%.*s", (int)len, p);
    if (name[0] && strcmp(name, "RootPOA") != 0) {
      PortableServer_POA child = PortableServer_POA_find_POA(poa, name, CORBA_TRUE, ev);
      CORBA_Object_release(poa, ev);
      if (!child) {
        char reason[256];
        CORBA_exception_free(ev);
        snprintf(reason, sizeof reason, "no POA %s", path);
        failed(ev, reason);
        return NULL;
      }
      poa = child;
    }
    p += len;
    if (*p == '/') p++;
  }
  return poa;
}

static CORBA_char* reference(PortableServer_Servant s, const CORBA_char* poa_name,
                             const CORBA_char* oid, CORBA_Environment* ev) {
  int i;
  PortableServer_POA poa;
  PortableServer_ObjectId* id;
  CORBA_Object ref;
  CORBA_char* r;
  (void)s;
  for (i = 0; i < 3; i++)
    if (strcmp(system_refs[i].poa, poa_name) == 0)
      return CORBA_ORB_object_to_string(the_orb, system_refs[i].ref, ev);
  poa = find(poa_name, ev);
  if (!poa) return NULL;
  id = PortableServer_string_to_ObjectId(oid);
  ref = PortableServer_POA_create_reference_with_id(poa, id, target_id, ev);
  CORBA_free(id);
  CORBA_Object_release(poa, ev);
  if (ev->_major != CORBA_NO_EXCEPTION) return NULL;
  r = CORBA_ORB_object_to_string(the_orb, ref, ev);
  CORBA_Object_release(ref, ev);
  return r;
}

static void set_state(PortableServer_Servant s, const CORBA_char* state, CORBA_Environment* ev) {
  (void)s;
  if (strcmp(state, "active") == 0)
    PortableServer_POAManager_activate(the_managed, ev);
  else if (strcmp(state, "holding") == 0)
    PortableServer_POAManager_hold_requests(the_managed, CORBA_FALSE, ev);
  else if (strcmp(state, "discarding") == 0)
    PortableServer_POAManager_discard_requests(the_managed, CORBA_FALSE, ev);
  else if (strcmp(state, "inactive") == 0)
    PortableServer_POAManager_deactivate(the_managed, CORBA_FALSE, CORBA_FALSE, ev);
  else {
    char reason[160];
    snprintf(reason, sizeof reason, "unknown state %s", state);
    failed(ev, reason);
    return;
  }
  if (ev->_major == CORBA_USER_EXCEPTION &&
      strcmp(ev->_id, ex_PortableServer_POAManager_AdapterInactive) == 0) {
    CORBA_exception_free(ev);
    failed(ev, "AdapterInactive");
  }
}

static void deactivate_object(PortableServer_Servant s, const CORBA_char* poa_name,
                              const CORBA_char* oid, CORBA_Environment* ev) {
  PortableServer_POA poa = find(poa_name, ev);
  PortableServer_ObjectId* id;
  (void)s;
  if (!poa) return;
  id = PortableServer_string_to_ObjectId(oid);
  PortableServer_POA_deactivate_object(poa, id, ev);
  CORBA_free(id);
  CORBA_Object_release(poa, ev);
  if (ev->_major == CORBA_USER_EXCEPTION &&
      strcmp(ev->_id, ex_PortableServer_POA_ObjectNotActive) == 0) {
    CORBA_exception_free(ev);
    failed(ev, "ObjectNotActive");
  }
}

static void activate_object(PortableServer_Servant s, const CORBA_char* poa_name,
                            const CORBA_char* oid, CORBA_Environment* ev) {
  PortableServer_POA poa = find(poa_name, ev);
  PortableServer_ObjectId* id;
  CORBA_Environment ev2 = {CORBA_NO_EXCEPTION, NULL, NULL};
  (void)s;
  if (!poa) return;
  id = PortableServer_string_to_ObjectId(oid);
  PortableServer_POA_activate_object_with_id(poa, id, target_new(), ev);
  CORBA_free(id);
  CORBA_Object_release(poa, &ev2);
  if (ev->_major == CORBA_USER_EXCEPTION) {
    const char* why = strcmp(ev->_id, ex_PortableServer_POA_ObjectAlreadyActive) == 0
                          ? "ObjectAlreadyActive"
                      : strcmp(ev->_id, ex_PortableServer_POA_ServantAlreadyActive) == 0
                          ? "ServantAlreadyActive"
                          : NULL;
    if (why) {
      CORBA_exception_free(ev);
      failed(ev, why);
    }
  }
}

static void destroy_poa(PortableServer_Servant s, const CORBA_char* poa_name,
                        CORBA_Environment* ev) {
  PortableServer_POA poa = find(poa_name, ev);
  CORBA_Environment ev2 = {CORBA_NO_EXCEPTION, NULL, NULL};
  (void)s;
  if (!poa) return;
  PortableServer_POA_destroy(poa, CORBA_TRUE, CORBA_FALSE, ev);
  CORBA_Object_release(poa, &ev2);
}

static PoaProbe_StringSeq* events_op(PortableServer_Servant s, CORBA_Environment* ev) {
  PoaProbe_StringSeq* seq = PoaProbe_StringSeq__alloc();
  int i;
  (void)s;
  (void)ev;
  pthread_mutex_lock(&lock);
  seq->_buffer = CORBA_sequence_string_allocbuf((CORBA_unsigned_long)event_count);
  seq->_maximum = seq->_length = (CORBA_unsigned_long)event_count;
  seq->_release = CORBA_TRUE;
  for (i = 0; i < event_count; i++) seq->_buffer[i] = events[i];
  event_count = 0;
  pthread_mutex_unlock(&lock);
  return seq;
}

static CORBA_char* shutdown_inside(PortableServer_Servant s, CORBA_boolean wait,
                                   CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  (void)s;
  (void)ev;
  CORBA_ORB_shutdown(the_orb, wait, &e);
  if (e._major != CORBA_NO_EXCEPTION) {
    char buf[160];
    CORBA_SystemException* v = (CORBA_SystemException*)e._params;
    snprintf(buf, sizeof buf, "%s minor 0x%08x", e._id, (unsigned)(v ? v->minor : 0));
    CORBA_exception_free(&e);
    return CORBA_string_dup(buf);
  }
  return CORBA_string_dup("");
}

static void* shutdown_thread(void* arg) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  sleep_ms((long)(intptr_t)arg);
  CORBA_ORB_shutdown(the_orb, CORBA_FALSE, &ev);
  CORBA_exception_free(&ev);
  return NULL;
}

static void shutdown_later(PortableServer_Servant s, CORBA_long delay_ms, CORBA_Environment* ev) {
  pthread_t t;
  (void)s;
  (void)ev;
  if (pthread_create(&t, NULL, shutdown_thread, (void*)(intptr_t)delay_ms) == 0) pthread_detach(t);
}

static POA_PoaProbe_Control__epv control_epv = {
    NULL,           reference,  set_state,       deactivate_object, activate_object,
    destroy_poa,    events_op,  shutdown_inside, shutdown_later};
static POA_PoaProbe_Control__vepv control_vepv = {&base_epv, &control_epv};
static POA_PoaProbe_Control control_impl = {NULL, &control_vepv};

/* --- main ------------------------------------------------------------------------------- */

static PortableServer_POA create(PortableServer_POA parent, const char* name,
                                 PortableServer_POAManager m, CORBA_PolicyList* pl) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  PortableServer_POA poa = PortableServer_POA_create_POA(parent, name, m, pl, &ev);
  die_on(&ev, name);
  return poa;
}

static void activate(PortableServer_POA poa, const char* oid, PortableServer_Servant s) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  PortableServer_ObjectId* id = PortableServer_string_to_ObjectId(oid);
  PortableServer_POA_activate_object_with_id(poa, id, s, &ev);
  die_on(&ev, oid);
  CORBA_free(id);
}

static CORBA_Object system_ref(PortableServer_POA poa) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  PortableServer_ObjectId* id = PortableServer_POA_activate_object(poa, target_new(), &ev);
  CORBA_Object r;
  die_on(&ev, "activate_object");
  r = PortableServer_POA_id_to_reference(poa, id, &ev);
  CORBA_free(id);
  return r;
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  const char* host = "";
  const char* ior_file = NULL;
  int port = 0, i, n = 1;
  char** args = (char**)calloc((size_t)argc + 3, sizeof *args);
  char endpoint[256];
  PortableServer_POAManager root_manager;
  PortableServer_POA poa, persistent, control_poa;
  PortableServer_ObjectId* id;
  CORBA_Object ref;
  CORBA_char* ior;
  FILE* f;
  char tmp[512];

  clock_gettime(CLOCK_MONOTONIC, &start_time);
  main_thread = pthread_self();
  args[0] = argv[0];
  for (i = 1; i < argc; i++) {
    if (strcmp(argv[i], "--host") == 0 && i + 1 < argc)
      host = argv[++i];
    else if (strcmp(argv[i], "--port") == 0 && i + 1 < argc)
      port = atoi(argv[++i]);
    else if (strcmp(argv[i], "--ior-file") == 0 && i + 1 < argc)
      ior_file = argv[++i];
    else
      args[n++] = argv[i];
  }
  if (!ior_file) {
    fprintf(stderr, "usage: %s --ior-file FILE [--host H] [--port N]\n", argv[0]);
    return 2;
  }
  if (port) {
    snprintf(endpoint, sizeof endpoint, "giop:tcp:%s:%d", host, port);
    args[n++] = (char*)"-ORBendPoint";
    args[n++] = endpoint;
  }
  the_orb = CORBA_ORB_init(&n, args, "", &ev);
  die_on(&ev, "ORB_init");
  the_root = CORBA_ORB_resolve_initial_references(the_orb, "RootPOA", &ev);
  current = CORBA_ORB_resolve_initial_references(the_orb, "POACurrent", &ev);
  root_manager = PortableServer_POA__get_the_POAManager(the_root, &ev);
  PortableServer_POAManager_activate(root_manager, &ev);
  die_on(&ev, "RootPOA");

  the_servant = target_new();
  /* RootPOA: TRANSIENT, SYSTEM_ID, IMPLICIT_ACTIVATION. */
  system_refs[0].poa = "RootPOA";
  system_refs[0].ref = PortableServer_POA_servant_to_reference(the_root, the_servant, &ev);
  die_on(&ev, "servant_to_reference");

  persistent = create(the_root, "persistent_user", root_manager, persistent_user(the_root, 0));
  activate(persistent, "obj", the_servant);
  activate(persistent, "victim", target_new());
  poa = create(persistent, "nested", root_manager, persistent_user(the_root, 0));
  activate(poa, "obj", the_servant);
  CORBA_Object_release(poa, &ev);

  poa = create(the_root, "persistent_system", root_manager,
               lifespan_ids(the_root, PortableServer_PERSISTENT, PortableServer_SYSTEM_ID));
  system_refs[1].poa = "persistent_system";
  system_refs[1].ref = system_ref(poa);
  CORBA_Object_release(poa, &ev);

  poa = create(the_root, "transient_user", root_manager,
               lifespan_ids(the_root, PortableServer_TRANSIENT, PortableServer_USER_ID));
  activate(poa, "obj", the_servant);
  CORBA_Object_release(poa, &ev);

  poa = create(the_root, "transient_system", root_manager,
               lifespan_ids(the_root, PortableServer_TRANSIENT, PortableServer_SYSTEM_ID));
  system_refs[2].poa = "transient_system";
  system_refs[2].ref = system_ref(poa);
  CORBA_Object_release(poa, &ev);

  for (i = 0; i < 3; i++) {
    static const char* const names[] = {"single", "single2", "main"};
    poa = create(the_root, names[i], root_manager,
                 persistent_user(the_root, 1,
                                 PortableServer_POA_create_thread_policy(
                                     the_root,
                                     i < 2 ? PortableServer_SINGLE_THREAD_MODEL
                                           : PortableServer_MAIN_THREAD_MODEL,
                                     &ev)));
    activate(poa, "a", the_servant);
    activate(poa, "b", target_new());
    CORBA_Object_release(poa, &ev);
  }

  poa = create(the_root, "managed", CORBA_OBJECT_NIL, persistent_user(the_root, 0));
  the_managed = PortableServer_POA__get_the_POAManager(poa, &ev);
  activate(poa, "obj", the_servant);
  CORBA_Object_release(poa, &ev);

  poa = create(the_root, "doomed", root_manager, persistent_user(the_root, 0));
  activate(poa, "obj", the_servant);
  CORBA_Object_release(poa, &ev);

  id = PortableServer_string_to_ObjectId("obj");
  the_forward = PortableServer_POA_id_to_reference(persistent, id, &ev);
  CORBA_free(id);

  POA_PortableServer_ServantActivator__init(&activator_impl, &ev);
  poa = create(the_root, "activator", root_manager,
               persistent_user(the_root, 1,
                               PortableServer_POA_create_request_processing_policy(
                                   the_root, PortableServer_USE_SERVANT_MANAGER, &ev)));
  ref = PortableServer_ServantActivator__create(&activator_impl, &ev);
  PortableServer_POA_set_servant_manager(poa, ref, &ev);
  die_on(&ev, "set_servant_manager (activator)");
  CORBA_Object_release(ref, &ev);
  CORBA_Object_release(poa, &ev);

  POA_PortableServer_ServantLocator__init(&locator_impl, &ev);
  poa = create(the_root, "locator", root_manager,
               persistent_user(the_root, 2,
                               PortableServer_POA_create_request_processing_policy(
                                   the_root, PortableServer_USE_SERVANT_MANAGER, &ev),
                               PortableServer_POA_create_servant_retention_policy(
                                   the_root, PortableServer_NON_RETAIN, &ev)));
  ref = PortableServer_ServantLocator__create(&locator_impl, &ev);
  PortableServer_POA_set_servant_manager(poa, ref, &ev);
  die_on(&ev, "set_servant_manager (locator)");
  CORBA_Object_release(ref, &ev);
  CORBA_Object_release(poa, &ev);

  poa = create(the_root, "default", root_manager,
               persistent_user(the_root, 3,
                               PortableServer_POA_create_request_processing_policy(
                                   the_root, PortableServer_USE_DEFAULT_SERVANT, &ev),
                               PortableServer_POA_create_servant_retention_policy(
                                   the_root, PortableServer_NON_RETAIN, &ev),
                               PortableServer_POA_create_id_uniqueness_policy(
                                   the_root, PortableServer_MULTIPLE_ID, &ev)));
  PortableServer_POA_set_servant(poa, the_servant, &ev);
  die_on(&ev, "set_servant");
  CORBA_Object_release(poa, &ev);

  POA_PortableServer_AdapterActivator__init(&aa_impl, &ev);
  ref = PortableServer_AdapterActivator__create(&aa_impl, &ev);
  PortableServer_POA__set_the_activator(the_root, ref, &ev);
  die_on(&ev, "the_activator");
  CORBA_Object_release(ref, &ev);

  /* The Control object has its own POAManager, so that it keeps working
     whatever the probe does to the others. */
  control_poa = create(the_root, "control", CORBA_OBJECT_NIL, persistent_user(the_root, 0));
  POA_PoaProbe_Control__init(&control_impl, &ev);
  activate(control_poa, "Control", &control_impl);
  {
    PortableServer_POAManager m = PortableServer_POA__get_the_POAManager(control_poa, &ev);
    PortableServer_POAManager_activate(m, &ev);
    CORBA_Object_release(m, &ev);
  }
  PortableServer_POAManager_activate(the_managed, &ev);
  die_on(&ev, "activate");

  id = PortableServer_string_to_ObjectId("Control");
  ref = PortableServer_POA_id_to_reference(control_poa, id, &ev);
  CORBA_free(id);
  ior = CORBA_ORB_object_to_string(the_orb, ref, &ev);
  die_on(&ev, "object_to_string");
  snprintf(tmp, sizeof tmp, "%s.tmp", ior_file);
  f = fopen(tmp, "w");
  if (!f) return 1;
  fprintf(f, "%s\n", ior);
  fclose(f);
  rename(tmp, ior_file);
  CORBA_free(ior);
  printf("server ready\n");
  fflush(stdout);

  CORBA_ORB_run(the_orb, &ev);
  die_on(&ev, "run");
  printf("server stopped\n");
  fflush(stdout);
  CORBA_ORB_destroy(the_orb, &ev);
  return 0;
}
