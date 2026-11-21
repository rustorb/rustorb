/* The stage 2 tests (idl/AnyTest.idl) on the OMG C mapping (anytest.h).
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include "anytest.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef _CORBA_fixed_31_4_defined
#define _CORBA_fixed_31_4_defined
RUSTORB_C_FIXED(31, 4);
#endif

static CORBA_ORB the_orb;
static DynamicAny_DynAnyFactory factory;
static CORBA_TypeCode fixed_tc;
static anytest_hidden_fn the_hidden;

void anytest_init(CORBA_ORB orb) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  the_orb = orb;
  factory = CORBA_ORB_resolve_initial_references(orb, "DynAnyFactory", &ev);
  fixed_tc = CORBA_ORB_create_fixed_tc(orb, 31, 4, &ev);
}

void anytest_fini(void) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_Object_release(factory, &ev);
  CORBA_Object_release(fixed_tc, &ev);
  factory = fixed_tc = NULL;
}

static const char* const names[] = {
    "long",   "short",  "ulonglong", "float",  "double",   "longdouble",     "boolean",
    "char",   "octet",  "string",    "bounded", "count",   "money",          "fixed",
    "point",  "shade",  "choice_l",  "choice_s", "choice_default", "longseq", "octets",
    "shortarray", "node", "nvlist",  "any",    "typecode", "objref",         "nil",
    "oops",   "empty",
};
#define N(a) (sizeof(a) / sizeof((a)[0]))

/* A copy of the any of type tc holding the value at value. */
static CORBA_any* any_of(CORBA_TypeCode tc, const void* value) {
  CORBA_any a;
  memset(&a, 0, sizeof a);
  a._type = tc;
  a._value = (void*)value;
  return (CORBA_any*)rustorb_c_copy(&CORBA_any__desc, &a);
}

static void set_choice(AnyTest_Choice* c, const char* name) {
  memset(c, 0, sizeof *c);
  if (strcmp(name, "choice_l") == 0) {
    c->_d = 1;
    c->_u.l = 10;
  } else if (strcmp(name, "choice_s") == 0) {
    c->_d = 3;
    c->_u.s = (CORBA_char*)"three";
  } else {
    c->_d = 9;
    c->_u.b = CORBA_TRUE;
  }
}

/* The value named name of the table in AnyTest.idl (freed with
   CORBA_free); self is the AnyEcho object (for "objref"). NULL for an
   unknown name. */
static CORBA_any* make(const char* name, AnyTest_AnyEcho self) {
  if (strcmp(name, "long") == 0) {
    CORBA_long v = 42;
    return any_of(TC_long, &v);
  }
  if (strcmp(name, "short") == 0) {
    CORBA_short v = -7;
    return any_of(TC_short, &v);
  }
  if (strcmp(name, "ulonglong") == 0) {
    CORBA_unsigned_long_long v = 18446744073709551615ULL;
    return any_of(TC_ulonglong, &v);
  }
  if (strcmp(name, "float") == 0) {
    CORBA_float v = 0.25f;
    return any_of(TC_float, &v);
  }
  if (strcmp(name, "double") == 0) {
    CORBA_double v = 1.5;
    return any_of(TC_double, &v);
  }
  if (strcmp(name, "longdouble") == 0) {
    /* The double nearest 0.1. */
    CORBA_long_double v = (CORBA_long_double)0.1;
    return any_of(TC_longdouble, &v);
  }
  if (strcmp(name, "boolean") == 0) {
    CORBA_boolean v = CORBA_TRUE;
    return any_of(TC_boolean, &v);
  }
  if (strcmp(name, "char") == 0) {
    CORBA_char v = 'x';
    return any_of(TC_char, &v);
  }
  if (strcmp(name, "octet") == 0) {
    CORBA_octet v = 255;
    return any_of(TC_octet, &v);
  }
  if (strcmp(name, "string") == 0) {
    const CORBA_char* v = "hello any";
    return any_of(TC_string, &v);
  }
  if (strcmp(name, "bounded") == 0) {
    const CORBA_char* v = "abc";
    return any_of(TC_AnyTest_Short5, &v);
  }
  if (strcmp(name, "count") == 0) {
    AnyTest_Count v = 7;
    return any_of(TC_AnyTest_Count, &v);
  }
  if (strcmp(name, "money") == 0) {
    AnyTest_Money v = {5, 2, {0x12, 0x34, 0x5d}};
    return any_of(TC_AnyTest_Money, &v);
  }
  if (strcmp(name, "fixed") == 0) {
    CORBA_fixed_31_4 v = {31, 4, {0x12, 0x34, 0x56, 0x78, 0x90, 0x12, 0x34, 0x56, 0x78, 0x90,
                                  0x12, 0x34, 0x56, 0x78, 0x90, 0x1c}};
    return any_of(fixed_tc, &v);
  }
  if (strcmp(name, "point") == 0) {
    AnyTest_Point v = {1, 2.5};
    return any_of(TC_AnyTest_Point, &v);
  }
  if (strcmp(name, "shade") == 0) {
    AnyTest_Shade v = AnyTest_DIM;
    return any_of(TC_AnyTest_Shade, &v);
  }
  if (strncmp(name, "choice_", 7) == 0) {
    AnyTest_Choice v;
    set_choice(&v, name);
    return any_of(TC_AnyTest_Choice, &v);
  }
  if (strcmp(name, "longseq") == 0) {
    CORBA_long b[] = {1, 2, 3};
    AnyTest_LongSeq v = {3, 3, b, CORBA_FALSE};
    return any_of(TC_AnyTest_LongSeq, &v);
  }
  if (strcmp(name, "octets") == 0) {
    CORBA_octet b[] = {0, 1, 255};
    AnyTest_Octets v = {3, 3, b, CORBA_FALSE};
    return any_of(TC_AnyTest_Octets, &v);
  }
  if (strcmp(name, "shortarray") == 0) {
    AnyTest_ShortArray v = {1, -2, 3};
    return any_of(TC_AnyTest_ShortArray, v);
  }
  if (strcmp(name, "node") == 0) {
    AnyTest_Node c[1] = {{(CORBA_char*)"c", {0, 0, NULL, CORBA_FALSE}}};
    AnyTest_Node kids[2] = {{(CORBA_char*)"a", {0, 0, NULL, CORBA_FALSE}},
                            {(CORBA_char*)"b", {1, 1, c, CORBA_FALSE}}};
    AnyTest_Node v = {(CORBA_char*)"root", {2, 2, kids, CORBA_FALSE}};
    return any_of(TC_AnyTest_Node, &v);
  }
  if (strcmp(name, "nvlist") == 0) {
    CORBA_long a = 1;
    const CORBA_char* b = "x";
    AnyTest_Point p = {3, -0.5};
    AnyTest_NameValue nv[3];
    AnyTest_NVList v = {3, 3, nv, CORBA_FALSE};
    memset(nv, 0, sizeof nv);
    nv[0].name = (CORBA_char*)"a";
    nv[0].value._type = TC_long;
    nv[0].value._value = &a;
    nv[1].name = (CORBA_char*)"b";
    nv[1].value._type = TC_string;
    nv[1].value._value = &b;
    nv[2].name = (CORBA_char*)"p";
    nv[2].value._type = TC_AnyTest_Point;
    nv[2].value._value = &p;
    return any_of(TC_AnyTest_NVList, &v);
  }
  if (strcmp(name, "any") == 0) {
    CORBA_long five = 5;
    CORBA_any inner;
    memset(&inner, 0, sizeof inner);
    inner._type = TC_long;
    inner._value = &five;
    return any_of(TC_any, &inner);
  }
  if (strcmp(name, "typecode") == 0) {
    CORBA_TypeCode v = TC_AnyTest_Node;
    return any_of(TC_TypeCode, &v);
  }
  if (strcmp(name, "objref") == 0) return any_of(TC_AnyTest_AnyEcho, &self);
  if (strcmp(name, "nil") == 0) {
    AnyTest_AnyEcho v = CORBA_OBJECT_NIL;
    return any_of(TC_AnyTest_AnyEcho, &v);
  }
  if (strcmp(name, "oops") == 0) {
    AnyTest_Oops v = {(CORBA_char*)"bad", 3};
    return any_of(TC_AnyTest_Oops, &v);
  }
  if (strcmp(name, "empty") == 0) return any_of(TC_null, NULL);
  return the_hidden ? the_hidden(name) : NULL;
}

/* Whether got is the value named name: an equal TypeCode and the same
   value (floats exact, references equivalent). */
static int same(const char* name, const CORBA_any* got, AnyTest_AnyEcho self) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* want = make(name, self);
  int r = want && got && got->_type &&
          CORBA_TypeCode_equal(got->_type, want->_type, &ev) && CORBA_any_equal(got, want);
  CORBA_exception_free(&ev);
  CORBA_free(want);
  return r;
}

/* --- the servant ----------------------------------------------------------------- */

static AnyTest_AnyEcho me(PortableServer_Servant s, CORBA_Environment* ev) {
  PortableServer_Current current;
  CORBA_Object r = PortableServer_ServantBase__this(s, ev);
  if (r || ev->_major != CORBA_NO_EXCEPTION) return r;
  /* Run by a DSI servant (rustorbc-dii), the request is that servant's:
     the target from the POA Current. */
  current = CORBA_ORB_resolve_initial_references(the_orb, "POACurrent", ev);
  if (ev->_major != CORBA_NO_EXCEPTION) return CORBA_OBJECT_NIL;
  r = PortableServer_Current_get_reference(current, ev);
  CORBA_Object_release(current, ev);
  return r;
}

static void bad_name(CORBA_Environment* ev) {
  CORBA_exception_set_system(ev, ex_CORBA_BAD_PARAM, 0, CORBA_COMPLETED_NO);
}

static CORBA_any* s_echo(PortableServer_Servant s, const CORBA_any* a, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return (CORBA_any*)rustorb_c_copy(&CORBA_any__desc, a);
}

static CORBA_any* s_make(PortableServer_Servant s, const CORBA_char* name, CORBA_Environment* ev) {
  AnyTest_AnyEcho self = me(s, ev);
  CORBA_any* r = make(name, self);
  CORBA_Object_release(self, ev);
  if (!r) bad_name(ev);
  return r;
}

static CORBA_boolean s_check(PortableServer_Servant s, const CORBA_char* name, const CORBA_any* a,
                             CORBA_Environment* ev) {
  AnyTest_AnyEcho self = me(s, ev);
  int r = same(name, a, self);
  CORBA_Object_release(self, ev);
  return r ? CORBA_TRUE : CORBA_FALSE;
}

static void s_out_inout(PortableServer_Servant s, const CORBA_char* name, CORBA_any** b,
                        CORBA_any* c, CORBA_Environment* ev) {
  AnyTest_AnyEcho self = me(s, ev);
  CORBA_any* l;
  if (!same(name, c, self)) {
    CORBA_Object_release(self, ev);
    bad_name(ev);
    return;
  }
  *b = make(name, self);
  CORBA_Object_release(self, ev);
  l = make("long", NULL);
  rustorb_c_free_value(&CORBA_any__desc, c);
  rustorb_c_copy_value(&CORBA_any__desc, c, l);
  CORBA_free(l);
}

static CORBA_TypeCode s_echo_tc(PortableServer_Servant s, CORBA_TypeCode tc,
                                CORBA_Environment* ev) {
  (void)s;
  return CORBA_Object_duplicate(tc, ev);
}

static CORBA_TypeCode s_make_tc(PortableServer_Servant s, const CORBA_char* name,
                                CORBA_Environment* ev) {
  CORBA_any* a = s_make(s, name, ev);
  CORBA_TypeCode r;
  if (!a) return NULL;
  r = CORBA_Object_duplicate(a->_type, ev);
  CORBA_free(a);
  return r;
}

static CORBA_boolean s_check_tc(PortableServer_Servant s, const CORBA_char* name,
                                CORBA_TypeCode tc, CORBA_Environment* ev) {
  CORBA_any* a = s_make(s, name, ev);
  CORBA_boolean r;
  if (!a) return CORBA_FALSE;
  r = CORBA_TypeCode_equal(a->_type, tc, ev);
  CORBA_free(a);
  return r;
}

static AnyTest_NVList* s_echo_nvlist(PortableServer_Servant s, const AnyTest_NVList* l,
                                     CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return (AnyTest_NVList*)rustorb_c_copy(&AnyTest_NVList__desc, l);
}

static const char* const kinds[] = {
    "null",     "void",       "short",    "long",      "ushort",  "ulong",      "float",
    "double",   "boolean",    "char",     "octet",     "any",     "TypeCode",   "Principal",
    "objref",   "struct",     "union",    "enum",      "string",  "sequence",   "array",
    "alias",    "except",     "longlong", "ulonglong", "longdouble", "wchar",   "wstring",
    "fixed",    "value",      "value_box", "native",   "abstract_interface", "local_interface",
};

static CORBA_char* s_describe(PortableServer_Servant s, const CORBA_any* a,
                              CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_TCKind k;
  CORBA_char* id;
  CORBA_char* r;
  size_t len;
  (void)s;
  (void)ev;
  k = CORBA_TypeCode_kind(a->_type, &e);
  id = CORBA_TypeCode_id(a->_type, &e);
  if (e._major != CORBA_NO_EXCEPTION) {
    /* BadKind: no repository id. */
    CORBA_exception_free(&e);
    id = NULL;
  }
  len = strlen(k < N(kinds) ? kinds[k] : "?") + 2 + (id ? strlen(id) : 0);
  r = CORBA_string_alloc((CORBA_unsigned_long)len);
  snprintf(r, len + 1, "%s %s", k < N(kinds) ? kinds[k] : "?", id ? id : "");
  CORBA_free(id);
  return r;
}

static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static POA_AnyTest_AnyEcho__epv anyecho_epv = {NULL,      s_echo,    s_make,
                                               s_check,   s_out_inout, s_echo_tc,
                                               s_make_tc, s_check_tc, s_echo_nvlist,
                                               s_describe};
static POA_AnyTest_AnyEcho__vepv anyecho_vepv = {&base_epv, &anyecho_epv};

PortableServer_Servant anytest_servant_new(CORBA_Environment* ev) {
  POA_AnyTest_AnyEcho* s = (POA_AnyTest_AnyEcho*)calloc(1, sizeof *s);
  if (!s) return NULL;
  s->vepv = &anyecho_vepv;
  POA_AnyTest_AnyEcho__init(s, ev);
  return s;
}

void anytest_servant_free(PortableServer_Servant servant) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  POA_AnyTest_AnyEcho__fini(servant, &ev);
  free(servant);
}

/* --- DynAny: the values taken apart and built again ------------------------------- */

static CORBA_any* dyn_rebuild(const CORBA_any* value, CORBA_Environment* ev);

#define OK(ev) ((ev)->_major == CORBA_NO_EXCEPTION)

/* Copies src into dst (of the same type) component by component. */
static void dyn_copy(DynamicAny_DynAny src, DynamicAny_DynAny dst, CORBA_Environment* ev) {
  CORBA_TypeCode t = DynamicAny_DynAny_type(src, ev);
  CORBA_TypeCode u;
  CORBA_TCKind k;
  if (!OK(ev)) return;
  u = CORBA_TypeCode_get_compact_typecode(t, ev);
  CORBA_Object_release(t, ev);
  /* The kind with the aliases removed. */
  while (CORBA_TypeCode_kind(u, ev) == CORBA_tk_alias) {
    CORBA_TypeCode c = CORBA_TypeCode_content_type(u, ev);
    CORBA_Object_release(u, ev);
    u = c;
  }
  k = CORBA_TypeCode_kind(u, ev);
  CORBA_Object_release(u, ev);
  switch (k) {
    case CORBA_tk_struct:
    case CORBA_tk_except:
    case CORBA_tk_sequence:
    case CORBA_tk_array: {
      CORBA_unsigned_long i, n;
      if (k == CORBA_tk_sequence)
        DynamicAny_DynSequence_set_length(dst, DynamicAny_DynSequence_get_length(src, ev), ev);
      n = DynamicAny_DynAny_component_count(src, ev);
      for (i = 0; i < n && OK(ev); i++) {
        DynamicAny_DynAny s, d;
        DynamicAny_DynAny_seek(src, (CORBA_long)i, ev);
        DynamicAny_DynAny_seek(dst, (CORBA_long)i, ev);
        s = DynamicAny_DynAny_current_component(src, ev);
        d = DynamicAny_DynAny_current_component(dst, ev);
        if (OK(ev)) dyn_copy(s, d, ev);
        CORBA_Object_release(s, NULL);
        CORBA_Object_release(d, NULL);
      }
      return;
    }
    case CORBA_tk_union: {
      DynamicAny_DynAny disc = DynamicAny_DynUnion_get_discriminator(src, ev);
      if (OK(ev)) DynamicAny_DynUnion_set_discriminator(dst, disc, ev);
      CORBA_Object_release(disc, NULL);
      if (OK(ev) && !DynamicAny_DynUnion_has_no_active_member(src, ev)) {
        DynamicAny_DynAny s = DynamicAny_DynUnion_member(src, ev);
        DynamicAny_DynAny d = DynamicAny_DynUnion_member(dst, ev);
        if (OK(ev)) dyn_copy(s, d, ev);
        CORBA_Object_release(s, NULL);
        CORBA_Object_release(d, NULL);
      }
      return;
    }
    case CORBA_tk_enum:
      DynamicAny_DynEnum_set_as_ulong(dst, DynamicAny_DynEnum_get_as_ulong(src, ev), ev);
      return;
    case CORBA_tk_fixed: {
      CORBA_char* v = DynamicAny_DynFixed_get_value(src, ev);
      if (OK(ev)) DynamicAny_DynFixed_set_value(dst, v, ev);
      CORBA_free(v);
      return;
    }
    case CORBA_tk_any: {
      CORBA_any* a = DynamicAny_DynAny_get_any(src, ev);
      CORBA_any* b = a ? dyn_rebuild(a, ev) : NULL;
      if (b) DynamicAny_DynAny_insert_any(dst, b, ev);
      CORBA_free(a);
      CORBA_free(b);
      return;
    }
#define COPY(tk, name)                                                            \
  case tk:                                                                        \
    DynamicAny_DynAny_insert_##name(dst, DynamicAny_DynAny_get_##name(src, ev), ev); \
    return;
    COPY(CORBA_tk_long, long)
    COPY(CORBA_tk_short, short)
    COPY(CORBA_tk_ulong, ulong)
    COPY(CORBA_tk_ushort, ushort)
    COPY(CORBA_tk_longlong, longlong)
    COPY(CORBA_tk_ulonglong, ulonglong)
    COPY(CORBA_tk_float, float)
    COPY(CORBA_tk_double, double)
    COPY(CORBA_tk_longdouble, longdouble)
    COPY(CORBA_tk_boolean, boolean)
    COPY(CORBA_tk_char, char)
    COPY(CORBA_tk_wchar, wchar)
    COPY(CORBA_tk_octet, octet)
#undef COPY
    case CORBA_tk_string: {
      CORBA_char* v = DynamicAny_DynAny_get_string(src, ev);
      if (OK(ev)) DynamicAny_DynAny_insert_string(dst, v, ev);
      CORBA_free(v);
      return;
    }
    case CORBA_tk_wstring: {
      CORBA_wchar* v = DynamicAny_DynAny_get_wstring(src, ev);
      if (OK(ev)) DynamicAny_DynAny_insert_wstring(dst, v, ev);
      CORBA_free(v);
      return;
    }
    case CORBA_tk_TypeCode: {
      CORBA_TypeCode v = DynamicAny_DynAny_get_typecode(src, ev);
      if (OK(ev)) DynamicAny_DynAny_insert_typecode(dst, v, ev);
      CORBA_Object_release(v, NULL);
      return;
    }
    case CORBA_tk_objref: {
      CORBA_Object v = DynamicAny_DynAny_get_reference(src, ev);
      if (OK(ev)) DynamicAny_DynAny_insert_reference(dst, v, ev);
      CORBA_Object_release(v, NULL);
      return;
    }
    default: {
      /* tk_null and tk_void. */
      CORBA_any* a = DynamicAny_DynAny_to_any(src, ev);
      if (a) DynamicAny_DynAny_from_any(dst, a, ev);
      CORBA_free(a);
      return;
    }
  }
}

/* value taken apart with a DynAny and built again in a DynAny made from its
   TypeCode alone; NULL with ev set on failure. */
static CORBA_any* dyn_rebuild(const CORBA_any* value, CORBA_Environment* ev) {
  DynamicAny_DynAny src = DynamicAny_DynAnyFactory_create_dyn_any(factory, value, ev);
  DynamicAny_DynAny dst = NULL;
  CORBA_any* r = NULL;
  if (OK(ev)) dst = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(factory, value->_type, ev);
  if (OK(ev)) dyn_copy(src, dst, ev);
  if (OK(ev)) r = DynamicAny_DynAny_to_any(dst, ev);
  if (src) DynamicAny_DynAny_destroy(src, NULL);
  if (dst) DynamicAny_DynAny_destroy(dst, NULL);
  CORBA_Object_release(src, NULL);
  CORBA_Object_release(dst, NULL);
  return r;
}

/* --- the client cases --------------------------------------------------------------- */

static char why[512];

static const char* unexpected(CORBA_Environment* ev) {
  snprintf(why, sizeof why, "unexpected %s", CORBA_exception_id(ev));
  CORBA_exception_free(ev);
  return why;
}

typedef const char* (*any_case)(AnyTest_AnyEcho echo, const char* name);

static void run(runner* r, const char* label, const char* name, any_case fn,
                AnyTest_AnyEcho echo) {
  char full[160];
  const char* failure = fn(echo, name);
  snprintf(full, sizeof full, "%s[%s]", label, name);
  if (!name[0]) snprintf(full, sizeof full, "%s", label);
  if (!failure) {
    r->passed++;
    printf("ok   %s\n", full);
  } else {
    size_t len = strlen(r->failed_names);
    r->failed++;
    printf("FAIL %s: %s\n", full, failure);
    snprintf(r->failed_names + len, sizeof r->failed_names - len, "%s%s", len ? ", " : "", full);
  }
  fflush(stdout);
}

#define CALL_OK(ev)                                       \
  do {                                                    \
    if ((ev)->_major != CORBA_NO_EXCEPTION) return unexpected(ev); \
  } while (0)

static const char* c_make(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* got = AnyTest_AnyEcho_make(echo, n, &ev);
  int ok;
  CALL_OK(&ev);
  ok = same(n, got, echo);
  CORBA_free(got);
  return ok ? NULL : "wrong value";
}

static const char* c_check(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = make(n, echo);
  CORBA_boolean ok = AnyTest_AnyEcho_check(echo, n, a, &ev);
  CORBA_free(a);
  CALL_OK(&ev);
  return ok ? NULL : "server says no";
}

static const char* c_echo(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = make(n, echo);
  CORBA_any* got = AnyTest_AnyEcho_echo(echo, a, &ev);
  int ok;
  CORBA_free(a);
  CALL_OK(&ev);
  ok = same(n, got, echo);
  CORBA_free(got);
  return ok ? NULL : "wrong value";
}

static const char* c_make_tc(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = make(n, echo);
  CORBA_TypeCode got = AnyTest_AnyEcho_make_tc(echo, n, &ev);
  int ok = 0;
  if (OK(&ev)) ok = CORBA_TypeCode_equal(got, a->_type, &ev);
  CORBA_free(a);
  CORBA_Object_release(got, NULL);
  CALL_OK(&ev);
  return ok ? NULL : "wrong TypeCode";
}

static const char* c_check_tc(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = make(n, echo);
  CORBA_boolean ok = AnyTest_AnyEcho_check_tc(echo, n, a->_type, &ev);
  CORBA_free(a);
  CALL_OK(&ev);
  return ok ? NULL : "server says no";
}

static const char* c_echo_tc(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = make(n, echo);
  CORBA_TypeCode got = AnyTest_AnyEcho_echo_tc(echo, a->_type, &ev);
  int ok = 0;
  if (OK(&ev)) ok = CORBA_TypeCode_equal(got, a->_type, &ev);
  CORBA_free(a);
  CORBA_Object_release(got, NULL);
  CALL_OK(&ev);
  return ok ? NULL : "wrong TypeCode";
}

static const char* c_out_inout(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* c = make(n, echo);
  CORBA_any* b = NULL;
  const char* r = NULL;
  AnyTest_AnyEcho_out_inout(echo, n, &b, c, &ev);
  if (!OK(&ev)) {
    CORBA_free(c);
    return unexpected(&ev);
  }
  if (!same(n, b, echo)) r = "wrong out value";
  else if (!same("long", c, echo)) r = "wrong inout value";
  CORBA_free(b);
  CORBA_free(c);
  return r;
}

static const char* c_echo_nvlist(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = make("nvlist", echo);
  AnyTest_NVList* got = AnyTest_AnyEcho_echo_nvlist(echo, (AnyTest_NVList*)a->_value, &ev);
  CORBA_any* b;
  int ok;
  (void)n;
  CORBA_free(a);
  CALL_OK(&ev);
  b = any_of(TC_AnyTest_NVList, got);
  ok = same("nvlist", b, echo);
  CORBA_free(b);
  CORBA_free(got);
  return ok ? NULL : "wrong value";
}

static const char* describe_is(AnyTest_AnyEcho echo, const char* n, const char* want) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = make(n, echo);
  CORBA_char* d = AnyTest_AnyEcho_describe(echo, a, &ev);
  CORBA_free(a);
  CALL_OK(&ev);
  if (strcmp(d, want) == 0) {
    CORBA_free(d);
    return NULL;
  }
  snprintf(why, sizeof why, "%s", d);
  CORBA_free(d);
  return why;
}

static const char* hidden_describe(AnyTest_AnyEcho echo, const char* n) {
  if (strcmp(n, "secret") == 0) return describe_is(echo, n, "struct IDL:AnyTestHidden/Secret:1.0");
  if (strcmp(n, "pick") == 0) return describe_is(echo, n, "union IDL:AnyTestHidden/Pick:1.0");
  return describe_is(echo, n, "alias IDL:AnyTestHidden/Moods:1.0");
}

static const char* c_dyn_check(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = make(n, echo);
  CORBA_any* b = dyn_rebuild(a, &ev);
  CORBA_boolean ok = CORBA_FALSE;
  CORBA_free(a);
  CALL_OK(&ev);
  ok = AnyTest_AnyEcho_check(echo, n, b, &ev);
  CORBA_free(b);
  CALL_OK(&ev);
  return ok ? NULL : "server says no";
}

static const char* c_dyn_make(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = AnyTest_AnyEcho_make(echo, n, &ev);
  CORBA_any* b;
  int ok;
  CALL_OK(&ev);
  b = dyn_rebuild(a, &ev);
  CORBA_free(a);
  CALL_OK(&ev);
  ok = same(n, b, echo);
  CORBA_free(b);
  return ok ? NULL : "wrong value";
}

static const char* c_dyn_echo(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* a = make(n, echo);
  CORBA_any* b = dyn_rebuild(a, &ev);
  CORBA_any* got;
  int ok;
  CORBA_free(a);
  CALL_OK(&ev);
  got = AnyTest_AnyEcho_echo(echo, b, &ev);
  CORBA_free(b);
  CALL_OK(&ev);
  ok = same(n, got, echo);
  CORBA_free(got);
  return ok ? NULL : "wrong value";
}

static const char* c_describe_long(AnyTest_AnyEcho echo, const char* n) {
  (void)n;
  return describe_is(echo, "long", "long ");
}

static const char* c_describe_node(AnyTest_AnyEcho echo, const char* n) {
  (void)n;
  return describe_is(echo, "node", "struct IDL:AnyTest/Node:1.0");
}

static const char* check_false(AnyTest_AnyEcho echo, const char* name, CORBA_any* a) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_boolean ok = AnyTest_AnyEcho_check(echo, name, a, &ev);
  CORBA_free(a);
  CALL_OK(&ev);
  return ok ? "server says yes" : NULL;
}

static const char* c_check_wrong_type(AnyTest_AnyEcho echo, const char* n) {
  CORBA_short v = 42;
  (void)n;
  return check_false(echo, "long", any_of(TC_short, &v));
}

static const char* c_check_unaliased(AnyTest_AnyEcho echo, const char* n) {
  CORBA_long v = 7;
  (void)n;
  return check_false(echo, "count", any_of(TC_long, &v));
}

static const char* c_check_tc_unaliased(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_boolean ok = AnyTest_AnyEcho_check_tc(echo, "count", TC_long, &ev);
  (void)n;
  CALL_OK(&ev);
  return ok ? "server says yes" : NULL;
}

static const char* c_out_inout_wrong(AnyTest_AnyEcho echo, const char* n) {
  CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
  CORBA_any* c = make("long", echo);
  CORBA_any* b = NULL;
  int bad;
  (void)n;
  AnyTest_AnyEcho_out_inout(echo, "point", &b, c, &ev);
  CORBA_free(c);
  CORBA_free(b);
  bad = ev._major == CORBA_SYSTEM_EXCEPTION && strcmp(CORBA_exception_id(&ev), ex_CORBA_BAD_PARAM) == 0;
  CORBA_exception_free(&ev);
  return bad ? NULL : "no BAD_PARAM";
}

void anytest_run(runner* r, AnyTest_AnyEcho echo, anytest_hidden_fn hidden) {
  static const char* const hidden_names[] = {"secret", "pick", "moods"};
  static const char* const out_inout_names[] = {"point", "nvlist", "node"};
  size_t i;
  the_hidden = hidden;
  for (i = 0; i < N(names); i++) {
    run(r, "make", names[i], c_make, echo);
    run(r, "check", names[i], c_check, echo);
    run(r, "echo", names[i], c_echo, echo);
  }
  for (i = 0; i < N(names); i++) {
    run(r, "make_tc", names[i], c_make_tc, echo);
    run(r, "check_tc", names[i], c_check_tc, echo);
    run(r, "echo_tc", names[i], c_echo_tc, echo);
  }
  for (i = 0; i < N(out_inout_names); i++) run(r, "out_inout", out_inout_names[i], c_out_inout, echo);
  run(r, "echo_nvlist", "", c_echo_nvlist, echo);
  for (i = 0; i < N(hidden_names); i++) {
    run(r, "echo", hidden_names[i], c_echo, echo);
    run(r, "describe", hidden_names[i], hidden_describe, echo);
  }
  for (i = 0; i < N(names); i++) {
    run(r, "dyn_check", names[i], c_dyn_check, echo);
    run(r, "dyn_make", names[i], c_dyn_make, echo);
  }
  for (i = 0; i < N(hidden_names); i++) run(r, "dyn_echo", hidden_names[i], c_dyn_echo, echo);
  run(r, "describe[long]", "", c_describe_long, echo);
  run(r, "describe[node]", "", c_describe_node, echo);
  run(r, "check_wrong_type", "", c_check_wrong_type, echo);
  run(r, "check_unaliased", "", c_check_unaliased, echo);
  run(r, "check_tc_unaliased", "", c_check_tc_unaliased, echo);
  run(r, "out_inout_wrong", "", c_out_inout_wrong, echo);
}
