/* Stub calls and servant dispatch. */

#include "internal.h"

/* --- client ------------------------------------------------------------------ */

typedef struct call {
  const rustorb_c_operation* op;
  void** args;
  void* result;
  CORBA_Environment* ev;
  rustorb_c_err err;
} call;

static rorb_status write_args(void* context, rorb_out* out) {
  call* c = (call*)context;
  CORBA_unsigned_long i;
  for (i = 0; i < c->op->param_count; i++) {
    const rustorb_c_param* p = &c->op->params[i];
    if (p->mode == RUSTORB_C_OUT) continue;
    if (rustorb_c_write(out, p->type, c->args[i], &c->err)) return RORB_CALLBACK_FAILED;
  }
  return RORB_OK;
}

/* Reads a value into the out cell of an argument or the result. */
static void read_out(rorb_in* in, const rustorb_c_type* t, int indirect, void* cell) {
  if (indirect) {
    void* v = rustorb_c_alloc(t, 1);
    *(void**)cell = v;
    if (!v) {
      rorb_in_fail(in);
      return;
    }
    rustorb_c_read(in, t, v);
  } else {
    rustorb_c_read(in, t, cell);
  }
}

static rorb_status read_reply(void* context, rorb_in* in) {
  call* c = (call*)context;
  CORBA_unsigned_long i;
  if (c->op->result) read_out(in, c->op->result, c->op->result_indirect, c->result);
  for (i = 0; i < c->op->param_count; i++) {
    const rustorb_c_param* p = &c->op->params[i];
    if (p->mode == RUSTORB_C_IN) continue;
    if (p->mode == RUSTORB_C_INOUT) {
      rustorb_c_free_value(p->type, c->args[i]);
      memset(c->args[i], 0, p->type->size);
      rustorb_c_read(in, p->type, c->args[i]);
    } else {
      read_out(in, p->type, p->indirect, c->args[i]);
    }
  }
  return RORB_OK;
}

static const rustorb_c_type* find_exception(const rustorb_c_operation* op, const char* id,
                                            size_t len) {
  CORBA_unsigned_long i;
  for (i = 0; i < op->exception_count; i++) {
    const char* e = op->exceptions[i]->id;
    if (strlen(e) == len && memcmp(e, id, len) == 0) return op->exceptions[i];
  }
  return NULL;
}

static rorb_status read_user_exception(void* context, const char* id, size_t len, rorb_in* in) {
  call* c = (call*)context;
  const rustorb_c_type* t = find_exception(c->op, id, len);
  void* v;
  char* rid;
  if (!t) return RORB_SYSTEM_EXCEPTION;
  v = t->count ? rustorb_c_alloc(t, 1) : NULL;
  if (v) rustorb_c_read(in, t, v);
  if (rorb_in_failed(in)) {
    CORBA_free(v);
    return RORB_USER_EXCEPTION;
  }
  rid = rustorb_c_strndup(id, len);
  CORBA_exception_set(c->ev, CORBA_USER_EXCEPTION, rid, v);
  CORBA_free(rid);
  return RORB_USER_EXCEPTION;
}

/* Frees the out values and the result after a failed call. */
static void clear_outs(call* c) {
  CORBA_unsigned_long i;
  const rustorb_c_operation* op = c->op;
  if (op->result) {
    if (op->result_indirect) {
      CORBA_free(*(void**)c->result);
      *(void**)c->result = NULL;
    } else {
      rustorb_c_free_value(op->result, c->result);
      memset(c->result, 0, op->result->size);
    }
  }
  for (i = 0; i < op->param_count; i++) {
    const rustorb_c_param* p = &op->params[i];
    if (p->mode != RUSTORB_C_OUT) continue;
    if (p->indirect) {
      CORBA_free(*(void**)c->args[i]);
      *(void**)c->args[i] = NULL;
    } else {
      rustorb_c_free_value(p->type, c->args[i]);
      memset(c->args[i], 0, p->type->size);
    }
  }
}

void rustorb_c_invoke(CORBA_Object obj, const rustorb_c_operation* op, void** args, void* result,
                      CORBA_Environment* ev) {
  call c;
  rorb_invocation inv;
  rorb_exception exc;
  rorb_status st;
  CORBA_unsigned_long i;
  rustorb_c_ev_clear(ev);
  rustorb_c_register_module(op->module);
  /* Out storage starts empty, as the caller may give any. */
  if (op->result) {
    if (op->result_indirect)
      *(void**)result = NULL;
    else
      memset(result, 0, op->result->size);
  }
  for (i = 0; i < op->param_count; i++) {
    const rustorb_c_param* p = &op->params[i];
    if (p->mode != RUSTORB_C_OUT) continue;
    if (p->indirect)
      *(void**)args[i] = NULL;
    else
      memset(args[i], 0, p->type->size);
  }
  if (!obj) {
    rustorb_c_ev_system(ev, ex_CORBA_INV_OBJREF, 0, CORBA_COMPLETED_NO);
    return;
  }
  if (obj->kind != OBJ_REF) {
    rustorb_c_ev_system(ev, ex_CORBA_NO_IMPLEMENT, 0, CORBA_COMPLETED_NO);
    return;
  }
  memset(&c, 0, sizeof c);
  c.op = op;
  c.args = args;
  c.result = result;
  c.ev = ev;
  memset(&inv, 0, sizeof inv);
  inv.operation = op->name;
  inv.operation_len = strlen(op->name);
  inv.oneway = op->oneway;
  inv.timeout_us = -1;
  inv.context = &c;
  inv.write_args = write_args;
  inv.read_reply = read_reply;
  inv.read_user_exception = read_user_exception;
  memset(&exc, 0, sizeof exc);
  st = rorb_object_invoke((rorb_object*)obj->handle, &inv, &exc);
  if (st == RORB_OK) return;
  clear_outs(&c);
  if (st == RORB_USER_EXCEPTION) {
    if (ev->_major == CORBA_NO_EXCEPTION)
      rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_YES);
    rorb_exception_clear(&exc);
  } else if (st == RORB_CALLBACK_FAILED && c.err.id) {
    rustorb_c_ev_system(ev, c.err.id, c.err.minor, CORBA_COMPLETED_NO);
    rorb_exception_clear(&exc);
  } else {
    rustorb_c_ev_status(ev, st, &exc);
  }
}

/* --- servants ------------------------------------------------------------------- */

#define SERVANT_MAGIC 0x52435356u /* "RCSV" */

typedef struct servant_private {
  uint32_t magic;
  const rustorb_c_class* cls;
  rorb_servant* handle;
} servant_private;

static servant_private* private_of(PortableServer_Servant servant) {
  servant_private* p;
  if (!servant) return NULL;
  p = (servant_private*)((PortableServer_ServantBase*)servant)->_private;
  return p && p->magic == SERVANT_MAGIC ? p : NULL;
}

rorb_servant* rustorb_c_servant_handle(PortableServer_Servant servant) {
  servant_private* p = private_of(servant);
  return p ? p->handle : NULL;
}

static void reply_system(rorb_server_reply* reply, const char* id, CORBA_unsigned_long minor,
                         CORBA_completion_status completed) {
  rorb_server_reply_system_exception(reply, id, strlen(id), minor, completed);
}

static rorb_status dispatch(void* self, const char* name, size_t name_len, rorb_in* in,
                            rorb_server_reply* reply) {
  servant_private* priv = private_of(self);
  const rustorb_c_entry* entry = NULL;
  const rustorb_c_operation* op;
  void* stack_args[16];
  void** args = stack_args;
  void* result = NULL;
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  rustorb_c_err err = {NULL, 0};
  rorb_status st = RORB_OK;
  CORBA_unsigned_long i, n;
  if (!priv) {
    reply_system(reply, ex_CORBA_OBJ_ADAPTER, 0, CORBA_COMPLETED_NO);
    return RORB_SYSTEM_EXCEPTION;
  }
  for (i = 0; i < priv->cls->entry_count; i++) {
    const char* e = priv->cls->entries[i].op->name;
    if (strlen(e) == name_len && memcmp(e, name, name_len) == 0) {
      entry = &priv->cls->entries[i];
      break;
    }
  }
  if (!entry) {
    reply_system(reply, ex_CORBA_BAD_OPERATION, 0x4f4d0002u, CORBA_COMPLETED_NO);
    return RORB_SYSTEM_EXCEPTION;
  }
  op = entry->op;
  n = op->param_count;
  if (n > 16) {
    args = (void**)calloc(n, sizeof *args);
    if (!args) {
      reply_system(reply, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
      return RORB_SYSTEM_EXCEPTION;
    }
  }
  /* A cell for each argument and the result: the value, or the T* of an
     indirect one. */
  for (i = 0; i < n; i++) {
    const rustorb_c_param* p = &op->params[i];
    size_t size = p->indirect ? sizeof(void*) : p->type->size;
    args[i] = calloc(1, size ? size : 1);
    if (!args[i]) {
      reply_system(reply, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
      st = RORB_SYSTEM_EXCEPTION;
      n = i;
      goto done;
    }
    if (p->mode != RUSTORB_C_OUT) rustorb_c_read(in, p->type, args[i]);
  }
  if (op->result) {
    result = calloc(1, op->result_indirect ? sizeof(void*) : op->result->size);
    if (!result) {
      reply_system(reply, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
      st = RORB_SYSTEM_EXCEPTION;
      goto done;
    }
  }
  if (rorb_in_failed(in)) {
    reply_system(reply, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_NO);
    st = RORB_SYSTEM_EXCEPTION;
    goto done;
  }
  entry->skel(self, args, result, &ev);
  if (ev._major == CORBA_USER_EXCEPTION) {
    const rustorb_c_type* t = ev._id ? find_exception(op, ev._id, strlen(ev._id)) : NULL;
    if (!t) {
      reply_system(reply, ex_CORBA_UNKNOWN, 0, CORBA_COMPLETED_MAYBE);
      st = RORB_SYSTEM_EXCEPTION;
    } else {
      rorb_out* out = rorb_server_reply_user_exception(reply, t->id, strlen(t->id));
      if (t->count && !ev._params) {
        reply_system(reply, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_YES);
        st = RORB_SYSTEM_EXCEPTION;
      } else if (t->count && rustorb_c_write(out, t, ev._params, &err)) {
        reply_system(reply, err.id, err.minor, CORBA_COMPLETED_YES);
        st = RORB_SYSTEM_EXCEPTION;
      }
    }
  } else if (ev._major == CORBA_SYSTEM_EXCEPTION) {
    CORBA_SystemException* v = (CORBA_SystemException*)ev._params;
    reply_system(reply, ev._id ? ev._id : ex_CORBA_UNKNOWN, v ? v->minor : 0,
                 v ? v->completed : CORBA_COMPLETED_MAYBE);
    st = RORB_SYSTEM_EXCEPTION;
  } else if (!op->oneway) {
    rorb_out* out = rorb_server_reply_output(reply);
    int bad = 0;
    if (op->result) {
      void* v = op->result_indirect ? *(void**)result : result;
      bad = !v ? (err.id = ex_CORBA_BAD_PARAM, 1) : rustorb_c_write(out, op->result, v, &err);
    }
    for (i = 0; i < n && !bad; i++) {
      const rustorb_c_param* p = &op->params[i];
      void* v;
      if (p->mode == RUSTORB_C_IN) continue;
      v = p->indirect ? *(void**)args[i] : args[i];
      bad = !v ? (err.id = ex_CORBA_BAD_PARAM, 1) : rustorb_c_write(out, p->type, v, &err);
    }
    if (bad) {
      reply_system(reply, err.id, err.minor, CORBA_COMPLETED_YES);
      st = RORB_SYSTEM_EXCEPTION;
    }
  }
done:
  for (i = 0; i < n; i++) {
    const rustorb_c_param* p = &op->params[i];
    if (p->indirect)
      CORBA_free(*(void**)args[i]);
    else
      rustorb_c_free_value(p->type, args[i]);
    free(args[i]);
  }
  if (result) {
    if (op->result_indirect)
      CORBA_free(*(void**)result);
    else
      rustorb_c_free_value(op->result, result);
    free(result);
  }
  if (args != stack_args) free(args);
  CORBA_exception_free(&ev);
  return st;
}

static int32_t is_a(void* self, const char* id, size_t len) {
  servant_private* priv = private_of(self);
  CORBA_unsigned_long i;
  if (len == 28 && memcmp(id, "IDL:omg.org/CORBA/Object:1.0", 28) == 0) return 1;
  if (!priv) return 0;
  for (i = 0; i < priv->cls->id_count; i++) {
    const char* e = priv->cls->ids[i];
    if (strlen(e) == len && memcmp(e, id, len) == 0) return 1;
  }
  return 0;
}

/* The ORB keeps the servant only while the POA holds it; the servant
   itself is the application's, alive until __fini. */
static void add_ref(void* self) { (void)self; }
static void remove_ref(void* self) { (void)self; }

static const rorb_servant_vtable vtable = {dispatch, is_a, add_ref, remove_ref};

static const char* const base_ids[] = {"IDL:omg.org/CORBA/Object:1.0"};
static const rustorb_c_class base_class = {base_ids, 1, NULL, 0, NULL};

void rustorb_c_servant_init(PortableServer_Servant servant, const rustorb_c_class* cls,
                            CORBA_Environment* ev) {
  servant_private* p;
  rustorb_c_ev_clear(ev);
  if (!servant) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return;
  }
  if (!cls) cls = &base_class;
  rustorb_c_register_module(cls->module);
  p = (servant_private*)calloc(1, sizeof *p);
  if (!p) {
    rustorb_c_ev_system(ev, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
    return;
  }
  p->magic = SERVANT_MAGIC;
  p->cls = cls;
  ((PortableServer_ServantBase*)servant)->_private = p;
  p->handle = rorb_servant_get(servant, cls->ids[0], strlen(cls->ids[0]), &vtable);
}

void rustorb_c_servant_fini(PortableServer_Servant servant, CORBA_Environment* ev) {
  servant_private* p = private_of(servant);
  rustorb_c_ev_clear(ev);
  if (!p) return;
  rorb_servant_release(p->handle);
  p->magic = 0;
  free(p);
  ((PortableServer_ServantBase*)servant)->_private = NULL;
}

void PortableServer_ServantBase__init(PortableServer_Servant servant, CORBA_Environment* ev) {
  rustorb_c_servant_init(servant, NULL, ev);
}

void PortableServer_ServantBase__fini(PortableServer_Servant servant, CORBA_Environment* ev) {
  rustorb_c_servant_fini(servant, ev);
}

PortableServer_POA PortableServer_ServantBase__default_POA(PortableServer_Servant servant,
                                                           CORBA_Environment* ev) {
  PortableServer_ServantBase* b = (PortableServer_ServantBase*)servant;
  rorb_poa* poa = NULL;
  rorb_exception exc;
  rustorb_c_ev_clear(ev);
  if (b && b->vepv && b->vepv->_base_epv && b->vepv->_base_epv->default_POA)
    return b->vepv->_base_epv->default_POA(servant, ev);
  memset(&exc, 0, sizeof exc);
  if (rustorb_c_failed(ev, rorb_default_root_poa(&poa, &exc), &exc)) return CORBA_OBJECT_NIL;
  return rustorb_c_wrap(OBJ_POA, poa);
}

CORBA_Object PortableServer_ServantBase__this(PortableServer_Servant servant,
                                              CORBA_Environment* ev) {
  rorb_servant* h = rustorb_c_servant_handle(servant);
  rorb_object* obj = NULL;
  rustorb_c_ev_clear(ev);
  if (h && rorb_servant_current_reference(h, &obj)) return rustorb_c_wrap(OBJ_REF, obj);
  return CORBA_OBJECT_NIL;
}
