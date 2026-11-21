/* The DII and the DSI (rustorb/dii.h) on rorb_request and
   rorb_server_request. */

#include <rustorb/dii.h>

#include "internal.h"

/* --- NVList ------------------------------------------------------------------- */

struct CORBA_NVList_struct {
  CORBA_unsigned_long count;
  CORBA_unsigned_long capacity;
  CORBA_NamedValue* items;
};

static const rustorb_c_type* desc_of(CORBA_TypeCode tc) {
  if (!tc) return NULL;
  return tc->desc ? tc->desc : rustorb_c_desc_for_tc(rustorb_c_tc_handle(tc));
}

static CORBA_NVList list_new(void) {
  return (CORBA_NVList)calloc(1, sizeof(struct CORBA_NVList_struct));
}

void CORBA_ORB_create_list(CORBA_ORB orb, CORBA_long count, CORBA_NVList* new_list,
                           CORBA_Environment* ev) {
  (void)orb;
  (void)count;
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, new_list, ex_CORBA_BAD_PARAM);
  *new_list = list_new();
  if (!*new_list) rustorb_c_ev_system(ev, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
}

void CORBA_NVList_add_item(CORBA_NVList list, const CORBA_char* item_name,
                           CORBA_TypeCode item_type, void* value, CORBA_long value_len,
                           CORBA_Flags item_flags, CORBA_Environment* ev) {
  CORBA_NamedValue* nv;
  CORBA_Flags mode = item_flags & 3u;
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, list && item_type && rustorb_c_tc_handle(item_type) && mode != 0,
                      ex_CORBA_BAD_PARAM);
  if (list->count == list->capacity) {
    CORBA_unsigned_long cap = list->capacity ? list->capacity * 2 : 4;
    CORBA_NamedValue* items = (CORBA_NamedValue*)realloc(list->items, cap * sizeof *items);
    if (!items) {
      rustorb_c_ev_system(ev, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
      return;
    }
    list->items = items;
    list->capacity = cap;
  }
  nv = &list->items[list->count];
  memset(nv, 0, sizeof *nv);
  nv->name = CORBA_string_dup(item_name ? item_name : "");
  nv->len = value_len;
  nv->arg_modes = mode;
  nv->argument._type = CORBA_Object_duplicate(item_type, ev);
  if (value && (item_flags & CORBA_IN_COPY_VALUE) && mode != CORBA_ARG_OUT) {
    const rustorb_c_type* t = desc_of(item_type);
    if (!t) {
      CORBA_free(nv->name);
      CORBA_Object_release(nv->argument._type, ev);
      rustorb_c_ev_system(ev, ex_CORBA_BAD_TYPECODE, 0, CORBA_COMPLETED_NO);
      return;
    }
    nv->argument._value = rustorb_c_copy(t, value);
    nv->argument._release = CORBA_TRUE;
  } else {
    nv->argument._value = value;
  }
  list->count++;
}

void CORBA_NVList_get_count(CORBA_NVList list, CORBA_long* count, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, list && count, ex_CORBA_BAD_PARAM);
  *count = (CORBA_long)list->count;
}

CORBA_NamedValue* CORBA_NVList_item(CORBA_NVList list, CORBA_unsigned_long index,
                                    CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (!list || index >= list->count) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  return &list->items[index];
}

/* Frees what the any owns, keeping its type. */
static void free_owned(CORBA_any* a) {
  if (a->_release && a->_value) CORBA_free(a->_value);
  a->_value = NULL;
  a->_release = CORBA_FALSE;
  if (a->_rustorb) rorb_any_release((rorb_any*)a->_rustorb);
  a->_rustorb = NULL;
}

void CORBA_NVList_free_memory(CORBA_NVList list, CORBA_Environment* ev) {
  CORBA_unsigned_long i;
  rustorb_c_ev_clear(ev);
  if (!list) return;
  for (i = 0; i < list->count; i++)
    if (list->items[i].argument._release || list->items[i].argument._rustorb)
      free_owned(&list->items[i].argument);
}

void CORBA_NVList_free(CORBA_NVList list, CORBA_Environment* ev) {
  CORBA_unsigned_long i;
  rustorb_c_ev_clear(ev);
  if (!list) return;
  for (i = 0; i < list->count; i++) {
    CORBA_free(list->items[i].name);
    rustorb_c_free_value(&CORBA_any__desc, &list->items[i].argument);
  }
  free(list->items);
  free(list);
}

/* The rorb_any of the value of a; NULL with *err set. */
static rorb_any* raw_of(const CORBA_any* a, rustorb_c_err* err) {
  if (!a->_value && !a->_rustorb) {
    rorb_typecode* tc = rustorb_c_tc_handle(a->_type);
    uint32_t kind = tc ? rorb_tc_kind(tc) : CORBA_tk_null;
    if (kind != CORBA_tk_null && kind != CORBA_tk_void) {
      err->id = ex_CORBA_BAD_PARAM;
      err->minor = 0;
      return NULL;
    }
  }
  return rustorb_c_any_to_raw(a, err);
}

/* Puts the value of raw into a (see rustorb/dii.h): 0, or -1. */
static int receive(CORBA_any* a, const rorb_any* raw, int inout) {
  const rustorb_c_type* t = desc_of(a->_type);
  CORBA_any got;
  if (a->_value && t) {
    if (inout) rustorb_c_free_value(t, a->_value);
    memset(a->_value, 0, t->size);
    return rustorb_c_raw_value(raw, t, a->_value);
  }
  free_owned(a);
  if (rustorb_c_any_from_raw(raw, &got)) return -1;
  rustorb_c_free_value(&CORBA_any__desc, a);
  *a = got;
  return 0;
}

/* --- Request ------------------------------------------------------------------ */

struct CORBA_Request_struct {
  CORBA_Object target;
  char* operation;
  CORBA_NVList args;
  CORBA_NamedValue* result;
  CORBA_NamedValue own_result;
  CORBA_TypeCode* exceptions;
  CORBA_unsigned_long exception_count;
  /* The call sent with CORBA_Request_send, if any. */
  rorb_request* pending;
};

void CORBA_Object_create_request(CORBA_Object obj, CORBA_Context ctx,
                                 const CORBA_char* operation, CORBA_NVList arg_list,
                                 CORBA_NamedValue* result, CORBA_Request* request,
                                 CORBA_Flags req_flags, CORBA_Environment* ev) {
  CORBA_Request r;
  (void)req_flags;
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, request && operation, ex_CORBA_BAD_PARAM);
  *request = NULL;
  RUSTORB_C_ARG_CHECK(ev, !ctx, ex_CORBA_NO_IMPLEMENT);
  RUSTORB_C_ARG_CHECK(ev, obj, ex_CORBA_INV_OBJREF);
  RUSTORB_C_ARG_CHECK(ev, obj->kind == OBJ_REF, ex_CORBA_NO_IMPLEMENT);
  r = (CORBA_Request)calloc(1, sizeof *r);
  if (!r) {
    rustorb_c_ev_system(ev, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
    return;
  }
  r->target = CORBA_Object_duplicate(obj, ev);
  r->operation = CORBA_string_dup(operation);
  r->args = arg_list ? arg_list : list_new();
  r->result = result ? result : &r->own_result;
  if (!r->result->argument._type) r->result->argument._type = TC_void;
  *request = r;
}

void CORBA_Request_add_arg(CORBA_Request req, const CORBA_char* name, CORBA_TypeCode arg_type,
                           void* value, CORBA_long len, CORBA_Flags arg_flags,
                           CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, req, ex_CORBA_BAD_PARAM);
  CORBA_NVList_add_item(req->args, name, arg_type, value, len, arg_flags, ev);
}

void CORBA_Request_add_exception(CORBA_Request req, CORBA_TypeCode exception_type,
                                 CORBA_Environment* ev) {
  CORBA_TypeCode* e;
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, req && exception_type && rustorb_c_tc_handle(exception_type),
                      ex_CORBA_BAD_PARAM);
  e = (CORBA_TypeCode*)realloc(req->exceptions, (req->exception_count + 1) * sizeof *e);
  if (!e) {
    rustorb_c_ev_system(ev, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
    return;
  }
  req->exceptions = e;
  e[req->exception_count++] = CORBA_Object_duplicate(exception_type, ev);
}

CORBA_NVList CORBA_Request_arguments(CORBA_Request req, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  return req ? req->args : NULL;
}

CORBA_NamedValue* CORBA_Request_result(CORBA_Request req, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  return req ? req->result : NULL;
}

/* A rorb_request with the arguments, return type and exceptions of req. */
static rorb_request* build(CORBA_Request req, CORBA_Environment* ev) {
  rorb_request* h = NULL;
  rorb_exception exc;
  CORBA_unsigned_long i;
  memset(&exc, 0, sizeof exc);
  if (rustorb_c_failed(ev, rorb_request_new((rorb_object*)req->target->handle, req->operation,
                                             strlen(req->operation), &h, &exc),
                       &exc))
    return NULL;
  for (i = 0; i < req->args->count; i++) {
    CORBA_NamedValue* nv = &req->args->items[i];
    rustorb_c_err err = {NULL, 0};
    rorb_any* raw = nv->arg_modes == CORBA_ARG_OUT ? rorb_any_typed(rustorb_c_tc_handle(nv->argument._type))
                                                   : raw_of(&nv->argument, &err);
    rorb_status st;
    if (!raw) {
      rustorb_c_ev_system(ev, err.id ? err.id : ex_CORBA_BAD_PARAM, err.minor, CORBA_COMPLETED_NO);
      rorb_request_release(h);
      return NULL;
    }
    st = rorb_request_add_arg(h, nv->name, strlen(nv->name), nv->arg_modes, raw, &exc);
    rorb_any_release(raw);
    if (rustorb_c_failed(ev, st, &exc)) {
      rorb_request_release(h);
      return NULL;
    }
  }
  rorb_request_set_return_type(h, rustorb_c_tc_handle(req->result->argument._type));
  for (i = 0; i < req->exception_count; i++)
    rorb_request_add_exception(h, rustorb_c_tc_handle(req->exceptions[i]));
  return h;
}

/* Takes the outcome st of the call h into req and ev. */
static void finish(CORBA_Request req, rorb_request* h, rorb_status st, rorb_exception* exc,
                   CORBA_Environment* ev) {
  CORBA_unsigned_long i;
  if (st == RORB_USER_EXCEPTION) {
    rorb_any* raw = rorb_request_exception(h);
    rorb_typecode* tc = raw ? rorb_any_type(raw) : NULL;
    CORBA_any got;
    const char* id = NULL;
    size_t len = 0;
    rorb_exception e2;
    memset(&e2, 0, sizeof e2);
    rorb_exception_clear(exc);
    if (!raw || rorb_tc_id(tc, &id, &len, &e2) != RORB_OK || rustorb_c_any_from_raw(raw, &got)) {
      rorb_exception_clear(&e2);
      rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_YES);
    } else {
      char* rid = rustorb_c_strndup(id, len);
      /* The value as the generated struct; NULL without generated code. */
      void* v = got._value;
      got._value = NULL;
      rustorb_c_free_value(&CORBA_any__desc, &got);
      CORBA_exception_set(ev, CORBA_USER_EXCEPTION, rid, v);
      CORBA_free(rid);
    }
    if (tc) rorb_tc_release(tc);
    rorb_any_release(raw);
    rorb_request_release(h);
    return;
  }
  if (rustorb_c_failed(ev, st, exc)) {
    rorb_request_release(h);
    return;
  }
  for (i = 0; i < req->args->count; i++) {
    CORBA_NamedValue* nv = &req->args->items[i];
    rorb_any* raw;
    int bad;
    if (nv->arg_modes == CORBA_ARG_IN) continue;
    raw = rorb_request_argument(h, i);
    bad = !raw || receive(&nv->argument, raw, nv->arg_modes == CORBA_ARG_INOUT);
    rorb_any_release(raw);
    if (bad) {
      rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_YES);
      rorb_request_release(h);
      return;
    }
  }
  {
    rorb_typecode* rt = rustorb_c_tc_handle(req->result->argument._type);
    if (rt && rorb_tc_kind(rt) != CORBA_tk_void) {
      rorb_any* raw = rorb_request_result(h);
      if (!raw || receive(&req->result->argument, raw, 0))
        rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_YES);
      rorb_any_release(raw);
    }
  }
  rorb_request_release(h);
}

void CORBA_Request_invoke(CORBA_Request req, CORBA_Flags invoke_flags, CORBA_Environment* ev) {
  rorb_request* h;
  rorb_exception exc;
  rorb_status st;
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, req, ex_CORBA_BAD_PARAM);
  if (invoke_flags & CORBA_INV_NO_RESPONSE) {
    CORBA_Request_send(req, invoke_flags, ev);
    return;
  }
  RUSTORB_C_ARG_CHECK(ev, !req->pending, ex_CORBA_BAD_INV_ORDER);
  h = build(req, ev);
  if (!h) return;
  memset(&exc, 0, sizeof exc);
  st = rorb_request_invoke(h, &exc);
  finish(req, h, st, &exc, ev);
}

void CORBA_Request_send(CORBA_Request req, CORBA_Flags invoke_flags, CORBA_Environment* ev) {
  rorb_request* h;
  rorb_exception exc;
  rorb_status st;
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, req, ex_CORBA_BAD_PARAM);
  RUSTORB_C_ARG_CHECK(ev, !req->pending, ex_CORBA_BAD_INV_ORDER);
  h = build(req, ev);
  if (!h) return;
  memset(&exc, 0, sizeof exc);
  if (invoke_flags & CORBA_INV_NO_RESPONSE) {
    st = rorb_request_send_oneway(h, &exc);
    rustorb_c_failed(ev, st, &exc);
    rorb_request_release(h);
    return;
  }
  st = rorb_request_send_deferred(h, &exc);
  if (rustorb_c_failed(ev, st, &exc)) {
    rorb_request_release(h);
    return;
  }
  req->pending = h;
}

CORBA_boolean CORBA_Request_poll_response(CORBA_Request req, CORBA_Environment* ev) {
  int32_t ready = 0;
  rorb_exception exc;
  rustorb_c_ev_clear(ev);
  if (!req || !req->pending) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_INV_ORDER, 0, CORBA_COMPLETED_NO);
    return CORBA_FALSE;
  }
  memset(&exc, 0, sizeof exc);
  if (rustorb_c_failed(ev, rorb_request_poll_response(req->pending, &ready, &exc), &exc))
    return CORBA_FALSE;
  return ready ? CORBA_TRUE : CORBA_FALSE;
}

void CORBA_Request_get_response(CORBA_Request req, CORBA_Flags response_flags,
                                CORBA_Environment* ev) {
  rorb_request* h;
  rorb_exception exc;
  rorb_status st;
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, req && req->pending, ex_CORBA_BAD_INV_ORDER);
  if ((response_flags & CORBA_RESP_NO_WAIT) && !CORBA_Request_poll_response(req, ev)) return;
  if (ev->_major != CORBA_NO_EXCEPTION) return;
  h = req->pending;
  req->pending = NULL;
  memset(&exc, 0, sizeof exc);
  st = rorb_request_get_response(h, &exc);
  finish(req, h, st, &exc, ev);
}

void CORBA_Request_delete(CORBA_Request req, CORBA_Environment* ev) {
  CORBA_unsigned_long i;
  rustorb_c_ev_clear(ev);
  if (!req) return;
  if (req->pending) rorb_request_release(req->pending);
  CORBA_NVList_free(req->args, ev);
  rustorb_c_free_value(&CORBA_any__desc, &req->own_result.argument);
  for (i = 0; i < req->exception_count; i++) CORBA_Object_release(req->exceptions[i], ev);
  free(req->exceptions);
  CORBA_free(req->operation);
  CORBA_Object_release(req->target, ev);
  free(req);
}

/* --- ServerRequest -------------------------------------------------------------- */

struct CORBA_ServerRequest_struct {
  rorb_server_request* h;
  char* operation;
  CORBA_NVList params;
  /* A system exception set by the servant. */
  const char* sys_id;
  char* sys_id_owned;
  CORBA_unsigned_long sys_minor;
  CORBA_completion_status sys_completed;
  /* An error to reply with instead (BAD_PARAM for a wrong result...). */
  rustorb_c_err err;
};

CORBA_Identifier CORBA_ServerRequest_operation(CORBA_ServerRequest req, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  return req ? req->operation : NULL;
}

void CORBA_ServerRequest_arguments(CORBA_ServerRequest req, CORBA_NVList parameters,
                                   CORBA_Environment* ev) {
  rorb_exception exc;
  CORBA_unsigned_long i;
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, req && parameters, ex_CORBA_BAD_PARAM);
  RUSTORB_C_ARG_CHECK(ev, !req->params, ex_CORBA_BAD_INV_ORDER);
  memset(&exc, 0, sizeof exc);
  req->params = parameters;
  for (i = 0; i < parameters->count; i++) {
    CORBA_NamedValue* nv = &parameters->items[i];
    if (rustorb_c_failed(ev,
                         rorb_server_request_add_param(req->h, nv->name, strlen(nv->name),
                                                       nv->arg_modes,
                                                       rustorb_c_tc_handle(nv->argument._type),
                                                       &exc),
                         &exc))
      return;
  }
  if (rustorb_c_failed(ev, rorb_server_request_arguments(req->h, &exc), &exc)) return;
  for (i = 0; i < parameters->count; i++) {
    CORBA_NamedValue* nv = &parameters->items[i];
    if (nv->arg_modes == CORBA_ARG_OUT) {
      /* Zeroed storage for the servant to set. */
      const rustorb_c_type* t = desc_of(nv->argument._type);
      if (!nv->argument._value && t && t->kind != CORBA_tk_void && t->kind != CORBA_tk_null) {
        nv->argument._value = rustorb_c_alloc(t, 1);
        nv->argument._release = CORBA_TRUE;
      }
      continue;
    }
    rorb_any* raw = rorb_server_request_argument(req->h, i);
    int bad = !raw || receive(&nv->argument, raw, 1);
    rorb_any_release(raw);
    if (bad) {
      rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_NO);
      return;
    }
  }
}

void CORBA_ServerRequest_set_result(CORBA_ServerRequest req, const CORBA_any* value,
                                    CORBA_Environment* ev) {
  rustorb_c_err err = {NULL, 0};
  rorb_any* raw;
  rorb_exception exc;
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, req && value, ex_CORBA_BAD_PARAM);
  RUSTORB_C_ARG_CHECK(ev, req->params, ex_CORBA_BAD_INV_ORDER);
  raw = raw_of(value, &err);
  if (!raw) {
    rustorb_c_ev_system(ev, err.id, err.minor, CORBA_COMPLETED_NO);
    return;
  }
  memset(&exc, 0, sizeof exc);
  rustorb_c_failed(ev, rorb_server_request_set_result(req->h, raw, &exc), &exc);
  rorb_any_release(raw);
}

void CORBA_ServerRequest_set_system_exception(CORBA_ServerRequest req, const CORBA_char* id,
                                              CORBA_unsigned_long minor,
                                              CORBA_completion_status completed,
                                              CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, req && id, ex_CORBA_BAD_PARAM);
  CORBA_free(req->sys_id_owned);
  req->sys_id_owned = CORBA_string_dup(id);
  req->sys_id = req->sys_id_owned;
  req->sys_minor = minor;
  req->sys_completed = completed;
}

void CORBA_ServerRequest_set_exception(CORBA_ServerRequest req, CORBA_exception_type major,
                                       const CORBA_any* value, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev, req && value && value->_type, ex_CORBA_BAD_PARAM);
  if (major == CORBA_SYSTEM_EXCEPTION) {
    CORBA_RepositoryId id = CORBA_TypeCode_id(value->_type, ev);
    const CORBA_SystemException* v = (const CORBA_SystemException*)value->_value;
    if (ev->_major != CORBA_NO_EXCEPTION) return;
    CORBA_ServerRequest_set_system_exception(req, id, v ? v->minor : 0,
                                             v ? v->completed : CORBA_COMPLETED_MAYBE, ev);
    CORBA_free(id);
  } else if (major == CORBA_USER_EXCEPTION) {
    rustorb_c_err err = {NULL, 0};
    rorb_any* raw = raw_of(value, &err);
    rorb_exception exc;
    if (!raw) {
      rustorb_c_ev_system(ev, err.id, err.minor, CORBA_COMPLETED_NO);
      return;
    }
    memset(&exc, 0, sizeof exc);
    rustorb_c_failed(ev, rorb_server_request_set_exception(req->h, raw, &exc), &exc);
    rorb_any_release(raw);
  } else {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
  }
}

static void reply_system(rorb_server_reply* reply, const char* id, CORBA_unsigned_long minor,
                         CORBA_completion_status completed) {
  rorb_server_reply_system_exception(reply, id, strlen(id), minor, completed);
}

/* Sends the out and inout values of the list: 0, or -1 with the reply
   made a system exception. */
static int send_outs(CORBA_ServerRequest req, rorb_server_reply* reply) {
  CORBA_unsigned_long i;
  rorb_exception exc;
  if (!req->params) return 0;
  memset(&exc, 0, sizeof exc);
  for (i = 0; i < req->params->count; i++) {
    CORBA_NamedValue* nv = &req->params->items[i];
    rustorb_c_err err = {NULL, 0};
    rorb_any* raw;
    rorb_status st;
    if (nv->arg_modes == CORBA_ARG_IN) continue;
    raw = raw_of(&nv->argument, &err);
    if (!raw) {
      reply_system(reply, err.id ? err.id : ex_CORBA_BAD_PARAM, err.minor, CORBA_COMPLETED_YES);
      return -1;
    }
    st = rorb_server_request_set_argument(req->h, i, raw, &exc);
    rorb_any_release(raw);
    if (st != RORB_OK) {
      reply_system(reply, exc.repository_id ? exc.repository_id : ex_CORBA_BAD_PARAM,
                   exc.minor, CORBA_COMPLETED_YES);
      rorb_exception_clear(&exc);
      return -1;
    }
  }
  return 0;
}

rorb_status rustorb_c_dsi_dispatch(PortableServer_Servant servant, const char* op, size_t len,
                                   rorb_in* in, rorb_server_reply* reply) {
  PortableServer_DynamicImpl* s = (PortableServer_DynamicImpl*)servant;
  struct CORBA_ServerRequest_struct req;
  rorb_exception exc;
  rorb_status st = RORB_OK;
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  memset(&req, 0, sizeof req);
  memset(&exc, 0, sizeof exc);
  if (!s->vepv || !s->vepv->PortableServer_DynamicImpl_epv ||
      !s->vepv->PortableServer_DynamicImpl_epv->invoke) {
    reply_system(reply, ex_CORBA_NO_IMPLEMENT, 0, CORBA_COMPLETED_NO);
    return RORB_SYSTEM_EXCEPTION;
  }
  if (rorb_server_request_new(in, op, len, &req.h, &exc) != RORB_OK) {
    reply_system(reply, exc.repository_id ? exc.repository_id : ex_CORBA_INTERNAL, exc.minor,
                 CORBA_COMPLETED_NO);
    rorb_exception_clear(&exc);
    return RORB_SYSTEM_EXCEPTION;
  }
  req.operation = rustorb_c_strndup(op, len);
  s->vepv->PortableServer_DynamicImpl_epv->invoke(servant, &req);
  if (req.sys_id) {
    reply_system(reply, req.sys_id, req.sys_minor, req.sys_completed);
    st = RORB_SYSTEM_EXCEPTION;
  } else if (send_outs(&req, reply)) {
    st = RORB_SYSTEM_EXCEPTION;
  } else if (rorb_server_request_reply(req.h, reply, &exc) != RORB_OK) {
    reply_system(reply, exc.repository_id ? exc.repository_id : ex_CORBA_BAD_PARAM, exc.minor,
                 CORBA_COMPLETED_YES);
    rorb_exception_clear(&exc);
    st = RORB_SYSTEM_EXCEPTION;
  }
  rorb_server_request_release(req.h);
  CORBA_NVList_free(req.params, &ev);
  CORBA_free(req.sys_id_owned);
  CORBA_free(req.operation);
  return st;
}

void PortableServer_DynamicImpl__init(PortableServer_Servant servant, CORBA_Environment* ev) {
  PortableServer_DynamicImpl* s = (PortableServer_DynamicImpl*)servant;
  CORBA_RepositoryId id;
  rustorb_c_ev_clear(ev);
  RUSTORB_C_ARG_CHECK(ev,
                      s && s->vepv && s->vepv->PortableServer_DynamicImpl_epv &&
                          s->vepv->PortableServer_DynamicImpl_epv->invoke &&
                          s->vepv->PortableServer_DynamicImpl_epv->primary_interface,
                      ex_CORBA_BAD_PARAM);
  id = s->vepv->PortableServer_DynamicImpl_epv->primary_interface(servant, NULL, CORBA_OBJECT_NIL,
                                                                 ev);
  if (ev->_major != CORBA_NO_EXCEPTION) return;
  if (!id) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return;
  }
  rustorb_c_servant_init_dynamic(servant, id, ev);
  CORBA_free(id);
}

void PortableServer_DynamicImpl__fini(PortableServer_Servant servant, CORBA_Environment* ev) {
  rustorb_c_servant_fini(servant, ev);
}
