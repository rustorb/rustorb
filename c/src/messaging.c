/* Callback AMI (docs/ami.md): rustorb_c_sendc over rorb_object_sendc,
   and Messaging::ExceptionHolder. */

#include <rustorb/messaging.h>

#include "internal.h"

/* --- descriptors ----------------------------------------------------------------- */

static rustorb_c_cache reply_handler_cache;
const rustorb_c_type Messaging_ReplyHandler__desc = {
    CORBA_tk_objref, sizeof(CORBA_Object), "IDL:omg.org/Messaging/ReplyHandler:1.0",
    "ReplyHandler", NULL, 0, 0, NULL, NULL, -1, 0, 0, &reply_handler_cache};

static rustorb_c_cache octets_cache;
static const rustorb_c_type octets_desc = {
    CORBA_tk_sequence, sizeof(CORBA_sequence_octet), NULL, NULL, &CORBA_octet__desc, 0, 0,
    NULL, NULL, -1, 0, 0, &octets_cache};

static const rustorb_c_member holder_members[] = {
    {"is_system_exception", &CORBA_boolean__desc,
     offsetof(Messaging_ExceptionHolder, is_system_exception), 0},
    {"byte_order", &CORBA_boolean__desc, offsetof(Messaging_ExceptionHolder, byte_order), 0},
    {"marshaled_exception", &octets_desc,
     offsetof(Messaging_ExceptionHolder, marshaled_exception), 0},
};

static rustorb_c_cache holder_cache;
const rustorb_c_type Messaging_ExceptionHolder__desc = {
    CORBA_tk_value, sizeof(Messaging_ExceptionHolder), "IDL:omg.org/Messaging/ExceptionHolder:1.0",
    "ExceptionHolder", NULL, 0, 3, holder_members, NULL, -1, 0, 0, &holder_cache};

Messaging_ExceptionHolder* Messaging_ExceptionHolder__alloc(void) {
  return (Messaging_ExceptionHolder*)rustorb_c_alloc(&Messaging_ExceptionHolder__desc, 1);
}

/* The minor code of UNKNOWN for a user exception not listed or not known. */
#define UNLISTED 0x4f4d0001u

static int little_endian(void) {
  const uint16_t one = 1;
  return *(const uint8_t*)&one == 1;
}

/* --- making holders ---------------------------------------------------------------- */

typedef struct encoding {
  int system;
  const char* id;
  CORBA_unsigned_long minor;
  CORBA_unsigned_long completed;
  const rustorb_c_type* type;
  const void* value;
} encoding;

static rorb_status write_exception(void* context, rorb_out* out) {
  encoding* e = (encoding*)context;
  rustorb_c_err err;
  rorb_out_write_string(out, e->id, strlen(e->id));
  if (e->system) {
    rorb_out_write_ulong(out, e->minor);
    rorb_out_write_ulong(out, e->completed);
  } else if (e->type && e->value) {
    if (rustorb_c_write(out, e->type, e->value, &err)) return RORB_CALLBACK_FAILED;
  }
  return RORB_OK;
}

/* The holder of an exception, into h (zeroed); 0, or -1. */
static int make_holder(Messaging_ExceptionHolder* h, encoding* e) {
  rorb_bytes b;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  memset(&b, 0, sizeof b);
  if (rorb_cdr_encode(write_exception, e, little_endian(), &b, &exc) != RORB_OK) {
    rorb_exception_clear(&exc);
    return -1;
  }
  h->is_system_exception = e->system ? CORBA_TRUE : CORBA_FALSE;
  h->byte_order = little_endian() ? CORBA_TRUE : CORBA_FALSE;
  h->marshaled_exception._release = CORBA_TRUE;
  if (b.len) {
    h->marshaled_exception._buffer = (CORBA_octet*)rustorb_c_alloc(NULL, (CORBA_unsigned_long)b.len);
    if (!h->marshaled_exception._buffer) {
      rorb_bytes_free(b);
      return -1;
    }
    memcpy(h->marshaled_exception._buffer, b.data, b.len);
    h->marshaled_exception._length = h->marshaled_exception._maximum = (CORBA_unsigned_long)b.len;
  }
  rorb_bytes_free(b);
  return 0;
}

/* --- raise_exception --------------------------------------------------------------- */

static int listed(const CORBA_TypeCode* list, CORBA_unsigned_long count, const char* id) {
  CORBA_unsigned_long i;
  for (i = 0; i < count; i++) {
    CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
    CORBA_RepositoryId tid;
    int same;
    if (!list[i]) continue;
    tid = CORBA_TypeCode_id(list[i], &ev);
    same = ev._major == CORBA_NO_EXCEPTION && tid && strcmp(tid, id) == 0;
    CORBA_free(tid);
    CORBA_exception_free(&ev);
    if (same) return 1;
  }
  return 0;
}

static void raise_held(const Messaging_ExceptionHolder* h, const CORBA_TypeCode* list,
                  CORBA_unsigned_long count, int with_list, CORBA_Environment* ev) {
  rorb_in* in = NULL;
  rorb_exception exc;
  const char* s;
  size_t len = 0;
  char* id;
  rustorb_c_ev_clear(ev);
  if (!h) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return;
  }
  if (with_list && count && !list) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return;
  }
  memset(&exc, 0, sizeof exc);
  if (rorb_in_from_bytes(h->marshaled_exception._buffer, h->marshaled_exception._length,
                         h->byte_order ? 1 : 0, NULL, &in, &exc) != RORB_OK) {
    rorb_exception_clear(&exc);
    rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_YES);
    return;
  }
  s = rorb_in_read_string(in, &len);
  if (rorb_in_failed(in) || !s) {
    rorb_in_release(in);
    rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_YES);
    return;
  }
  id = rustorb_c_strndup(s, len);
  if (h->is_system_exception) {
    CORBA_unsigned_long minor = rorb_in_read_ulong(in);
    CORBA_unsigned_long completed = rorb_in_read_ulong(in);
    if (rorb_in_failed(in))
      rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_YES);
    else
      rustorb_c_ev_system(ev, id, minor, (CORBA_completion_status)(completed % 3));
  } else {
    const rustorb_c_type* t = rustorb_c_desc_for_id(id);
    if ((with_list && !listed(list, count, id)) || !t || t->kind != CORBA_tk_except) {
      rustorb_c_ev_system(ev, ex_CORBA_UNKNOWN, UNLISTED, CORBA_COMPLETED_YES);
    } else {
      void* v = t->count ? rustorb_c_alloc(t, 1) : NULL;
      if (v) rustorb_c_read(in, t, v);
      if (rorb_in_failed(in)) {
        CORBA_free(v);
        rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_YES);
      } else {
        CORBA_exception_set(ev, CORBA_USER_EXCEPTION, id, v);
      }
    }
  }
  CORBA_free(id);
  rorb_in_release(in);
}

void Messaging_ExceptionHolder_raise_exception(const Messaging_ExceptionHolder* holder,
                                               CORBA_Environment* ev) {
  raise_held(holder, NULL, 0, 0, ev);
}

void Messaging_ExceptionHolder_raise_exception_with_list(const Messaging_ExceptionHolder* holder,
                                                         const CORBA_TypeCode* list,
                                                         CORBA_unsigned_long count,
                                                         CORBA_Environment* ev) {
  raise_held(holder, list, count, 1, ev);
}

/* --- sendc -------------------------------------------------------------------------- */

typedef struct sendc_call {
  const rustorb_c_operation* op;
  const rustorb_c_operation* reply;
  const rustorb_c_operation* excep;
  CORBA_Object handler;
  /* For each parameter of op: a copy of an in or inout value, the out or
     inout value read from the reply. */
  void** ins;
  void** outs;
  void* result;
  /* A user exception of the list. */
  char* exc_id;
  const rustorb_c_type* exc_type;
  void* exc_value;
  rustorb_c_err err;
} sendc_call;

static void free_call(sendc_call* c) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_unsigned_long i;
  for (i = 0; i < c->op->param_count; i++) {
    if (c->ins) CORBA_free(c->ins[i]);
    if (c->outs) CORBA_free(c->outs[i]);
  }
  free(c->ins);
  free(c->outs);
  CORBA_free(c->result);
  CORBA_free(c->exc_id);
  CORBA_free(c->exc_value);
  CORBA_Object_release(c->handler, &ev);
  free(c);
}

static rorb_status sendc_write(void* context, rorb_out* out) {
  sendc_call* c = (sendc_call*)context;
  CORBA_unsigned_long i;
  for (i = 0; i < c->op->param_count; i++) {
    const rustorb_c_param* p = &c->op->params[i];
    if (p->mode == RUSTORB_C_OUT) continue;
    if (rustorb_c_write(out, p->type, c->ins[i], &c->err)) return RORB_CALLBACK_FAILED;
  }
  return RORB_OK;
}

/* Storage for a value of t, read from in; NULL (the stream failed) if
   there is no memory. */
static void* read_new(rorb_in* in, const rustorb_c_type* t) {
  void* v = rustorb_c_alloc(t, 1);
  if (!v) {
    rorb_in_fail(in);
    return NULL;
  }
  rustorb_c_read(in, t, v);
  return v;
}

static rorb_status sendc_read(void* context, rorb_in* in) {
  sendc_call* c = (sendc_call*)context;
  CORBA_unsigned_long i;
  if (c->op->result) {
    CORBA_free(c->result);
    c->result = read_new(in, c->op->result);
  }
  for (i = 0; i < c->op->param_count; i++) {
    const rustorb_c_param* p = &c->op->params[i];
    if (p->mode == RUSTORB_C_IN) continue;
    CORBA_free(c->outs[i]);
    c->outs[i] = read_new(in, p->type);
  }
  return RORB_OK;
}

static rorb_status sendc_user_exception(void* context, const char* id, size_t len, rorb_in* in) {
  sendc_call* c = (sendc_call*)context;
  CORBA_unsigned_long i;
  const rustorb_c_type* t = NULL;
  for (i = 0; i < c->op->exception_count; i++) {
    const char* e = c->op->exceptions[i]->id;
    if (strlen(e) == len && memcmp(e, id, len) == 0) t = c->op->exceptions[i];
  }
  if (!t) return RORB_SYSTEM_EXCEPTION;
  CORBA_free(c->exc_value);
  CORBA_free(c->exc_id);
  c->exc_value = t->count ? read_new(in, t) : NULL;
  c->exc_id = rustorb_c_strndup(id, len);
  c->exc_type = t;
  return RORB_USER_EXCEPTION;
}

static void call_excep(sendc_call* c, encoding* e) {
  Messaging_ExceptionHolder h;
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  void* args[1];
  memset(&h, 0, sizeof h);
  if (make_holder(&h, e) == 0) {
    args[0] = &h;
    rustorb_c_invoke(c->handler, c->excep, args, NULL, &ev);
    /* The handler's failure is its own. */
    CORBA_exception_free(&ev);
  }
  rustorb_c_free_value(&Messaging_ExceptionHolder__desc, &h);
}

static void call_system(sendc_call* c, const char* id, CORBA_unsigned_long minor,
                        CORBA_unsigned_long completed) {
  encoding e;
  memset(&e, 0, sizeof e);
  e.system = 1;
  e.id = id;
  e.minor = minor;
  e.completed = completed;
  call_excep(c, &e);
}

static void sendc_done(void* context, rorb_status status, const rorb_exception* exc) {
  sendc_call* c = (sendc_call*)context;
  if (!c->handler) {
    free_call(c);
    return;
  }
  if (status == RORB_OK) {
    CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
    void** args = (void**)calloc(c->reply->param_count ? c->reply->param_count : 1, sizeof *args);
    CORBA_unsigned_long i, n = 0;
    int complete = args != NULL;
    if (c->op->result) {
      if (!c->result) complete = 0;
      if (args && n < c->reply->param_count) args[n++] = c->result;
    }
    for (i = 0; i < c->op->param_count; i++) {
      if (c->op->params[i].mode == RUSTORB_C_IN) continue;
      if (!c->outs[i]) complete = 0;
      if (args && n < c->reply->param_count) args[n++] = c->outs[i];
    }
    if (n != c->reply->param_count) complete = 0;
    if (complete) {
      rustorb_c_invoke(c->handler, c->reply, args, NULL, &ev);
      CORBA_exception_free(&ev);
    } else {
      call_system(c, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_YES);
    }
    free(args);
  } else if (status == RORB_USER_EXCEPTION && c->exc_id) {
    encoding e;
    memset(&e, 0, sizeof e);
    e.id = c->exc_id;
    e.type = c->exc_type;
    e.value = c->exc_value;
    call_excep(c, &e);
  } else if (status == RORB_SYSTEM_EXCEPTION && exc && exc->repository_id) {
    char* id = rustorb_c_strndup(exc->repository_id, exc->repository_id_len);
    call_system(c, id, exc->minor, exc->completed);
    CORBA_free(id);
  } else if (c->err.id) {
    /* A value that cannot be sent. */
    call_system(c, c->err.id, c->err.minor, CORBA_COMPLETED_NO);
  } else {
    call_system(c, status == RORB_CALLBACK_FAILED ? ex_CORBA_MARSHAL : ex_CORBA_UNKNOWN, 0,
                CORBA_COMPLETED_MAYBE);
  }
  free_call(c);
}

void rustorb_c_sendc(CORBA_Object obj, const rustorb_c_operation* op, void** ins,
                     CORBA_Object handler, const rustorb_c_operation* reply,
                     const rustorb_c_operation* excep, CORBA_Environment* ev) {
  sendc_call* c;
  rorb_invocation inv;
  rorb_exception exc;
  rorb_status st;
  CORBA_unsigned_long i, k = 0;
  CORBA_Environment ev2 = {CORBA_NO_EXCEPTION, NULL, NULL};
  rustorb_c_ev_clear(ev);
  rustorb_c_register_module(op->module);
  if (!obj) {
    rustorb_c_ev_system(ev, ex_CORBA_INV_OBJREF, 0, CORBA_COMPLETED_NO);
    return;
  }
  if (obj->kind != OBJ_REF) {
    rustorb_c_ev_system(ev, ex_CORBA_NO_IMPLEMENT, 0, CORBA_COMPLETED_NO);
    return;
  }
  if (handler && (!reply || !excep)) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return;
  }
  c = (sendc_call*)calloc(1, sizeof *c);
  if (c) {
    c->op = op;
    c->ins = (void**)calloc(op->param_count ? op->param_count : 1, sizeof *c->ins);
    c->outs = (void**)calloc(op->param_count ? op->param_count : 1, sizeof *c->outs);
  }
  if (!c || !c->ins || !c->outs) {
    if (c) {
      free(c->ins);
      free(c->outs);
      free(c);
    }
    rustorb_c_ev_system(ev, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
    return;
  }
  c->reply = reply;
  c->excep = excep;
  c->handler = CORBA_Object_duplicate(handler, &ev2);
  /* The runtime writes the arguments later, maybe more than once. */
  for (i = 0; i < op->param_count; i++) {
    const rustorb_c_param* p = &op->params[i];
    if (p->mode == RUSTORB_C_OUT) continue;
    c->ins[i] = rustorb_c_copy(p->type, ins[k++]);
    if (!c->ins[i]) {
      free_call(c);
      rustorb_c_ev_system(ev, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
      return;
    }
  }
  memset(&inv, 0, sizeof inv);
  inv.operation = op->name;
  inv.operation_len = strlen(op->name);
  inv.oneway = 0;
  inv.timeout_us = -1;
  inv.context = c;
  inv.write_args = sendc_write;
  inv.read_reply = sendc_read;
  inv.read_user_exception = sendc_user_exception;
  memset(&exc, 0, sizeof exc);
  st = rorb_object_sendc((rorb_object*)obj->handle, &inv, sendc_done, &exc);
  if (st != RORB_OK) {
    free_call(c);
    rustorb_c_ev_status(ev, st, &exc);
  }
}
