/* DynamicAny on the rorb_dynany_* functions of the C ABI (dynany.h). */

#include <rustorb/dynany.h>

#include "internal.h"

#define NO_EXC(e) memset(&(e), 0, sizeof(e))

/* --- descriptors of the module's types ---------------------------------------- */

static const rustorb_c_member name_value_members[] = {
    {"id", &CORBA_string__desc, offsetof(DynamicAny_NameValuePair, id), 0},
    {"value", &CORBA_any__desc, offsetof(DynamicAny_NameValuePair, value), 0},
};
static const rustorb_c_type name_value_desc = {
    CORBA_tk_struct, sizeof(DynamicAny_NameValuePair), "IDL:omg.org/DynamicAny/NameValuePair:1.0",
    "NameValuePair", NULL, 0, 2, name_value_members, NULL, -1, 0, 0, NULL};
static const rustorb_c_member name_dyn_members[] = {
    {"id", &CORBA_string__desc, offsetof(DynamicAny_NameDynAnyPair, id), 0},
    {"value", &CORBA_Object__desc, offsetof(DynamicAny_NameDynAnyPair, value), 0},
};
static const rustorb_c_type name_dyn_desc = {
    CORBA_tk_struct, sizeof(DynamicAny_NameDynAnyPair), "IDL:omg.org/DynamicAny/NameDynAnyPair:1.0",
    "NameDynAnyPair", NULL, 0, 2, name_dyn_members, NULL, -1, 0, 0, NULL};

#define SEQUENCE_ALLOC(name, elem, desc)                                       \
  name* name##__alloc(void) { return (name*)rustorb_c_sequence_alloc(); }       \
  elem* name##_allocbuf(CORBA_unsigned_long len) {                              \
    return (elem*)rustorb_c_alloc((desc), len);                                 \
  }
SEQUENCE_ALLOC(DynamicAny_NameValuePairSeq, DynamicAny_NameValuePair, &name_value_desc)
SEQUENCE_ALLOC(DynamicAny_NameDynAnyPairSeq, DynamicAny_NameDynAnyPair, &name_dyn_desc)
SEQUENCE_ALLOC(DynamicAny_DynAnySeq, DynamicAny_DynAny, &CORBA_Object__desc)
SEQUENCE_ALLOC(DynamicAny_AnySeq, CORBA_any, &CORBA_any__desc)

/* --- handles -------------------------------------------------------------------- */

static const rorb_dynany* dyn_of(DynamicAny_DynAny obj, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (!obj || obj->kind != OBJ_DYNANY) {
    rustorb_c_ev_system(ev, obj ? ex_CORBA_BAD_PARAM : ex_CORBA_INV_OBJREF, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  return (const rorb_dynany*)obj->handle;
}

static DynamicAny_DynAny wrap(rorb_dynany* d) { return rustorb_c_wrap(OBJ_DYNANY, d); }

static const char* const kind_ids[] = {
    "IDL:omg.org/DynamicAny/DynStruct:1.0",   /* tk_struct, tk_except */
    "IDL:omg.org/DynamicAny/DynUnion:1.0",    "IDL:omg.org/DynamicAny/DynEnum:1.0",
    "IDL:omg.org/DynamicAny/DynSequence:1.0", "IDL:omg.org/DynamicAny/DynArray:1.0",
    "IDL:omg.org/DynamicAny/DynFixed:1.0",    "IDL:omg.org/DynamicAny/DynValue:1.0",
    "IDL:omg.org/DynamicAny/DynValueBox:1.0",
};

int rustorb_c_dynany_is_a(CORBA_Object obj, const char* id) {
  int i = -1;
  if (strcmp(id, "IDL:omg.org/DynamicAny/DynAny:1.0") == 0) return 1;
  switch (rorb_dynany_kind((const rorb_dynany*)obj->handle)) {
    case CORBA_tk_struct:
    case CORBA_tk_except: i = 0; break;
    case CORBA_tk_union: i = 1; break;
    case CORBA_tk_enum: i = 2; break;
    case CORBA_tk_sequence: i = 3; break;
    case CORBA_tk_array: i = 4; break;
    case CORBA_tk_fixed: i = 5; break;
    case CORBA_tk_value: i = 6; break;
    case CORBA_tk_value_box: i = 7; break;
    default: break;
  }
  if (i >= 6 && strcmp(id, "IDL:omg.org/DynamicAny/DynValueCommon:1.0") == 0) return 1;
  return i >= 0 && strcmp(id, kind_ids[i]) == 0;
}

static int raw_failed(CORBA_Environment* ev, rorb_any* raw, rustorb_c_err* err) {
  if (raw) return 0;
  rustorb_c_ev_system(ev, err->id ? err->id : ex_CORBA_BAD_PARAM, err->minor, CORBA_COMPLETED_NO);
  return 1;
}

/* A CORBA_any (new) of raw, released; NULL with ev set. */
static CORBA_any* any_of_raw(rorb_any* raw, CORBA_Environment* ev) {
  CORBA_any* a = CORBA_any__alloc();
  if (!a || rustorb_c_any_from_raw(raw, a)) {
    CORBA_free(a);
    a = NULL;
    rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_NO);
  }
  rorb_any_release(raw);
  return a;
}

static char* string_of(rorb_bytes b) {
  char* s = rustorb_c_strndup((const char*)b.data, b.len);
  rorb_bytes_free(b);
  return s;
}

/* --- DynAnyFactory ---------------------------------------------------------------- */

static int factory_ok(DynamicAny_DynAnyFactory f, CORBA_Environment* ev) {
  rustorb_c_ev_clear(ev);
  if (!f || f->kind != OBJ_DYNANY_FACTORY) {
    rustorb_c_ev_system(ev, f ? ex_CORBA_BAD_PARAM : ex_CORBA_INV_OBJREF, 0, CORBA_COMPLETED_NO);
    return 0;
  }
  return 1;
}

static DynamicAny_DynAny create(const CORBA_any* value, int32_t truncate, CORBA_Environment* ev) {
  rustorb_c_err err = {NULL, 0};
  rorb_any* raw = rustorb_c_any_to_raw(value, &err);
  rorb_dynany* d = NULL;
  rorb_exception exc;
  rorb_status st;
  NO_EXC(exc);
  if (raw_failed(ev, raw, &err)) return NULL;
  st = rorb_dynany_create(raw, truncate, &d, &exc);
  rorb_any_release(raw);
  if (rustorb_c_failed(ev, st, &exc)) return NULL;
  return wrap(d);
}

DynamicAny_DynAny DynamicAny_DynAnyFactory_create_dyn_any(DynamicAny_DynAnyFactory _obj,
                                                          const CORBA_any* value,
                                                          CORBA_Environment* ev) {
  return factory_ok(_obj, ev) ? create(value, 1, ev) : NULL;
}

DynamicAny_DynAny DynamicAny_DynAnyFactory_create_dyn_any_without_truncation(
    DynamicAny_DynAnyFactory _obj, const CORBA_any* value, CORBA_Environment* ev) {
  return factory_ok(_obj, ev) ? create(value, 0, ev) : NULL;
}

DynamicAny_DynAny DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(
    DynamicAny_DynAnyFactory _obj, CORBA_TypeCode type, CORBA_Environment* ev) {
  rorb_typecode* tc;
  rorb_dynany* d = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!factory_ok(_obj, ev)) return NULL;
  tc = rustorb_c_tc_handle(type);
  if (!tc) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  if (rustorb_c_failed(ev, rorb_dynany_from_typecode(tc, &d, &exc), &exc)) return NULL;
  return wrap(d);
}

DynamicAny_DynAnySeq* DynamicAny_DynAnyFactory_create_multiple_dyn_anys(
    DynamicAny_DynAnyFactory _obj, const DynamicAny_AnySeq* values, CORBA_boolean allow_truncate,
    CORBA_Environment* ev) {
  DynamicAny_DynAnySeq* r;
  CORBA_unsigned_long i, n = values ? values->_length : 0;
  if (!factory_ok(_obj, ev)) return NULL;
  r = DynamicAny_DynAnySeq__alloc();
  if (!r) return NULL;
  r->_release = CORBA_TRUE;
  if (n) {
    r->_buffer = DynamicAny_DynAnySeq_allocbuf(n);
    r->_maximum = n;
  }
  for (i = 0; i < n; i++) {
    r->_buffer[i] = create(&values->_buffer[i], allow_truncate ? 1 : 0, ev);
    r->_length = i + 1;
    if (ev && ev->_major != CORBA_NO_EXCEPTION) {
      CORBA_free(r);
      return NULL;
    }
  }
  return r;
}

DynamicAny_AnySeq* DynamicAny_DynAnyFactory_create_multiple_anys(DynamicAny_DynAnyFactory _obj,
                                                                 const DynamicAny_DynAnySeq* values,
                                                                 CORBA_Environment* ev) {
  DynamicAny_AnySeq* r;
  CORBA_unsigned_long i, n = values ? values->_length : 0;
  if (!factory_ok(_obj, ev)) return NULL;
  r = DynamicAny_AnySeq__alloc();
  if (!r) return NULL;
  r->_release = CORBA_TRUE;
  if (n) {
    r->_buffer = DynamicAny_AnySeq_allocbuf(n);
    r->_maximum = n;
  }
  for (i = 0; i < n; i++) {
    CORBA_any* a = DynamicAny_DynAny_to_any(values->_buffer[i], ev);
    if (!a) {
      CORBA_free(r);
      return NULL;
    }
    r->_buffer[i] = *a;
    r->_length = i + 1;
    /* The any's contents now belong to the sequence: free the shell only. */
    memset(a, 0, sizeof *a);
    CORBA_free(a);
  }
  return r;
}

/* --- DynAny ---------------------------------------------------------------------- */

CORBA_TypeCode DynamicAny_DynAny_type(DynamicAny_DynAny _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_typecode* tc = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_type(d, &tc, &exc), &exc)) return NULL;
  return rustorb_c_wrap_tc(tc, rustorb_c_desc_for_tc(tc));
}

static const rorb_dynany* other_of(DynamicAny_DynAny obj, CORBA_Environment* ev) {
  if (!obj || obj->kind != OBJ_DYNANY) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return NULL;
  }
  return (const rorb_dynany*)obj->handle;
}

void DynamicAny_DynAny_assign(DynamicAny_DynAny _obj, DynamicAny_DynAny dyn_any,
                              CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  const rorb_dynany* o;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || !(o = other_of(dyn_any, ev))) return;
  rustorb_c_failed(ev, rorb_dynany_assign(d, o, &exc), &exc);
}

void DynamicAny_DynAny_from_any(DynamicAny_DynAny _obj, const CORBA_any* value,
                                CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rustorb_c_err err = {NULL, 0};
  rorb_any* raw;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d) return;
  raw = rustorb_c_any_to_raw(value, &err);
  if (raw_failed(ev, raw, &err)) return;
  rustorb_c_failed(ev, rorb_dynany_from_any(d, raw, &exc), &exc);
  rorb_any_release(raw);
}

CORBA_any* DynamicAny_DynAny_to_any(DynamicAny_DynAny _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_any* raw = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_to_any(d, &raw, &exc), &exc)) return NULL;
  return any_of_raw(raw, ev);
}

CORBA_boolean DynamicAny_DynAny_equal(DynamicAny_DynAny _obj, DynamicAny_DynAny dyn_any,
                                      CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  const rorb_dynany* o;
  int32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || !(o = other_of(dyn_any, ev))) return CORBA_FALSE;
  if (rustorb_c_failed(ev, rorb_dynany_equal(d, o, &r, &exc), &exc)) return CORBA_FALSE;
  return r ? CORBA_TRUE : CORBA_FALSE;
}

void DynamicAny_DynAny_destroy(DynamicAny_DynAny _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (d) rustorb_c_failed(ev, rorb_dynany_destroy(d, &exc), &exc);
}

DynamicAny_DynAny DynamicAny_DynAny_copy(DynamicAny_DynAny _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_dynany* r = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_copy(d, &r, &exc), &exc)) return NULL;
  return wrap(r);
}

/* insert_<type>: the value at value, of the type of tc. */
static void insert(DynamicAny_DynAny obj, uint32_t kind, CORBA_TypeCode tc, const void* value,
                   CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(obj, ev);
  rustorb_c_err err = {NULL, 0};
  CORBA_any a;
  rorb_any* raw;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d) return;
  memset(&a, 0, sizeof a);
  a._type = tc;
  a._value = (void*)value;
  raw = rustorb_c_any_to_raw(&a, &err);
  if (raw_failed(ev, raw, &err)) return;
  rustorb_c_failed(ev, rorb_dynany_insert(d, kind, raw, &exc), &exc);
  rorb_any_release(raw);
}

/* get_<type>: into value (zeroed), of the type of desc; 0 or -1. */
static int get(DynamicAny_DynAny obj, uint32_t kind, const rustorb_c_type* desc, void* value,
               CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(obj, ev);
  rorb_any* raw = NULL;
  rorb_exception exc;
  int r;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_get(d, kind, &raw, &exc), &exc)) return -1;
  r = rustorb_c_raw_value(raw, desc, value);
  rorb_any_release(raw);
  if (r) rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_NO);
  return r;
}

#define DYN_BASIC(name, type, tk, tc, desc)                                              \
  void DynamicAny_DynAny_insert_##name(DynamicAny_DynAny _obj, type value,              \
                                       CORBA_Environment* ev) {                         \
    insert(_obj, tk, tc, &value, ev);                                                   \
  }                                                                                      \
  type DynamicAny_DynAny_get_##name(DynamicAny_DynAny _obj, CORBA_Environment* ev) {    \
    type v;                                                                              \
    memset(&v, 0, sizeof v);                                                             \
    get(_obj, tk, &desc, &v, ev);                                                        \
    return v;                                                                            \
  }
DYN_BASIC(boolean, CORBA_boolean, CORBA_tk_boolean, TC_boolean, CORBA_boolean__desc)
DYN_BASIC(octet, CORBA_octet, CORBA_tk_octet, TC_octet, CORBA_octet__desc)
DYN_BASIC(char, CORBA_char, CORBA_tk_char, TC_char, CORBA_char__desc)
DYN_BASIC(short, CORBA_short, CORBA_tk_short, TC_short, CORBA_short__desc)
DYN_BASIC(ushort, CORBA_unsigned_short, CORBA_tk_ushort, TC_ushort, CORBA_unsigned_short__desc)
DYN_BASIC(long, CORBA_long, CORBA_tk_long, TC_long, CORBA_long__desc)
DYN_BASIC(ulong, CORBA_unsigned_long, CORBA_tk_ulong, TC_ulong, CORBA_unsigned_long__desc)
DYN_BASIC(float, CORBA_float, CORBA_tk_float, TC_float, CORBA_float__desc)
DYN_BASIC(double, CORBA_double, CORBA_tk_double, TC_double, CORBA_double__desc)
DYN_BASIC(longlong, CORBA_long_long, CORBA_tk_longlong, TC_longlong, CORBA_long_long__desc)
DYN_BASIC(ulonglong, CORBA_unsigned_long_long, CORBA_tk_ulonglong, TC_ulonglong,
          CORBA_unsigned_long_long__desc)
DYN_BASIC(longdouble, CORBA_long_double, CORBA_tk_longdouble, TC_longdouble,
          CORBA_long_double__desc)
DYN_BASIC(wchar, CORBA_wchar, CORBA_tk_wchar, TC_wchar, CORBA_wchar__desc)

void DynamicAny_DynAny_insert_string(DynamicAny_DynAny _obj, const CORBA_char* value,
                                     CORBA_Environment* ev) {
  insert(_obj, CORBA_tk_string, TC_string, &value, ev);
}

CORBA_char* DynamicAny_DynAny_get_string(DynamicAny_DynAny _obj, CORBA_Environment* ev) {
  CORBA_char* v = NULL;
  get(_obj, CORBA_tk_string, &CORBA_string__desc, &v, ev);
  return v;
}

void DynamicAny_DynAny_insert_wstring(DynamicAny_DynAny _obj, const CORBA_wchar* value,
                                      CORBA_Environment* ev) {
  insert(_obj, CORBA_tk_wstring, TC_wstring, &value, ev);
}

CORBA_wchar* DynamicAny_DynAny_get_wstring(DynamicAny_DynAny _obj, CORBA_Environment* ev) {
  CORBA_wchar* v = NULL;
  get(_obj, CORBA_tk_wstring, &CORBA_wstring__desc, &v, ev);
  return v;
}

void DynamicAny_DynAny_insert_reference(DynamicAny_DynAny _obj, CORBA_Object value,
                                        CORBA_Environment* ev) {
  insert(_obj, CORBA_tk_objref, TC_Object, &value, ev);
}

CORBA_Object DynamicAny_DynAny_get_reference(DynamicAny_DynAny _obj, CORBA_Environment* ev) {
  CORBA_Object v = NULL;
  get(_obj, CORBA_tk_objref, &CORBA_Object__desc, &v, ev);
  return v;
}

void DynamicAny_DynAny_insert_typecode(DynamicAny_DynAny _obj, CORBA_TypeCode value,
                                       CORBA_Environment* ev) {
  insert(_obj, CORBA_tk_TypeCode, TC_TypeCode, &value, ev);
}

CORBA_TypeCode DynamicAny_DynAny_get_typecode(DynamicAny_DynAny _obj, CORBA_Environment* ev) {
  CORBA_TypeCode v = NULL;
  get(_obj, CORBA_tk_TypeCode, &CORBA_TypeCode__desc, &v, ev);
  return v;
}

void DynamicAny_DynAny_insert_any(DynamicAny_DynAny _obj, const CORBA_any* value,
                                  CORBA_Environment* ev) {
  if (!value) {
    dyn_of(_obj, ev);
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return;
  }
  insert(_obj, CORBA_tk_any, TC_any, value, ev);
}

CORBA_any* DynamicAny_DynAny_get_any(DynamicAny_DynAny _obj, CORBA_Environment* ev) {
  CORBA_any* a = CORBA_any__alloc();
  if (a && get(_obj, CORBA_tk_any, &CORBA_any__desc, a, ev)) {
    CORBA_free(a);
    return NULL;
  }
  return a;
}

void DynamicAny_DynAny_insert_dyn_any(DynamicAny_DynAny _obj, DynamicAny_DynAny value,
                                      CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  const rorb_dynany* o;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || !(o = other_of(value, ev))) return;
  rustorb_c_failed(ev, rorb_dynany_insert_dyn_any(d, o, &exc), &exc);
}

DynamicAny_DynAny DynamicAny_DynAny_get_dyn_any(DynamicAny_DynAny _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_dynany* r = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_get_dyn_any(d, &r, &exc), &exc)) return NULL;
  return wrap(r);
}

CORBA_boolean DynamicAny_DynAny_seek(DynamicAny_DynAny _obj, CORBA_long index,
                                     CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  int32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_seek(d, index, &r, &exc), &exc)) return CORBA_FALSE;
  return r ? CORBA_TRUE : CORBA_FALSE;
}

void DynamicAny_DynAny_rewind(DynamicAny_DynAny _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (d) rustorb_c_failed(ev, rorb_dynany_rewind(d, &exc), &exc);
}

CORBA_boolean DynamicAny_DynAny_next(DynamicAny_DynAny _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  int32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_next(d, &r, &exc), &exc)) return CORBA_FALSE;
  return r ? CORBA_TRUE : CORBA_FALSE;
}

CORBA_unsigned_long DynamicAny_DynAny_component_count(DynamicAny_DynAny _obj,
                                                      CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  uint32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_component_count(d, &r, &exc), &exc)) return 0;
  return r;
}

DynamicAny_DynAny DynamicAny_DynAny_current_component(DynamicAny_DynAny _obj,
                                                      CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_dynany* r = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_current_component(d, &r, &exc), &exc)) return NULL;
  return r ? wrap(r) : CORBA_OBJECT_NIL;
}

/* --- DynFixed, DynEnum -------------------------------------------------------------- */

CORBA_char* DynamicAny_DynFixed_get_value(DynamicAny_DynFixed _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_bytes b = {NULL, 0};
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_get_fixed_value(d, &b, &exc), &exc)) return NULL;
  return string_of(b);
}

CORBA_boolean DynamicAny_DynFixed_set_value(DynamicAny_DynFixed _obj, const CORBA_char* val,
                                            CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  int32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d) return CORBA_FALSE;
  if (!val) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return CORBA_FALSE;
  }
  if (rustorb_c_failed(ev, rorb_dynany_set_fixed_value(d, val, strlen(val), &r, &exc), &exc))
    return CORBA_FALSE;
  return r ? CORBA_TRUE : CORBA_FALSE;
}

CORBA_char* DynamicAny_DynEnum_get_as_string(DynamicAny_DynEnum _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_bytes b = {NULL, 0};
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_get_as_string(d, &b, &exc), &exc)) return NULL;
  return string_of(b);
}

void DynamicAny_DynEnum_set_as_string(DynamicAny_DynEnum _obj, const CORBA_char* value,
                                      CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (!d) return;
  if (!value) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return;
  }
  rustorb_c_failed(ev, rorb_dynany_set_as_string(d, value, strlen(value), &exc), &exc);
}

CORBA_unsigned_long DynamicAny_DynEnum_get_as_ulong(DynamicAny_DynEnum _obj,
                                                    CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  uint32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_get_as_ulong(d, &r, &exc), &exc)) return 0;
  return r;
}

void DynamicAny_DynEnum_set_as_ulong(DynamicAny_DynEnum _obj, CORBA_unsigned_long value,
                                     CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (d) rustorb_c_failed(ev, rorb_dynany_set_as_ulong(d, value, &exc), &exc);
}

/* --- DynStruct ------------------------------------------------------------------------ */

DynamicAny_FieldName DynamicAny_DynStruct_current_member_name(DynamicAny_DynStruct _obj,
                                                              CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_bytes b = {NULL, 0};
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_current_member_name(d, &b, &exc), &exc))
    return NULL;
  return string_of(b);
}

CORBA_TCKind DynamicAny_DynStruct_current_member_kind(DynamicAny_DynStruct _obj,
                                                      CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  uint32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_current_member_kind(d, &r, &exc), &exc))
    return CORBA_tk_null;
  return (CORBA_TCKind)r;
}

/* The members: count and, for each, the name and the member's DynAny. */
static int members(const rorb_dynany* d, uint32_t* count, CORBA_Environment* ev) {
  rorb_exception exc;
  NO_EXC(exc);
  return rustorb_c_failed(ev, rorb_dynany_member_count(d, count, &exc), &exc) ? -1 : 0;
}

DynamicAny_NameValuePairSeq* DynamicAny_DynStruct_get_members(DynamicAny_DynStruct _obj,
                                                              CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  DynamicAny_NameValuePairSeq* r;
  uint32_t i, n = 0;
  if (!d || members(d, &n, ev)) return NULL;
  r = DynamicAny_NameValuePairSeq__alloc();
  if (!r) return NULL;
  r->_release = CORBA_TRUE;
  if (n) {
    r->_buffer = DynamicAny_NameValuePairSeq_allocbuf(n);
    r->_maximum = n;
  }
  for (i = 0; i < n; i++) {
    rorb_bytes name = {NULL, 0};
    rorb_dynany* m = NULL;
    rorb_any* raw = NULL;
    rorb_exception exc;
    rorb_status st;
    NO_EXC(exc);
    st = rorb_dynany_member_at(d, i, &name, &m, &exc);
    if (st == RORB_OK) {
      st = rorb_dynany_to_any(m, &raw, &exc);
      rorb_dynany_release(m);
    }
    if (rustorb_c_failed(ev, st, &exc)) {
      rorb_bytes_free(name);
      CORBA_free(r);
      return NULL;
    }
    r->_buffer[i].id = string_of(name);
    r->_length = i + 1;
    if (rustorb_c_any_from_raw(raw, &r->_buffer[i].value)) {
      rorb_any_release(raw);
      CORBA_free(r);
      rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_NO);
      return NULL;
    }
    rorb_any_release(raw);
  }
  return r;
}

DynamicAny_NameDynAnyPairSeq* DynamicAny_DynStruct_get_members_as_dyn_any(
    DynamicAny_DynStruct _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  DynamicAny_NameDynAnyPairSeq* r;
  uint32_t i, n = 0;
  if (!d || members(d, &n, ev)) return NULL;
  r = DynamicAny_NameDynAnyPairSeq__alloc();
  if (!r) return NULL;
  r->_release = CORBA_TRUE;
  if (n) {
    r->_buffer = DynamicAny_NameDynAnyPairSeq_allocbuf(n);
    r->_maximum = n;
  }
  for (i = 0; i < n; i++) {
    rorb_bytes name = {NULL, 0};
    rorb_dynany* m = NULL;
    rorb_exception exc;
    NO_EXC(exc);
    if (rustorb_c_failed(ev, rorb_dynany_member_at(d, i, &name, &m, &exc), &exc)) {
      CORBA_free(r);
      return NULL;
    }
    r->_buffer[i].id = string_of(name);
    r->_buffer[i].value = wrap(m);
    r->_length = i + 1;
  }
  return r;
}

void DynamicAny_DynStruct_set_members(DynamicAny_DynStruct _obj,
                                      const DynamicAny_NameValuePairSeq* value,
                                      CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  size_t i, n = value ? value->_length : 0;
  const char** names;
  size_t* lens;
  rorb_any** raws;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d) return;
  names = (const char**)calloc(n + 1, sizeof *names);
  lens = (size_t*)calloc(n + 1, sizeof *lens);
  raws = (rorb_any**)calloc(n + 1, sizeof *raws);
  if (names && lens && raws) {
    int ok = 1;
    for (i = 0; i < n && ok; i++) {
      rustorb_c_err err = {NULL, 0};
      names[i] = value->_buffer[i].id ? value->_buffer[i].id : "";
      lens[i] = strlen(names[i]);
      raws[i] = rustorb_c_any_to_raw(&value->_buffer[i].value, &err);
      if (raw_failed(ev, raws[i], &err)) ok = 0;
    }
    if (ok)
      rustorb_c_failed(ev,
                       rorb_dynany_set_members(d, n, names, lens, (const rorb_any* const*)raws, &exc),
                       &exc);
    for (i = 0; i < n; i++) rorb_any_release(raws[i]);
  } else {
    rustorb_c_ev_system(ev, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
  }
  free(names);
  free(lens);
  free(raws);
}

void DynamicAny_DynStruct_set_members_as_dyn_any(DynamicAny_DynStruct _obj,
                                                 const DynamicAny_NameDynAnyPairSeq* value,
                                                 CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  size_t i, n = value ? value->_length : 0;
  const char** names;
  size_t* lens;
  const rorb_dynany** dyns;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d) return;
  names = (const char**)calloc(n + 1, sizeof *names);
  lens = (size_t*)calloc(n + 1, sizeof *lens);
  dyns = (const rorb_dynany**)calloc(n + 1, sizeof *dyns);
  if (names && lens && dyns) {
    int ok = 1;
    for (i = 0; i < n && ok; i++) {
      names[i] = value->_buffer[i].id ? value->_buffer[i].id : "";
      lens[i] = strlen(names[i]);
      if (!(dyns[i] = other_of(value->_buffer[i].value, ev))) ok = 0;
    }
    if (ok)
      rustorb_c_failed(ev, rorb_dynany_set_members_as_dyn_any(d, n, names, lens, dyns, &exc),
                       &exc);
  } else {
    rustorb_c_ev_system(ev, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
  }
  free(names);
  free(lens);
  free(dyns);
}

/* --- DynUnion ------------------------------------------------------------------------- */

DynamicAny_DynAny DynamicAny_DynUnion_get_discriminator(DynamicAny_DynUnion _obj,
                                                        CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_dynany* r = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_get_discriminator(d, &r, &exc), &exc)) return NULL;
  return wrap(r);
}

void DynamicAny_DynUnion_set_discriminator(DynamicAny_DynUnion _obj, DynamicAny_DynAny disc,
                                           CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  const rorb_dynany* o;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || !(o = other_of(disc, ev))) return;
  rustorb_c_failed(ev, rorb_dynany_set_discriminator(d, o, &exc), &exc);
}

void DynamicAny_DynUnion_set_to_default_member(DynamicAny_DynUnion _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (d) rustorb_c_failed(ev, rorb_dynany_set_to_default_member(d, &exc), &exc);
}

void DynamicAny_DynUnion_set_to_no_active_member(DynamicAny_DynUnion _obj,
                                                 CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (d) rustorb_c_failed(ev, rorb_dynany_set_to_no_active_member(d, &exc), &exc);
}

CORBA_boolean DynamicAny_DynUnion_has_no_active_member(DynamicAny_DynUnion _obj,
                                                       CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  int32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_has_no_active_member(d, &r, &exc), &exc))
    return CORBA_FALSE;
  return r ? CORBA_TRUE : CORBA_FALSE;
}

CORBA_TCKind DynamicAny_DynUnion_discriminator_kind(DynamicAny_DynUnion _obj,
                                                    CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  uint32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_discriminator_kind(d, &r, &exc), &exc))
    return CORBA_tk_null;
  return (CORBA_TCKind)r;
}

DynamicAny_DynAny DynamicAny_DynUnion_member(DynamicAny_DynUnion _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_dynany* r = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_member(d, &r, &exc), &exc)) return NULL;
  return wrap(r);
}

DynamicAny_FieldName DynamicAny_DynUnion_member_name(DynamicAny_DynUnion _obj,
                                                     CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_bytes b = {NULL, 0};
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_member_name(d, &b, &exc), &exc)) return NULL;
  return string_of(b);
}

CORBA_TCKind DynamicAny_DynUnion_member_kind(DynamicAny_DynUnion _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  uint32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_member_kind(d, &r, &exc), &exc))
    return CORBA_tk_null;
  return (CORBA_TCKind)r;
}

/* --- DynSequence, DynArray ---------------------------------------------------------------- */

CORBA_unsigned_long DynamicAny_DynSequence_get_length(DynamicAny_DynSequence _obj,
                                                      CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  uint32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_get_length(d, &r, &exc), &exc)) return 0;
  return r;
}

void DynamicAny_DynSequence_set_length(DynamicAny_DynSequence _obj, CORBA_unsigned_long len,
                                       CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (d) rustorb_c_failed(ev, rorb_dynany_set_length(d, len, &exc), &exc);
}

static DynamicAny_AnySeq* get_elements(DynamicAny_DynAny obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(obj, ev);
  DynamicAny_AnySeq* r;
  uint32_t i, n = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_element_count(d, &n, &exc), &exc)) return NULL;
  r = DynamicAny_AnySeq__alloc();
  if (!r) return NULL;
  r->_release = CORBA_TRUE;
  if (n) {
    r->_buffer = DynamicAny_AnySeq_allocbuf(n);
    r->_maximum = n;
  }
  for (i = 0; i < n; i++) {
    rorb_dynany* e = NULL;
    rorb_any* raw = NULL;
    rorb_status st = rorb_dynany_element_at(d, i, &e, &exc);
    if (st == RORB_OK) {
      st = rorb_dynany_to_any(e, &raw, &exc);
      rorb_dynany_release(e);
    }
    if (rustorb_c_failed(ev, st, &exc)) {
      CORBA_free(r);
      return NULL;
    }
    r->_length = i + 1;
    if (rustorb_c_any_from_raw(raw, &r->_buffer[i])) {
      rorb_any_release(raw);
      CORBA_free(r);
      rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_NO);
      return NULL;
    }
    rorb_any_release(raw);
  }
  return r;
}

static DynamicAny_DynAnySeq* get_elements_as_dyn_any(DynamicAny_DynAny obj,
                                                     CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(obj, ev);
  DynamicAny_DynAnySeq* r;
  uint32_t i, n = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_element_count(d, &n, &exc), &exc)) return NULL;
  r = DynamicAny_DynAnySeq__alloc();
  if (!r) return NULL;
  r->_release = CORBA_TRUE;
  if (n) {
    r->_buffer = DynamicAny_DynAnySeq_allocbuf(n);
    r->_maximum = n;
  }
  for (i = 0; i < n; i++) {
    rorb_dynany* e = NULL;
    if (rustorb_c_failed(ev, rorb_dynany_element_at(d, i, &e, &exc), &exc)) {
      CORBA_free(r);
      return NULL;
    }
    r->_buffer[i] = wrap(e);
    r->_length = i + 1;
  }
  return r;
}

static void set_elements(DynamicAny_DynAny obj, const DynamicAny_AnySeq* value,
                         CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(obj, ev);
  size_t i, n = value ? value->_length : 0;
  rorb_any** raws;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d) return;
  raws = (rorb_any**)calloc(n + 1, sizeof *raws);
  if (!raws) {
    rustorb_c_ev_system(ev, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
    return;
  }
  for (i = 0; i < n; i++) {
    rustorb_c_err err = {NULL, 0};
    raws[i] = rustorb_c_any_to_raw(&value->_buffer[i], &err);
    if (raw_failed(ev, raws[i], &err)) break;
  }
  if (i == n)
    rustorb_c_failed(ev, rorb_dynany_set_elements(d, n, (const rorb_any* const*)raws, &exc), &exc);
  for (i = 0; i < n; i++) rorb_any_release(raws[i]);
  free(raws);
}

static void set_elements_as_dyn_any(DynamicAny_DynAny obj, const DynamicAny_DynAnySeq* value,
                                    CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(obj, ev);
  size_t i, n = value ? value->_length : 0;
  const rorb_dynany** dyns;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d) return;
  dyns = (const rorb_dynany**)calloc(n + 1, sizeof *dyns);
  if (!dyns) {
    rustorb_c_ev_system(ev, ex_CORBA_NO_MEMORY, 0, CORBA_COMPLETED_NO);
    return;
  }
  for (i = 0; i < n; i++)
    if (!(dyns[i] = other_of(value->_buffer[i], ev))) break;
  if (i == n) rustorb_c_failed(ev, rorb_dynany_set_elements_as_dyn_any(d, n, dyns, &exc), &exc);
  free(dyns);
}

DynamicAny_AnySeq* DynamicAny_DynSequence_get_elements(DynamicAny_DynSequence _obj,
                                                       CORBA_Environment* ev) {
  return get_elements(_obj, ev);
}
void DynamicAny_DynSequence_set_elements(DynamicAny_DynSequence _obj,
                                         const DynamicAny_AnySeq* value, CORBA_Environment* ev) {
  set_elements(_obj, value, ev);
}
DynamicAny_DynAnySeq* DynamicAny_DynSequence_get_elements_as_dyn_any(
    DynamicAny_DynSequence _obj, CORBA_Environment* ev) {
  return get_elements_as_dyn_any(_obj, ev);
}
void DynamicAny_DynSequence_set_elements_as_dyn_any(DynamicAny_DynSequence _obj,
                                                    const DynamicAny_DynAnySeq* value,
                                                    CORBA_Environment* ev) {
  set_elements_as_dyn_any(_obj, value, ev);
}
DynamicAny_AnySeq* DynamicAny_DynArray_get_elements(DynamicAny_DynArray _obj,
                                                    CORBA_Environment* ev) {
  return get_elements(_obj, ev);
}
void DynamicAny_DynArray_set_elements(DynamicAny_DynArray _obj, const DynamicAny_AnySeq* value,
                                      CORBA_Environment* ev) {
  set_elements(_obj, value, ev);
}
DynamicAny_DynAnySeq* DynamicAny_DynArray_get_elements_as_dyn_any(DynamicAny_DynArray _obj,
                                                                  CORBA_Environment* ev) {
  return get_elements_as_dyn_any(_obj, ev);
}
void DynamicAny_DynArray_set_elements_as_dyn_any(DynamicAny_DynArray _obj,
                                                 const DynamicAny_DynAnySeq* value,
                                                 CORBA_Environment* ev) {
  set_elements_as_dyn_any(_obj, value, ev);
}

/* --- insert_<type>_seq, get_<type>_seq ------------------------------------------- */

/* The values as an any of an unbounded sequence of the element type. */
static void insert_seq(DynamicAny_DynAny obj, uint32_t kind, const rustorb_c_type* seq_desc,
                       const void* value, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(obj, ev);
  rustorb_c_err err = {NULL, 0};
  CORBA_any a;
  rorb_any* raw;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d) return;
  if (!value) {
    rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
    return;
  }
  memset(&a, 0, sizeof a);
  a._type = rustorb_c_type_tc(seq_desc, NULL);
  a._value = (void*)value;
  raw = rustorb_c_any_to_raw(&a, &err);
  if (raw_failed(ev, raw, &err)) return;
  rustorb_c_failed(ev, rorb_dynany_insert_seq(d, kind, raw, &exc), &exc);
  rorb_any_release(raw);
}

static void* get_seq(DynamicAny_DynAny obj, uint32_t kind, const rustorb_c_type* seq_desc,
                     CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(obj, ev);
  rorb_any* raw = NULL;
  rorb_exception exc;
  void* r;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_get_seq(d, kind, &raw, &exc), &exc)) return NULL;
  r = rustorb_c_sequence_alloc();
  if (r && rustorb_c_raw_value(raw, seq_desc, r)) {
    CORBA_free(r);
    r = NULL;
    rustorb_c_ev_system(ev, ex_CORBA_MARSHAL, 0, CORBA_COMPLETED_NO);
  }
  rorb_any_release(raw);
  return r;
}

#define DYN_SEQ(name, cap, elem, tk)                                                        \
  static rustorb_c_cache name##_seq_cache;                                                  \
  static const rustorb_c_type name##_seq_desc = {CORBA_tk_sequence,                        \
                                                 sizeof(CORBA_sequence_octet),             \
                                                 NULL,                                     \
                                                 NULL,                                     \
                                                 &elem##__desc,                            \
                                                 0,                                        \
                                                 0,                                        \
                                                 NULL,                                     \
                                                 NULL,                                     \
                                                 -1,                                       \
                                                 0,                                        \
                                                 0,                                        \
                                                 &name##_seq_cache};                       \
  void DynamicAny_DynAny_insert_##name##_seq(DynamicAny_DynAny _obj,                       \
                                             const CORBA_##cap##Seq* value,                \
                                             CORBA_Environment* ev) {                      \
    insert_seq(_obj, tk, &name##_seq_desc, value, ev);                                      \
  }                                                                                         \
  CORBA_##cap##Seq* DynamicAny_DynAny_get_##name##_seq(DynamicAny_DynAny _obj,             \
                                                       CORBA_Environment* ev) {            \
    return (CORBA_##cap##Seq*)get_seq(_obj, tk, &name##_seq_desc, ev);                      \
  }
DYN_SEQ(boolean, Boolean, CORBA_boolean, CORBA_tk_boolean)
DYN_SEQ(octet, Octet, CORBA_octet, CORBA_tk_octet)
DYN_SEQ(char, Char, CORBA_char, CORBA_tk_char)
DYN_SEQ(wchar, WChar, CORBA_wchar, CORBA_tk_wchar)
DYN_SEQ(short, Short, CORBA_short, CORBA_tk_short)
DYN_SEQ(ushort, UShort, CORBA_unsigned_short, CORBA_tk_ushort)
DYN_SEQ(long, Long, CORBA_long, CORBA_tk_long)
DYN_SEQ(ulong, ULong, CORBA_unsigned_long, CORBA_tk_ulong)
DYN_SEQ(longlong, LongLong, CORBA_long_long, CORBA_tk_longlong)
DYN_SEQ(ulonglong, ULongLong, CORBA_unsigned_long_long, CORBA_tk_ulonglong)
DYN_SEQ(float, Float, CORBA_float, CORBA_tk_float)
DYN_SEQ(double, Double, CORBA_double, CORBA_tk_double)
DYN_SEQ(longdouble, LongDouble, CORBA_long_double, CORBA_tk_longdouble)

/* --- DynValueCommon, DynValue, DynValueBox ---------------------------------------- */

CORBA_boolean DynamicAny_DynValueCommon_is_null(DynamicAny_DynValueCommon _obj,
                                                CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  int32_t r = 0;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_is_null(d, &r, &exc), &exc)) return CORBA_FALSE;
  return r ? CORBA_TRUE : CORBA_FALSE;
}

void DynamicAny_DynValueCommon_set_to_null(DynamicAny_DynValueCommon _obj, CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (d) rustorb_c_failed(ev, rorb_dynany_set_to_null(d, &exc), &exc);
}

void DynamicAny_DynValueCommon_set_to_value(DynamicAny_DynValueCommon _obj,
                                            CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_exception exc;
  NO_EXC(exc);
  if (d) rustorb_c_failed(ev, rorb_dynany_set_to_value(d, &exc), &exc);
}

/* The member operations of DynValue are those of DynStruct, on the core
   DynAny of a valuetype. */
DynamicAny_FieldName DynamicAny_DynValue_current_member_name(DynamicAny_DynValue _obj,
                                                             CORBA_Environment* ev) {
  return DynamicAny_DynStruct_current_member_name(_obj, ev);
}

CORBA_TCKind DynamicAny_DynValue_current_member_kind(DynamicAny_DynValue _obj,
                                                     CORBA_Environment* ev) {
  return DynamicAny_DynStruct_current_member_kind(_obj, ev);
}

DynamicAny_NameValuePairSeq* DynamicAny_DynValue_get_members(DynamicAny_DynValue _obj,
                                                             CORBA_Environment* ev) {
  return DynamicAny_DynStruct_get_members(_obj, ev);
}

void DynamicAny_DynValue_set_members(DynamicAny_DynValue _obj,
                                     const DynamicAny_NameValuePairSeq* value,
                                     CORBA_Environment* ev) {
  DynamicAny_DynStruct_set_members(_obj, value, ev);
}

DynamicAny_NameDynAnyPairSeq* DynamicAny_DynValue_get_members_as_dyn_any(
    DynamicAny_DynValue _obj, CORBA_Environment* ev) {
  return DynamicAny_DynStruct_get_members_as_dyn_any(_obj, ev);
}

void DynamicAny_DynValue_set_members_as_dyn_any(DynamicAny_DynValue _obj,
                                                const DynamicAny_NameDynAnyPairSeq* value,
                                                CORBA_Environment* ev) {
  DynamicAny_DynStruct_set_members_as_dyn_any(_obj, value, ev);
}

CORBA_any* DynamicAny_DynValueBox_get_boxed_value(DynamicAny_DynValueBox _obj,
                                                  CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_any* raw = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_get_boxed_value(d, &raw, &exc), &exc)) return NULL;
  return any_of_raw(raw, ev);
}

void DynamicAny_DynValueBox_set_boxed_value(DynamicAny_DynValueBox _obj, const CORBA_any* boxed,
                                            CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rustorb_c_err err = {NULL, 0};
  rorb_any* raw;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d) return;
  raw = rustorb_c_any_to_raw(boxed, &err);
  if (raw_failed(ev, raw, &err)) return;
  rustorb_c_failed(ev, rorb_dynany_set_boxed_value(d, raw, &exc), &exc);
  rorb_any_release(raw);
}

DynamicAny_DynAny DynamicAny_DynValueBox_get_boxed_value_as_dyn_any(DynamicAny_DynValueBox _obj,
                                                                   CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  rorb_dynany* r = NULL;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || rustorb_c_failed(ev, rorb_dynany_get_boxed_value_as_dyn_any(d, &r, &exc), &exc))
    return NULL;
  return wrap(r);
}

void DynamicAny_DynValueBox_set_boxed_value_as_dyn_any(DynamicAny_DynValueBox _obj,
                                                       DynamicAny_DynAny boxed,
                                                       CORBA_Environment* ev) {
  const rorb_dynany* d = dyn_of(_obj, ev);
  const rorb_dynany* o;
  rorb_exception exc;
  NO_EXC(exc);
  if (!d || !(o = other_of(boxed, ev))) return;
  rustorb_c_failed(ev, rorb_dynany_set_boxed_value_as_dyn_any(d, o, &exc), &exc);
}
