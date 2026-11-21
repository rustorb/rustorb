/* PortableServer::POA, POAManager and Current. */

#include "internal.h"

#define NO_EXC(e) memset(&(e), 0, sizeof(e))

static rorb_poa* poa_of(PortableServer_POA poa, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (!poa || poa->kind != OBJ_POA) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  return (rorb_poa*)poa->handle;
}

static rorb_poa_manager* manager_of(PortableServer_POAManager m, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (!m || m->kind != OBJ_POA_MANAGER) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  return (rorb_poa_manager*)m->handle;
}

static rorb_servant* servant_of(PortableServer_Servant servant, CORBA_Environment* ev) {
  rorb_servant* s = rustorb_c_servant_handle(servant);
  if (!s) rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
  return s;
}

static rorb_object* ref_of(CORBA_Object obj, CORBA_Environment* ev) {
  if (!obj || obj->kind != OBJ_REF) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  return (rorb_object*)obj->handle;
}

static int id_ok(const PortableServer_ObjectId* id, CORBA_Environment* ev) {
  if (!id || (id->_length && !id->_buffer)) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return 0;
  }
  return 1;
}

static const uint8_t* id_data(const PortableServer_ObjectId* id) {
  static const uint8_t empty = 0;
  return id->_buffer ? id->_buffer : &empty;
}

/* --- policies ----------------------------------------------------------------- */

static CORBA_Policy make_policy(PortableServer_POA poa, CORBA_unsigned_long type,
                                CORBA_unsigned_long value, CORBA_unsigned_long count,
                                CORBA_Environment* ev) {
  CORBA_Policy p;
  if (!poa_of(poa, ev)) return CORBA_OBJECT_NIL;
  if (value >= count) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return CORBA_OBJECT_NIL;
  }
  p = rustorb_c_wrap(OBJ_POLICY, NULL);
  if (p) {
    p->policy_type = type;
    p->policy_value = value;
  }
  return p;
}

PortableServer_ThreadPolicy PortableServer_POA_create_thread_policy(
    PortableServer_POA poa, PortableServer_ThreadPolicyValue value, CORBA_Environment* ev) {
  return make_policy(poa, RORB_THREAD_POLICY_ID, (CORBA_unsigned_long)value, 3, ev);
}
PortableServer_LifespanPolicy PortableServer_POA_create_lifespan_policy(
    PortableServer_POA poa, PortableServer_LifespanPolicyValue value, CORBA_Environment* ev) {
  return make_policy(poa, RORB_LIFESPAN_POLICY_ID, (CORBA_unsigned_long)value, 2, ev);
}
PortableServer_IdUniquenessPolicy PortableServer_POA_create_id_uniqueness_policy(
    PortableServer_POA poa, PortableServer_IdUniquenessPolicyValue value, CORBA_Environment* ev) {
  return make_policy(poa, RORB_ID_UNIQUENESS_POLICY_ID, (CORBA_unsigned_long)value, 2, ev);
}
PortableServer_IdAssignmentPolicy PortableServer_POA_create_id_assignment_policy(
    PortableServer_POA poa, PortableServer_IdAssignmentPolicyValue value, CORBA_Environment* ev) {
  return make_policy(poa, RORB_ID_ASSIGNMENT_POLICY_ID, (CORBA_unsigned_long)value, 2, ev);
}
PortableServer_ImplicitActivationPolicy PortableServer_POA_create_implicit_activation_policy(
    PortableServer_POA poa, PortableServer_ImplicitActivationPolicyValue value,
    CORBA_Environment* ev) {
  return make_policy(poa, RORB_IMPLICIT_ACTIVATION_POLICY_ID, (CORBA_unsigned_long)value, 2, ev);
}
PortableServer_ServantRetentionPolicy PortableServer_POA_create_servant_retention_policy(
    PortableServer_POA poa, PortableServer_ServantRetentionPolicyValue value,
    CORBA_Environment* ev) {
  return make_policy(poa, RORB_SERVANT_RETENTION_POLICY_ID, (CORBA_unsigned_long)value, 2, ev);
}
PortableServer_RequestProcessingPolicy PortableServer_POA_create_request_processing_policy(
    PortableServer_POA poa, PortableServer_RequestProcessingPolicyValue value,
    CORBA_Environment* ev) {
  return make_policy(poa, RORB_REQUEST_PROCESSING_POLICY_ID, (CORBA_unsigned_long)value, 3, ev);
}

/* --- POAManager ---------------------------------------------------------------- */

void PortableServer_POAManager_activate(PortableServer_POAManager m, CORBA_Environment* ev) {
  rorb_poa_manager* h = manager_of(m, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (h) rustorb_c_failed(ev, rorb_poa_manager_activate(h, &exc), &exc);
}

void PortableServer_POAManager_hold_requests(PortableServer_POAManager m,
                                             CORBA_boolean wait_for_completion,
                                             CORBA_Environment* ev) {
  rorb_poa_manager* h = manager_of(m, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (h) rustorb_c_failed(ev, rorb_poa_manager_hold_requests(h, wait_for_completion, &exc), &exc);
}

void PortableServer_POAManager_discard_requests(PortableServer_POAManager m,
                                                CORBA_boolean wait_for_completion,
                                                CORBA_Environment* ev) {
  rorb_poa_manager* h = manager_of(m, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (h)
    rustorb_c_failed(ev, rorb_poa_manager_discard_requests(h, wait_for_completion, &exc), &exc);
}

void PortableServer_POAManager_deactivate(PortableServer_POAManager m, CORBA_boolean etherealize,
                                          CORBA_boolean wait_for_completion,
                                          CORBA_Environment* ev) {
  rorb_poa_manager* h = manager_of(m, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (h)
    rustorb_c_failed(ev, rorb_poa_manager_deactivate(h, etherealize, wait_for_completion, &exc),
                     &exc);
}

PortableServer_POAManager_State PortableServer_POAManager_get_state(PortableServer_POAManager m,
                                                                    CORBA_Environment* ev) {
  rorb_poa_manager* h = manager_of(m, ev);
  return h ? (PortableServer_POAManager_State)rorb_poa_manager_get_state(h)
           : PortableServer_POAManager_INACTIVE;
}

CORBA_char* PortableServer_POAManager_get_id(PortableServer_POAManager m, CORBA_Environment* ev) {
  rorb_poa_manager* h = manager_of(m, ev);
  size_t len = 0;
  const char* s;
  if (!h) return NULL;
  s = rorb_poa_manager_get_id(h, &len);
  return rustorb_c_strndup(s, len);
}

/* --- POA ------------------------------------------------------------------------- */

PortableServer_POA PortableServer_POA_create_POA(PortableServer_POA poa,
                                                 const CORBA_char* adapter_name,
                                                 PortableServer_POAManager a_POAManager,
                                                 const CORBA_PolicyList* policies,
                                                 CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_poa* child = NULL;
  rorb_poa_manager* m = NULL;
  rorb_policy* list = NULL;
  rorb_exception exc;
  CORBA_unsigned_long i, n = policies ? policies->_length : 0;
  rorb_status st;
  NO_EXC(exc);
  if (!h) return CORBA_OBJECT_NIL;
  if (!adapter_name || (n && !policies->_buffer)) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return CORBA_OBJECT_NIL;
  }
  if (a_POAManager) {
    if (a_POAManager->kind != OBJ_POA_MANAGER) {
      rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
      return CORBA_OBJECT_NIL;
    }
    m = (rorb_poa_manager*)a_POAManager->handle;
  }
  if (n) {
    list = (rorb_policy*)calloc(n, sizeof *list);
    if (!list) {
      rustorb_c_ev_system(ev, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
      return CORBA_OBJECT_NIL;
    }
    for (i = 0; i < n; i++) {
      CORBA_Policy p = policies->_buffer[i];
      if (!p || p->kind != OBJ_POLICY) {
        free(list);
        rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
        return CORBA_OBJECT_NIL;
      }
      list[i].type = p->policy_type;
      list[i].value = (uint32_t)p->policy_value;
    }
  }
  st = rorb_poa_create_poa(h, adapter_name, strlen(adapter_name), m, list, n, &child, &exc);
  free(list);
  if (rustorb_c_failed(ev, st, &exc)) return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_POA, child);
}

PortableServer_POA PortableServer_POA_find_POA(PortableServer_POA poa,
                                               const CORBA_char* adapter_name,
                                               CORBA_boolean activate_it, CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_poa* child = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!h) return CORBA_OBJECT_NIL;
  if (!adapter_name) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return CORBA_OBJECT_NIL;
  }
  if (rustorb_c_failed(
          ev, rorb_poa_find_poa(h, adapter_name, strlen(adapter_name), activate_it, &child, &exc),
          &exc))
    return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_POA, child);
}

void PortableServer_POA_destroy(PortableServer_POA poa, CORBA_boolean etherealize_objects,
                                CORBA_boolean wait_for_completion, CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (h)
    rustorb_c_failed(ev, rorb_poa_destroy(h, etherealize_objects, wait_for_completion, &exc),
                     &exc);
}

CORBA_char* PortableServer_POA__get_the_name(PortableServer_POA poa, CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  size_t len = 0;
  const char* s;
  if (!h) return NULL;
  s = rorb_poa_name(h, &len);
  return rustorb_c_strndup(s, len);
}

PortableServer_POA PortableServer_POA__get_the_parent(PortableServer_POA poa,
                                                      CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  return h ? rustorb_c_wrap(OBJ_POA, rorb_poa_parent(h)) : CORBA_OBJECT_NIL;
}

typedef struct children {
  PortableServer_POA* list;
  size_t len, cap;
} children;

static void add_child(void* context, rorb_poa* child) {
  children* c = (children*)context;
  if (c->len == c->cap) {
    size_t cap = c->cap ? c->cap * 2 : 8;
    PortableServer_POA* r = (PortableServer_POA*)realloc(c->list, cap * sizeof *r);
    if (!r) return;
    c->list = r;
    c->cap = cap;
  }
  c->list[c->len++] = rustorb_c_wrap(OBJ_POA, rorb_poa_clone(child));
}

PortableServer_POAList* PortableServer_POA__get_the_children(PortableServer_POA poa,
                                                             CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  children c = {NULL, 0, 0};
  PortableServer_POAList* list;
  size_t i;
  if (!h) return NULL;
  rorb_poa_children(h, &c, add_child);
  list = (PortableServer_POAList*)rustorb_c_sequence_alloc();
  if (list) {
    list->_buffer = (PortableServer_POA*)rustorb_c_alloc(&CORBA_Object__desc,
                                                         (CORBA_unsigned_long)c.len);
    list->_length = list->_maximum = (CORBA_unsigned_long)c.len;
    list->_release = CORBA_TRUE;
  }
  for (i = 0; i < c.len; i++) {
    if (list && list->_buffer)
      list->_buffer[i] = c.list[i];
    else
      CORBA_Object_release(c.list[i], NULL);
  }
  free(c.list);
  return list;
}

PortableServer_POAManager PortableServer_POA__get_the_POAManager(PortableServer_POA poa,
                                                                 CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  return h ? rustorb_c_wrap(OBJ_POA_MANAGER, rorb_poa_the_manager(h)) : CORBA_OBJECT_NIL;
}

PortableServer_Servant PortableServer_POA_get_servant(PortableServer_POA poa,
                                                      CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  void* self = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (h) rustorb_c_failed(ev, rorb_poa_get_servant(h, &self, &exc), &exc);
  return self;
}

void PortableServer_POA_set_servant(PortableServer_POA poa, PortableServer_Servant servant,
                                    CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_servant* s;
  rorb_exception exc;
  NO_EXC(exc);
  if (!h || !(s = servant_of(servant, ev))) return;
  rustorb_c_failed(ev, rorb_poa_set_servant(h, s, &exc), &exc);
}

PortableServer_ObjectId* PortableServer_POA_activate_object(PortableServer_POA poa,
                                                            PortableServer_Servant servant,
                                                            CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_servant* s;
  rorb_bytes id;
  rorb_exception exc;
  NO_EXC(exc);
  if (!h || !(s = servant_of(servant, ev))) return NULL;
  if (rustorb_c_failed(ev, rorb_poa_activate_object(h, s, &id, &exc), &exc)) return NULL;
  return rustorb_c_object_id(id);
}

void PortableServer_POA_activate_object_with_id(PortableServer_POA poa,
                                                const PortableServer_ObjectId* id,
                                                PortableServer_Servant servant,
                                                CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_servant* s;
  rorb_exception exc;
  NO_EXC(exc);
  if (!h || !id_ok(id, ev) || !(s = servant_of(servant, ev))) return;
  rustorb_c_failed(ev, rorb_poa_activate_object_with_id(h, id_data(id), id->_length, s, &exc),
                   &exc);
}

void PortableServer_POA_deactivate_object(PortableServer_POA poa,
                                          const PortableServer_ObjectId* oid,
                                          CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (!h || !id_ok(oid, ev)) return;
  rustorb_c_failed(ev, rorb_poa_deactivate_object(h, id_data(oid), oid->_length, &exc), &exc);
}

CORBA_Object PortableServer_POA_create_reference(PortableServer_POA poa, const CORBA_char* intf,
                                                 CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_object* obj = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!h) return CORBA_OBJECT_NIL;
  if (!intf) intf = "";
  if (rustorb_c_failed(ev, rorb_poa_create_reference(h, intf, strlen(intf), &obj, &exc), &exc))
    return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_REF, obj);
}

CORBA_Object PortableServer_POA_create_reference_with_id(PortableServer_POA poa,
                                                         const PortableServer_ObjectId* oid,
                                                         const CORBA_char* intf,
                                                         CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_object* obj = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!h || !id_ok(oid, ev)) return CORBA_OBJECT_NIL;
  if (!intf) intf = "";
  if (rustorb_c_failed(ev,
                       rorb_poa_create_reference_with_id(h, id_data(oid), oid->_length, intf,
                                                         strlen(intf), &obj, &exc),
                       &exc))
    return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_REF, obj);
}

PortableServer_ObjectId* PortableServer_POA_servant_to_id(PortableServer_POA poa,
                                                          PortableServer_Servant servant,
                                                          CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_servant* s;
  rorb_bytes id;
  rorb_exception exc;
  NO_EXC(exc);
  if (!h || !(s = servant_of(servant, ev))) return NULL;
  if (rustorb_c_failed(ev, rorb_poa_servant_to_id(h, s, &id, &exc), &exc)) return NULL;
  return rustorb_c_object_id(id);
}

CORBA_Object PortableServer_POA_servant_to_reference(PortableServer_POA poa,
                                                     PortableServer_Servant servant,
                                                     CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_servant* s;
  rorb_object* obj = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!h || !(s = servant_of(servant, ev))) return CORBA_OBJECT_NIL;
  if (rustorb_c_failed(ev, rorb_poa_servant_to_reference(h, s, &obj, &exc), &exc))
    return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_REF, obj);
}

PortableServer_Servant PortableServer_POA_reference_to_servant(PortableServer_POA poa,
                                                               CORBA_Object reference,
                                                               CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_object* r;
  void* self = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!h || !(r = ref_of(reference, ev))) return NULL;
  rustorb_c_failed(ev, rorb_poa_reference_to_servant(h, r, &self, &exc), &exc);
  return self;
}

PortableServer_ObjectId* PortableServer_POA_reference_to_id(PortableServer_POA poa,
                                                            CORBA_Object reference,
                                                            CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_object* r;
  rorb_bytes id;
  rorb_exception exc;
  NO_EXC(exc);
  if (!h || !(r = ref_of(reference, ev))) return NULL;
  if (rustorb_c_failed(ev, rorb_poa_reference_to_id(h, r, &id, &exc), &exc)) return NULL;
  return rustorb_c_object_id(id);
}

PortableServer_Servant PortableServer_POA_id_to_servant(PortableServer_POA poa,
                                                        const PortableServer_ObjectId* oid,
                                                        CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  void* self = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!h || !id_ok(oid, ev)) return NULL;
  rustorb_c_failed(ev, rorb_poa_id_to_servant(h, id_data(oid), oid->_length, &self, &exc), &exc);
  return self;
}

CORBA_Object PortableServer_POA_id_to_reference(PortableServer_POA poa,
                                                const PortableServer_ObjectId* oid,
                                                CORBA_Environment* ev) {
  rorb_poa* h = poa_of(poa, ev);
  rorb_object* obj = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!h || !id_ok(oid, ev)) return CORBA_OBJECT_NIL;
  if (rustorb_c_failed(ev, rorb_poa_id_to_reference(h, id_data(oid), oid->_length, &obj, &exc),
                       &exc))
    return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_REF, obj);
}

/* --- Current --------------------------------------------------------------------- */

static int current_ok(PortableServer_Current current, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (!current || current->kind != OBJ_CURRENT) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return 0;
  }
  return 1;
}

PortableServer_POA PortableServer_Current_get_POA(PortableServer_Current current,
                                                  CORBA_Environment* ev) {
  rorb_poa* poa = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!current_ok(current, ev) || rustorb_c_failed(ev, rorb_current_get_poa(&poa, &exc), &exc))
    return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_POA, poa);
}

PortableServer_ObjectId* PortableServer_Current_get_object_id(PortableServer_Current current,
                                                              CORBA_Environment* ev) {
  rorb_bytes id;
  rorb_exception exc;
  NO_EXC(exc);
  if (!current_ok(current, ev) || rustorb_c_failed(ev, rorb_current_get_object_id(&id, &exc), &exc))
    return NULL;
  return rustorb_c_object_id(id);
}

CORBA_Object PortableServer_Current_get_reference(PortableServer_Current current,
                                                  CORBA_Environment* ev) {
  rorb_object* obj = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!current_ok(current, ev) ||
      rustorb_c_failed(ev, rorb_current_get_reference(&obj, &exc), &exc))
    return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_REF, obj);
}

PortableServer_Servant PortableServer_Current_get_servant(PortableServer_Current current,
                                                          CORBA_Environment* ev) {
  void* self = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!current_ok(current, ev)) return NULL;
  rustorb_c_failed(ev, rorb_current_get_servant(&self, &exc), &exc);
  return self;
}
