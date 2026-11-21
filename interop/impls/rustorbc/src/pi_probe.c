/* The rustorb C side of run-pi-probe.sh: the probe of
 * impls/rustorb/src/pi_probe.rs on <rustorb/pi.h>, printing the same lines.
 *
 *   pi_probe server [ORB options]       prints the IOR of an Echo object,
 *                                       then serves
 *   pi_probe client IOR [ORB options]   calls echo
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <rustorb/pi.h>

#include "PiProbe.h"

#define CONTEXT 0x50495052u
#define TAG 0x50495052u
#define NAME "rustorbc"

static IOP_Codec codec;
static PortableInterceptor_SlotId slot;
static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static char* seen[8];
static int seen_count;

static void see(const char* prefix, const char* s) {
  size_t n = strlen(prefix) + strlen(s) + 1;
  char* line = (char*)malloc(n);
  if (!line) return;
  snprintf(line, n, "%s%s", prefix, s);
  pthread_mutex_lock(&lock);
  if (seen_count < 8)
    seen[seen_count++] = line;
  else
    free(line);
  pthread_mutex_unlock(&lock);
}

/* The CDR of the string s (encode_value), to CORBA_free. */
static CORBA_sequence_octet* encode(const char* s, CORBA_Environment* ev) {
  CORBA_any a;
  CORBA_char* v = (CORBA_char*)s;
  memset(&a, 0, sizeof a);
  a._type = TC_string;
  a._value = &v;
  return IOP_Codec_encode_value(codec, &a, ev);
}

/* The string in data, or "<id>" of what failed; to CORBA_free. */
static CORBA_char* decode(const CORBA_sequence_octet* data) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = IOP_Codec_decode_value(codec, data, TC_string, &ev);
  CORBA_char* r;
  char buf[256];
  if (ev._major != CORBA_NO_EXCEPTION) {
    snprintf(buf, sizeof buf, "<%s>", CORBA_exception_id(&ev));
    CORBA_exception_free(&ev);
    return CORBA_string_dup(buf);
  }
  r = CORBA_string_dup(a && a->_value ? *(CORBA_char**)a->_value : "<not a string>");
  CORBA_free(a);
  return r;
}

/* The decoded context, or the id of the exception in ev. */
static CORBA_char* context_text(IOP_ServiceContext* c, CORBA_Environment* ev) {
  CORBA_char* r;
  if (ev->_major != CORBA_NO_EXCEPTION) {
    r = CORBA_string_dup(CORBA_exception_id(ev));
    CORBA_exception_free(ev);
    return r;
  }
  r = decode(&c->context_data);
  CORBA_free(c);
  return r;
}

static void add_context(void (*add)(CORBA_Object, const IOP_ServiceContext*, CORBA_boolean,
                                    CORBA_Environment*),
                        CORBA_Object ri, const char* text, CORBA_Environment* ev) {
  IOP_ServiceContext c;
  CORBA_sequence_octet* data = encode(text, ev);
  if (!data) return;
  memset(&c, 0, sizeof c);
  c.context_id = CONTEXT;
  c.context_data = *data;
  add(ri, &c, CORBA_FALSE, ev);
  CORBA_free(data);
}

/* --- the interceptors ---------------------------------------------------------------- */

static CORBA_char* get_name(PortableServer_Servant s, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return CORBA_string_dup("pi-probe");
}

static void send_request(PortableServer_Servant s, PortableInterceptor_ClientRequestInfo ri,
                         CORBA_Environment* ev) {
  (void)s;
  add_context(PortableInterceptor_ClientRequestInfo_add_request_service_context, ri,
              "from " NAME, ev);
}

static void receive_reply(PortableServer_Servant s, PortableInterceptor_ClientRequestInfo ri,
                          CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_char* op = PortableInterceptor_RequestInfo__get_operation(ri, ev);
  IOP_ServiceContext* c;
  IOP_TaggedComponent* t;
  CORBA_char* text;
  (void)s;
  if (!op || strcmp(op, "echo") != 0) {
    CORBA_free(op);
    return;
  }
  CORBA_free(op);
  c = PortableInterceptor_RequestInfo_get_reply_service_context(ri, CONTEXT, &e);
  text = context_text(c, &e);
  see("reply context: ", text);
  CORBA_free(text);
  t = PortableInterceptor_ClientRequestInfo_get_effective_component(ri, TAG, &e);
  if (e._major != CORBA_NO_EXCEPTION) {
    see("component: ", CORBA_exception_id(&e));
    CORBA_exception_free(&e);
  } else {
    text = decode(&t->component_data);
    see("component: ", text);
    CORBA_free(text);
    CORBA_free(t);
  }
}

static void receive_request_service_contexts(PortableServer_Servant s,
                                             PortableInterceptor_ServerRequestInfo ri,
                                             CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  IOP_ServiceContext* c = PortableInterceptor_RequestInfo_get_request_service_context(ri, CONTEXT, &e);
  CORBA_char* got = context_text(c, &e);
  CORBA_any a;
  (void)s;
  /* For clients that cannot read the reply context (run-pi-probe.sh). */
  printf("request context: %s\n", got);
  fflush(stdout);
  memset(&a, 0, sizeof a);
  a._type = TC_string;
  a._value = &got;
  PortableInterceptor_ServerRequestInfo_set_slot(ri, slot, &a, ev);
  CORBA_free(got);
}

static void send_reply(PortableServer_Servant s, PortableInterceptor_ServerRequestInfo ri,
                       CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = PortableInterceptor_RequestInfo_get_slot(ri, slot, &e);
  char buf[512];
  (void)s;
  if (e._major != CORBA_NO_EXCEPTION) {
    snprintf(buf, sizeof buf, NAME " got: <%s>", CORBA_exception_id(&e));
    CORBA_exception_free(&e);
  } else if (!a || !a->_value || CORBA_TypeCode_kind(a->_type, &e) != CORBA_tk_string) {
    snprintf(buf, sizeof buf, NAME " got: <not a string>");
  } else {
    snprintf(buf, sizeof buf, NAME " got: %s", *(CORBA_char**)a->_value);
  }
  CORBA_free(a);
  add_context(PortableInterceptor_ServerRequestInfo_add_reply_service_context, ri, buf, ev);
}

static void establish_components(PortableServer_Servant s, PortableInterceptor_IORInfo info,
                                 CORBA_Environment* ev) {
  IOP_TaggedComponent c;
  CORBA_sequence_octet* data = encode(NAME " component", ev);
  (void)s;
  if (!data) return;
  memset(&c, 0, sizeof c);
  c.tag = TAG;
  c.component_data = *data;
  PortableInterceptor_IORInfo_add_ior_component(info, &c, ev);
  CORBA_free(data);
}

static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static POA_PortableInterceptor_Interceptor__epv interceptor_epv = {NULL, get_name, NULL};

static POA_PortableInterceptor_ClientRequestInterceptor__epv client_epv = {
    NULL, send_request, NULL, receive_reply, NULL, NULL};
static POA_PortableInterceptor_ClientRequestInterceptor__vepv client_vepv = {
    &base_epv, &interceptor_epv, &client_epv};
static POA_PortableInterceptor_ClientRequestInterceptor client_impl = {NULL, &client_vepv};

static POA_PortableInterceptor_ServerRequestInterceptor__epv server_epv = {
    NULL, receive_request_service_contexts, NULL, send_reply, NULL, NULL};
static POA_PortableInterceptor_ServerRequestInterceptor__vepv server_vepv = {
    &base_epv, &interceptor_epv, &server_epv};
static POA_PortableInterceptor_ServerRequestInterceptor server_impl = {NULL, &server_vepv};

static POA_PortableInterceptor_IORInterceptor__epv ior_epv = {NULL, establish_components};
static POA_PortableInterceptor_IORInterceptor__vepv ior_vepv = {&base_epv, &interceptor_epv,
                                                                &ior_epv};
static POA_PortableInterceptor_IORInterceptor ior_impl = {NULL, &ior_vepv};

/* --- the initializer ------------------------------------------------------------------------- */

static void pre_init(PortableServer_Servant s, PortableInterceptor_ORBInitInfo info,
                     CORBA_Environment* ev) {
  IOP_Encoding enc = {IOP_ENCODING_CDR_ENCAPS, 1, 2};
  IOP_CodecFactory f = PortableInterceptor_ORBInitInfo__get_codec_factory(info, ev);
  CORBA_Object i;
  CORBA_Environment ev2 = {CORBA_NO_EXCEPTION, NULL, NULL};
  (void)s;
  codec = IOP_CodecFactory_create_codec(f, &enc, ev);
  CORBA_Object_release(f, &ev2);
  if (ev->_major != CORBA_NO_EXCEPTION) return;
  slot = PortableInterceptor_ORBInitInfo_allocate_slot_id(info, ev);
  i = PortableInterceptor_ClientRequestInterceptor__create(&client_impl, ev);
  PortableInterceptor_ORBInitInfo_add_client_request_interceptor(info, i, ev);
  CORBA_Object_release(i, &ev2);
  if (ev->_major != CORBA_NO_EXCEPTION) return;
  i = PortableInterceptor_ServerRequestInterceptor__create(&server_impl, ev);
  PortableInterceptor_ORBInitInfo_add_server_request_interceptor(info, i, ev);
  CORBA_Object_release(i, &ev2);
  if (ev->_major != CORBA_NO_EXCEPTION) return;
  i = PortableInterceptor_IORInterceptor__create(&ior_impl, ev);
  PortableInterceptor_ORBInitInfo_add_ior_interceptor(info, i, ev);
  CORBA_Object_release(i, &ev2);
}

static POA_PortableInterceptor_ORBInitializer__epv init_epv = {NULL, pre_init, NULL};
static POA_PortableInterceptor_ORBInitializer__vepv init_vepv = {&base_epv, &init_epv};
static POA_PortableInterceptor_ORBInitializer init_impl = {NULL, &init_vepv};

/* --- Echo ------------------------------------------------------------------------------------- */

static CORBA_char* echo(PortableServer_Servant s, const CORBA_char* v, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return CORBA_string_dup(v);
}

static POA_PiProbe_Echo__epv echo_epv = {NULL, echo};
static POA_PiProbe_Echo__vepv echo_vepv = {&base_epv, &echo_epv};
static POA_PiProbe_Echo echo_servant = {NULL, &echo_vepv};

static void die_on(CORBA_Environment* ev, const char* what) {
  if (ev->_major == CORBA_NO_EXCEPTION) return;
  fprintf(stderr, "%s: %s\n", what, CORBA_exception_id(ev));
  exit(1);
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_ORB orb;
  int n, i;
  CORBA_Object init;
  char* args[64];
  const char* mode = argc > 1 ? argv[1] : "";
  int client = strcmp(mode, "client") == 0 && argc > 2;
  if ((!client && strcmp(mode, "server") != 0) || argc > 60) {
    fprintf(stderr, "usage: pi_probe server|client IOR [ORB options]\n");
    return 2;
  }
  POA_PortableInterceptor_ORBInitializer__init(&init_impl, &ev);
  POA_PortableInterceptor_ClientRequestInterceptor__init(&client_impl, &ev);
  POA_PortableInterceptor_ServerRequestInterceptor__init(&server_impl, &ev);
  POA_PortableInterceptor_IORInterceptor__init(&ior_impl, &ev);
  die_on(&ev, "init");
  init = PortableInterceptor_ORBInitializer__create(&init_impl, &ev);
  PortableInterceptor_register_orb_initializer(init, &ev);
  die_on(&ev, "register_orb_initializer");
  CORBA_Object_release(init, &ev);

  n = 0;
  args[n++] = argv[0];
  for (i = client ? 3 : 2; i < argc; i++) args[n++] = argv[i];
  args[n] = NULL;
  orb = CORBA_ORB_init(&n, args, "", &ev);
  die_on(&ev, "ORB_init");
  if (!client) {
    PortableServer_POA root =
        (PortableServer_POA)CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
    PortableServer_POAManager mgr;
    CORBA_Object ref;
    CORBA_char* ior;
    die_on(&ev, "RootPOA");
    mgr = PortableServer_POA__get_the_POAManager(root, &ev);
    PortableServer_POAManager_activate(mgr, &ev);
    POA_PiProbe_Echo__init(&echo_servant, &ev);
    ref = PortableServer_POA_servant_to_reference(root, &echo_servant, &ev);
    die_on(&ev, "servant_to_reference");
    ior = CORBA_ORB_object_to_string(orb, ref, &ev);
    die_on(&ev, "object_to_string");
    printf("%s\n", ior);
    fflush(stdout);
    CORBA_free(ior);
    CORBA_ORB_run(orb, &ev);
    return 0;
  } else {
    CORBA_Object obj = CORBA_ORB_string_to_object(orb, argv[2], &ev);
    CORBA_char* r = NULL;
    char buf[256];
    if (ev._major == CORBA_NO_EXCEPTION) r = PiProbe_Echo_echo(obj, "hello", &ev);
    if (ev._major != CORBA_NO_EXCEPTION) {
      snprintf(buf, sizeof buf, "%s", CORBA_exception_id(&ev));
      CORBA_exception_free(&ev);
    } else {
      snprintf(buf, sizeof buf, "%s", r);
    }
    CORBA_free(r);
    pthread_mutex_lock(&lock);
    for (i = 0; i < seen_count; i++) {
      printf("%s\n", seen[i]);
      free(seen[i]);
    }
    seen_count = 0;
    pthread_mutex_unlock(&lock);
    printf("result: %s\n", buf);
    CORBA_Object_release(obj, &ev);
    CORBA_Object_release(codec, &ev);
    CORBA_ORB_destroy(orb, &ev);
    CORBA_Object_release(orb, &ev);
    return 0;
  }
}
