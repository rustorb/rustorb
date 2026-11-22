/* See dii_adapters.h.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include "dii_adapters.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int dii_enabled(void) {
  const char* v = getenv("RUSTORB_DII");
  return v && *v && strcmp(v, "0") != 0;
}

/* --- the client side ------------------------------------------------------------- */

static const rustorb_c_module* modules[8];
static int module_count;
static int use_deferred;

static int mine(const rustorb_c_operation* op) {
  int i;
  for (i = 0; i < module_count; i++)
    if (modules[i] == op->module) return 1;
  return 0;
}

static CORBA_TypeCode tc_of(const rustorb_c_type* t, const rustorb_c_operation* op) {
  return rustorb_c_type_tc(t, op->module);
}

static int call(CORBA_Object obj, const rustorb_c_operation* op, void** args, void* result,
                CORBA_Environment* ev) {
  CORBA_Request req = NULL;
  CORBA_NVList list;
  CORBA_NamedValue* res;
  CORBA_Environment ev2 = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_unsigned_long i;
  char name[16];
  if (!obj || !mine(op)) return 0;
  /* Out storage starts empty, as with the stubs. */
  for (i = 0; i < op->param_count; i++) {
    const rustorb_c_param* p = &op->params[i];
    if (p->mode != RUSTORB_C_OUT) continue;
    if (p->indirect)
      *(void**)args[i] = NULL;
    else
      memset(args[i], 0, p->type->size);
  }
  if (op->result) {
    if (op->result_indirect)
      *(void**)result = NULL;
    else
      memset(result, 0, op->result->size);
  }
  CORBA_Object_create_request(obj, NULL, op->name, NULL, NULL, &req, 0, ev);
  if (ev->_major != CORBA_NO_EXCEPTION) return 1;
  for (i = 0; i < op->param_count; i++) {
    const rustorb_c_param* p = &op->params[i];
    CORBA_Flags mode = p->mode == RUSTORB_C_IN    ? CORBA_ARG_IN
                       : p->mode == RUSTORB_C_OUT ? CORBA_ARG_OUT
                                                  : CORBA_ARG_INOUT;
    snprintf(name, sizeof name, "p%u", (unsigned)i);
    CORBA_Request_add_arg(req, name, tc_of(p->type, op), p->indirect ? NULL : args[i], 0, mode,
                          ev);
    if (ev->_major != CORBA_NO_EXCEPTION) goto done;
  }
  for (i = 0; i < op->exception_count; i++) {
    CORBA_Request_add_exception(req, tc_of(op->exceptions[i], op), ev);
    if (ev->_major != CORBA_NO_EXCEPTION) goto done;
  }
  res = CORBA_Request_result(req, ev);
  if (op->result) {
    res->argument._type = tc_of(op->result, op);
    res->argument._value = op->result_indirect ? NULL : result;
  }
  if (op->oneway) {
    CORBA_Request_send(req, CORBA_INV_NO_RESPONSE, ev);
    goto done;
  }
  if (use_deferred) {
    CORBA_Request_send(req, 0, ev);
    if (ev->_major == CORBA_NO_EXCEPTION) CORBA_Request_get_response(req, 0, ev);
  } else {
    CORBA_Request_invoke(req, 0, ev);
  }
  if (ev->_major != CORBA_NO_EXCEPTION) goto done;
  /* Take the values the ORB allocated for the indirect ones. */
  list = CORBA_Request_arguments(req, &ev2);
  for (i = 0; i < op->param_count; i++) {
    const rustorb_c_param* p = &op->params[i];
    CORBA_NamedValue* nv;
    if (!p->indirect) continue;
    nv = CORBA_NVList_item(list, i, &ev2);
    *(void**)args[i] = nv->argument._value;
    nv->argument._value = NULL;
    nv->argument._release = CORBA_FALSE;
  }
  if (op->result && op->result_indirect) {
    *(void**)result = res->argument._value;
    res->argument._value = NULL;
    res->argument._release = CORBA_FALSE;
  }
done:
  if (op->result && !op->result_indirect) {
    /* The result storage is the caller's: not the request's to free. */
    res = CORBA_Request_result(req, &ev2);
    if (res) res->argument._value = NULL;
  }
  CORBA_Request_delete(req, &ev2);
  return 1;
}

void dii_client_install(const rustorb_c_module* module, int deferred) {
  if (module_count < 8) modules[module_count++] = module;
  use_deferred = deferred;
  rustorb_c_set_invoke_hook(call);
}

void dii_client_uninstall(void) {
  rustorb_c_set_invoke_hook(NULL);
  module_count = 0;
}

/* --- the server side ------------------------------------------------------------- */

typedef struct dsi_servant {
  POA_PortableServer_DynamicImpl base;
  PortableServer_Servant target;
  const rustorb_c_class* cls;
} dsi_servant;

static CORBA_RepositoryId primary_interface(PortableServer_Servant servant,
                                            PortableServer_ObjectId* oid, PortableServer_POA poa,
                                            CORBA_Environment* ev) {
  (void)oid;
  (void)poa;
  (void)ev;
  return CORBA_string_dup(((dsi_servant*)servant)->cls->ids[0]);
}

static void invoke(PortableServer_Servant servant, CORBA_ServerRequest sr) {
  dsi_servant* s = (dsi_servant*)servant;
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_Environment uev = {CORBA_NO_EXCEPTION, NULL, NULL};
  const char* opname = CORBA_ServerRequest_operation(sr, &ev);
  const rustorb_c_entry* entry = NULL;
  const rustorb_c_operation* op;
  CORBA_NVList list = NULL;
  void* cells[16];
  void* indirect[16];
  void* result = NULL;
  CORBA_unsigned_long i;
  char name[16];
  for (i = 0; i < s->cls->entry_count; i++)
    if (strcmp(s->cls->entries[i].op->name, opname) == 0) entry = &s->cls->entries[i];
  if (!entry || entry->op->param_count > 16) {
    CORBA_ServerRequest_set_system_exception(sr, ex_CORBA_BAD_OPERATION, 0x4f4d0002u,
                                             CORBA_COMPLETED_NO, &ev);
    return;
  }
  op = entry->op;
  CORBA_ORB_create_list(CORBA_OBJECT_NIL, (CORBA_long)op->param_count, &list, &ev);
  for (i = 0; i < op->param_count; i++) {
    const rustorb_c_param* p = &op->params[i];
    CORBA_Flags mode = p->mode == RUSTORB_C_IN    ? CORBA_ARG_IN
                       : p->mode == RUSTORB_C_OUT ? CORBA_ARG_OUT
                                                  : CORBA_ARG_INOUT;
    snprintf(name, sizeof name, "p%u", (unsigned)i);
    CORBA_NVList_add_item(list, name, rustorb_c_type_tc(p->type, op->module), NULL, 0, mode, &ev);
  }
  CORBA_ServerRequest_arguments(sr, list, &ev);
  if (ev._major != CORBA_NO_EXCEPTION) {
    CORBA_SystemException* v = (CORBA_SystemException*)ev._params;
    CORBA_ServerRequest_set_system_exception(sr, ev._id, v ? v->minor : 0, CORBA_COMPLETED_NO,
                                             &uev);
    CORBA_exception_free(&ev);
    return;
  }
  /* The argument cells: the values the ORB read (or storage for the out
     ones), and for an indirect out one the T* the skeleton sets. */
  for (i = 0; i < op->param_count; i++) {
    const rustorb_c_param* p = &op->params[i];
    CORBA_NamedValue* nv = CORBA_NVList_item(list, i, &ev);
    indirect[i] = NULL;
    if (p->indirect) {
      cells[i] = &indirect[i];
    } else {
      if (!nv->argument._value) {
        nv->argument._value = rustorb_c_alloc(p->type, 1);
        nv->argument._release = CORBA_TRUE;
      }
      cells[i] = nv->argument._value;
    }
  }
  if (op->result)
    result = calloc(1, op->result_indirect ? sizeof(void*) : op->result->size);
  entry->skel(s->target, cells, result, &uev);
  if (uev._major == CORBA_USER_EXCEPTION) {
    CORBA_any a;
    CORBA_unsigned_long k;
    memset(&a, 0, sizeof a);
    for (k = 0; k < op->exception_count; k++)
      if (strcmp(op->exceptions[k]->id, uev._id) == 0)
        a._type = rustorb_c_type_tc(op->exceptions[k], op->module);
    if (a._type) {
      static CORBA_octet nothing;
      a._value = uev._params ? uev._params : &nothing;
      CORBA_ServerRequest_set_exception(sr, CORBA_USER_EXCEPTION, &a, &ev);
    } else {
      CORBA_ServerRequest_set_system_exception(sr, ex_CORBA_UNKNOWN, 0, CORBA_COMPLETED_MAYBE,
                                               &ev);
    }
  } else if (uev._major == CORBA_SYSTEM_EXCEPTION) {
    CORBA_SystemException* v = (CORBA_SystemException*)uev._params;
    CORBA_ServerRequest_set_system_exception(sr, uev._id, v ? v->minor : 0,
                                             v ? v->completed : CORBA_COMPLETED_MAYBE, &ev);
  } else {
    for (i = 0; i < op->param_count; i++) {
      CORBA_NamedValue* nv = CORBA_NVList_item(list, i, &ev);
      if (!op->params[i].indirect) continue;
      /* In place of the storage the ORB made. */
      if (nv->argument._release) CORBA_free(nv->argument._value);
      nv->argument._value = indirect[i];
      nv->argument._release = CORBA_TRUE;
      indirect[i] = NULL;
    }
    if (op->result) {
      CORBA_any a;
      memset(&a, 0, sizeof a);
      a._type = rustorb_c_type_tc(op->result, op->module);
      a._value = op->result_indirect ? *(void**)result : result;
      if (a._value) CORBA_ServerRequest_set_result(sr, &a, &ev);
      if (ev._major != CORBA_NO_EXCEPTION) {
        CORBA_ServerRequest_set_system_exception(sr, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_YES,
                                                 &uev);
        CORBA_exception_free(&ev);
      }
    }
  }
  for (i = 0; i < op->param_count; i++) CORBA_free(indirect[i]);
  if (result) {
    if (op->result_indirect)
      CORBA_free(*(void**)result);
    else
      rustorb_c_free_value(op->result, result);
    free(result);
  }
  CORBA_exception_free(&uev);
}

static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static PortableServer_DynamicImpl__epv dsi_epv = {NULL, invoke, primary_interface};
static POA_PortableServer_DynamicImpl__vepv vepv = {&base_epv, &dsi_epv};

PortableServer_Servant dii_servant_new(PortableServer_Servant target, CORBA_Environment* ev) {
  dsi_servant* s = (dsi_servant*)calloc(1, sizeof *s);
  if (!s) return NULL;
  s->base.vepv = &vepv;
  s->target = target;
  s->cls = rustorb_c_servant_class(target);
  if (!s->cls) {
    free(s);
    CORBA_exception_set_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  PortableServer_DynamicImpl__init(s, ev);
  if (ev->_major != CORBA_NO_EXCEPTION) {
    free(s);
    return NULL;
  }
  return s;
}

void dii_servant_free(PortableServer_Servant servant) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  if (!servant) return;
  PortableServer_DynamicImpl__fini(servant, &ev);
  free(servant);
}
