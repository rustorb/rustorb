/* Servant managers and adapter activators (rustorb/poa.h): local objects
   whose implementations the POA calls through rorb_*_vtable. */

#include <stddef.h>

#include "internal.h"

/* --- ForwardRequest ------------------------------------------------------------- */

static const rustorb_c_member forward_members[] = {
    {"forward_reference", &CORBA_Object__desc, offsetof(PortableServer_ForwardRequest, forward_reference), 0},
};
static rustorb_c_cache forward_cache;
static const rustorb_c_type forward_desc = {CORBA_tk_except,
                                            sizeof(PortableServer_ForwardRequest),
                                            ex_PortableServer_ForwardRequest,
                                            "ForwardRequest",
                                            NULL,
                                            0,
                                            1,
                                            forward_members,
                                            NULL,
                                            -1,
                                            0,
                                            0,
                                            &forward_cache};

PortableServer_ForwardRequest* PortableServer_ForwardRequest__alloc(void) {
  return (PortableServer_ForwardRequest*)rustorb_c_alloc(&forward_desc, 1);
}

/* --- the classes -------------------------------------------------------------------- */

static const char* const activator_ids[] = {"IDL:omg.org/PortableServer/ServantActivator:1.0",
                                            "IDL:omg.org/PortableServer/ServantManager:1.0"};
static const char* const locator_ids[] = {"IDL:omg.org/PortableServer/ServantLocator:1.0",
                                          "IDL:omg.org/PortableServer/ServantManager:1.0"};
static const char* const adapter_activator_ids[] = {
    "IDL:omg.org/PortableServer/AdapterActivator:1.0"};
static const rustorb_c_class activator_class = {activator_ids, 2, NULL, 0, NULL};
static const rustorb_c_class locator_class = {locator_ids, 2, NULL, 0, NULL};
static const rustorb_c_class adapter_activator_class = {adapter_activator_ids, 1, NULL, 0, NULL};

void POA_PortableServer_ServantActivator__init(PortableServer_Servant servant,
                                               CORBA_Environment* ev) {
  rustorb_c_local_init(servant, &activator_class, ev);
}
void POA_PortableServer_ServantActivator__fini(PortableServer_Servant servant,
                                               CORBA_Environment* ev) {
  rustorb_c_servant_fini(servant, ev);
}
PortableServer_ServantActivator PortableServer_ServantActivator__create(PortableServer_Servant impl,
                                                                        CORBA_Environment* ev) {
  if (rustorb_c_servant_class(impl) != &activator_class) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return CORBA_OBJECT_NIL;
  }
  return rustorb_c_local_object(impl, ev);
}

void POA_PortableServer_ServantLocator__init(PortableServer_Servant servant,
                                             CORBA_Environment* ev) {
  rustorb_c_local_init(servant, &locator_class, ev);
}
void POA_PortableServer_ServantLocator__fini(PortableServer_Servant servant,
                                             CORBA_Environment* ev) {
  rustorb_c_servant_fini(servant, ev);
}
PortableServer_ServantLocator PortableServer_ServantLocator__create(PortableServer_Servant impl,
                                                                    CORBA_Environment* ev) {
  if (rustorb_c_servant_class(impl) != &locator_class) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return CORBA_OBJECT_NIL;
  }
  return rustorb_c_local_object(impl, ev);
}

void POA_PortableServer_AdapterActivator__init(PortableServer_Servant servant,
                                               CORBA_Environment* ev) {
  rustorb_c_local_init(servant, &adapter_activator_class, ev);
}
void POA_PortableServer_AdapterActivator__fini(PortableServer_Servant servant,
                                               CORBA_Environment* ev) {
  rustorb_c_servant_fini(servant, ev);
}
PortableServer_AdapterActivator PortableServer_AdapterActivator__create(
    PortableServer_Servant impl, CORBA_Environment* ev) {
  if (rustorb_c_servant_class(impl) != &adapter_activator_class) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return CORBA_OBJECT_NIL;
  }
  return rustorb_c_local_object(impl, ev);
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

static POA_PortableServer_ServantActivator__epv* activator_epv(PortableServer_Servant s) {
  POA_PortableServer_ServantActivator* a = (POA_PortableServer_ServantActivator*)s;
  return a->vepv ? a->vepv->PortableServer_ServantActivator_epv : NULL;
}

static POA_PortableServer_ServantLocator__epv* locator_epv(PortableServer_Servant s) {
  POA_PortableServer_ServantLocator* l = (POA_PortableServer_ServantLocator*)s;
  return l->vepv ? l->vepv->PortableServer_ServantLocator_epv : NULL;
}

static POA_PortableServer_AdapterActivator__epv* adapter_activator_epv(PortableServer_Servant s) {
  POA_PortableServer_AdapterActivator* a = (POA_PortableServer_AdapterActivator*)s;
  return a->vepv ? a->vepv->PortableServer_AdapterActivator_epv : NULL;
}

PortableServer_Servant PortableServer_ServantActivator_incarnate(
    PortableServer_ServantActivator obj, const PortableServer_ObjectId* oid,
    PortableServer_POA adapter, CORBA_Environment* ev) {
  PortableServer_Servant s;
  rustorb_c_ev_clear(ev);
  s = impl_of(obj, &activator_class, ev);
  if (!s) return NULL;
  if (!activator_epv(s) || !activator_epv(s)->incarnate) {
    rustorb_c_ev_system(ev, ex_CORBA_NO_IMPLEMENT, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  return activator_epv(s)->incarnate(s, oid, adapter, ev);
}

void PortableServer_ServantActivator_etherealize(PortableServer_ServantActivator obj,
                                                 const PortableServer_ObjectId* oid,
                                                 PortableServer_POA adapter,
                                                 PortableServer_Servant serv,
                                                 CORBA_boolean cleanup_in_progress,
                                                 CORBA_boolean remaining_activations,
                                                 CORBA_Environment* ev) {
  PortableServer_Servant s;
  rustorb_c_ev_clear(ev);
  s = impl_of(obj, &activator_class, ev);
  if (s && activator_epv(s) && activator_epv(s)->etherealize)
    activator_epv(s)->etherealize(s, oid, adapter, serv, cleanup_in_progress,
                                  remaining_activations, ev);
}

PortableServer_Servant PortableServer_ServantLocator_preinvoke(
    PortableServer_ServantLocator obj, const PortableServer_ObjectId* oid,
    PortableServer_POA adapter, const CORBA_char* operation,
    PortableServer_ServantLocator_Cookie* the_cookie, CORBA_Environment* ev) {
  PortableServer_Servant s;
  rustorb_c_ev_clear(ev);
  s = impl_of(obj, &locator_class, ev);
  if (!s) return NULL;
  if (!locator_epv(s) || !locator_epv(s)->preinvoke) {
    rustorb_c_ev_system(ev, ex_CORBA_NO_IMPLEMENT, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  return locator_epv(s)->preinvoke(s, oid, adapter, operation, the_cookie, ev);
}

void PortableServer_ServantLocator_postinvoke(PortableServer_ServantLocator obj,
                                              const PortableServer_ObjectId* oid,
                                              PortableServer_POA adapter,
                                              const CORBA_char* operation,
                                              PortableServer_ServantLocator_Cookie the_cookie,
                                              PortableServer_Servant the_servant,
                                              CORBA_Environment* ev) {
  PortableServer_Servant s;
  rustorb_c_ev_clear(ev);
  s = impl_of(obj, &locator_class, ev);
  if (s && locator_epv(s) && locator_epv(s)->postinvoke)
    locator_epv(s)->postinvoke(s, oid, adapter, operation, the_cookie, the_servant, ev);
}

CORBA_boolean PortableServer_AdapterActivator_unknown_adapter(PortableServer_AdapterActivator obj,
                                                              PortableServer_POA parent,
                                                              const CORBA_char* name,
                                                              CORBA_Environment* ev) {
  PortableServer_Servant s;
  rustorb_c_ev_clear(ev);
  s = impl_of(obj, &adapter_activator_class, ev);
  if (!s || !adapter_activator_epv(s) || !adapter_activator_epv(s)->unknown_adapter)
    return CORBA_FALSE;
  return adapter_activator_epv(s)->unknown_adapter(s, parent, name, ev);
}

/* --- the callbacks of the POA ------------------------------------------------------------ */

/* An ObjectId over the bytes of the callback (borrowed). */
static PortableServer_ObjectId borrowed_id(const uint8_t* id, size_t len) {
  PortableServer_ObjectId o;
  memset(&o, 0, sizeof o);
  o._maximum = o._length = (CORBA_unsigned_long)len;
  o._buffer = (CORBA_octet*)id;
  return o;
}

/* What the implementation set in ev, as the result of incarnate or
   preinvoke. */
static rorb_status provide(PortableServer_Servant servant, CORBA_Environment* ev,
                           rorb_servant** out, rorb_manager_error* error) {
  if (ev->_major == CORBA_USER_EXCEPTION && ev->_id &&
      strcmp(ev->_id, ex_PortableServer_ForwardRequest) == 0) {
    PortableServer_ForwardRequest* f = (PortableServer_ForwardRequest*)ev->_params;
    if (f && f->forward_reference && f->forward_reference->kind == OBJ_REF)
      rorb_manager_error_forward(error, (rorb_object*)f->forward_reference->handle);
    else
      rorb_manager_error_system(error, ex_CORBA_BAD_PARAM, strlen(ex_CORBA_BAD_PARAM), 0,
                                CORBA_COMPLETED_NO);
    CORBA_exception_free(ev);
    return RORB_SYSTEM_EXCEPTION;
  }
  if (ev->_major == CORBA_SYSTEM_EXCEPTION) {
    CORBA_SystemException* v = (CORBA_SystemException*)ev->_params;
    const char* id = ev->_id ? ev->_id : ex_CORBA_UNKNOWN;
    rorb_manager_error_system(error, id, strlen(id), v ? v->minor : 0,
                              v ? (uint32_t)v->completed : CORBA_COMPLETED_MAYBE);
    CORBA_exception_free(ev);
    return RORB_SYSTEM_EXCEPTION;
  }
  if (ev->_major != CORBA_NO_EXCEPTION) {
    /* Another user exception: UNKNOWN. */
    CORBA_exception_free(ev);
    return RORB_CALLBACK_FAILED;
  }
  *out = servant ? rustorb_c_servant_ref(servant) : NULL;
  return RORB_OK;
}

static rorb_status incarnate_cb(void* self, const uint8_t* id, size_t id_len, rorb_poa* adapter,
                                rorb_servant** servant, rorb_manager_error* error) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_Environment ev2 = {CORBA_NO_EXCEPTION, NULL, NULL};
  PortableServer_ObjectId oid = borrowed_id(id, id_len);
  PortableServer_POA poa = rustorb_c_wrap(OBJ_POA, rorb_poa_clone(adapter));
  POA_PortableServer_ServantActivator__epv* e = activator_epv(self);
  PortableServer_Servant s = NULL;
  rorb_status st;
  if (e && e->incarnate)
    s = e->incarnate(self, &oid, poa, &ev);
  else
    rustorb_c_ev_system(&ev, ex_CORBA_NO_IMPLEMENT, 0, CORBA_COMPLETED_NO);
  st = provide(s, &ev, servant, error);
  CORBA_Object_release(poa, &ev2);
  return st;
}

static void etherealize_cb(void* self, const uint8_t* id, size_t id_len, rorb_poa* adapter,
                           void* servant, int32_t cleanup_in_progress,
                           int32_t remaining_activations) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  PortableServer_ObjectId oid = borrowed_id(id, id_len);
  PortableServer_POA poa;
  POA_PortableServer_ServantActivator__epv* e = activator_epv(self);
  if (!e || !e->etherealize) return;
  poa = rustorb_c_wrap(OBJ_POA, rorb_poa_clone(adapter));
  e->etherealize(self, &oid, poa, servant, cleanup_in_progress ? CORBA_TRUE : CORBA_FALSE,
                 remaining_activations ? CORBA_TRUE : CORBA_FALSE, &ev);
  CORBA_exception_free(&ev);
  CORBA_Object_release(poa, &ev);
}

static rorb_status preinvoke_cb(void* self, const uint8_t* id, size_t id_len, rorb_poa* adapter,
                                const char* op, size_t op_len, rorb_servant** servant,
                                void** cookie, rorb_manager_error* error) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_Environment ev2 = {CORBA_NO_EXCEPTION, NULL, NULL};
  PortableServer_ObjectId oid = borrowed_id(id, id_len);
  PortableServer_POA poa = rustorb_c_wrap(OBJ_POA, rorb_poa_clone(adapter));
  POA_PortableServer_ServantLocator__epv* e = locator_epv(self);
  char* name = rustorb_c_strndup(op, op_len);
  PortableServer_Servant s = NULL;
  rorb_status st;
  *cookie = NULL;
  if (e && e->preinvoke)
    s = e->preinvoke(self, &oid, poa, name, cookie, &ev);
  else
    rustorb_c_ev_system(&ev, ex_CORBA_NO_IMPLEMENT, 0, CORBA_COMPLETED_NO);
  st = provide(s, &ev, servant, error);
  CORBA_free(name);
  CORBA_Object_release(poa, &ev2);
  return st;
}

static void postinvoke_cb(void* self, const uint8_t* id, size_t id_len, rorb_poa* adapter,
                          const char* op, size_t op_len, void* cookie, void* servant) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  PortableServer_ObjectId oid = borrowed_id(id, id_len);
  PortableServer_POA poa;
  POA_PortableServer_ServantLocator__epv* e = locator_epv(self);
  char* name;
  if (!e || !e->postinvoke) return;
  poa = rustorb_c_wrap(OBJ_POA, rorb_poa_clone(adapter));
  name = rustorb_c_strndup(op, op_len);
  e->postinvoke(self, &oid, poa, name, cookie, servant, &ev);
  CORBA_exception_free(&ev);
  CORBA_free(name);
  CORBA_Object_release(poa, &ev);
}

static int32_t unknown_adapter_cb(void* self, rorb_poa* parent, const char* name, size_t len) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  POA_PortableServer_AdapterActivator__epv* e = adapter_activator_epv(self);
  PortableServer_POA poa;
  char* n;
  CORBA_boolean r;
  if (!e || !e->unknown_adapter) return 0;
  poa = rustorb_c_wrap(OBJ_POA, rorb_poa_clone(parent));
  n = rustorb_c_strndup(name, len);
  r = e->unknown_adapter(self, poa, n, &ev);
  if (ev._major != CORBA_NO_EXCEPTION) r = CORBA_FALSE;
  CORBA_exception_free(&ev);
  CORBA_free(n);
  CORBA_Object_release(poa, &ev);
  return r ? 1 : 0;
}

/* The implementations are the application's: borrowed while the POA has
   them. */
static void no_ref(void* self) { (void)self; }

static const rorb_servant_activator_vtable activator_vtable = {incarnate_cb, etherealize_cb,
                                                               no_ref, no_ref};
static const rorb_servant_locator_vtable locator_vtable = {preinvoke_cb, postinvoke_cb, no_ref,
                                                           no_ref};
static const rorb_adapter_activator_vtable adapter_activator_vtable = {unknown_adapter_cb, no_ref,
                                                                       no_ref};

static rorb_poa* poa_of(PortableServer_POA poa, CORBA_Environment* ev) {
  if (!poa || poa->kind != OBJ_POA) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  return (rorb_poa*)poa->handle;
}

void PortableServer_POA_set_servant_manager(PortableServer_POA poa,
                                            PortableServer_ServantManager imgr,
                                            CORBA_Environment* ev) {
  rorb_poa* h;
  const rustorb_c_class* cls;
  rorb_exception exc;
  rustorb_c_ev_clear(ev);
  if (!(h = poa_of(poa, ev))) return;
  RUSTORB_C_ARG_CHECK(ev, imgr && imgr->kind == OBJ_LOCAL, ex_CORBA_OBJ_ADAPTER);
  cls = rustorb_c_servant_class(imgr->handle);
  memset(&exc, 0, sizeof exc);
  if (cls == &activator_class)
    rustorb_c_failed(ev, rorb_poa_set_servant_activator(h, imgr->handle, &activator_vtable, &exc),
                     &exc);
  else if (cls == &locator_class)
    rustorb_c_failed(ev, rorb_poa_set_servant_locator(h, imgr->handle, &locator_vtable, &exc),
                     &exc);
  else
    rustorb_c_ev_system(ev, ex_CORBA_OBJ_ADAPTER, 0, CORBA_COMPLETED_NO);
}

PortableServer_ServantManager PortableServer_POA_get_servant_manager(PortableServer_POA poa,
                                                                     CORBA_Environment* ev) {
  rorb_poa* h;
  int32_t kind = 0;
  void* self = NULL;
  rorb_exception exc;
  rustorb_c_ev_clear(ev);
  if (!(h = poa_of(poa, ev))) return CORBA_OBJECT_NIL;
  memset(&exc, 0, sizeof exc);
  if (rustorb_c_failed(ev, rorb_poa_get_servant_manager(h, &kind, &self, &exc), &exc) || !self)
    return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_LOCAL, self);
}

void PortableServer_POA__set_the_activator(PortableServer_POA poa,
                                           PortableServer_AdapterActivator activator,
                                           CORBA_Environment* ev) {
  rorb_poa* h;
  rorb_exception exc;
  rustorb_c_ev_clear(ev);
  if (!(h = poa_of(poa, ev))) return;
  memset(&exc, 0, sizeof exc);
  if (!activator) {
    rustorb_c_failed(ev, rorb_poa_set_the_activator(h, NULL, &adapter_activator_vtable, &exc),
                     &exc);
    return;
  }
  if (!impl_of(activator, &adapter_activator_class, ev)) return;
  rustorb_c_failed(ev,
                   rorb_poa_set_the_activator(h, activator->handle, &adapter_activator_vtable,
                                              &exc),
                   &exc);
}

PortableServer_AdapterActivator PortableServer_POA__get_the_activator(PortableServer_POA poa,
                                                                      CORBA_Environment* ev) {
  rorb_poa* h;
  void* self;
  rustorb_c_ev_clear(ev);
  if (!(h = poa_of(poa, ev))) return CORBA_OBJECT_NIL;
  self = rorb_poa_the_activator(h);
  return self ? rustorb_c_wrap(OBJ_LOCAL, self) : CORBA_OBJECT_NIL;
}
