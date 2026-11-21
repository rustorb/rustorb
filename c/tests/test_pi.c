/* Portable Interceptors and IOP::Codec (<rustorb/pi.h>) against the Echo
 * servant in this process: an ORB initializer (arguments, ORB id, slots,
 * DuplicateName, resolve_initial_references), service contexts both ways,
 * the request scope slot, the request infos, an IOR component,
 * ForwardRequest and a system exception from interceptors, PICurrent, the
 * Codec and destroy.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <rustorb/pi.h>

#include "echo_impl.h"

static int failures;

#define CHECK(cond)                                                    \
  do {                                                                 \
    if (!(cond)) {                                                     \
      failures++;                                                      \
      fprintf(stderr, "%s:%d: CHECK(%s) failed\n", __FILE__, __LINE__, #cond); \
    }                                                                  \
  } while (0)

#define CONTEXT 0x54455354u
#define TAG 0x54455354u

static IOP_Codec codec;
static PortableInterceptor_SlotId slot;
static int pre_init_ran, post_init_ran, destroyed;
static CORBA_Object forward_to;
static int forwarded;

/* What the points saw, for the checks after each call. */
static struct {
  char op[64];
  CORBA_unsigned_long client_id, server_id;
  CORBA_long context_value;
  CORBA_long reply_value;
  CORBA_long slot_value;
  CORBA_long client_slot;
  int component;
  int receive_reply, receive_exception, receive_other;
  int send_reply, send_exception, send_other;
  PortableInterceptor_ReplyStatus client_status, server_status;
  char exception_id[128];
  char sending_id[128];
  int target_is_a;
  char most_derived[128];
  CORBA_unsigned_long oid_len;
  CORBA_unsigned_long adapter_names;
  int args_major;
  CORBA_boolean response_expected;
} seen;

static CORBA_sequence_octet* encode_long(CORBA_long v, CORBA_Environment* ev) {
  CORBA_any a;
  memset(&a, 0, sizeof a);
  a._type = TC_long;
  a._value = &v;
  return IOP_Codec_encode_value(codec, &a, ev);
}

static CORBA_long decode_long(const CORBA_sequence_octet* data) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = IOP_Codec_decode_value(codec, data, TC_long, &ev);
  CORBA_long v = -1;
  if (ev._major == CORBA_NO_EXCEPTION && a && a->_value) v = *(CORBA_long*)a->_value;
  CORBA_exception_free(&ev);
  CORBA_free(a);
  return v;
}

/* --- client ---------------------------------------------------------------------------- */

static CORBA_char* name_test(PortableServer_Servant s, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return CORBA_string_dup("test");
}

static void destroy(PortableServer_Servant s, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  destroyed++;
}

static void send_request(PortableServer_Servant s, PortableInterceptor_ClientRequestInfo ri,
                         CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_char* op = PortableInterceptor_RequestInfo__get_operation(ri, ev);
  CORBA_any* a;
  IOP_ServiceContext c;
  CORBA_sequence_octet* data;
  (void)s;
  snprintf(seen.op, sizeof seen.op, "%s", op ? op : "");
  seen.client_id = PortableInterceptor_RequestInfo__get_request_id(ri, ev);
  seen.response_expected = PortableInterceptor_RequestInfo__get_response_expected(ri, ev);
  /* The request scope slot, from the PICurrent of the calling thread. */
  a = PortableInterceptor_RequestInfo_get_slot(ri, slot, &e);
  seen.client_slot = e._major == CORBA_NO_EXCEPTION && a && a->_value &&
                             CORBA_TypeCode_kind(a->_type, &e) == CORBA_tk_long
                         ? *(CORBA_long*)a->_value
                         : -1;
  CORBA_exception_free(&e);
  CORBA_free(a);
  /* Not at this point. */
  PortableInterceptor_RequestInfo__get_reply_status(ri, &e);
  CHECK(e._major == CORBA_SYSTEM_EXCEPTION && strcmp(e._id, ex_CORBA_BAD_INV_ORDER) == 0);
  CORBA_exception_free(&e);
  if (op && strcmp(op, "echo_short") == 0 && forward_to && !forwarded) {
    PortableInterceptor_ForwardRequest* f = PortableInterceptor_ForwardRequest__alloc();
    forwarded = 1;
    f->forward = CORBA_Object_duplicate(forward_to, &e);
    CORBA_exception_set(ev, CORBA_USER_EXCEPTION, ex_PortableInterceptor_ForwardRequest, f);
    CORBA_free(op);
    return;
  }
  CORBA_free(op);
  data = encode_long(7, ev);
  if (!data) return;
  memset(&c, 0, sizeof c);
  c.context_id = CONTEXT;
  c.context_data = *data;
  PortableInterceptor_ClientRequestInfo_add_request_service_context(ri, &c, CORBA_FALSE, ev);
  /* Twice without replace: BAD_INV_ORDER (minor 15). */
  PortableInterceptor_ClientRequestInfo_add_request_service_context(ri, &c, CORBA_FALSE, &e);
  CHECK(e._major == CORBA_SYSTEM_EXCEPTION && strcmp(e._id, ex_CORBA_BAD_INV_ORDER) == 0 &&
        ((CORBA_SystemException*)e._params)->minor == (0x4f4d0000u | 15));
  CORBA_exception_free(&e);
  CORBA_free(data);
}

static void receive_reply(PortableServer_Servant s, PortableInterceptor_ClientRequestInfo ri,
                          CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  IOP_ServiceContext* c;
  IOP_TaggedComponent* t;
  Dynamic_ParameterList* args;
  (void)s;
  seen.receive_reply++;
  seen.client_status = PortableInterceptor_RequestInfo__get_reply_status(ri, ev);
  c = PortableInterceptor_RequestInfo_get_reply_service_context(ri, CONTEXT, &e);
  seen.reply_value = e._major == CORBA_NO_EXCEPTION ? decode_long(&c->context_data) : -1;
  CORBA_exception_free(&e);
  CORBA_free(c);
  t = PortableInterceptor_ClientRequestInfo_get_effective_component(ri, TAG, &e);
  seen.component = e._major == CORBA_NO_EXCEPTION ? (int)decode_long(&t->component_data) : -1;
  CORBA_exception_free(&e);
  CORBA_free(t);
  /* Static stubs: NO_RESOURCES (minor 1). */
  args = PortableInterceptor_RequestInfo__get_arguments(ri, &e);
  seen.args_major = e._major;
  CHECK(args == NULL && e._major == CORBA_SYSTEM_EXCEPTION &&
        strcmp(e._id, ex_CORBA_NO_RESOURCES) == 0);
  CORBA_exception_free(&e);
}

static void receive_exception(PortableServer_Servant s, PortableInterceptor_ClientRequestInfo ri,
                              CORBA_Environment* ev) {
  CORBA_char* id;
  (void)s;
  seen.receive_exception++;
  seen.client_status = PortableInterceptor_RequestInfo__get_reply_status(ri, ev);
  id = PortableInterceptor_ClientRequestInfo__get_received_exception_id(ri, ev);
  snprintf(seen.exception_id, sizeof seen.exception_id, "%s", id ? id : "");
  CORBA_free(id);
}

static void receive_other(PortableServer_Servant s, PortableInterceptor_ClientRequestInfo ri,
                          CORBA_Environment* ev) {
  (void)s;
  seen.receive_other++;
  seen.client_status = PortableInterceptor_RequestInfo__get_reply_status(ri, ev);
}

/* --- server ------------------------------------------------------------------------------ */

static void receive_request_service_contexts(PortableServer_Servant s,
                                             PortableInterceptor_ServerRequestInfo ri,
                                             CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  IOP_ServiceContext* c = PortableInterceptor_RequestInfo_get_request_service_context(ri, CONTEXT, &e);
  CORBA_any a;
  CORBA_long v;
  (void)s;
  seen.server_id = PortableInterceptor_RequestInfo__get_request_id(ri, ev);
  if (e._major != CORBA_NO_EXCEPTION) {
    /* BAD_PARAM (minor 26). */
    CHECK(strcmp(e._id, ex_CORBA_BAD_PARAM) == 0 && ((CORBA_SystemException*)e._params)->minor == 26);
    CORBA_exception_free(&e);
    seen.context_value = -1;
    return;
  }
  v = decode_long(&c->context_data);
  CORBA_free(c);
  seen.context_value = v;
  v *= 10;
  memset(&a, 0, sizeof a);
  a._type = TC_long;
  a._value = &v;
  PortableInterceptor_ServerRequestInfo_set_slot(ri, slot, &a, ev);
  /* A slot not allocated. */
  PortableInterceptor_ServerRequestInfo_set_slot(ri, slot + 100, &a, &e);
  CHECK(e._major == CORBA_USER_EXCEPTION && strcmp(e._id, ex_PortableInterceptor_InvalidSlot) == 0);
  CORBA_exception_free(&e);
}

static void receive_request(PortableServer_Servant s, PortableInterceptor_ServerRequestInfo ri,
                            CORBA_Environment* ev) {
  CORBA_char* op = PortableInterceptor_RequestInfo__get_operation(ri, ev);
  CORBA_char* id;
  CORBA_sequence_octet* oid;
  CORBA_sequence_string* names;
  (void)s;
  seen.target_is_a =
      PortableInterceptor_ServerRequestInfo_target_is_a(ri, "IDL:rustorb.org/InteropTest/Echo:1.0", ev);
  id = PortableInterceptor_ServerRequestInfo__get_target_most_derived_interface(ri, ev);
  snprintf(seen.most_derived, sizeof seen.most_derived, "%s", id ? id : "");
  CORBA_free(id);
  oid = PortableInterceptor_ServerRequestInfo__get_object_id(ri, ev);
  seen.oid_len = oid ? oid->_length : 0;
  CORBA_free(oid);
  names = PortableInterceptor_ServerRequestInfo__get_adapter_name(ri, ev);
  seen.adapter_names = names ? names->_length : 99;
  CORBA_free(names);
  if (op && strcmp(op, "echo_ushort") == 0)
    CORBA_exception_set_system(ev, ex_CORBA_NO_PERMISSION, 3, CORBA_COMPLETED_NO);
  CORBA_free(op);
}

static void send_reply(PortableServer_Servant s, PortableInterceptor_ServerRequestInfo ri,
                       CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = PortableInterceptor_RequestInfo_get_slot(ri, slot, &e);
  CORBA_long v = a && a->_value ? *(CORBA_long*)a->_value : -1;
  IOP_ServiceContext c;
  CORBA_sequence_octet* data;
  (void)s;
  CORBA_exception_free(&e);
  CORBA_free(a);
  seen.send_reply++;
  seen.slot_value = v;
  seen.server_status = PortableInterceptor_RequestInfo__get_reply_status(ri, ev);
  data = encode_long(v + 1, ev);
  if (!data) return;
  memset(&c, 0, sizeof c);
  c.context_id = CONTEXT;
  c.context_data = *data;
  PortableInterceptor_ServerRequestInfo_add_reply_service_context(ri, &c, CORBA_FALSE, ev);
  CORBA_free(data);
}

static void send_exception(PortableServer_Servant s, PortableInterceptor_ServerRequestInfo ri,
                           CORBA_Environment* ev) {
  CORBA_any* a;
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  (void)s;
  seen.send_exception++;
  seen.server_status = PortableInterceptor_RequestInfo__get_reply_status(ri, ev);
  a = PortableInterceptor_ServerRequestInfo__get_sending_exception(ri, &e);
  if (e._major == CORBA_NO_EXCEPTION && a && a->_type) {
    CORBA_char* id = CORBA_TypeCode_id(a->_type, &e);
    snprintf(seen.sending_id, sizeof seen.sending_id, "%s", id ? id : "");
    CORBA_free(id);
  }
  CORBA_exception_free(&e);
  CORBA_free(a);
}

static void send_other(PortableServer_Servant s, PortableInterceptor_ServerRequestInfo ri,
                       CORBA_Environment* ev) {
  (void)s;
  (void)ri;
  (void)ev;
  seen.send_other++;
}

static void establish_components(PortableServer_Servant s, PortableInterceptor_IORInfo info,
                                 CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  IOP_TaggedComponent c;
  CORBA_sequence_octet* data = encode_long(99, ev);
  CORBA_Policy p;
  (void)s;
  if (!data) return;
  memset(&c, 0, sizeof c);
  c.tag = TAG;
  c.component_data = *data;
  PortableInterceptor_IORInfo_add_ior_component(info, &c, ev);
  /* Only IIOP profiles. */
  PortableInterceptor_IORInfo_add_ior_component_to_profile(info, &c, 5, &e);
  CHECK(e._major == CORBA_SYSTEM_EXCEPTION && strcmp(e._id, ex_CORBA_BAD_PARAM) == 0);
  CORBA_exception_free(&e);
  /* The lifespan policy of the RootPOA (TRANSIENT, 0). */
  p = PortableInterceptor_IORInfo_get_effective_policy(info, 17, &e);
  CHECK(e._major == CORBA_NO_EXCEPTION && p && CORBA_Policy__get_policy_type(p, &e) == 17);
  CORBA_exception_free(&e);
  CORBA_Object_release(p, &e);
  CORBA_free(data);
}

static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static POA_PortableInterceptor_Interceptor__epv interceptor_epv = {NULL, name_test, destroy};

static POA_PortableInterceptor_ClientRequestInterceptor__epv client_epv = {
    NULL, send_request, NULL, receive_reply, receive_exception, receive_other};
static POA_PortableInterceptor_ClientRequestInterceptor__vepv client_vepv = {
    &base_epv, &interceptor_epv, &client_epv};
static POA_PortableInterceptor_ClientRequestInterceptor client_impl = {NULL, &client_vepv};
/* A second one with the same name. */
static POA_PortableInterceptor_ClientRequestInterceptor other_impl = {NULL, &client_vepv};

static POA_PortableInterceptor_ServerRequestInterceptor__epv server_epv = {
    NULL, receive_request_service_contexts, receive_request, send_reply, send_exception, send_other};
static POA_PortableInterceptor_ServerRequestInterceptor__vepv server_vepv = {
    &base_epv, &interceptor_epv, &server_epv};
static POA_PortableInterceptor_ServerRequestInterceptor server_impl = {NULL, &server_vepv};

static POA_PortableInterceptor_IORInterceptor__epv ior_epv = {NULL, establish_components};
static POA_PortableInterceptor_IORInterceptor__vepv ior_vepv = {&base_epv, &interceptor_epv,
                                                                &ior_epv};
static POA_PortableInterceptor_IORInterceptor ior_impl = {NULL, &ior_vepv};

/* --- the initializer ------------------------------------------------------------------------ */

static void add(void (*fn)(CORBA_Object, CORBA_Object, CORBA_Environment*), CORBA_Object info,
                CORBA_Object i, CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  fn(info, i, ev);
  CORBA_Object_release(i, &e);
}

static void pre_init(PortableServer_Servant s, PortableInterceptor_ORBInitInfo info,
                     CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  IOP_Encoding enc = {IOP_ENCODING_CDR_ENCAPS, 1, 2};
  IOP_CodecFactory f;
  CORBA_sequence_string* args;
  CORBA_char* id;
  CORBA_Object o;
  CORBA_unsigned_long i;
  int found = 0;
  (void)s;
  pre_init_ran++;
  args = PortableInterceptor_ORBInitInfo__get_arguments(info, ev);
  for (i = 0; args && i < args->_length; i++)
    if (strcmp(args->_buffer[i], "test-pi") == 0) found = 1;
  CHECK(found);
  CORBA_free(args);
  id = PortableInterceptor_ORBInitInfo__get_orb_id(info, ev);
  CHECK(id && strcmp(id, "test-pi-orb") == 0);
  CORBA_free(id);
  o = PortableInterceptor_ORBInitInfo_resolve_initial_references(info, "PICurrent", &e);
  CHECK(o == CORBA_OBJECT_NIL && e._major == CORBA_SYSTEM_EXCEPTION &&
        strcmp(e._id, ex_CORBA_BAD_INV_ORDER) == 0);
  CORBA_exception_free(&e);

  f = PortableInterceptor_ORBInitInfo__get_codec_factory(info, ev);
  codec = IOP_CodecFactory_create_codec(f, &enc, ev);
  CORBA_Object_release(f, &e);
  slot = PortableInterceptor_ORBInitInfo_allocate_slot_id(info, ev);
  add(PortableInterceptor_ORBInitInfo_add_client_request_interceptor, info,
      PortableInterceptor_ClientRequestInterceptor__create(&client_impl, ev), ev);
  add(PortableInterceptor_ORBInitInfo_add_client_request_interceptor, info,
      PortableInterceptor_ClientRequestInterceptor__create(&other_impl, &e), &e);
  CHECK(e._major == CORBA_USER_EXCEPTION &&
        strcmp(e._id, ex_PortableInterceptor_ORBInitInfo_DuplicateName) == 0);
  CORBA_exception_free(&e);
  /* Not a client interceptor. */
  add(PortableInterceptor_ORBInitInfo_add_client_request_interceptor, info,
      PortableInterceptor_ServerRequestInterceptor__create(&server_impl, &e), &e);
  CHECK(e._major == CORBA_SYSTEM_EXCEPTION && strcmp(e._id, ex_CORBA_BAD_PARAM) == 0);
  CORBA_exception_free(&e);
  add(PortableInterceptor_ORBInitInfo_add_server_request_interceptor, info,
      PortableInterceptor_ServerRequestInterceptor__create(&server_impl, ev), ev);
  add(PortableInterceptor_ORBInitInfo_add_ior_interceptor, info,
      PortableInterceptor_IORInterceptor__create(&ior_impl, ev), ev);
}

static void post_init(PortableServer_Servant s, PortableInterceptor_ORBInitInfo info,
                      CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_Object o = PortableInterceptor_ORBInitInfo_resolve_initial_references(info, "PICurrent", ev);
  (void)s;
  post_init_ran++;
  CHECK(o != CORBA_OBJECT_NIL);
  CORBA_Object_release(o, &e);
}

static POA_PortableInterceptor_ORBInitializer__epv init_epv = {NULL, pre_init, post_init};
static POA_PortableInterceptor_ORBInitializer__vepv init_vepv = {&base_epv, &init_epv};
static POA_PortableInterceptor_ORBInitializer init_impl = {NULL, &init_vepv};

/* --- the Codec ---------------------------------------------------------------------------------- */

static void test_codec(CORBA_ORB orb) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  IOP_CodecFactory f = CORBA_ORB_resolve_initial_references(orb, "CodecFactory", &ev);
  IOP_Encoding enc = {IOP_ENCODING_CDR_ENCAPS, 1, 0};
  IOP_Encoding bad = {3, 1, 0};
  IOP_Codec c, dup;
  CORBA_any a, *back;
  CORBA_char* s = "codec";
  CORBA_sequence_octet* data;
  CORBA_sequence_octet junk = {2, 2, NULL, CORBA_FALSE};
  CORBA_octet bytes[2] = {0xff, 0xff};
  CHECK(ev._major == CORBA_NO_EXCEPTION && f);
  c = IOP_CodecFactory_create_codec(f, &bad, &ev);
  CHECK(!c && ev._major == CORBA_USER_EXCEPTION &&
        strcmp(ev._id, ex_IOP_CodecFactory_UnknownEncoding) == 0);
  CORBA_exception_free(&ev);
  c = IOP_CodecFactory_create_codec(f, &enc, &ev);
  CHECK(ev._major == CORBA_NO_EXCEPTION && c);
  dup = CORBA_Object_duplicate(c, &ev);
  CORBA_Object_release(c, &ev);
  memset(&a, 0, sizeof a);
  a._type = TC_string;
  a._value = &s;
  data = IOP_Codec_encode(dup, &a, &ev);
  CHECK(ev._major == CORBA_NO_EXCEPTION && data && data->_length > 0);
  back = IOP_Codec_decode(dup, data, &ev);
  CHECK(ev._major == CORBA_NO_EXCEPTION && back && back->_value &&
        strcmp(*(CORBA_char**)back->_value, "codec") == 0);
  CORBA_free(back);
  CORBA_free(data);
  junk._buffer = bytes;
  back = IOP_Codec_decode_value(dup, &junk, TC_string, &ev);
  CHECK(!back && ev._major == CORBA_USER_EXCEPTION &&
        strcmp(ev._id, ex_IOP_Codec_FormatMismatch) == 0);
  CORBA_exception_free(&ev);
  CORBA_Object_release(dup, &ev);
  CORBA_Object_release(f, &ev);
}

int main(int argc, char** argv) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  char* args[] = {argv[0], "-ORBid", "test-pi-orb", "test-pi", NULL};
  int n = 4;
  CORBA_ORB orb;
  CORBA_Object init;
  PortableServer_POA poa;
  PortableServer_POAManager manager;
  PortableServer_Servant servant;
  PortableServer_ObjectId* oid;
  InteropTest_Echo echo;
  PortableInterceptor_Current current;
  CORBA_any a;
  CORBA_long v = 5;
  CORBA_long r;
  (void)argc;

  POA_PortableInterceptor_ORBInitializer__init(&init_impl, &ev);
  POA_PortableInterceptor_ClientRequestInterceptor__init(&client_impl, &ev);
  POA_PortableInterceptor_ClientRequestInterceptor__init(&other_impl, &ev);
  POA_PortableInterceptor_ServerRequestInterceptor__init(&server_impl, &ev);
  POA_PortableInterceptor_IORInterceptor__init(&ior_impl, &ev);
  init = PortableInterceptor_ORBInitializer__create(&init_impl, &ev);
  /* Not an initializer. */
  PortableInterceptor_register_orb_initializer(CORBA_OBJECT_NIL, &ev);
  CHECK(ev._major == CORBA_SYSTEM_EXCEPTION);
  CORBA_exception_free(&ev);
  PortableInterceptor_register_orb_initializer(init, &ev);
  CHECK(ev._major == CORBA_NO_EXCEPTION);
  CORBA_Object_release(init, &ev);

  orb = CORBA_ORB_init(&n, args, "", &ev);
  if (ev._major != CORBA_NO_EXCEPTION) {
    fprintf(stderr, "ORB_init: %s\n", CORBA_exception_id(&ev));
    return 1;
  }
  CHECK(pre_init_ran == 1 && post_init_ran == 1);
  poa = CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
  manager = PortableServer_POA__get_the_POAManager(poa, &ev);
  PortableServer_POAManager_activate(manager, &ev);
  servant = echo_impl_new(orb, &ev);
  oid = PortableServer_POA_activate_object(poa, servant, &ev);
  echo = PortableServer_POA_id_to_reference(poa, oid, &ev);
  CHECK(ev._major == CORBA_NO_EXCEPTION);

  /* The request scope slot comes from the PICurrent of this thread. */
  current = CORBA_ORB_resolve_initial_references(orb, "PICurrent", &ev);
  CHECK(ev._major == CORBA_NO_EXCEPTION && current);
  memset(&a, 0, sizeof a);
  a._type = TC_long;
  a._value = &v;
  PortableInterceptor_Current_set_slot(current, slot, &a, &ev);
  CHECK(ev._major == CORBA_NO_EXCEPTION);
  {
    CORBA_any* b = PortableInterceptor_Current_get_slot(current, slot, &ev);
    CHECK(ev._major == CORBA_NO_EXCEPTION && b && *(CORBA_long*)b->_value == 5);
    CORBA_free(b);
  }
  PortableInterceptor_Current_get_slot(current, slot + 7, &ev);
  CHECK(ev._major == CORBA_USER_EXCEPTION && strcmp(ev._id, ex_PortableInterceptor_InvalidSlot) == 0);
  CORBA_exception_free(&ev);

  /* A reply, with contexts both ways, the slot and the component. */
  r = InteropTest_Echo_echo_long(echo, 3, &ev);
  CHECK(ev._major == CORBA_NO_EXCEPTION && r == 3);
  CHECK(strcmp(seen.op, "echo_long") == 0);
  CHECK(seen.response_expected);
  CHECK(seen.client_slot == 5);
  CHECK(seen.context_value == 7);
  CHECK(seen.slot_value == 70);
  CHECK(seen.reply_value == 71);
  CHECK(seen.component == 99);
  CHECK(seen.receive_reply == 1 && seen.send_reply == 1);
  CHECK(seen.client_status == PortableInterceptor_SUCCESSFUL &&
        seen.server_status == PortableInterceptor_SUCCESSFUL);
  CHECK(seen.target_is_a);
  CHECK(strcmp(seen.most_derived, "IDL:rustorb.org/InteropTest/Echo:1.0") == 0);
  CHECK(seen.oid_len == oid->_length);
  CHECK(seen.adapter_names == 0 || seen.adapter_names == 1);

  /* A user exception. */
  {
    CORBA_long rem = 0;
    InteropTest_Echo_divide(echo, 1, 0, &rem, &ev);
    CHECK(ev._major == CORBA_USER_EXCEPTION);
    CORBA_exception_free(&ev);
    CHECK(seen.receive_exception == 1 && seen.send_exception == 1);
    CHECK(seen.client_status == PortableInterceptor_USER_EXCEPTION);
    CHECK(strcmp(seen.exception_id, "IDL:rustorb.org/InteropTest/DivideByZero:1.0") == 0);
    /* A static skeleton's user exception is an any of UNKNOWN. */
    CHECK(strcmp(seen.sending_id, ex_CORBA_UNKNOWN) == 0);
  }

  /* A system exception from receive_request. */
  InteropTest_Echo_echo_ushort(echo, 1, &ev);
  CHECK(ev._major == CORBA_SYSTEM_EXCEPTION && strcmp(ev._id, ex_CORBA_NO_PERMISSION) == 0 &&
        ((CORBA_SystemException*)ev._params)->minor == 3);
  CORBA_exception_free(&ev);
  CHECK(seen.receive_exception == 2 && seen.client_status == PortableInterceptor_SYSTEM_EXCEPTION);
  CHECK(strcmp(seen.exception_id, ex_CORBA_NO_PERMISSION) == 0);

  /* ForwardRequest from send_request: the call again to the new target.
     The interceptor that raised it is not on the flow stack, so it gets
     no receive_other. */
  forward_to = echo;
  {
    CORBA_short s = InteropTest_Echo_echo_short(echo, 4, &ev);
    CHECK(ev._major == CORBA_NO_EXCEPTION && s == 4);
    CORBA_exception_free(&ev);
  }
  CHECK(forwarded && seen.receive_other == 0);

  test_codec(orb);

  CORBA_Object_release(current, &ev);
  CORBA_Object_release(echo, &ev);
  PortableServer_POA_deactivate_object(poa, oid, &ev);
  CORBA_free(oid);
  CORBA_Object_release(manager, &ev);
  CORBA_Object_release(poa, &ev);
  CORBA_ORB_destroy(orb, &ev);
  CORBA_Object_release(orb, &ev);
  /* The three interceptors. */
  CHECK(destroyed == 3);
  CORBA_Object_release(codec, &ev);
  echo_impl_free(servant);
  if (failures) {
    fprintf(stderr, "%d failure(s)\n", failures);
    return 1;
  }
  printf("pi: ok\n");
  return 0;
}
