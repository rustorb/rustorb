/* CORBA::Object, CORBA::ORB and CORBA::Policy. */

#include "internal.h"

#define NO_EXC(e) memset(&(e), 0, sizeof(e))

static const char* const pseudo_ids[] = {
    NULL,
    "IDL:omg.org/CORBA/Object:1.0",
    "IDL:omg.org/CORBA/ORB:1.0",
    "IDL:omg.org/PortableServer/POA:1.0",
    "IDL:omg.org/PortableServer/POAManager:1.0",
    "IDL:omg.org/CORBA/TypeCode:1.0",
    "IDL:omg.org/CORBA/Policy:1.0",
    "IDL:omg.org/PortableServer/Current:1.0",
    "IDL:omg.org/DynamicAny/DynAnyFactory:1.0",
    "IDL:omg.org/DynamicAny/DynAny:1.0",
    "IDL:omg.org/CORBA/LocalObject:1.0",
    "IDL:omg.org/PortableInterceptor/ClientRequestInfo:1.0",
    "IDL:omg.org/PortableInterceptor/ServerRequestInfo:1.0",
    "IDL:omg.org/PortableInterceptor/IORInfo:1.0",
    "IDL:omg.org/PortableInterceptor/ORBInitInfo:1.0",
    "IDL:omg.org/IOP/Codec:1.0",
    "IDL:omg.org/PortableInterceptor/Current:1.0",
    "IDL:omg.org/IOP/CodecFactory:1.0",
};

int rustorb_c_dynany_is_a(CORBA_Object obj, const char* id);

/* --- Object ------------------------------------------------------------------ */

CORBA_Object CORBA_Object_duplicate(CORBA_Object obj, CORBA_Environment* ev) {
  CORBA_Object o;
  rustorb_c_ev_clear(ev);
  if (!obj || obj->immortal) return obj;
  switch (obj->kind) {
    case OBJ_REF: return rustorb_c_wrap(OBJ_REF, rorb_object_clone((rorb_object*)obj->handle));
    case OBJ_ORB: return rustorb_c_wrap(OBJ_ORB, rorb_orb_clone((rorb_orb*)obj->handle));
    case OBJ_POA: return rustorb_c_wrap(OBJ_POA, rorb_poa_clone((rorb_poa*)obj->handle));
    case OBJ_POA_MANAGER:
      return rustorb_c_wrap(OBJ_POA_MANAGER,
                            rorb_poa_manager_clone((rorb_poa_manager*)obj->handle));
    case OBJ_TYPECODE:
      return rustorb_c_wrap_tc(rorb_tc_clone((rorb_typecode*)obj->handle), obj->desc);
    case OBJ_DYNANY: return rustorb_c_wrap(OBJ_DYNANY, rorb_dynany_clone((rorb_dynany*)obj->handle));
    case OBJ_LOCAL:
    case OBJ_CLIENT_INFO:
    case OBJ_SERVER_INFO:
    case OBJ_IOR_INFO:
    case OBJ_INIT_INFO: return rustorb_c_wrap(obj->kind, obj->handle);
    case OBJ_CODEC:
    case OBJ_PI_CURRENT:
      rustorb_c_shared_ref((rustorb_c_shared*)obj->handle);
      o = rustorb_c_wrap(obj->kind, obj->handle);
      if (!o) rustorb_c_shared_unref((rustorb_c_shared*)obj->handle);
      return o;
    case OBJ_CODEC_FACTORY:
      o = rustorb_c_wrap(OBJ_CODEC_FACTORY,
                         obj->flags ? obj->handle : rorb_orb_clone((rorb_orb*)obj->handle));
      if (o) o->flags = obj->flags;
      return o;
    default:
      o = rustorb_c_wrap(obj->kind, NULL);
      if (o) {
        o->policy_type = obj->policy_type;
        o->policy_value = obj->policy_value;
      }
      return o;
  }
}

void CORBA_Object_release(CORBA_Object obj, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (!obj || obj->immortal) return;
  switch (obj->kind) {
    case OBJ_REF: rorb_object_release((rorb_object*)obj->handle); break;
    case OBJ_ORB: rorb_orb_release((rorb_orb*)obj->handle); break;
    case OBJ_POA: rorb_poa_release((rorb_poa*)obj->handle); break;
    case OBJ_POA_MANAGER: rorb_poa_manager_release((rorb_poa_manager*)obj->handle); break;
    case OBJ_TYPECODE: rorb_tc_release((rorb_typecode*)obj->handle); break;
    case OBJ_DYNANY: rorb_dynany_release((rorb_dynany*)obj->handle); break;
    case OBJ_CODEC:
    case OBJ_PI_CURRENT: rustorb_c_shared_unref((rustorb_c_shared*)obj->handle); break;
    case OBJ_CODEC_FACTORY:
      if (!obj->flags) rorb_orb_release((rorb_orb*)obj->handle);
      break;
    default: break;
  }
  free(obj);
}

CORBA_boolean CORBA_Object_is_nil(CORBA_Object obj, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  return obj == CORBA_OBJECT_NIL;
}

CORBA_boolean CORBA_Object_is_a(CORBA_Object obj, const CORBA_char* logical_type_id,
                                CORBA_Environment* ev) {
  int32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  rustorb_c_ev_clear(ev);
  if (!logical_type_id) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return CORBA_FALSE;
  }
  if (!obj) return CORBA_FALSE;
  if (obj->kind == OBJ_DYNANY)
    return rustorb_c_dynany_is_a(obj, logical_type_id) ||
           strcmp(logical_type_id, pseudo_ids[OBJ_REF]) == 0;
  if (obj->kind == OBJ_LOCAL && rustorb_c_local_is_a(obj, logical_type_id)) return CORBA_TRUE;
  if (obj->kind != OBJ_REF)
    return strcmp(logical_type_id, pseudo_ids[obj->kind]) == 0 ||
           strcmp(logical_type_id, pseudo_ids[OBJ_REF]) == 0;
  if (rustorb_c_failed(ev,
                       rorb_object_is_a((rorb_object*)obj->handle, logical_type_id,
                                        strlen(logical_type_id), &r, &exc),
                       &exc))
    return CORBA_FALSE;
  return r != 0;
}

CORBA_boolean CORBA_Object_non_existent(CORBA_Object obj, CORBA_Environment* ev) {
  int32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  rustorb_c_ev_clear(ev);
  if (!obj) return CORBA_TRUE;
  if (obj->kind != OBJ_REF) return CORBA_FALSE;
  if (rustorb_c_failed(ev, rorb_object_non_existent((rorb_object*)obj->handle, &r, &exc), &exc))
    return CORBA_FALSE;
  return r != 0;
}

CORBA_boolean CORBA_Object_is_equivalent(CORBA_Object obj, CORBA_Object other,
                                         CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (!obj || !other) return obj == other;
  if (obj->kind == OBJ_REF && other->kind == OBJ_REF)
    return rorb_object_is_equivalent((rorb_object*)obj->handle, (rorb_object*)other->handle) != 0;
  if (obj->kind == OBJ_LOCAL && other->kind == OBJ_LOCAL) return obj->handle == other->handle;
  if (obj->kind == OBJ_TYPECODE && other->kind == OBJ_TYPECODE)
    return rorb_tc_equal((rorb_typecode*)obj->handle, (rorb_typecode*)other->handle) != 0;
  if (obj->kind == OBJ_POA_MANAGER && other->kind == OBJ_POA_MANAGER)
    return rorb_poa_manager_equal((rorb_poa_manager*)obj->handle,
                                  (rorb_poa_manager*)other->handle) != 0;
  if (obj->kind == OBJ_POA && other->kind == OBJ_POA) {
    size_t a_len, b_len;
    const char* a;
    const char* b;
    rorb_poa* pa = (rorb_poa*)obj->handle;
    rorb_poa* pb = (rorb_poa*)other->handle;
    /* The same POA has the same name and the same parent. */
    while (pa && pb) {
      rorb_poa* na;
      rorb_poa* nb;
      a = rorb_poa_name(pa, &a_len);
      b = rorb_poa_name(pb, &b_len);
      if (a_len != b_len || memcmp(a, b, a_len) != 0) {
        if (pa != obj->handle) rorb_poa_release(pa);
        if (pb != other->handle) rorb_poa_release(pb);
        return CORBA_FALSE;
      }
      na = rorb_poa_parent(pa);
      nb = rorb_poa_parent(pb);
      if (pa != obj->handle) rorb_poa_release(pa);
      if (pb != other->handle) rorb_poa_release(pb);
      pa = na;
      pb = nb;
    }
    if (pa) rorb_poa_release(pa);
    if (pb) rorb_poa_release(pb);
    return pa == pb;
  }
  return obj->kind == other->kind && obj->handle == other->handle &&
         obj->policy_type == other->policy_type && obj->policy_value == other->policy_value;
}

CORBA_unsigned_long CORBA_Object_hash(CORBA_Object obj, CORBA_unsigned_long maximum,
                                      CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (!obj || maximum == 0) return 0;
  if (obj->kind == OBJ_REF) return rorb_object_hash((rorb_object*)obj->handle, maximum);
  return (CORBA_unsigned_long)(((uintptr_t)obj->handle >> 4) % maximum);
}

CORBA_boolean CORBA_Object_validate_connection(CORBA_Object obj, CORBA_PolicyList** inconsistent,
                                               CORBA_Environment* ev) {
  int32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  rustorb_c_ev_clear(ev);
  if (inconsistent) {
    *inconsistent = CORBA_PolicyList__alloc();
    if (*inconsistent) (*inconsistent)->_release = CORBA_TRUE;
  }
  if (!obj || obj->kind != OBJ_REF) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return CORBA_FALSE;
  }
  if (rustorb_c_failed(ev, rorb_object_validate_connection((rorb_object*)obj->handle, &r, &exc),
                       &exc))
    return CORBA_FALSE;
  return r != 0;
}

CORBA_char* CORBA_Object__repository_id(CORBA_Object obj, CORBA_Environment* ev) {
  size_t len = 0;
  const char* s;
  rustorb_c_ev_clear(ev);
  if (!obj) return CORBA_string_dup("");
  if (obj->kind == OBJ_LOCAL) return CORBA_string_dup(rustorb_c_local_id(obj));
  if (obj->kind != OBJ_REF) return CORBA_string_dup(pseudo_ids[obj->kind]);
  s = rorb_object_type_id((rorb_object*)obj->handle, &len);
  return rustorb_c_strndup(s, len);
}

void rustorb_Object_set_call_timeout(CORBA_Object obj, CORBA_long_long us) {
  if (obj && obj->kind == OBJ_REF) rorb_object_set_call_timeout((rorb_object*)obj->handle, us);
}

/* --- ORB -------------------------------------------------------------------------- */

static rorb_orb* orb_of(CORBA_ORB orb, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (!orb || orb->kind != OBJ_ORB) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  return (rorb_orb*)orb->handle;
}

CORBA_ORB CORBA_ORB_init(int* argc, char** argv, const CORBA_char* orb_identifier,
                         CORBA_Environment* ev) {
  rorb_orb* orb = NULL;
  rorb_exception exc;
  int zero = 0;
  NO_EXC(exc);
  (void)orb_identifier;
  rustorb_c_ev_clear(ev);
  if (rorb_abi_version() != RORB_ABI_VERSION) {
    rustorb_c_ev_system(ev, ex_CORBA_INITIALIZE, 0, CORBA_COMPLETED_NO);
    return CORBA_OBJECT_NIL;
  }
  if (rustorb_c_failed(ev, rorb_orb_init(argc ? argc : &zero, argv, &orb, &exc), &exc))
    return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_ORB, orb);
}

CORBA_char* CORBA_ORB_object_to_string(CORBA_ORB orb, CORBA_Object obj, CORBA_Environment* ev) {
  rorb_orb* o = orb_of(orb, ev);
  rorb_bytes s;
  rorb_exception exc;
  CORBA_char* r;
  NO_EXC(exc);
  if (!o) return NULL;
  if (obj && obj->kind != OBJ_REF) {
    rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 4, CORBA_COMPLETED_NO);
    return NULL;
  }
  if (rustorb_c_failed(ev,
                       rorb_orb_object_to_string(o, obj ? (rorb_object*)obj->handle : NULL, &s,
                                                 &exc),
                       &exc))
    return NULL;
  r = rustorb_c_strndup((const char*)s.data, s.len);
  rorb_bytes_free(s);
  return r;
}

CORBA_Object CORBA_ORB_string_to_object(CORBA_ORB orb, const CORBA_char* str,
                                        CORBA_Environment* ev) {
  rorb_orb* o = orb_of(orb, ev);
  rorb_object* obj = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!o) return NULL;
  if (!str) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  if (rustorb_c_failed(ev, rorb_orb_string_to_object(o, str, strlen(str), &obj, &exc), &exc))
    return NULL;
  return rustorb_c_wrap(OBJ_REF, obj);
}

CORBA_Object CORBA_ORB_resolve_initial_references(CORBA_ORB orb, const CORBA_char* identifier,
                                                  CORBA_Environment* ev) {
  rorb_orb* o = orb_of(orb, ev);
  rorb_object* obj = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!o) return NULL;
  if (!identifier) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  if (strcmp(identifier, "RootPOA") == 0) {
    rorb_poa* poa = NULL;
    if (rustorb_c_failed(ev, rorb_orb_root_poa(o, &poa, &exc), &exc)) return NULL;
    return rustorb_c_wrap(OBJ_POA, poa);
  }
  if (strcmp(identifier, "POACurrent") == 0) return rustorb_c_wrap(OBJ_CURRENT, NULL);
  if (strcmp(identifier, "DynAnyFactory") == 0) return rustorb_c_wrap(OBJ_DYNANY_FACTORY, NULL);
  if (strcmp(identifier, "PICurrent") == 0) return rustorb_c_pi_current(rorb_orb_pi_current(o));
  if (strcmp(identifier, "CodecFactory") == 0)
    return rustorb_c_wrap(OBJ_CODEC_FACTORY, rorb_orb_clone(o));
  if (rustorb_c_failed(
          ev, rorb_orb_resolve_initial_references(o, identifier, strlen(identifier), &obj, &exc),
          &exc))
    return NULL;
  return rustorb_c_wrap(OBJ_REF, obj);
}

typedef struct names {
  char** names;
  size_t len, cap;
} names;

static void add_name(void* context, const char* name, size_t len) {
  names* n = (names*)context;
  size_t i;
  for (i = 0; i < n->len; i++)
    if (strlen(n->names[i]) == len && memcmp(n->names[i], name, len) == 0) return;
  if (n->len == n->cap) {
    size_t cap = n->cap ? n->cap * 2 : 16;
    char** r = (char**)realloc(n->names, cap * sizeof *r);
    if (!r) return;
    n->names = r;
    n->cap = cap;
  }
  n->names[n->len++] = rustorb_c_strndup(name, len);
}

CORBA_ORB_ObjectIdList* CORBA_ORB_list_initial_services(CORBA_ORB orb, CORBA_Environment* ev) {
  rorb_orb* o = orb_of(orb, ev);
  names n = {NULL, 0, 0};
  CORBA_ORB_ObjectIdList* list;
  size_t i;
  if (!o) return NULL;
  rorb_orb_list_initial_services(o, &n, add_name);
  add_name(&n, "RootPOA", 7);
  add_name(&n, "POACurrent", 10);
  add_name(&n, "DynAnyFactory", 13);
  add_name(&n, "PICurrent", 9);
  add_name(&n, "CodecFactory", 12);
  list = CORBA_sequence_string__alloc();
  if (list) {
    list->_buffer = CORBA_sequence_string_allocbuf((CORBA_unsigned_long)n.len);
    list->_length = list->_maximum = (CORBA_unsigned_long)n.len;
    list->_release = CORBA_TRUE;
  }
  for (i = 0; i < n.len; i++) {
    if (list && list->_buffer)
      list->_buffer[i] = n.names[i];
    else
      CORBA_free(n.names[i]);
  }
  free(n.names);
  return list;
}

void CORBA_ORB_register_initial_reference(CORBA_ORB orb, const CORBA_char* identifier,
                                          CORBA_Object obj, CORBA_Environment* ev) {
  rorb_orb* o = orb_of(orb, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (!o) return;
  if (!identifier) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return;
  }
  if (obj && obj->kind != OBJ_REF) {
    rustorb_c_ev_system(ev, ex_CORBA_NO_IMPLEMENT, 0, CORBA_COMPLETED_NO);
    return;
  }
  rustorb_c_failed(ev,
                   rorb_orb_register_initial_reference(o, identifier, strlen(identifier),
                                                       obj ? (rorb_object*)obj->handle : NULL,
                                                       &exc),
                   &exc);
}

CORBA_boolean CORBA_ORB_work_pending(CORBA_ORB orb, CORBA_Environment* ev) {
  rorb_orb* o = orb_of(orb, ev);
  int32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (o) rustorb_c_failed(ev, rorb_orb_work_pending(o, &r, &exc), &exc);
  return r != 0;
}

void CORBA_ORB_perform_work(CORBA_ORB orb, CORBA_Environment* ev) {
  rorb_orb* o = orb_of(orb, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (o) rustorb_c_failed(ev, rorb_orb_perform_work(o, &exc), &exc);
}

void CORBA_ORB_run(CORBA_ORB orb, CORBA_Environment* ev) {
  rorb_orb* o = orb_of(orb, ev);
  if (o) rorb_orb_run(o);
}

void CORBA_ORB_shutdown(CORBA_ORB orb, CORBA_boolean wait_for_completion, CORBA_Environment* ev) {
  rorb_orb* o = orb_of(orb, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (o) rustorb_c_failed(ev, rorb_orb_shutdown(o, wait_for_completion, &exc), &exc);
}

void CORBA_ORB_destroy(CORBA_ORB orb, CORBA_Environment* ev) {
  rorb_orb* o = orb_of(orb, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (o) rustorb_c_failed(ev, rorb_orb_destroy(o, &exc), &exc);
}

void rustorb_ORB_set_call_timeout(CORBA_ORB orb, CORBA_long_long us) {
  if (orb && orb->kind == OBJ_ORB) rorb_orb_set_call_timeout((rorb_orb*)orb->handle, us);
}

CORBA_Policy CORBA_ORB_create_policy(CORBA_ORB orb, CORBA_PolicyType type, const CORBA_any* val,
                                     CORBA_Environment* ev) {
  rorb_orb* o = orb_of(orb, ev);
  rorb_exception exc;
  rustorb_c_err err = {NULL, 0};
  rorb_any* raw;
  uint64_t value = 0;
  CORBA_Policy p;
  NO_EXC(exc);
  if (!o) return NULL;
  raw = rustorb_c_any_to_raw(val, &err);
  if (!raw) {
    rustorb_c_ev_system(ev, err.id, err.minor, CORBA_COMPLETED_NO);
    return NULL;
  }
  if (rustorb_c_failed(ev, rorb_orb_create_policy(o, type, raw, &value, &exc), &exc)) {
    rorb_any_release(raw);
    return NULL;
  }
  rorb_any_release(raw);
  p = rustorb_c_wrap(OBJ_POLICY, NULL);
  if (p) {
    p->policy_type = type;
    p->policy_value = value;
  }
  return p;
}

/* r is read after st is computed (a call argument would not be). */
static CORBA_TypeCode made_tc(CORBA_Environment* ev, rorb_status st, rorb_typecode** r,
                              rorb_exception* exc) {
  if (rustorb_c_failed(ev, st, exc)) return CORBA_OBJECT_NIL;
  return rustorb_c_wrap_tc(*r, NULL);
}

CORBA_TypeCode CORBA_ORB_create_string_tc(CORBA_ORB orb, CORBA_unsigned_long bound,
                                          CORBA_Environment* ev) {
  rorb_typecode* r = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!orb_of(orb, ev)) return NULL;
  return made_tc(ev, rorb_tc_string(0, bound, &r, &exc), &r, &exc);
}

CORBA_TypeCode CORBA_ORB_create_wstring_tc(CORBA_ORB orb, CORBA_unsigned_long bound,
                                           CORBA_Environment* ev) {
  rorb_typecode* r = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!orb_of(orb, ev)) return NULL;
  return made_tc(ev, rorb_tc_string(1, bound, &r, &exc), &r, &exc);
}

static rorb_typecode* content_of(CORBA_TypeCode tc, CORBA_Environment* ev) {
  rorb_typecode* h = rustorb_c_tc_handle(tc);
  if (!h) rustorb_c_ev_system(ev, ex_CORBA_BAD_TYPECODE, 0, CORBA_COMPLETED_NO);
  return h;
}

CORBA_TypeCode CORBA_ORB_create_sequence_tc(CORBA_ORB orb, CORBA_unsigned_long bound,
                                            CORBA_TypeCode element_type, CORBA_Environment* ev) {
  rorb_typecode* r = NULL;
  rorb_typecode* c;
  rorb_exception exc;
  NO_EXC(exc);
  if (!orb_of(orb, ev) || !(c = content_of(element_type, ev))) return NULL;
  return made_tc(ev, rorb_tc_sequence(c, bound, &r, &exc), &r, &exc);
}

CORBA_TypeCode CORBA_ORB_create_array_tc(CORBA_ORB orb, CORBA_unsigned_long length,
                                         CORBA_TypeCode element_type, CORBA_Environment* ev) {
  rorb_typecode* r = NULL;
  rorb_typecode* c;
  rorb_exception exc;
  NO_EXC(exc);
  if (!orb_of(orb, ev) || !(c = content_of(element_type, ev))) return NULL;
  return made_tc(ev, rorb_tc_array(c, length, &r, &exc), &r, &exc);
}

CORBA_TypeCode CORBA_ORB_create_alias_tc(CORBA_ORB orb, const CORBA_char* id,
                                         const CORBA_char* name, CORBA_TypeCode original_type,
                                         CORBA_Environment* ev) {
  rorb_typecode* r = NULL;
  rorb_typecode* c;
  rorb_exception exc;
  NO_EXC(exc);
  if (!orb_of(orb, ev) || !(c = content_of(original_type, ev))) return NULL;
  if (!id) id = "";
  if (!name) name = "";
  return made_tc(ev,
                 rorb_tc_alias(RORB_TK_ALIAS, id, strlen(id), name, strlen(name), c, &r, &exc),
                 &r, &exc);
}

CORBA_TypeCode CORBA_ORB_create_interface_tc(CORBA_ORB orb, const CORBA_char* id,
                                             const CORBA_char* name, CORBA_Environment* ev) {
  rorb_typecode* r = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!orb_of(orb, ev)) return NULL;
  if (!id) id = "";
  if (!name) name = "";
  return made_tc(
      ev, rorb_tc_interface(RORB_TK_OBJREF, id, strlen(id), name, strlen(name), &r, &exc), &r,
      &exc);
}

CORBA_TypeCode CORBA_ORB_create_fixed_tc(CORBA_ORB orb, CORBA_unsigned_short digits,
                                         CORBA_short scale, CORBA_Environment* ev) {
  rorb_typecode* r = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!orb_of(orb, ev)) return NULL;
  return made_tc(ev, rorb_tc_fixed(digits, scale, &r, &exc), &r, &exc);
}

/* --- Policy ------------------------------------------------------------------------ */

CORBA_PolicyType CORBA_Policy__get_policy_type(CORBA_Policy policy, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (!policy || policy->kind != OBJ_POLICY) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return 0;
  }
  return policy->policy_type;
}

CORBA_Policy CORBA_Policy_copy(CORBA_Policy policy, CORBA_Environment* ev) {
  return CORBA_Object_duplicate(policy, ev);
}

void CORBA_Policy_destroy(CORBA_Policy policy, CORBA_Environment* ev) {
  (void)policy;
  rustorb_c_ev_clear(ev);
}

CORBA_unsigned_long rustorb_POA_policy_value(CORBA_Policy policy, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (!policy || policy->kind != OBJ_POLICY) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return 0;
  }
  return (CORBA_unsigned_long)policy->policy_value;
}
