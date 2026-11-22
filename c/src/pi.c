/* Portable Interceptors and IOP::Codec (pi.h) on the rorb_pi_* and
   rorb_codec_* functions of the C ABI. */

#include <rustorb/pi.h>

#include <stddef.h>

#include "internal.h"

#define NO_EXC(e) memset(&(e), 0, sizeof(e))

/* --- shared handles ----------------------------------------------------------- */

rustorb_c_shared* rustorb_c_shared_new(void* handle, void (*release)(void* handle)) {
  rustorb_c_shared* s;
  if (!handle) return NULL;
  s = (rustorb_c_shared*)calloc(1, sizeof *s);
  if (!s) {
    release(handle);
    return NULL;
  }
  s->refs = 1;
  s->handle = handle;
  s->release = release;
  return s;
}

void rustorb_c_shared_ref(rustorb_c_shared* s) {
  rustorb_c_lock();
  s->refs++;
  rustorb_c_unlock();
}

void rustorb_c_shared_unref(rustorb_c_shared* s) {
  int last;
  if (!s) return;
  rustorb_c_lock();
  last = --s->refs == 0;
  rustorb_c_unlock();
  if (last) {
    s->release(s->handle);
    free(s);
  }
}

static void release_current(void* h) { rorb_pi_current_release((rorb_pi_current*)h); }
static void release_codec(void* h) { rorb_codec_release((rorb_codec*)h); }

static CORBA_Object wrap_shared(int kind, void* handle, void (*release)(void*)) {
  rustorb_c_shared* s = rustorb_c_shared_new(handle, release);
  CORBA_Object o;
  if (!s) return CORBA_OBJECT_NIL;
  o = rustorb_c_wrap(kind, s);
  if (!o) rustorb_c_shared_unref(s);
  return o;
}

CORBA_Object rustorb_c_pi_current(rorb_pi_current* current) {
  return wrap_shared(OBJ_PI_CURRENT, current, release_current);
}

CORBA_Policy rustorb_c_policy(CORBA_PolicyType type, uint64_t value) {
  CORBA_Policy p = rustorb_c_wrap(OBJ_POLICY, NULL);
  if (p) {
    p->policy_type = type;
    p->policy_value = value;
  }
  return p;
}

/* --- descriptors -------------------------------------------------------------------- */

static rustorb_c_cache octets_cache;
static const rustorb_c_type octets_desc = {
    CORBA_tk_sequence, sizeof(CORBA_sequence_octet), NULL, NULL, &CORBA_octet__desc, 0, 0,
    NULL, NULL, -1, 0, 0, &octets_cache};

static const rustorb_c_member profile_members[] = {
    {"tag", &CORBA_unsigned_long__desc, offsetof(IOP_TaggedProfile, tag), 0},
    {"profile_data", &octets_desc, offsetof(IOP_TaggedProfile, profile_data), 0},
};
static const rustorb_c_type profile_desc = {
    CORBA_tk_struct, sizeof(IOP_TaggedProfile), "IDL:omg.org/IOP/TaggedProfile:1.0",
    "TaggedProfile", NULL, 0, 2, profile_members, NULL, -1, 0, 0, NULL};

static const rustorb_c_member component_members[] = {
    {"tag", &CORBA_unsigned_long__desc, offsetof(IOP_TaggedComponent, tag), 0},
    {"component_data", &octets_desc, offsetof(IOP_TaggedComponent, component_data), 0},
};
static const rustorb_c_type component_desc = {
    CORBA_tk_struct, sizeof(IOP_TaggedComponent), "IDL:omg.org/IOP/TaggedComponent:1.0",
    "TaggedComponent", NULL, 0, 2, component_members, NULL, -1, 0, 0, NULL};

static const rustorb_c_member context_members[] = {
    {"context_id", &CORBA_unsigned_long__desc, offsetof(IOP_ServiceContext, context_id), 0},
    {"context_data", &octets_desc, offsetof(IOP_ServiceContext, context_data), 0},
};
static const rustorb_c_type context_desc = {
    CORBA_tk_struct, sizeof(IOP_ServiceContext), "IDL:omg.org/IOP/ServiceContext:1.0",
    "ServiceContext", NULL, 0, 2, context_members, NULL, -1, 0, 0, NULL};

static const rustorb_c_member parameter_members[] = {
    {"argument", &CORBA_any__desc, offsetof(Dynamic_Parameter, argument), 0},
    {"mode", &CORBA_unsigned_long__desc, offsetof(Dynamic_Parameter, mode), 0},
};
static const rustorb_c_type parameter_desc = {
    CORBA_tk_struct, sizeof(Dynamic_Parameter), "IDL:omg.org/Dynamic/Parameter:1.0", "Parameter",
    NULL, 0, 2, parameter_members, NULL, -1, 0, 0, NULL};

static const rustorb_c_member forward_members[] = {
    {"forward", &CORBA_Object__desc, offsetof(PortableInterceptor_ForwardRequest, forward), 0},
};
static rustorb_c_cache forward_cache;
static const rustorb_c_type forward_desc = {
    CORBA_tk_except, sizeof(PortableInterceptor_ForwardRequest),
    ex_PortableInterceptor_ForwardRequest, "ForwardRequest", NULL, 0, 1, forward_members, NULL,
    -1, 0, 0, &forward_cache};

IOP_TaggedProfile* IOP_TaggedProfile__alloc(void) {
  return (IOP_TaggedProfile*)rustorb_c_alloc(&profile_desc, 1);
}
IOP_TaggedComponent* IOP_TaggedComponent__alloc(void) {
  return (IOP_TaggedComponent*)rustorb_c_alloc(&component_desc, 1);
}
IOP_TaggedComponentSeq* IOP_TaggedComponentSeq__alloc(void) {
  return (IOP_TaggedComponentSeq*)rustorb_c_sequence_alloc();
}
IOP_TaggedComponent* CORBA_sequence_IOP_TaggedComponent_allocbuf(CORBA_unsigned_long len) {
  return (IOP_TaggedComponent*)rustorb_c_alloc(&component_desc, len);
}
IOP_ServiceContext* IOP_ServiceContext__alloc(void) {
  return (IOP_ServiceContext*)rustorb_c_alloc(&context_desc, 1);
}
PortableInterceptor_ForwardRequest* PortableInterceptor_ForwardRequest__alloc(void) {
  return (PortableInterceptor_ForwardRequest*)rustorb_c_alloc(&forward_desc, 1);
}

/* --- conversions ---------------------------------------------------------------------- */

/* bytes (freed) into seq, which holds nothing. */
static void set_octets(CORBA_sequence_octet* seq, rorb_bytes b) {
  memset(seq, 0, sizeof *seq);
  seq->_release = CORBA_TRUE;
  if (b.len) {
    seq->_buffer = CORBA_sequence_octet_allocbuf((CORBA_unsigned_long)b.len);
    if (seq->_buffer) {
      memcpy(seq->_buffer, b.data, b.len);
      seq->_length = seq->_maximum = (CORBA_unsigned_long)b.len;
    }
  }
  rorb_bytes_free(b);
}

/* A new CORBA_any with the value of raw (released), or NULL. */
static CORBA_any* any_of(rorb_any* raw) {
  CORBA_any* a;
  if (!raw) return NULL;
  a = CORBA_any__alloc();
  if (a && rustorb_c_any_from_raw(raw, a) != 0) {
    CORBA_free(a);
    a = NULL;
  }
  rorb_any_release(raw);
  return a;
}

/* The rorb_any of a, or NULL with ev set. */
static rorb_any* raw_of(const CORBA_any* a, CORBA_Environment* ev) {
  rustorb_c_err err = {NULL, 0};
  rorb_any* r;
  if (!a) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  r = rustorb_c_any_to_raw(a, &err);
  if (!r) rustorb_c_ev_system(ev, err.id ? err.id : ex_CORBA_BAD_PARAM, err.minor, CORBA_COMPLETED_NO);
  return r;
}

static CORBA_char* string_of(const char* s, size_t len) {
  return s ? rustorb_c_strndup(s, len) : CORBA_string_dup("");
}

typedef struct strings {
  CORBA_char** items;
  size_t len, cap;
} strings;

static void add_string(void* context, const char* s, size_t len) {
  strings* l = (strings*)context;
  if (l->len == l->cap) {
    size_t cap = l->cap ? l->cap * 2 : 8;
    CORBA_char** r = (CORBA_char**)realloc(l->items, cap * sizeof *r);
    if (!r) return;
    l->items = r;
    l->cap = cap;
  }
  l->items[l->len++] = rustorb_c_strndup(s, len);
}

static CORBA_sequence_string* string_seq(strings* l) {
  CORBA_sequence_string* seq = CORBA_sequence_string__alloc();
  size_t i;
  if (seq) {
    seq->_release = CORBA_TRUE;
    seq->_buffer = CORBA_sequence_string_allocbuf((CORBA_unsigned_long)l->len);
    if (seq->_buffer || !l->len) seq->_length = seq->_maximum = (CORBA_unsigned_long)l->len;
  }
  for (i = 0; i < l->len; i++) {
    if (seq && seq->_buffer)
      seq->_buffer[i] = l->items[i];
    else
      CORBA_free(l->items[i]);
  }
  free(l->items);
  return seq;
}

/* --- the implementations' classes ---------------------------------------------------------- */

static const char* const client_ids[] = {
    "IDL:omg.org/PortableInterceptor/ClientRequestInterceptor:1.0",
    "IDL:omg.org/PortableInterceptor/Interceptor:1.0"};
static const char* const server_ids[] = {
    "IDL:omg.org/PortableInterceptor/ServerRequestInterceptor:1.0",
    "IDL:omg.org/PortableInterceptor/Interceptor:1.0"};
static const char* const ior_ids[] = {"IDL:omg.org/PortableInterceptor/IORInterceptor:1.0",
                                      "IDL:omg.org/PortableInterceptor/Interceptor:1.0"};
static const char* const initializer_ids[] = {
    "IDL:omg.org/PortableInterceptor/ORBInitializer:1.0"};
static const rustorb_c_class client_class = {client_ids, 2, NULL, 0, NULL};
static const rustorb_c_class server_class = {server_ids, 2, NULL, 0, NULL};
static const rustorb_c_class ior_class = {ior_ids, 2, NULL, 0, NULL};
static const rustorb_c_class initializer_class = {initializer_ids, 1, NULL, 0, NULL};

#define LOCAL_CLASS(I, cls)                                                                 \
  void POA_PortableInterceptor_##I##__init(PortableServer_Servant servant,                 \
                                           CORBA_Environment* ev) {                         \
    rustorb_c_local_init(servant, &cls, ev);                                                \
  }                                                                                         \
  void POA_PortableInterceptor_##I##__fini(PortableServer_Servant servant,                 \
                                           CORBA_Environment* ev) {                         \
    rustorb_c_servant_fini(servant, ev);                                                    \
  }                                                                                         \
  PortableInterceptor_##I PortableInterceptor_##I##__create(PortableServer_Servant impl,  \
                                                            CORBA_Environment* ev) {        \
    if (rustorb_c_servant_class(impl) != &cls) {                                            \
      rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);                   \
      return CORBA_OBJECT_NIL;                                                              \
    }                                                                                       \
    return rustorb_c_local_object(impl, ev);                                                \
  }
LOCAL_CLASS(ClientRequestInterceptor, client_class)
LOCAL_CLASS(ServerRequestInterceptor, server_class)
LOCAL_CLASS(IORInterceptor, ior_class)
LOCAL_CLASS(ORBInitializer, initializer_class)

/* The three interceptors' vepvs start alike. */
static POA_PortableInterceptor_Interceptor__epv* interceptor_epv(PortableServer_Servant s) {
  POA_PortableInterceptor_ClientRequestInterceptor* i =
      (POA_PortableInterceptor_ClientRequestInterceptor*)s;
  return i->vepv ? i->vepv->PortableInterceptor_Interceptor_epv : NULL;
}
static POA_PortableInterceptor_ClientRequestInterceptor__epv* client_epv(PortableServer_Servant s) {
  POA_PortableInterceptor_ClientRequestInterceptor* i =
      (POA_PortableInterceptor_ClientRequestInterceptor*)s;
  return i->vepv ? i->vepv->PortableInterceptor_ClientRequestInterceptor_epv : NULL;
}
static POA_PortableInterceptor_ServerRequestInterceptor__epv* server_epv(PortableServer_Servant s) {
  POA_PortableInterceptor_ServerRequestInterceptor* i =
      (POA_PortableInterceptor_ServerRequestInterceptor*)s;
  return i->vepv ? i->vepv->PortableInterceptor_ServerRequestInterceptor_epv : NULL;
}
static POA_PortableInterceptor_IORInterceptor__epv* ior_epv(PortableServer_Servant s) {
  POA_PortableInterceptor_IORInterceptor* i = (POA_PortableInterceptor_IORInterceptor*)s;
  return i->vepv ? i->vepv->PortableInterceptor_IORInterceptor_epv : NULL;
}
static POA_PortableInterceptor_ORBInitializer__epv* initializer_epv(PortableServer_Servant s) {
  POA_PortableInterceptor_ORBInitializer* i = (POA_PortableInterceptor_ORBInitializer*)s;
  return i->vepv ? i->vepv->PortableInterceptor_ORBInitializer_epv : NULL;
}

/* The implementation of a local object of class cls, or NULL with
   BAD_PARAM. */
static PortableServer_Servant impl_of(CORBA_Object obj, const rustorb_c_class* cls,
                                      CORBA_Environment* ev) {
  if (!obj || obj->kind != OBJ_LOCAL || rustorb_c_servant_class(obj->handle) != cls) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  return obj->handle;
}

static CORBA_char* name_of(PortableServer_Servant s) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  POA_PortableInterceptor_Interceptor__epv* e = interceptor_epv(s);
  CORBA_char* n = e && e->_get_name ? e->_get_name(s, &ev) : NULL;
  if (ev._major != CORBA_NO_EXCEPTION) {
    CORBA_exception_free(&ev);
    CORBA_free(n);
    n = NULL;
  }
  return n ? n : CORBA_string_dup("");
}

CORBA_char* PortableInterceptor_Interceptor__get_name(PortableInterceptor_Interceptor obj,
                                                      CORBA_Environment* ev) {
  const rustorb_c_class* cls;
  rustorb_c_ev_clear(ev);
  cls = obj && obj->kind == OBJ_LOCAL ? rustorb_c_servant_class(obj->handle) : NULL;
  if (cls != &client_class && cls != &server_class && cls != &ior_class) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  return name_of(obj->handle);
}

/* --- the points ------------------------------------------------------------------------------ */

/* What a point set in ev, for the core. */
static rorb_status outcome(CORBA_Environment* ev, rorb_pi_error* error) {
  if (ev->_major == CORBA_NO_EXCEPTION) return RORB_OK;
  if (ev->_major == CORBA_USER_EXCEPTION && ev->_id &&
      strcmp(ev->_id, ex_PortableInterceptor_ForwardRequest) == 0) {
    PortableInterceptor_ForwardRequest* f = (PortableInterceptor_ForwardRequest*)ev->_params;
    if (f && f->forward && f->forward->kind == OBJ_REF)
      rorb_pi_error_forward(error, (rorb_object*)f->forward->handle);
    else
      rorb_pi_error_system(error, ex_CORBA_BAD_PARAM, strlen(ex_CORBA_BAD_PARAM), 0,
                           CORBA_COMPLETED_NO);
  } else if (ev->_major == CORBA_SYSTEM_EXCEPTION) {
    CORBA_SystemException* v = (CORBA_SystemException*)ev->_params;
    const char* id = ev->_id ? ev->_id : ex_CORBA_UNKNOWN;
    rorb_pi_error_system(error, id, strlen(id), v ? v->minor : 0,
                         v ? (uint32_t)v->completed : CORBA_COMPLETED_MAYBE);
  } else {
    /* Only ForwardRequest may be raised. */
    rorb_pi_error_system(error, ex_CORBA_UNKNOWN, strlen(ex_CORBA_UNKNOWN), 0,
                         CORBA_COMPLETED_MAYBE);
  }
  CORBA_exception_free(ev);
  return RORB_SYSTEM_EXCEPTION;
}

typedef void (*point_fn)(PortableServer_Servant, CORBA_Object, CORBA_Environment*);

/* Calls fn (may be NULL) with an info object of kind over handle. */
static rorb_status point(void* self, point_fn fn, int kind, const void* handle, int flags,
                         rorb_pi_error* error) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  struct CORBA_Object_struct info;
  if (!fn) return RORB_OK;
  memset(&info, 0, sizeof info);
  info.kind = kind;
  info.handle = (void*)handle;
  info.flags = flags;
  /* The info lives on this frame: never released, so immortal. */
  info.immortal = 1;
  fn(self, &info, &ev);
  return outcome(&ev, error);
}

#define CLIENT_POINT(p)                                                                        \
  static rorb_status client_##p(void* self, const rorb_client_request_info* ri,              \
                                rorb_pi_error* error) {                                       \
    POA_PortableInterceptor_ClientRequestInterceptor__epv* e = client_epv(self);              \
    return point(self, e ? (point_fn)e->p : NULL, OBJ_CLIENT_INFO, ri, 0, error);             \
  }
CLIENT_POINT(send_request)
CLIENT_POINT(receive_reply)
CLIENT_POINT(receive_exception)
CLIENT_POINT(receive_other)

#define SERVER_POINT(p)                                                                        \
  static rorb_status server_##p(void* self, const rorb_server_request_info* ri,              \
                                rorb_pi_error* error) {                                       \
    POA_PortableInterceptor_ServerRequestInterceptor__epv* e = server_epv(self);              \
    return point(self, e ? (point_fn)e->p : NULL, OBJ_SERVER_INFO, ri, 0, error);             \
  }
SERVER_POINT(receive_request_service_contexts)
SERVER_POINT(receive_request)
SERVER_POINT(send_reply)
SERVER_POINT(send_exception)
SERVER_POINT(send_other)

static rorb_status ior_establish_components(void* self, const rorb_ior_info* info,
                                            rorb_pi_error* error) {
  POA_PortableInterceptor_IORInterceptor__epv* e = ior_epv(self);
  return point(self, e ? (point_fn)e->establish_components : NULL, OBJ_IOR_INFO, info, 0, error);
}

static rorb_status init_pre(void* self, rorb_orb_init_info* info, rorb_pi_error* error) {
  POA_PortableInterceptor_ORBInitializer__epv* e = initializer_epv(self);
  return point(self, e ? (point_fn)e->pre_init : NULL, OBJ_INIT_INFO, info, 0, error);
}

static rorb_status init_post(void* self, rorb_orb_init_info* info, rorb_pi_error* error) {
  POA_PortableInterceptor_ORBInitializer__epv* e = initializer_epv(self);
  return point(self, e ? (point_fn)e->post_init : NULL, OBJ_INIT_INFO, info, 1, error);
}

static void destroy_cb(void* self) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  POA_PortableInterceptor_Interceptor__epv* e = interceptor_epv(self);
  if (e && e->destroy) e->destroy(self, &ev);
  CORBA_exception_free(&ev);
}

/* The implementations are the application's. */
static void no_ref(void* self) { (void)self; }

static const rorb_client_interceptor_vtable client_vtable = {
    client_send_request, client_receive_reply, client_receive_exception, client_receive_other,
    destroy_cb,          no_ref,               no_ref};
static const rorb_server_interceptor_vtable server_vtable = {
    server_receive_request_service_contexts,
    server_receive_request,
    server_send_reply,
    server_send_exception,
    server_send_other,
    destroy_cb,
    no_ref,
    no_ref};
static const rorb_ior_interceptor_vtable ior_vtable = {ior_establish_components, destroy_cb,
                                                       no_ref, no_ref};
static const rorb_orb_initializer_vtable initializer_vtable = {init_pre, init_post, no_ref,
                                                               no_ref};

void PortableInterceptor_register_orb_initializer(PortableInterceptor_ORBInitializer init,
                                                  CORBA_Environment* ev) {
  PortableServer_Servant s;
  rustorb_c_ev_clear(ev);
  s = impl_of(init, &initializer_class, ev);
  if (s) rorb_register_orb_initializer(s, &initializer_vtable);
}

/* --- RequestInfo --------------------------------------------------------------------------- */

#define CRI(ri) ((const rorb_client_request_info*)(ri)->handle)
#define SRI(ri) ((const rorb_server_request_info*)(ri)->handle)

/* 1 for a client info, 2 for a server info; 0 with BAD_PARAM. */
static int which(CORBA_Object ri, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (ri && ri->kind == OBJ_CLIENT_INFO) return 1;
  if (ri && ri->kind == OBJ_SERVER_INFO) return 2;
  rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
  return 0;
}

static int is_client(CORBA_Object ri, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (ri && ri->kind == OBJ_CLIENT_INFO) return 1;
  rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
  return 0;
}

static int is_server(CORBA_Object ri, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (ri && ri->kind == OBJ_SERVER_INFO) return 1;
  rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
  return 0;
}

CORBA_unsigned_long PortableInterceptor_RequestInfo__get_request_id(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev) {
  switch (which(ri, ev)) {
    case 1: return rorb_cri_request_id(CRI(ri));
    case 2: return rorb_sri_request_id(SRI(ri));
    default: return 0;
  }
}

CORBA_char* PortableInterceptor_RequestInfo__get_operation(PortableInterceptor_RequestInfo ri,
                                                           CORBA_Environment* ev) {
  size_t len = 0;
  const char* s;
  switch (which(ri, ev)) {
    case 1: s = rorb_cri_operation(CRI(ri), &len); break;
    case 2: s = rorb_sri_operation(SRI(ri), &len); break;
    default: return NULL;
  }
  return string_of(s, len);
}

typedef struct params {
  Dynamic_Parameter* items;
  size_t len, cap;
} params;

static void add_param(void* context, rorb_any* argument, uint32_t mode) {
  params* l = (params*)context;
  if (l->len == l->cap) {
    size_t cap = l->cap ? l->cap * 2 : 8;
    Dynamic_Parameter* r = (Dynamic_Parameter*)realloc(l->items, cap * sizeof *r);
    if (!r) {
      rorb_any_release(argument);
      return;
    }
    l->items = r;
    l->cap = cap;
  }
  memset(&l->items[l->len], 0, sizeof l->items[0]);
  if (argument) rustorb_c_any_from_raw(argument, &l->items[l->len].argument);
  rorb_any_release(argument);
  l->items[l->len++].mode = mode;
}

Dynamic_ParameterList* PortableInterceptor_RequestInfo__get_arguments(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev) {
  params l = {NULL, 0, 0};
  rorb_exception exc;
  rorb_status st;
  Dynamic_ParameterList* seq;
  NO_EXC(exc);
  switch (which(ri, ev)) {
    case 1: st = rorb_cri_arguments(CRI(ri), &l, add_param, &exc); break;
    case 2: st = rorb_sri_arguments(SRI(ri), &l, add_param, &exc); break;
    default: return NULL;
  }
  seq = rustorb_c_failed(ev, st, &exc) ? NULL : (Dynamic_ParameterList*)rustorb_c_sequence_alloc();
  if (seq) {
    seq->_release = CORBA_TRUE;
    seq->_buffer = (Dynamic_Parameter*)rustorb_c_alloc(&parameter_desc, (CORBA_unsigned_long)l.len);
    if (seq->_buffer) {
      memcpy(seq->_buffer, l.items, l.len * sizeof l.items[0]);
      seq->_length = seq->_maximum = (CORBA_unsigned_long)l.len;
      l.len = 0;
    }
  }
  while (l.len > 0) rustorb_c_free_value(&parameter_desc, &l.items[--l.len]);
  free(l.items);
  return seq;
}

typedef struct tcs {
  CORBA_TypeCode* items;
  size_t len, cap;
} tcs;

static void add_tc(void* context, rorb_typecode* tc) {
  tcs* l = (tcs*)context;
  if (l->len == l->cap) {
    size_t cap = l->cap ? l->cap * 2 : 8;
    CORBA_TypeCode* r = (CORBA_TypeCode*)realloc(l->items, cap * sizeof *r);
    if (!r) {
      rorb_tc_release(tc);
      return;
    }
    l->items = r;
    l->cap = cap;
  }
  l->items[l->len++] = rustorb_c_wrap_tc(tc, rustorb_c_desc_for_tc(tc));
}

Dynamic_ExceptionList* PortableInterceptor_RequestInfo__get_exceptions(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev) {
  tcs l = {NULL, 0, 0};
  rorb_exception exc;
  rorb_status st;
  Dynamic_ExceptionList* seq;
  CORBA_Environment ev2 = {CORBA_NO_EXCEPTION, NULL, NULL};
  NO_EXC(exc);
  switch (which(ri, ev)) {
    case 1: st = rorb_cri_exceptions(CRI(ri), &l, add_tc, &exc); break;
    case 2: st = rorb_sri_exceptions(SRI(ri), &l, add_tc, &exc); break;
    default: return NULL;
  }
  seq = rustorb_c_failed(ev, st, &exc) ? NULL : (Dynamic_ExceptionList*)rustorb_c_sequence_alloc();
  if (seq) {
    seq->_release = CORBA_TRUE;
    seq->_buffer = (CORBA_TypeCode*)rustorb_c_alloc(&CORBA_TypeCode__desc, (CORBA_unsigned_long)l.len);
    if (seq->_buffer) {
      memcpy(seq->_buffer, l.items, l.len * sizeof l.items[0]);
      seq->_length = seq->_maximum = (CORBA_unsigned_long)l.len;
      l.len = 0;
    }
  }
  while (l.len > 0) CORBA_Object_release(l.items[--l.len], &ev2);
  free(l.items);
  return seq;
}

CORBA_any* PortableInterceptor_RequestInfo__get_result(PortableInterceptor_RequestInfo ri,
                                                       CORBA_Environment* ev) {
  rorb_any* raw = NULL;
  rorb_exception exc;
  rorb_status st;
  NO_EXC(exc);
  switch (which(ri, ev)) {
    case 1: st = rorb_cri_result(CRI(ri), &raw, &exc); break;
    case 2: st = rorb_sri_result(SRI(ri), &raw, &exc); break;
    default: return NULL;
  }
  if (rustorb_c_failed(ev, st, &exc)) return NULL;
  return any_of(raw);
}

CORBA_boolean PortableInterceptor_RequestInfo__get_response_expected(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev) {
  switch (which(ri, ev)) {
    case 1: return rorb_cri_response_expected(CRI(ri)) ? CORBA_TRUE : CORBA_FALSE;
    case 2: return rorb_sri_response_expected(SRI(ri)) ? CORBA_TRUE : CORBA_FALSE;
    default: return CORBA_FALSE;
  }
}

Messaging_SyncScope PortableInterceptor_RequestInfo__get_sync_scope(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev) {
  switch (which(ri, ev)) {
    case 1: return rorb_cri_sync_scope(CRI(ri));
    case 2: return rorb_sri_sync_scope(SRI(ri));
    default: return 0;
  }
}

PortableInterceptor_ReplyStatus PortableInterceptor_RequestInfo__get_reply_status(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev) {
  int16_t status = 0;
  rorb_exception exc;
  rorb_status st;
  NO_EXC(exc);
  switch (which(ri, ev)) {
    case 1: st = rorb_cri_reply_status(CRI(ri), &status, &exc); break;
    case 2: st = rorb_sri_reply_status(SRI(ri), &status, &exc); break;
    default: return 0;
  }
  rustorb_c_failed(ev, st, &exc);
  return status;
}

CORBA_Object PortableInterceptor_RequestInfo__get_forward_reference(
    PortableInterceptor_RequestInfo ri, CORBA_Environment* ev) {
  rorb_object* obj = NULL;
  rorb_exception exc;
  rorb_status st;
  NO_EXC(exc);
  switch (which(ri, ev)) {
    case 1: st = rorb_cri_forward_reference(CRI(ri), &obj, &exc); break;
    case 2: st = rorb_sri_forward_reference(SRI(ri), &obj, &exc); break;
    default: return CORBA_OBJECT_NIL;
  }
  if (rustorb_c_failed(ev, st, &exc)) return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_REF, obj);
}

CORBA_any* PortableInterceptor_RequestInfo_get_slot(PortableInterceptor_RequestInfo ri,
                                                    PortableInterceptor_SlotId id,
                                                    CORBA_Environment* ev) {
  rorb_any* raw = NULL;
  rorb_exception exc;
  rorb_status st;
  NO_EXC(exc);
  switch (which(ri, ev)) {
    case 1: st = rorb_cri_get_slot(CRI(ri), id, &raw, &exc); break;
    case 2: st = rorb_sri_get_slot(SRI(ri), id, &raw, &exc); break;
    default: return NULL;
  }
  if (rustorb_c_failed(ev, st, &exc)) return NULL;
  return any_of(raw);
}

static IOP_ServiceContext* context_of(CORBA_Environment* ev, rorb_status st, rorb_exception* exc,
                                      IOP_ServiceId id, rorb_bytes b) {
  IOP_ServiceContext* c;
  if (rustorb_c_failed(ev, st, exc)) return NULL;
  c = IOP_ServiceContext__alloc();
  if (!c) {
    rorb_bytes_free(b);
    return NULL;
  }
  c->context_id = id;
  set_octets(&c->context_data, b);
  return c;
}

IOP_ServiceContext* PortableInterceptor_RequestInfo_get_request_service_context(
    PortableInterceptor_RequestInfo ri, IOP_ServiceId id, CORBA_Environment* ev) {
  rorb_bytes b = {NULL, 0};
  rorb_exception exc;
  rorb_status st;
  NO_EXC(exc);
  switch (which(ri, ev)) {
    case 1: st = rorb_cri_get_request_service_context(CRI(ri), id, &b, &exc); break;
    case 2: st = rorb_sri_get_request_service_context(SRI(ri), id, &b, &exc); break;
    default: return NULL;
  }
  return context_of(ev, st, &exc, id, b);
}

IOP_ServiceContext* PortableInterceptor_RequestInfo_get_reply_service_context(
    PortableInterceptor_RequestInfo ri, IOP_ServiceId id, CORBA_Environment* ev) {
  rorb_bytes b = {NULL, 0};
  rorb_exception exc;
  rorb_status st;
  NO_EXC(exc);
  switch (which(ri, ev)) {
    case 1: st = rorb_cri_get_reply_service_context(CRI(ri), id, &b, &exc); break;
    case 2: st = rorb_sri_get_reply_service_context(SRI(ri), id, &b, &exc); break;
    default: return NULL;
  }
  return context_of(ev, st, &exc, id, b);
}

/* --- ClientRequestInfo ------------------------------------------------------------------------- */

CORBA_Object PortableInterceptor_ClientRequestInfo__get_target(
    PortableInterceptor_ClientRequestInfo ri, CORBA_Environment* ev) {
  if (!is_client(ri, ev)) return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_REF, rorb_cri_target(CRI(ri)));
}

CORBA_Object PortableInterceptor_ClientRequestInfo__get_effective_target(
    PortableInterceptor_ClientRequestInfo ri, CORBA_Environment* ev) {
  if (!is_client(ri, ev)) return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_REF, rorb_cri_effective_target(CRI(ri)));
}

IOP_TaggedProfile* PortableInterceptor_ClientRequestInfo__get_effective_profile(
    PortableInterceptor_ClientRequestInfo ri, CORBA_Environment* ev) {
  rorb_bytes b = {NULL, 0};
  IOP_TaggedProfile* p;
  uint32_t tag;
  if (!is_client(ri, ev)) return NULL;
  tag = rorb_cri_effective_profile(CRI(ri), &b);
  p = IOP_TaggedProfile__alloc();
  if (!p) {
    rorb_bytes_free(b);
    return NULL;
  }
  p->tag = tag;
  set_octets(&p->profile_data, b);
  return p;
}

CORBA_any* PortableInterceptor_ClientRequestInfo__get_received_exception(
    PortableInterceptor_ClientRequestInfo ri, CORBA_Environment* ev) {
  rorb_any* raw = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!is_client(ri, ev)) return NULL;
  if (rustorb_c_failed(ev, rorb_cri_received_exception(CRI(ri), &raw, &exc), &exc)) return NULL;
  return any_of(raw);
}

CORBA_char* PortableInterceptor_ClientRequestInfo__get_received_exception_id(
    PortableInterceptor_ClientRequestInfo ri, CORBA_Environment* ev) {
  rorb_bytes b = {NULL, 0};
  rorb_exception exc;
  CORBA_char* s;
  NO_EXC(exc);
  if (!is_client(ri, ev)) return NULL;
  if (rustorb_c_failed(ev, rorb_cri_received_exception_id(CRI(ri), &b, &exc), &exc)) return NULL;
  s = rustorb_c_strndup((const char*)b.data, b.len);
  rorb_bytes_free(b);
  return s;
}

IOP_TaggedComponent* PortableInterceptor_ClientRequestInfo_get_effective_component(
    PortableInterceptor_ClientRequestInfo ri, IOP_ComponentId id, CORBA_Environment* ev) {
  rorb_bytes b = {NULL, 0};
  rorb_exception exc;
  IOP_TaggedComponent* c;
  NO_EXC(exc);
  if (!is_client(ri, ev)) return NULL;
  if (rustorb_c_failed(ev, rorb_cri_get_effective_component(CRI(ri), id, &b, &exc), &exc))
    return NULL;
  c = IOP_TaggedComponent__alloc();
  if (!c) {
    rorb_bytes_free(b);
    return NULL;
  }
  c->tag = id;
  set_octets(&c->component_data, b);
  return c;
}

typedef struct components {
  IOP_TaggedComponent* items;
  size_t len, cap;
} components;

static void add_component(void* context, uint32_t tag, const uint8_t* data, size_t len) {
  components* l = (components*)context;
  IOP_TaggedComponent* c;
  if (l->len == l->cap) {
    size_t cap = l->cap ? l->cap * 2 : 4;
    IOP_TaggedComponent* r = (IOP_TaggedComponent*)realloc(l->items, cap * sizeof *r);
    if (!r) return;
    l->items = r;
    l->cap = cap;
  }
  c = &l->items[l->len++];
  memset(c, 0, sizeof *c);
  c->tag = tag;
  c->component_data._release = CORBA_TRUE;
  if (len) {
    c->component_data._buffer = CORBA_sequence_octet_allocbuf((CORBA_unsigned_long)len);
    if (c->component_data._buffer) {
      memcpy(c->component_data._buffer, data, len);
      c->component_data._length = c->component_data._maximum = (CORBA_unsigned_long)len;
    }
  }
}

IOP_TaggedComponentSeq* PortableInterceptor_ClientRequestInfo_get_effective_components(
    PortableInterceptor_ClientRequestInfo ri, IOP_ComponentId id, CORBA_Environment* ev) {
  components l = {NULL, 0, 0};
  rorb_exception exc;
  IOP_TaggedComponentSeq* seq = NULL;
  NO_EXC(exc);
  if (!is_client(ri, ev)) return NULL;
  if (!rustorb_c_failed(ev,
                        rorb_cri_get_effective_components(CRI(ri), id, &l, add_component, &exc),
                        &exc))
    seq = IOP_TaggedComponentSeq__alloc();
  if (seq) {
    seq->_release = CORBA_TRUE;
    seq->_buffer = CORBA_sequence_IOP_TaggedComponent_allocbuf((CORBA_unsigned_long)l.len);
    if (seq->_buffer) {
      memcpy(seq->_buffer, l.items, l.len * sizeof l.items[0]);
      seq->_length = seq->_maximum = (CORBA_unsigned_long)l.len;
      l.len = 0;
    }
  }
  while (l.len > 0) rustorb_c_free_value(&component_desc, &l.items[--l.len]);
  free(l.items);
  return seq;
}

CORBA_Policy PortableInterceptor_ClientRequestInfo_get_request_policy(
    PortableInterceptor_ClientRequestInfo ri, CORBA_PolicyType type, CORBA_Environment* ev) {
  int32_t found = 0;
  uint64_t value = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!is_client(ri, ev)) return CORBA_OBJECT_NIL;
  if (rustorb_c_failed(ev, rorb_cri_get_request_policy(CRI(ri), type, &found, &value, &exc),
                       &exc) ||
      !found)
    return CORBA_OBJECT_NIL;
  return rustorb_c_policy(type, value);
}

void PortableInterceptor_ClientRequestInfo_add_request_service_context(
    PortableInterceptor_ClientRequestInfo ri, const IOP_ServiceContext* service_context,
    CORBA_boolean replace, CORBA_Environment* ev) {
  rorb_exception exc;
  NO_EXC(exc);
  if (!is_client(ri, ev)) return;
  RUSTORB_C_ARG_CHECK(ev, service_context, ex_CORBA_BAD_PARAM);
  rustorb_c_failed(ev,
                   rorb_cri_add_request_service_context(
                       CRI(ri), service_context->context_id, service_context->context_data._buffer,
                       service_context->context_data._length, replace ? 1 : 0, &exc),
                   &exc);
}

/* --- ServerRequestInfo --------------------------------------------------------------------------- */

CORBA_any* PortableInterceptor_ServerRequestInfo__get_sending_exception(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev) {
  rorb_any* raw = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!is_server(ri, ev)) return NULL;
  if (rustorb_c_failed(ev, rorb_sri_sending_exception(SRI(ri), &raw, &exc), &exc)) return NULL;
  return any_of(raw);
}

static CORBA_sequence_octet* octets_result(CORBA_Environment* ev, rorb_status st,
                                           rorb_exception* exc, rorb_bytes b) {
  if (rustorb_c_failed(ev, st, exc)) return NULL;
  return rustorb_c_object_id(b);
}

CORBA_sequence_octet* PortableInterceptor_ServerRequestInfo__get_object_id(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev) {
  rorb_bytes b = {NULL, 0};
  rorb_exception exc;
  rorb_status st;
  NO_EXC(exc);
  if (!is_server(ri, ev)) return NULL;
  st = rorb_sri_object_id(SRI(ri), &b, &exc);
  return octets_result(ev, st, &exc, b);
}

CORBA_sequence_octet* PortableInterceptor_ServerRequestInfo__get_adapter_id(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev) {
  rorb_bytes b = {NULL, 0};
  rorb_exception exc;
  rorb_status st;
  NO_EXC(exc);
  if (!is_server(ri, ev)) return NULL;
  st = rorb_sri_adapter_id(SRI(ri), &b, &exc);
  return octets_result(ev, st, &exc, b);
}

CORBA_char* PortableInterceptor_ServerRequestInfo__get_server_id(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev) {
  size_t len = 0;
  const char* s;
  if (!is_server(ri, ev)) return NULL;
  s = rorb_sri_server_id(SRI(ri), &len);
  return string_of(s, len);
}

CORBA_char* PortableInterceptor_ServerRequestInfo__get_orb_id(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev) {
  size_t len = 0;
  const char* s;
  if (!is_server(ri, ev)) return NULL;
  s = rorb_sri_orb_id(SRI(ri), &len);
  return string_of(s, len);
}

CORBA_sequence_string* PortableInterceptor_ServerRequestInfo__get_adapter_name(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev) {
  strings l = {NULL, 0, 0};
  rorb_exception exc;
  NO_EXC(exc);
  if (!is_server(ri, ev)) return NULL;
  if (rustorb_c_failed(ev, rorb_sri_adapter_name(SRI(ri), &l, add_string, &exc), &exc)) {
    while (l.len > 0) CORBA_free(l.items[--l.len]);
    free(l.items);
    return NULL;
  }
  return string_seq(&l);
}

CORBA_char* PortableInterceptor_ServerRequestInfo__get_target_most_derived_interface(
    PortableInterceptor_ServerRequestInfo ri, CORBA_Environment* ev) {
  rorb_bytes b = {NULL, 0};
  rorb_exception exc;
  CORBA_char* s;
  NO_EXC(exc);
  if (!is_server(ri, ev)) return NULL;
  if (rustorb_c_failed(ev, rorb_sri_target_most_derived_interface(SRI(ri), &b, &exc), &exc))
    return NULL;
  s = rustorb_c_strndup((const char*)b.data, b.len);
  rorb_bytes_free(b);
  return s;
}

CORBA_Policy PortableInterceptor_ServerRequestInfo_get_server_policy(
    PortableInterceptor_ServerRequestInfo ri, CORBA_PolicyType type, CORBA_Environment* ev) {
  uint64_t value = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!is_server(ri, ev)) return CORBA_OBJECT_NIL;
  if (rustorb_c_failed(ev, rorb_sri_get_server_policy(SRI(ri), type, &value, &exc), &exc))
    return CORBA_OBJECT_NIL;
  return rustorb_c_policy(type, value);
}

void PortableInterceptor_ServerRequestInfo_set_slot(PortableInterceptor_ServerRequestInfo ri,
                                                    PortableInterceptor_SlotId id,
                                                    const CORBA_any* data, CORBA_Environment* ev) {
  rorb_exception exc;
  rorb_any* raw;
  NO_EXC(exc);
  if (!is_server(ri, ev)) return;
  if (!(raw = raw_of(data, ev))) return;
  rustorb_c_failed(ev, rorb_sri_set_slot(SRI(ri), id, raw, &exc), &exc);
  rorb_any_release(raw);
}

CORBA_boolean PortableInterceptor_ServerRequestInfo_target_is_a(
    PortableInterceptor_ServerRequestInfo ri, const CORBA_char* id, CORBA_Environment* ev) {
  int32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!is_server(ri, ev)) return CORBA_FALSE;
  if (!id) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return CORBA_FALSE;
  }
  if (rustorb_c_failed(ev, rorb_sri_target_is_a(SRI(ri), id, strlen(id), &r, &exc), &exc))
    return CORBA_FALSE;
  return r ? CORBA_TRUE : CORBA_FALSE;
}

void PortableInterceptor_ServerRequestInfo_add_reply_service_context(
    PortableInterceptor_ServerRequestInfo ri, const IOP_ServiceContext* service_context,
    CORBA_boolean replace, CORBA_Environment* ev) {
  rorb_exception exc;
  NO_EXC(exc);
  if (!is_server(ri, ev)) return;
  RUSTORB_C_ARG_CHECK(ev, service_context, ex_CORBA_BAD_PARAM);
  rustorb_c_failed(ev,
                   rorb_sri_add_reply_service_context(
                       SRI(ri), service_context->context_id, service_context->context_data._buffer,
                       service_context->context_data._length, replace ? 1 : 0, &exc),
                   &exc);
}

/* --- IORInfo ------------------------------------------------------------------------------------- */

#define IORI(info) ((const rorb_ior_info*)(info)->handle)

static int is_ior_info(CORBA_Object info, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (info && info->kind == OBJ_IOR_INFO) return 1;
  rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
  return 0;
}

CORBA_Policy PortableInterceptor_IORInfo_get_effective_policy(PortableInterceptor_IORInfo info,
                                                              CORBA_PolicyType type,
                                                              CORBA_Environment* ev) {
  uint64_t value = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!is_ior_info(info, ev)) return CORBA_OBJECT_NIL;
  if (rustorb_c_failed(ev, rorb_ior_info_get_effective_policy(IORI(info), type, &value, &exc),
                       &exc))
    return CORBA_OBJECT_NIL;
  return rustorb_c_policy(type, value);
}

void PortableInterceptor_IORInfo_add_ior_component(PortableInterceptor_IORInfo info,
                                                   const IOP_TaggedComponent* a_component,
                                                   CORBA_Environment* ev) {
  if (!is_ior_info(info, ev)) return;
  RUSTORB_C_ARG_CHECK(ev, a_component, ex_CORBA_BAD_PARAM);
  rorb_ior_info_add_ior_component(IORI(info), a_component->tag,
                                  a_component->component_data._buffer,
                                  a_component->component_data._length);
}

void PortableInterceptor_IORInfo_add_ior_component_to_profile(
    PortableInterceptor_IORInfo info, const IOP_TaggedComponent* a_component,
    IOP_ProfileId profile_id, CORBA_Environment* ev) {
  rorb_exception exc;
  NO_EXC(exc);
  if (!is_ior_info(info, ev)) return;
  RUSTORB_C_ARG_CHECK(ev, a_component, ex_CORBA_BAD_PARAM);
  rustorb_c_failed(ev,
                   rorb_ior_info_add_ior_component_to_profile(
                       IORI(info), a_component->tag, a_component->component_data._buffer,
                       a_component->component_data._length, profile_id, &exc),
                   &exc);
}

/* --- ORBInitInfo ----------------------------------------------------------------------------------- */

#define INITI(info) ((rorb_orb_init_info*)(info)->handle)

static int is_init_info(CORBA_Object info, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (info && info->kind == OBJ_INIT_INFO) return 1;
  rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
  return 0;
}

CORBA_sequence_string* PortableInterceptor_ORBInitInfo__get_arguments(
    PortableInterceptor_ORBInitInfo info, CORBA_Environment* ev) {
  strings l = {NULL, 0, 0};
  if (!is_init_info(info, ev)) return NULL;
  rorb_init_info_arguments(INITI(info), &l, add_string);
  return string_seq(&l);
}

CORBA_char* PortableInterceptor_ORBInitInfo__get_orb_id(PortableInterceptor_ORBInitInfo info,
                                                        CORBA_Environment* ev) {
  size_t len = 0;
  const char* s;
  if (!is_init_info(info, ev)) return NULL;
  s = rorb_init_info_orb_id(INITI(info), &len);
  return string_of(s, len);
}

IOP_CodecFactory PortableInterceptor_ORBInitInfo__get_codec_factory(
    PortableInterceptor_ORBInitInfo info, CORBA_Environment* ev) {
  CORBA_Object f;
  if (!is_init_info(info, ev)) return CORBA_OBJECT_NIL;
  f = rustorb_c_wrap(OBJ_CODEC_FACTORY, INITI(info));
  if (f) f->flags = 1;
  return f;
}

void PortableInterceptor_ORBInitInfo_register_initial_reference(
    PortableInterceptor_ORBInitInfo info, const CORBA_char* id, CORBA_Object obj,
    CORBA_Environment* ev) {
  rorb_exception exc;
  NO_EXC(exc);
  if (!is_init_info(info, ev)) return;
  RUSTORB_C_ARG_CHECK(ev, id, ex_CORBA_BAD_PARAM);
  RUSTORB_C_ARG_CHECK(ev, obj && obj->kind == OBJ_REF, ex_CORBA_BAD_PARAM);
  rustorb_c_failed(ev,
                   rorb_init_info_register_initial_reference(INITI(info), id, strlen(id),
                                                             (rorb_object*)obj->handle, &exc),
                   &exc);
}

CORBA_Object PortableInterceptor_ORBInitInfo_resolve_initial_references(
    PortableInterceptor_ORBInitInfo info, const CORBA_char* id, CORBA_Environment* ev) {
  rorb_object* obj = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!is_init_info(info, ev)) return CORBA_OBJECT_NIL;
  if (!id) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return CORBA_OBJECT_NIL;
  }
  if (!info->flags) {
    /* pre_init */
    rustorb_c_ev_system(ev, ex_CORBA_BAD_INV_ORDER, 14, CORBA_COMPLETED_NO);
    return CORBA_OBJECT_NIL;
  }
  if (strcmp(id, "PICurrent") == 0)
    return rustorb_c_pi_current(rorb_init_info_pi_current(INITI(info)));
  if (strcmp(id, "CodecFactory") == 0)
    return PortableInterceptor_ORBInitInfo__get_codec_factory(info, ev);
  if (strcmp(id, "ORB") == 0 || strcmp(id, "RootPOA") == 0) {
    /* Pseudo objects: from the ORB once it is made. */
    rustorb_c_ev_system(ev, ex_CORBA_NO_IMPLEMENT, 0, CORBA_COMPLETED_NO);
    return CORBA_OBJECT_NIL;
  }
  if (rustorb_c_failed(ev,
                       rorb_init_info_resolve_initial_references(INITI(info), id, strlen(id),
                                                                 &obj, &exc),
                       &exc))
    return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_REF, obj);
}

void PortableInterceptor_ORBInitInfo_add_client_request_interceptor(
    PortableInterceptor_ORBInitInfo info, PortableInterceptor_ClientRequestInterceptor interceptor,
    CORBA_Environment* ev) {
  PortableServer_Servant s;
  rorb_exception exc;
  CORBA_char* name;
  NO_EXC(exc);
  if (!is_init_info(info, ev)) return;
  if (!(s = impl_of(interceptor, &client_class, ev))) return;
  name = name_of(s);
  rustorb_c_failed(ev,
                   rorb_init_info_add_client_request_interceptor(INITI(info), name, strlen(name),
                                                                 s, &client_vtable, &exc),
                   &exc);
  CORBA_free(name);
}

void PortableInterceptor_ORBInitInfo_add_server_request_interceptor(
    PortableInterceptor_ORBInitInfo info, PortableInterceptor_ServerRequestInterceptor interceptor,
    CORBA_Environment* ev) {
  PortableServer_Servant s;
  rorb_exception exc;
  CORBA_char* name;
  NO_EXC(exc);
  if (!is_init_info(info, ev)) return;
  if (!(s = impl_of(interceptor, &server_class, ev))) return;
  name = name_of(s);
  rustorb_c_failed(ev,
                   rorb_init_info_add_server_request_interceptor(INITI(info), name, strlen(name),
                                                                 s, &server_vtable, &exc),
                   &exc);
  CORBA_free(name);
}

void PortableInterceptor_ORBInitInfo_add_ior_interceptor(
    PortableInterceptor_ORBInitInfo info, PortableInterceptor_IORInterceptor interceptor,
    CORBA_Environment* ev) {
  PortableServer_Servant s;
  rorb_exception exc;
  CORBA_char* name;
  NO_EXC(exc);
  if (!is_init_info(info, ev)) return;
  if (!(s = impl_of(interceptor, &ior_class, ev))) return;
  name = name_of(s);
  rustorb_c_failed(ev,
                   rorb_init_info_add_ior_interceptor(INITI(info), name, strlen(name), s,
                                                      &ior_vtable, &exc),
                   &exc);
  CORBA_free(name);
}

PortableInterceptor_SlotId PortableInterceptor_ORBInitInfo_allocate_slot_id(
    PortableInterceptor_ORBInitInfo info, CORBA_Environment* ev) {
  if (!is_init_info(info, ev)) return 0;
  return rorb_init_info_allocate_slot_id(INITI(info));
}

/* --- PICurrent --------------------------------------------------------------------------------------- */

static const rorb_pi_current* current_of(CORBA_Object current, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (current && current->kind == OBJ_PI_CURRENT)
    return (const rorb_pi_current*)((rustorb_c_shared*)current->handle)->handle;
  rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
  return NULL;
}

CORBA_any* PortableInterceptor_Current_get_slot(PortableInterceptor_Current current,
                                                PortableInterceptor_SlotId id,
                                                CORBA_Environment* ev) {
  const rorb_pi_current* c = current_of(current, ev);
  rorb_any* raw = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!c) return NULL;
  if (rustorb_c_failed(ev, rorb_pi_current_get_slot(c, id, &raw, &exc), &exc)) return NULL;
  return any_of(raw);
}

void PortableInterceptor_Current_set_slot(PortableInterceptor_Current current,
                                          PortableInterceptor_SlotId id, const CORBA_any* data,
                                          CORBA_Environment* ev) {
  const rorb_pi_current* c = current_of(current, ev);
  rorb_exception exc;
  rorb_any* raw;
  NO_EXC(exc);
  if (!c) return;
  if (!(raw = raw_of(data, ev))) return;
  rustorb_c_failed(ev, rorb_pi_current_set_slot(c, id, raw, &exc), &exc);
  rorb_any_release(raw);
}

/* --- IOP::Codec ------------------------------------------------------------------------------------------ */

IOP_Codec IOP_CodecFactory_create_codec(IOP_CodecFactory factory, const IOP_Encoding* enc,
                                        CORBA_Environment* ev) {
  rorb_codec* codec = NULL;
  rorb_exception exc;
  rorb_status st;
  NO_EXC(exc);
  rustorb_c_ev_clear(ev);
  if (!factory || factory->kind != OBJ_CODEC_FACTORY || !enc) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return CORBA_OBJECT_NIL;
  }
  if (factory->flags)
    st = rorb_init_info_create_codec((const rorb_orb_init_info*)factory->handle, enc->format,
                                     enc->major_version, enc->minor_version, &codec, &exc);
  else
    st = rorb_orb_create_codec((rorb_orb*)factory->handle, enc->format, enc->major_version,
                               enc->minor_version, &codec, &exc);
  if (rustorb_c_failed(ev, st, &exc)) return CORBA_OBJECT_NIL;
  return wrap_shared(OBJ_CODEC, codec, release_codec);
}

static const rorb_codec* codec_of(IOP_Codec codec, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (codec && codec->kind == OBJ_CODEC)
    return (const rorb_codec*)((rustorb_c_shared*)codec->handle)->handle;
  rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
  return NULL;
}

static CORBA_sequence_octet* encode(IOP_Codec codec, const CORBA_any* data, int value_only,
                                    CORBA_Environment* ev) {
  const rorb_codec* c = codec_of(codec, ev);
  rorb_bytes b = {NULL, 0};
  rorb_exception exc;
  rorb_any* raw;
  rorb_status st;
  NO_EXC(exc);
  if (!c || !(raw = raw_of(data, ev))) return NULL;
  st = rorb_codec_encode(c, raw, value_only, &b, &exc);
  rorb_any_release(raw);
  if (rustorb_c_failed(ev, st, &exc)) return NULL;
  return rustorb_c_object_id(b);
}

static CORBA_any* decode(IOP_Codec codec, const CORBA_sequence_octet* data, CORBA_TypeCode tc,
                         int value_only, CORBA_Environment* ev) {
  const rorb_codec* c = codec_of(codec, ev);
  rorb_any* raw = NULL;
  rorb_exception exc;
  rorb_typecode* t = NULL;
  NO_EXC(exc);
  if (!c) return NULL;
  if (!data || (value_only && !(t = rustorb_c_tc_handle(tc)))) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  if (rustorb_c_failed(ev, rorb_codec_decode(c, data->_buffer, data->_length, t, &raw, &exc),
                       &exc))
    return NULL;
  return any_of(raw);
}

CORBA_sequence_octet* IOP_Codec_encode(IOP_Codec codec, const CORBA_any* data,
                                       CORBA_Environment* ev) {
  return encode(codec, data, 0, ev);
}

CORBA_sequence_octet* IOP_Codec_encode_value(IOP_Codec codec, const CORBA_any* data,
                                             CORBA_Environment* ev) {
  return encode(codec, data, 1, ev);
}

CORBA_any* IOP_Codec_decode(IOP_Codec codec, const CORBA_sequence_octet* data,
                            CORBA_Environment* ev) {
  return decode(codec, data, NULL, 0, ev);
}

CORBA_any* IOP_Codec_decode_value(IOP_Codec codec, const CORBA_sequence_octet* data,
                                  CORBA_TypeCode tc, CORBA_Environment* ev) {
  return decode(codec, data, tc, 1, ev);
}
