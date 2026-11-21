/* Descriptors, their TypeCodes and the TypeCode pseudo object. */

#include "internal.h"

#define BASIC(name, ctype, tk)                                                  \
  static rustorb_c_cache name##__cache;                                         \
  const rustorb_c_type name##__desc = {tk, sizeof(ctype), NULL, NULL, NULL, 0, 0, \
                                       NULL, NULL, -1, 0, 0, &name##__cache}

BASIC(CORBA_short, CORBA_short, CORBA_tk_short);
BASIC(CORBA_long, CORBA_long, CORBA_tk_long);
BASIC(CORBA_long_long, CORBA_long_long, CORBA_tk_longlong);
BASIC(CORBA_unsigned_short, CORBA_unsigned_short, CORBA_tk_ushort);
BASIC(CORBA_unsigned_long, CORBA_unsigned_long, CORBA_tk_ulong);
BASIC(CORBA_unsigned_long_long, CORBA_unsigned_long_long, CORBA_tk_ulonglong);
BASIC(CORBA_float, CORBA_float, CORBA_tk_float);
BASIC(CORBA_double, CORBA_double, CORBA_tk_double);
BASIC(CORBA_long_double, CORBA_long_double, CORBA_tk_longdouble);
BASIC(CORBA_boolean, CORBA_boolean, CORBA_tk_boolean);
BASIC(CORBA_char, CORBA_char, CORBA_tk_char);
BASIC(CORBA_wchar, CORBA_wchar, CORBA_tk_wchar);
BASIC(CORBA_octet, CORBA_octet, CORBA_tk_octet);
BASIC(CORBA_string, CORBA_char*, CORBA_tk_string);
BASIC(CORBA_wstring, CORBA_wchar*, CORBA_tk_wstring);
BASIC(CORBA_any, CORBA_any, CORBA_tk_any);
BASIC(CORBA_TypeCode, CORBA_TypeCode, CORBA_tk_TypeCode);
static rustorb_c_cache CORBA_Object__cache;
const rustorb_c_type CORBA_Object__desc = {
    CORBA_tk_objref, sizeof(CORBA_Object), "IDL:omg.org/CORBA/Object:1.0", "Object", NULL, 0, 0,
    NULL, NULL, -1, 0, 0, &CORBA_Object__cache};

const rustorb_c_type CORBA_sequence__desc = {
    CORBA_tk_sequence, sizeof(CORBA_sequence_octet), NULL, NULL, NULL, 0, 0,
    NULL, NULL, -1, 0, 0, NULL};

static const rustorb_c_type* basic_desc(uint32_t kind) {
  switch (kind) {
    case CORBA_tk_short: return &CORBA_short__desc;
    case CORBA_tk_long: return &CORBA_long__desc;
    case CORBA_tk_longlong: return &CORBA_long_long__desc;
    case CORBA_tk_ushort: return &CORBA_unsigned_short__desc;
    case CORBA_tk_ulong: return &CORBA_unsigned_long__desc;
    case CORBA_tk_ulonglong: return &CORBA_unsigned_long_long__desc;
    case CORBA_tk_float: return &CORBA_float__desc;
    case CORBA_tk_double: return &CORBA_double__desc;
    case CORBA_tk_longdouble: return &CORBA_long_double__desc;
    case CORBA_tk_boolean: return &CORBA_boolean__desc;
    case CORBA_tk_char: return &CORBA_char__desc;
    case CORBA_tk_wchar: return &CORBA_wchar__desc;
    case CORBA_tk_octet: return &CORBA_octet__desc;
    case CORBA_tk_string: return &CORBA_string__desc;
    case CORBA_tk_wstring: return &CORBA_wstring__desc;
    case CORBA_tk_any: return &CORBA_any__desc;
    case CORBA_tk_TypeCode: return &CORBA_TypeCode__desc;
    case CORBA_tk_objref:
    case CORBA_tk_abstract_interface:
    case CORBA_tk_local_interface:
      return &CORBA_Object__desc;
    default: return NULL;
  }
}

/* --- object wrappers --------------------------------------------------------- */

CORBA_Object rustorb_c_wrap(int kind, void* handle) {
  CORBA_Object o;
  if (!handle && kind != OBJ_CURRENT && kind != OBJ_POLICY && kind != OBJ_DYNANY_FACTORY)
    return CORBA_OBJECT_NIL;
  o = (CORBA_Object)calloc(1, sizeof *o);
  if (!o) return CORBA_OBJECT_NIL;
  o->kind = kind;
  o->handle = handle;
  return o;
}

CORBA_TypeCode rustorb_c_wrap_tc(rorb_typecode* tc, const rustorb_c_type* desc) {
  CORBA_TypeCode o = rustorb_c_wrap(OBJ_TYPECODE, tc);
  if (o) o->desc = desc;
  return o;
}

rorb_typecode* rustorb_c_tc_handle(CORBA_TypeCode tc) {
  return tc && tc->kind == OBJ_TYPECODE ? (rorb_typecode*)tc->handle : NULL;
}

/* --- the registry of named types ------------------------------------------------ */

static const rustorb_c_type** registry;
static size_t registry_len, registry_cap;

/* Under the lock. */
static void register_type(const rustorb_c_type* t) {
  if (!t->id || !t->cache || t->cache->registered) return;
  t->cache->registered = 1;
  if (registry_len == registry_cap) {
    size_t cap = registry_cap ? registry_cap * 2 : 64;
    const rustorb_c_type** r = (const rustorb_c_type**)realloc((void*)registry, cap * sizeof *r);
    if (!r) return;
    registry = r;
    registry_cap = cap;
  }
  registry[registry_len++] = t;
}

void rustorb_c_register_module(const rustorb_c_module* module) {
  CORBA_unsigned_long i;
  if (!module || *module->registered) return;
  rustorb_c_lock();
  if (!*module->registered) {
    for (i = 0; i < module->count; i++) register_type(module->types[i]);
    *module->registered = 1;
  }
  rustorb_c_unlock();
}

/* Under the lock. */
static const rustorb_c_type* lookup(const char* id, size_t len) {
  size_t i;
  for (i = 0; i < registry_len; i++) {
    const char* rid = registry[i]->id;
    if (strlen(rid) == len && memcmp(rid, id, len) == 0) return registry[i];
  }
  return NULL;
}

/* --- TypeCodes of descriptors -------------------------------------------------------- */

typedef struct building {
  const rustorb_c_type* type;
  const struct building* up;
} building;

static rorb_typecode* build(const rustorb_c_type* t, const building* up);

static rorb_typecode* build_members(const rustorb_c_type* t, const building* up) {
  building here;
  CORBA_unsigned_long i, n = t->count;
  const char** names = (const char**)calloc(n ? n : 1, sizeof *names);
  size_t* lens = (size_t*)calloc(n ? n : 1, sizeof *lens);
  rorb_typecode** types = (rorb_typecode**)calloc(n ? n : 1, sizeof *types);
  uint64_t* labels = (uint64_t*)calloc(n ? n : 1, sizeof *labels);
  rorb_typecode* r = NULL;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  here.type = t;
  here.up = up;
  if (!names || !lens || !types || !labels) goto done;
  for (i = 0; i < n; i++) {
    names[i] = t->members ? t->members[i].name : t->enumerators[i];
    lens[i] = strlen(names[i]);
    if (t->members) {
      labels[i] = t->members[i].label;
      types[i] = build(t->members[i].type, &here);
      if (!types[i]) goto done;
    }
  }
  if (t->kind == CORBA_tk_enum) {
    rorb_tc_enum(t->id, strlen(t->id), t->name, strlen(t->name), n, names, lens, &r, &exc);
  } else if (t->kind == CORBA_tk_union) {
    rorb_typecode* d = build(t->content, &here);
    if (d)
      rorb_tc_union(t->id, strlen(t->id), t->name, strlen(t->name), d, n, labels,
                    t->default_index, names, lens, (const rorb_typecode* const*)types, &r, &exc);
    rorb_tc_release(d);
  } else {
    rorb_tc_struct(t->kind, t->id, strlen(t->id), t->name, strlen(t->name), n, names, lens,
                   (const rorb_typecode* const*)types, &r, &exc);
  }
done:
  if (types)
    for (i = 0; i < n; i++) rorb_tc_release(types[i]);
  free((void*)names);
  free(lens);
  free(types);
  free(labels);
  rorb_exception_clear(&exc);
  return r;
}

/* Under the lock. */
static rorb_typecode* build(const rustorb_c_type* t, const building* up) {
  rorb_typecode* r = NULL;
  rorb_typecode* c;
  rorb_exception exc;
  const building* b;
  memset(&exc, 0, sizeof exc);
  if (!t) return NULL;
  if (t->cache && t->cache->tc) return rorb_tc_clone((rorb_typecode*)t->cache->tc->handle);
  for (b = up; b; b = b->up)
    if (b->type == t) {
      rorb_tc_recursive(t->id, strlen(t->id), &r, &exc);
      rorb_exception_clear(&exc);
      return r;
    }
  switch (t->kind) {
    case CORBA_tk_string:
    case CORBA_tk_wstring:
      rorb_tc_string(t->kind == CORBA_tk_wstring, t->length, &r, &exc);
      break;
    case CORBA_tk_sequence:
    case CORBA_tk_array:
      c = build(t->content, up);
      if (c) {
        if (t->kind == CORBA_tk_sequence)
          rorb_tc_sequence(c, t->length, &r, &exc);
        else
          rorb_tc_array(c, t->length, &r, &exc);
      }
      rorb_tc_release(c);
      break;
    case CORBA_tk_alias:
      c = build(t->content, up);
      if (c)
        rorb_tc_alias(RORB_TK_ALIAS, t->id, strlen(t->id), t->name, strlen(t->name), c, &r, &exc);
      rorb_tc_release(c);
      break;
    case CORBA_tk_objref:
    case CORBA_tk_abstract_interface:
    case CORBA_tk_local_interface:
      rorb_tc_interface(t->kind, t->id, strlen(t->id), t->name, strlen(t->name), &r, &exc);
      break;
    case CORBA_tk_struct:
    case CORBA_tk_except:
    case CORBA_tk_union:
    case CORBA_tk_enum:
      r = build_members(t, up);
      break;
    case CORBA_tk_fixed:
      rorb_tc_fixed(t->digits, t->scale, &r, &exc);
      break;
    default:
      rorb_tc_simple(t->kind, &r, &exc);
      break;
  }
  rorb_exception_clear(&exc);
  return r;
}

CORBA_TypeCode rustorb_c_type_tc(const rustorb_c_type* type, const rustorb_c_module* module) {
  CORBA_TypeCode tc;
  if (module) rustorb_c_register_module(module);
  if (!type) return CORBA_OBJECT_NIL;
  rustorb_c_lock();
  if (type->cache && type->cache->tc) {
    tc = type->cache->tc;
  } else {
    rorb_typecode* r = build(type, NULL);
    tc = rustorb_c_wrap_tc(r, type);
    if (tc) {
      tc->immortal = 1;
      if (type->cache) {
        type->cache->tc = tc;
        register_type(type);
      }
    }
  }
  rustorb_c_unlock();
  return tc;
}

CORBA_TypeCode rustorb_c_tc_simple(CORBA_TCKind kind) {
  static CORBA_TypeCode simple[CORBA_tk_event + 1];
  CORBA_TypeCode tc;
  if ((unsigned)kind > CORBA_tk_event) return CORBA_OBJECT_NIL;
  rustorb_c_lock();
  tc = simple[kind];
  if (!tc) {
    rorb_typecode* r = NULL;
    rorb_exception exc;
    memset(&exc, 0, sizeof exc);
    if (kind == CORBA_tk_objref)
      rorb_tc_interface(RORB_TK_OBJREF, CORBA_Object__desc.id, strlen(CORBA_Object__desc.id),
                        "Object", 6, &r, &exc);
    else
      rorb_tc_simple(kind, &r, &exc);
    rorb_exception_clear(&exc);
    tc = rustorb_c_wrap_tc(r, basic_desc(kind));
    if (tc) tc->immortal = 1;
    simple[kind] = tc;
  }
  rustorb_c_unlock();
  return tc;
}

/* --- descriptors of TypeCodes ---------------------------------------------------- */

typedef struct made {
  rorb_typecode* tc;
  rustorb_c_type* type;
  struct made* next;
} made;

static made* made_types;

static const rustorb_c_type* desc_for(const rorb_typecode* tc);

/* A descriptor for an anonymous sequence, array or fixed (kept for the life
   of the program). Under the lock. */
static const rustorb_c_type* make_desc(const rorb_typecode* tc, uint32_t kind) {
  made* m;
  rustorb_c_type* t;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  for (m = made_types; m; m = m->next)
    if (rorb_tc_equal(m->tc, tc)) return m->type;
  t = (rustorb_c_type*)calloc(1, sizeof *t);
  m = (made*)calloc(1, sizeof *m);
  if (!t || !m) {
    free(t);
    free(m);
    return NULL;
  }
  t->kind = kind;
  t->default_index = -1;
  if (kind == CORBA_tk_fixed) {
    uint16_t d = 0;
    int16_t s = 0;
    rorb_tc_fixed_digits(tc, &d, &exc);
    rorb_tc_fixed_scale(tc, &s, &exc);
    t->digits = d;
    t->scale = s;
    t->size = 2 * sizeof(CORBA_short) + (size_t)(d + 2) / 2;
    /* CORBA_fixed_d_s: two shorts and the octets, padded to a short. */
    t->size = (t->size + 1) & ~(size_t)1;
  } else {
    rorb_typecode* c = NULL;
    uint32_t len = 0;
    rorb_tc_content_type(tc, &c, &exc);
    rorb_tc_length(tc, &len, &exc);
    t->content = c ? desc_for(c) : NULL;
    rorb_tc_release(c);
    if (!t->content) {
      free(t);
      free(m);
      rorb_exception_clear(&exc);
      return NULL;
    }
    t->length = len;
    t->size = kind == CORBA_tk_sequence ? sizeof(CORBA_sequence_octet) : len * t->content->size;
  }
  rorb_exception_clear(&exc);
  m->tc = rorb_tc_clone(tc);
  m->type = t;
  m->next = made_types;
  made_types = m;
  return t;
}

/* Under the lock. */
static const rustorb_c_type* desc_for(const rorb_typecode* tc) {
  uint32_t kind = rorb_tc_kind(tc);
  const rustorb_c_type* t = basic_desc(kind);
  const char* id;
  size_t len;
  rorb_exception exc;
  if (t) return t;
  memset(&exc, 0, sizeof exc);
  switch (kind) {
    case CORBA_tk_struct:
    case CORBA_tk_union:
    case CORBA_tk_enum:
    case CORBA_tk_except:
    case CORBA_tk_alias:
      if (rorb_tc_id(tc, &id, &len, &exc) != RORB_OK) break;
      t = lookup(id, len);
      if (!t && kind == CORBA_tk_alias) {
        rorb_typecode* c = NULL;
        rorb_tc_content_type(tc, &c, &exc);
        if (c) t = desc_for(c);
        rorb_tc_release(c);
      }
      break;
    case CORBA_tk_sequence:
    case CORBA_tk_array:
    case CORBA_tk_fixed:
      t = make_desc(tc, kind);
      break;
    default:
      break;
  }
  rorb_exception_clear(&exc);
  return t;
}

const rustorb_c_type* rustorb_c_desc_for_tc(const rorb_typecode* tc) {
  const rustorb_c_type* t;
  if (!tc) return NULL;
  rustorb_c_lock();
  t = desc_for(tc);
  rustorb_c_unlock();
  return t;
}

/* --- the TypeCode pseudo object -------------------------------------------------- */

static rorb_typecode* tc_of(CORBA_TypeCode tc, CORBA_Environment* ev) {
  rorb_typecode* h = rustorb_c_tc_handle(tc);
  rustorb_c_ev_clear(ev);
  if (!h) rustorb_c_ev_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
  return h;
}

CORBA_TCKind CORBA_TypeCode_kind(CORBA_TypeCode tc, CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  return h ? (CORBA_TCKind)rorb_tc_kind(h) : CORBA_tk_null;
}

CORBA_boolean CORBA_TypeCode_equal(CORBA_TypeCode tc, CORBA_TypeCode other,
                                   CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  rorb_typecode* o = rustorb_c_tc_handle(other);
  return h && o && rorb_tc_equal(h, o);
}

CORBA_boolean CORBA_TypeCode_equivalent(CORBA_TypeCode tc, CORBA_TypeCode other,
                                        CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  rorb_typecode* o = rustorb_c_tc_handle(other);
  return h && o && rorb_tc_equivalent(h, o);
}

CORBA_TypeCode CORBA_TypeCode_get_compact_typecode(CORBA_TypeCode tc, CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  return h ? rustorb_c_wrap_tc(rorb_tc_compact(h), NULL) : CORBA_OBJECT_NIL;
}

static CORBA_char* tc_string(CORBA_TypeCode tc, CORBA_Environment* ev,
                             rorb_status (*f)(const rorb_typecode*, const char**, size_t*,
                                              rorb_exception*)) {
  rorb_typecode* h = tc_of(tc, ev);
  const char* s;
  size_t len;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  if (!h || rustorb_c_failed(ev, f(h, &s, &len, &exc), &exc)) return NULL;
  return rustorb_c_strndup(s, len);
}

CORBA_RepositoryId CORBA_TypeCode_id(CORBA_TypeCode tc, CORBA_Environment* ev) {
  return tc_string(tc, ev, rorb_tc_id);
}

CORBA_Identifier CORBA_TypeCode_name(CORBA_TypeCode tc, CORBA_Environment* ev) {
  return tc_string(tc, ev, rorb_tc_name);
}

CORBA_unsigned_long CORBA_TypeCode_member_count(CORBA_TypeCode tc, CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  uint32_t n = 0;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  if (h) rustorb_c_failed(ev, rorb_tc_member_count(h, &n, &exc), &exc);
  return n;
}

CORBA_Identifier CORBA_TypeCode_member_name(CORBA_TypeCode tc, CORBA_unsigned_long index,
                                            CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  const char* s;
  size_t len;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  if (!h || rustorb_c_failed(ev, rorb_tc_member_name(h, index, &s, &len, &exc), &exc))
    return NULL;
  return rustorb_c_strndup(s, len);
}

/* r is read after st is computed (a call argument would not be). */
static CORBA_TypeCode tc_result(CORBA_Environment* ev, rorb_status st, rorb_typecode** r,
                                rorb_exception* exc) {
  if (rustorb_c_failed(ev, st, exc)) return CORBA_OBJECT_NIL;
  return rustorb_c_wrap_tc(*r, NULL);
}

CORBA_TypeCode CORBA_TypeCode_member_type(CORBA_TypeCode tc, CORBA_unsigned_long index,
                                          CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  rorb_typecode* r = NULL;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  if (!h) return CORBA_OBJECT_NIL;
  return tc_result(ev, rorb_tc_member_type(h, index, &r, &exc), &r, &exc);
}

CORBA_TypeCode CORBA_TypeCode_discriminator_type(CORBA_TypeCode tc, CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  rorb_typecode* r = NULL;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  if (!h) return CORBA_OBJECT_NIL;
  return tc_result(ev, rorb_tc_discriminator_type(h, &r, &exc), &r, &exc);
}

CORBA_TypeCode CORBA_TypeCode_content_type(CORBA_TypeCode tc, CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  rorb_typecode* r = NULL;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  if (!h) return CORBA_OBJECT_NIL;
  return tc_result(ev, rorb_tc_content_type(h, &r, &exc), &r, &exc);
}

CORBA_long CORBA_TypeCode_default_index(CORBA_TypeCode tc, CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  int32_t i = -1;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  if (h) rustorb_c_failed(ev, rorb_tc_default_index(h, &i, &exc), &exc);
  return i;
}

CORBA_unsigned_long CORBA_TypeCode_length(CORBA_TypeCode tc, CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  uint32_t n = 0;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  if (h) rustorb_c_failed(ev, rorb_tc_length(h, &n, &exc), &exc);
  return n;
}

CORBA_unsigned_short CORBA_TypeCode_fixed_digits(CORBA_TypeCode tc, CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  uint16_t n = 0;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  if (h) rustorb_c_failed(ev, rorb_tc_fixed_digits(h, &n, &exc), &exc);
  return n;
}

CORBA_short CORBA_TypeCode_fixed_scale(CORBA_TypeCode tc, CORBA_Environment* ev) {
  rorb_typecode* h = tc_of(tc, ev);
  int16_t n = 0;
  rorb_exception exc;
  memset(&exc, 0, sizeof exc);
  if (h) rustorb_c_failed(ev, rorb_tc_fixed_scale(h, &n, &exc), &exc);
  return n;
}
