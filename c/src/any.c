/* CORBA_any beyond reading and writing. */

#include "internal.h"

typedef struct value_ctx {
  const rustorb_c_type* type;
  const void* value;
  rustorb_c_err err;
} value_ctx;

static rorb_status write_value(void* context, rorb_out* out) {
  value_ctx* c = (value_ctx*)context;
  return rustorb_c_write(out, c->type, c->value, &c->err) ? RORB_CALLBACK_FAILED : RORB_OK;
}

rorb_any* rustorb_c_any_to_raw(const CORBA_any* any, rustorb_c_err* err) {
  rorb_typecode* tc;
  value_ctx c;
  rorb_any* r = NULL;
  rorb_exception exc;
  uint32_t kind;
  if (!any || !any->_type) return rorb_any_new();
  tc = rustorb_c_tc_handle(any->_type);
  if (!tc) {
    err->id = ex_CORBA_BAD_PARAM;
    err->minor = 0;
    return NULL;
  }
  if (!any->_value && any->_rustorb) return rorb_any_clone((rorb_any*)any->_rustorb);
  kind = rorb_tc_kind(tc);
  memset(&c, 0, sizeof c);
  c.type = any->_type->desc ? any->_type->desc : rustorb_c_desc_for_tc(tc);
  c.value = any->_value;
  if (!c.value && kind != CORBA_tk_null && kind != CORBA_tk_void) {
    err->id = ex_CORBA_BAD_PARAM;
    err->minor = 0;
    return NULL;
  }
  if (!c.type && c.value) {
    err->id = ex_CORBA_BAD_TYPECODE;
    err->minor = 0;
    return NULL;
  }
  if (!c.value) c.type = NULL;
  memset(&exc, 0, sizeof exc);
  if (!c.type) {
    static const rustorb_c_type nothing = {CORBA_tk_null, 0, NULL, NULL, NULL, 0, 0,
                                           NULL, NULL, -1, 0, 0, NULL};
    c.type = &nothing;
  }
  if (rorb_any_insert(tc, write_value, &c, &r, &exc) != RORB_OK) {
    if (c.err.id) {
      *err = c.err;
    } else {
      err->id = ex_CORBA_BAD_PARAM;
      err->minor = exc.minor;
    }
    rorb_exception_clear(&exc);
    return NULL;
  }
  return r;
}

CORBA_boolean CORBA_any_equal(const CORBA_any* a, const CORBA_any* b) {
  rustorb_c_err err;
  rorb_any* ra = rustorb_c_any_to_raw(a, &err);
  rorb_any* rb = rustorb_c_any_to_raw(b, &err);
  CORBA_boolean r = ra && rb && rorb_any_equal(ra, rb);
  rorb_any_release(ra);
  rorb_any_release(rb);
  return r;
}
