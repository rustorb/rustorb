/* The constructs of cpp/tests/idl/CxxMapping.idl and the any and TypeCode
 * functions, through rustorb-idl -bc stubs and skeletons over GIOP in one
 * process.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "AnyTest.h"
#include "CxxMapping.h"

/* sequence<double>, which no IDL here names: an application declares it
   the way a generated header would. */
#ifndef _CORBA_sequence_double_defined
#define _CORBA_sequence_double_defined
RUSTORB_C_SEQUENCE(CORBA_sequence_double, CORBA_double);
#endif

static int failures;

#define CHECK(cond)                                                    \
  do {                                                                 \
    if (!(cond)) {                                                     \
      failures++;                                                      \
      fprintf(stderr, "%s:%d: CHECK(%s) failed\n", __FILE__, __LINE__, #cond); \
    }                                                                  \
  } while (0)

/* ev has no exception (else prints it and frees it). */
static int ok(CORBA_Environment* ev, int line) {
  if (ev->_major == CORBA_NO_EXCEPTION) return 1;
  failures++;
  fprintf(stderr, "%s:%d: unexpected %s\n", __FILE__, line, CORBA_exception_id(ev));
  CORBA_exception_free(ev);
  return 0;
}
#define OK(ev) ok((ev), __LINE__)

/* ev has the exception id (and frees it). */
static int raised(CORBA_Environment* ev, const char* id, int line) {
  int r = ev->_major != CORBA_NO_EXCEPTION && strcmp(CORBA_exception_id(ev), id) == 0;
  if (!r) {
    failures++;
    fprintf(stderr, "%s:%d: expected %s, got %s\n", __FILE__, line, id,
            ev->_major == CORBA_NO_EXCEPTION ? "nothing" : CORBA_exception_id(ev));
  }
  CORBA_exception_free(ev);
  return r;
}
#define RAISED(ev, id) raised((ev), (id), __LINE__)

static CORBA_Environment ev0;
#define ENV CORBA_Environment ev = ev0

/* --- the Types servant --------------------------------------------------------- */

#define ECHO_COPY(op, T)                                                      \
  static T* op(PortableServer_Servant s, const T* v, CORBA_Environment* ev) { \
    (void)s;                                                                  \
    (void)ev;                                                                 \
    return (T*)rustorb_c_copy(&T##__desc, v);                                 \
  }
#define ECHO_VALUE(op, T)                                                    \
  static T op(PortableServer_Servant s, const T* v, CORBA_Environment* ev) { \
    (void)s;                                                                 \
    (void)ev;                                                                \
    return *v;                                                               \
  }

ECHO_COPY(echo_tree, CxxMapping_Tree)
ECHO_COPY(echo_node, CxxMapping_Node)
ECHO_COPY(echo_by_bool, CxxMapping_ByBool)
ECHO_COPY(echo_by_char, CxxMapping_ByChar)
ECHO_COPY(echo_by_enum, CxxMapping_ByEnum)
ECHO_VALUE(echo_by_ulonglong, CxxMapping_ByULongLong)
ECHO_VALUE(echo_with_array, CxxMapping_WithArray)
ECHO_COPY(echo_holder, CxxMapping_Holder)
ECHO_VALUE(echo_outer, CxxMapping_Outer)
ECHO_COPY(echo_keywords, CxxMapping_Keywords)
ECHO_VALUE(echo_global, GlobalPoint)
ECHO_COPY(echo_price, CxxMapping_Price)
ECHO_COPY(echo_measure, CxxMapping_Measure)

static CORBA_char* echo_short_string(PortableServer_Servant s, const CORBA_char* v,
                                     CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return CORBA_string_dup(v);
}

/* Returns m, sets total to m and whole to 123. */
static CxxMapping_Money echo_money(PortableServer_Servant s, const CxxMapping_Money* m,
                                   CxxMapping_Money* total, CORBA_fixed_3_0* whole,
                                   CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  *total = *m;
  whole->_digits = 3;
  whole->_scale = 0;
  whole->_value[0] = 0x12;
  whole->_value[1] = 0x3c;
  return *m;
}

static CxxMapping_Quad echo_quad(PortableServer_Servant s, CxxMapping_Quad q,
                                 CORBA_long_double* twice, CORBA_long_double* negated,
                                 CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  *twice = 2 * *twice;
  *negated = -q;
  return q;
}

/* 1: Failure; 2: Empty; else NO_PERMISSION minor 5. */
static void fail(PortableServer_Servant s, CORBA_long code, CORBA_Environment* ev) {
  (void)s;
  if (code == 1) {
    CxxMapping_Failure* f = CxxMapping_Failure__alloc();
    f->code = code;
    f->message = CORBA_string_dup("failed");
    f->kind = CxxMapping_CHERRY;
    CORBA_exception_set(ev, CORBA_USER_EXCEPTION, ex_CxxMapping_Failure, f);
  } else if (code == 2) {
    CORBA_exception_set(ev, CORBA_USER_EXCEPTION, ex_CxxMapping_Empty, NULL);
  } else {
    CORBA_exception_set_system(ev, ex_CORBA_NO_PERMISSION, 5, CORBA_COMPLETED_YES);
  }
}

/* n nil references. */
static CxxMapping_LaterSeq* laters(PortableServer_Servant s, CORBA_long n, CORBA_Environment* ev) {
  CxxMapping_LaterSeq* r = CxxMapping_LaterSeq__alloc();
  (void)s;
  (void)ev;
  r->_buffer = CORBA_sequence_CxxMapping_Later_allocbuf((CORBA_unsigned_long)n);
  r->_length = r->_maximum = (CORBA_unsigned_long)n;
  r->_release = CORBA_TRUE;
  return r;
}

static char deleted[32];

static void delete_(PortableServer_Servant s, const CORBA_char* what, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  snprintf(deleted, sizeof deleted, "%s", what);
}

static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static POA_CxxMapping_Types__epv types_epv = {
    NULL,          echo_tree,      echo_node,       echo_by_bool,  echo_by_char,
    echo_by_enum,  echo_by_ulonglong, echo_with_array, echo_holder, echo_outer,
    echo_keywords, echo_global,    echo_short_string, echo_money, echo_price,
    echo_quad,     echo_measure,   fail,            laters,        delete_,
};
static POA_CxxMapping_Types__vepv types_vepv = {&base_epv, &types_epv};
static POA_CxxMapping_Types types_servant = {NULL, &types_vepv};

/* --- the Multi servant ------------------------------------------------------------ */

static char label[64] = "first";

static CORBA_long get_id(PortableServer_Servant s, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return 42;
}
static CORBA_char* get_label(PortableServer_Servant s, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return CORBA_string_dup(label);
}
static void set_label(PortableServer_Servant s, const CORBA_char* v, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  snprintf(label, sizeof label, "%s", v);
}
static void base_op(PortableServer_Servant s, CORBA_boolean f, CORBA_Environment* ev) {
  (void)s;
  if (f) {
    CxxMapping_Base_BaseError* e = CxxMapping_Base_BaseError__alloc();
    e->why = CORBA_string_dup("asked to");
    CORBA_exception_set(ev, CORBA_USER_EXCEPTION, ex_CxxMapping_Base_BaseError, e);
  }
}
/* Returns a + b, sets c to b and b to a. */
static CORBA_long derived_op(PortableServer_Servant s, CORBA_long a, CORBA_long* b, CORBA_long* c,
                             CORBA_Environment* ev) {
  CORBA_long r = a + *b;
  (void)s;
  (void)ev;
  *c = *b;
  *b = a;
  return r;
}
static void notify(PortableServer_Servant s, const CORBA_char* msg, CORBA_Environment* ev) {
  (void)s;
  (void)msg;
  (void)ev;
}
static CxxMapping_Base as_base(PortableServer_Servant s, CORBA_Environment* ev) {
  return PortableServer_ServantBase__this(s, ev);
}
static CORBA_char* other_op(PortableServer_Servant s, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return CORBA_string_dup("other");
}
static CxxMapping_Base_BaseInfo multi_op(PortableServer_Servant s,
                                         const CxxMapping_Base_BaseInfo* info,
                                         CORBA_Environment* ev) {
  CxxMapping_Base_BaseInfo r;
  (void)s;
  (void)ev;
  r.id = info->id + 1;
  return r;
}

static POA_CxxMapping_Base__epv mbase_epv = {NULL, get_id, get_label, set_label, base_op};
static POA_CxxMapping_Other__epv mother_epv = {NULL, other_op};
static POA_CxxMapping_Derived__epv mderived_epv = {NULL, derived_op, notify, as_base};
static POA_CxxMapping_Multi__epv mmulti_epv = {NULL, multi_op};
static POA_CxxMapping_Multi__vepv multi_vepv = {&base_epv, &mbase_epv, &mother_epv,
                                                &mderived_epv, &mmulti_epv};
static POA_CxxMapping_Multi multi_servant = {NULL, &multi_vepv};

/* --- the AnyEcho servant ---------------------------------------------------------- */

static CORBA_any* any_echo(PortableServer_Servant s, const CORBA_any* a, CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return (CORBA_any*)rustorb_c_copy(&CORBA_any__desc, a);
}
static CORBA_TypeCode any_echo_tc(PortableServer_Servant s, CORBA_TypeCode tc,
                                  CORBA_Environment* ev) {
  (void)s;
  return CORBA_Object_duplicate(tc, ev);
}
static AnyTest_NVList* any_echo_nvlist(PortableServer_Servant s, const AnyTest_NVList* l,
                                       CORBA_Environment* ev) {
  (void)s;
  (void)ev;
  return (AnyTest_NVList*)rustorb_c_copy(&AnyTest_NVList__desc, l);
}
/* Sets b to a copy of c and c to an any of the long 7. */
static void any_out_inout(PortableServer_Servant s, const CORBA_char* name, CORBA_any** b,
                          CORBA_any* c, CORBA_Environment* ev) {
  CORBA_long* v = (CORBA_long*)rustorb_c_alloc(&CORBA_long__desc, 1);
  (void)s;
  (void)name;
  *b = (CORBA_any*)rustorb_c_copy(&CORBA_any__desc, c);
  rustorb_c_free_value(&CORBA_any__desc, c);
  *v = 7;
  c->_type = CORBA_Object_duplicate(TC_long, ev);
  c->_value = v;
  c->_release = CORBA_TRUE;
}

static POA_AnyTest_AnyEcho__epv anyecho_epv;
static POA_AnyTest_AnyEcho__vepv anyecho_vepv = {&base_epv, &anyecho_epv};
static POA_AnyTest_AnyEcho anyecho_servant = {NULL, &anyecho_vepv};

/* --- helpers ----------------------------------------------------------------------- */

static CORBA_Object activate(PortableServer_POA poa, CORBA_ORB orb, PortableServer_Servant s) {
  ENV;
  CORBA_Object ref = PortableServer_POA_servant_to_reference(poa, s, &ev);
  CORBA_char* ior;
  CORBA_Object r;
  if (!OK(&ev)) exit(1);
  /* Through a string, so that calls go over GIOP. */
  ior = CORBA_ORB_object_to_string(orb, ref, &ev);
  r = CORBA_ORB_string_to_object(orb, ior, &ev);
  if (!OK(&ev)) exit(1);
  CORBA_free(ior);
  CORBA_Object_release(ref, &ev);
  return r;
}

static void test_constants(void) {
  CxxMapping_Money money = CxxMapping_C_MONEY;
  CORBA_fixed_4_2 fixed = CxxMapping_C_FIXED;
  const CORBA_wchar* w = CxxMapping_C_WSTRING;
  CHECK(GLOBAL_LIMIT == 10);
  CHECK(CxxMapping_C_SHORT == -3);
  CHECK(CxxMapping_C_ULONGLONG == 18000000000000000000ULL);
  CHECK(CxxMapping_C_DOUBLE == 0.25);
  CHECK(CxxMapping_C_TRUE == CORBA_TRUE);
  CHECK(CxxMapping_C_CHAR == 'q');
  CHECK(CxxMapping_C_WCHAR == 'w');
  CHECK(strcmp(CxxMapping_C_STRING, "abcd") == 0);
  CHECK(w[0] == 'w' && w[3] == 'e' && w[4] == 0);
  CHECK(CxxMapping_C_OCTET == 255);
  CHECK(CxxMapping_C_EXPR == (1 << 20) * 2 - 3);
  CHECK(CxxMapping_C_ENUM == CxxMapping_BANANA);
  CHECK(money._digits == 5 && money._scale == 2 && money._value[2] == 0x0d);
  CHECK(fixed._value[0] == 0x01 && fixed._value[1] == 0x25 && fixed._value[2] == 0x0c);
  CHECK(CxxMapping_C_QUAD == 2.5);
  CHECK(CxxMapping_Base_BASE_CONST == 7);
  CHECK(CxxMapping_REOPENED == 1);
  CHECK(CxxMapping_Outer_OFF == 1);
}

static void test_typecodes(void) {
  ENV;
  CORBA_TypeCode tc, member, content;
  CORBA_char* s;
  tc = TC_CxxMapping_Tree;
  CHECK(CORBA_TypeCode_kind(tc, &ev) == CORBA_tk_struct);
  s = CORBA_TypeCode_id(tc, &ev);
  CHECK(s && strcmp(s, "IDL:rustorb.org/CxxMapping/Tree:1.0") == 0);
  CORBA_free(s);
  CHECK(CORBA_TypeCode_member_count(tc, &ev) == 2);
  member = CORBA_TypeCode_member_type(tc, 1, &ev);
  CHECK(CORBA_TypeCode_kind(member, &ev) == CORBA_tk_sequence);
  content = CORBA_TypeCode_content_type(member, &ev);
  /* The recursion is resolved: the content is the Tree itself. */
  CHECK(CORBA_TypeCode_equal(content, tc, &ev));
  CORBA_Object_release(content, &ev);
  CORBA_Object_release(member, &ev);
  OK(&ev);

  tc = TC_CxxMapping_ByEnum;
  CHECK(CORBA_TypeCode_kind(tc, &ev) == CORBA_tk_union);
  CHECK(CORBA_TypeCode_default_index(tc, &ev) == 2);
  CHECK(CORBA_TypeCode_member_count(tc, &ev) == 3);
  tc = TC_CxxMapping_ByChar;
  CHECK(CORBA_TypeCode_member_count(tc, &ev) == 3);
  s = CORBA_TypeCode_member_name(tc, 2, &ev);
  CHECK(s && strcmp(s, "bc") == 0);
  CORBA_free(s);

  tc = TC_CxxMapping_Money;
  CHECK(CORBA_TypeCode_kind(tc, &ev) == CORBA_tk_alias);
  content = CORBA_TypeCode_content_type(tc, &ev);
  CHECK(CORBA_TypeCode_fixed_digits(content, &ev) == 5);
  CHECK(CORBA_TypeCode_fixed_scale(content, &ev) == 2);
  CORBA_Object_release(content, &ev);

  tc = TC_CxxMapping_Grid;
  content = CORBA_TypeCode_content_type(tc, &ev);
  CHECK(CORBA_TypeCode_kind(content, &ev) == CORBA_tk_array);
  CHECK(CORBA_TypeCode_length(content, &ev) == 2);
  CORBA_Object_release(content, &ev);

  CHECK(CORBA_TypeCode_kind(TC_CxxMapping_Fruit, &ev) == CORBA_tk_enum);
  CHECK(CORBA_TypeCode_kind(TC_CxxMapping_Failure, &ev) == CORBA_tk_except);
  CHECK(CORBA_TypeCode_kind(TC_CxxMapping_Multi, &ev) == CORBA_tk_objref);
  CHECK(CORBA_TypeCode_kind(TC_CxxMapping_ShortString, &ev) == CORBA_tk_alias);
  OK(&ev);

  /* BadKind for a query of the wrong kind. */
  CORBA_TypeCode_length(TC_long, &ev);
  RAISED(&ev, ex_CORBA_TypeCode_BadKind);
  CORBA_TypeCode_member_name(TC_CxxMapping_Tree, 5, &ev);
  RAISED(&ev, ex_CORBA_TypeCode_Bounds);
}

static int tree_eq(const CxxMapping_Tree* a, const CxxMapping_Tree* b) {
  CORBA_unsigned_long i;
  if (strcmp(a->label, b->label) != 0 || a->children._length != b->children._length) return 0;
  for (i = 0; i < a->children._length; i++)
    if (!tree_eq(&a->children._buffer[i], &b->children._buffer[i])) return 0;
  return 1;
}

static void test_types(CxxMapping_Types t) {
  ENV;
  {
    /* Recursive struct: a root with two children, one with a child. */
    CxxMapping_Tree leaf = {(CORBA_char*)"leaf", {0, 0, NULL, 0}};
    CxxMapping_Tree mids[2];
    CxxMapping_Tree root, *got;
    mids[0].label = (CORBA_char*)"a";
    mids[0].children._buffer = &leaf;
    mids[0].children._length = mids[0].children._maximum = 1;
    mids[0].children._release = 0;
    mids[1].label = (CORBA_char*)"b";
    memset(&mids[1].children, 0, sizeof mids[1].children);
    root.label = (CORBA_char*)"root";
    root.children._buffer = mids;
    root.children._length = root.children._maximum = 2;
    root.children._release = 0;
    got = CxxMapping_Types_echo_tree(t, &root, &ev);
    if (OK(&ev)) CHECK(tree_eq(got, &root));
    CORBA_free(got);
  }
  {
    /* Recursive union. */
    CxxMapping_Node leaves[2], branch, *got;
    leaves[0]._d = 1;
    leaves[0]._u.leaf = 10;
    leaves[1]._d = 1;
    leaves[1]._u.leaf = 20;
    branch._d = 2;
    branch._u.branch._buffer = leaves;
    branch._u.branch._length = branch._u.branch._maximum = 2;
    branch._u.branch._release = 0;
    got = CxxMapping_Types_echo_node(t, &branch, &ev);
    if (OK(&ev))
      CHECK(got->_d == 2 && got->_u.branch._length == 2 && got->_u.branch._buffer[1]._d == 1 &&
            got->_u.branch._buffer[1]._u.leaf == 20);
    CORBA_free(got);
  }
  {
    CxxMapping_ByBool b, *gb;
    CxxMapping_ByChar c, *gc;
    CxxMapping_ByEnum e, *ge;
    CxxMapping_ByULongLong u, gu;
    CxxMapping_WithArray wa, gwa;
    b._d = CORBA_FALSE;
    b._u.no = (CORBA_char*)"no";
    gb = CxxMapping_Types_echo_by_bool(t, &b, &ev);
    if (OK(&ev)) CHECK(gb->_d == CORBA_FALSE && strcmp(gb->_u.no, "no") == 0);
    CORBA_free(gb);
    c._d = 'c';
    c._u.bc = (CORBA_char*)"bc";
    gc = CxxMapping_Types_echo_by_char(t, &c, &ev);
    if (OK(&ev)) CHECK(gc->_d == 'c' && strcmp(gc->_u.bc, "bc") == 0);
    CORBA_free(gc);
    e._d = CxxMapping_BANANA;
    e._u.other = CORBA_TRUE;
    ge = CxxMapping_Types_echo_by_enum(t, &e, &ev);
    if (OK(&ev)) CHECK(ge->_d == CxxMapping_BANANA && ge->_u.other == CORBA_TRUE);
    CORBA_free(ge);
    e._d = CxxMapping_APPLE;
    e._u.apple_count = 3;
    ge = CxxMapping_Types_echo_by_enum(t, &e, &ev);
    if (OK(&ev)) CHECK(ge->_d == CxxMapping_APPLE && ge->_u.apple_count == 3);
    CORBA_free(ge);
    u._d = 7;
    u._u.seven = 7.5;
    gu = CxxMapping_Types_echo_by_ulonglong(t, &u, &ev);
    if (OK(&ev)) CHECK(gu._d == 7 && gu._u.seven == 7.5);
    u._d = 8; /* the implicit default: no member */
    gu = CxxMapping_Types_echo_by_ulonglong(t, &u, &ev);
    if (OK(&ev)) CHECK(gu._d == 8);
    memset(&wa, 0, sizeof wa);
    wa._d = 0;
    wa._u.cells[1][2] = -9;
    gwa = CxxMapping_Types_echo_with_array(t, &wa, &ev);
    if (OK(&ev)) CHECK(gwa._d == 0 && gwa._u.cells[1][2] == -9);
  }
  {
    CxxMapping_Holder h, *g;
    CxxMapping_Pair pairs[2] = {{1, 2}, {3, 4}};
    CORBA_long bounded[5] = {1, 2, 3, 4, 5};
    CORBA_wchar wide[] = {'w', 'i', 'd', 'e', 0};
    CORBA_boolean flags[3] = {1, 0, 1};
    CORBA_wchar wchars[2] = {0x3042, 'z'};
    CORBA_char* inner_strings[2] = {(CORBA_char*)"x", (CORBA_char*)"yy"};
    CORBA_sequence_string inner[2] = {{2, 2, inner_strings, 0}, {0, 0, NULL, 0}};
    memset(&h, 0, sizeof h);
    h.numbers[0] = 1;
    h.numbers[2] = 3;
    h.pairs._buffer = pairs;
    h.pairs._length = h.pairs._maximum = 2;
    h.cells[0][1] = 5;
    h.tag = (CORBA_char*)"eight888";
    h.bounded._buffer = bounded;
    h.bounded._length = h.bounded._maximum = 5;
    h.wide = wide;
    h.flags._buffer = flags;
    h.flags._length = 3;
    h.wchars._buffer = wchars;
    h.wchars._length = 2;
    h.nested._buffer = inner;
    h.nested._length = 2;
    h.target = CORBA_OBJECT_NIL;
    h.choice._d = CxxMapping_CHERRY;
    h.choice._u.cherry_name = (CORBA_char*)"bing";
    g = CxxMapping_Types_echo_holder(t, &h, &ev);
    if (OK(&ev)) {
      CHECK(g->numbers[2] == 3 && g->pairs._length == 2 && g->pairs._buffer[1].second == 4);
      CHECK(g->cells[0][1] == 5 && strcmp(g->tag, "eight888") == 0);
      CHECK(g->bounded._length == 5 && g->bounded._buffer[4] == 5);
      CHECK(CORBA_wstring_len(g->wide) == 4 && g->wide[3] == 'e');
      CHECK(g->flags._length == 3 && g->flags._buffer[0] == 1 && g->flags._buffer[1] == 0);
      CHECK(g->wchars._length == 2 && g->wchars._buffer[0] == 0x3042);
      CHECK(g->nested._length == 2 && g->nested._buffer[0]._length == 2 &&
            strcmp(g->nested._buffer[0]._buffer[1], "yy") == 0 && g->nested._buffer[1]._length == 0);
      CHECK(g->target == CORBA_OBJECT_NIL);
      CHECK(g->choice._d == CxxMapping_CHERRY && strcmp(g->choice._u.cherry_name, "bing") == 0);
    }
    CORBA_free(g);
    /* Bounds are checked when sending: BAD_PARAM, completed no. */
    h.tag = (CORBA_char*)"nine99999";
    g = CxxMapping_Types_echo_holder(t, &h, &ev);
    CHECK(g == NULL);
    RAISED(&ev, ex_CORBA_BAD_PARAM);
    h.tag = (CORBA_char*)"ok";
    h.bounded._length = 6;
    g = CxxMapping_Types_echo_holder(t, &h, &ev);
    RAISED(&ev, ex_CORBA_BAD_PARAM);
  }
  {
    CxxMapping_Outer o = {{5}, CxxMapping_Outer_OFF}, go;
    CxxMapping_Keywords k = {12, (CORBA_char*)"ns"}, *gk;
    GlobalPoint p = {3, -4}, gp;
    CORBA_char* s;
    go = CxxMapping_Types_echo_outer(t, &o, &ev);
    if (OK(&ev)) CHECK(go.nested.v == 5 && go.switch_mode == CxxMapping_Outer_OFF);
    gk = CxxMapping_Types_echo_keywords(t, &k, &ev);
    if (OK(&ev)) CHECK(gk->_class == 12 && strcmp(gk->_namespace, "ns") == 0);
    CORBA_free(gk);
    gp = CxxMapping_Types_echo_global(t, &p, &ev);
    if (OK(&ev)) CHECK(gp.x == 3 && gp.y == -4);
    s = CxxMapping_Types_echo_short_string(t, "12345678", &ev);
    if (OK(&ev)) CHECK(strcmp(s, "12345678") == 0);
    CORBA_free(s);
    s = CxxMapping_Types_echo_short_string(t, "123456789", &ev);
    RAISED(&ev, ex_CORBA_BAD_PARAM);
    s = CxxMapping_Types_echo_short_string(t, NULL, &ev);
    RAISED(&ev, ex_CORBA_BAD_PARAM);
  }
  {
    /* fixed: 123.45 as packed decimal (0x12 0x34 0x5c). */
    CxxMapping_Money m = {5, 2, {0x12, 0x34, 0x5c}}, total, r;
    CORBA_fixed_3_0 whole;
    memset(&total, 0, sizeof total);
    r = CxxMapping_Types_echo_money(t, &m, &total, &whole, &ev);
    if (OK(&ev)) {
      CHECK(memcmp(r._value, m._value, 3) == 0 && r._digits == 5 && r._scale == 2);
      CHECK(memcmp(total._value, m._value, 3) == 0);
      CHECK(whole._digits == 3 && whole._value[0] == 0x12 && whole._value[1] == 0x3c);
    }
  }
  {
    CORBA_long_double twice = 1.25, negated = 0, q;
    CORBA_long_double samples[2] = {0.5, -8};
    CxxMapping_Measure meas, *gm;
    q = CxxMapping_Types_echo_quad(t, 2.5, &twice, &negated, &ev);
    if (OK(&ev)) CHECK(q == 2.5 && twice == 2.5 && negated == -2.5);
    meas.value = 1.5;
    meas.samples._buffer = samples;
    meas.samples._length = 2;
    meas.samples._release = 0;
    gm = CxxMapping_Types_echo_measure(t, &meas, &ev);
    if (OK(&ev)) CHECK(gm->value == 1.5 && gm->samples._length == 2 && gm->samples._buffer[1] == -8);
    CORBA_free(gm);
  }
  {
    CxxMapping_Price p, *gp;
    CxxMapping_Money history[1] = {{5, 2, {0x00, 0x10, 0x0d}}};
    memset(&p, 0, sizeof p);
    p.amount._digits = 5;
    p.amount._scale = 2;
    p.amount._value[2] = 0x0c;
    p.tiny._value[15] = 0x1c; /* 0.000...1 */
    p.history._buffer = history;
    p.history._length = 1;
    gp = CxxMapping_Types_echo_price(t, &p, &ev);
    if (OK(&ev))
      CHECK(gp->tiny._digits == 31 && gp->tiny._value[15] == 0x1c && gp->history._length == 1 &&
            gp->history._buffer[0]._value[2] == 0x0d);
    CORBA_free(gp);
  }
  {
    CxxMapping_Failure* f;
    CORBA_SystemException* sys;
    CxxMapping_LaterSeq* l;
    CxxMapping_Types_fail(t, 1, &ev);
    f = (CxxMapping_Failure*)CORBA_exception_value(&ev);
    CHECK(ev._major == CORBA_USER_EXCEPTION && f && f->code == 1 &&
          strcmp(f->message, "failed") == 0 && f->kind == CxxMapping_CHERRY);
    RAISED(&ev, ex_CxxMapping_Failure);
    CxxMapping_Types_fail(t, 2, &ev);
    RAISED(&ev, ex_CxxMapping_Empty);
    CxxMapping_Types_fail(t, 3, &ev);
    sys = (CORBA_SystemException*)CORBA_exception_value(&ev);
    CHECK(ev._major == CORBA_SYSTEM_EXCEPTION && sys && sys->minor == 5 &&
          sys->completed == CORBA_COMPLETED_YES);
    RAISED(&ev, ex_CORBA_NO_PERMISSION);
    l = CxxMapping_Types_laters(t, 3, &ev);
    if (OK(&ev)) CHECK(l->_length == 3 && l->_buffer[2] == CORBA_OBJECT_NIL);
    CORBA_free(l);
    CxxMapping_Types_delete(t, "gone", &ev);
    if (OK(&ev)) CHECK(strcmp(deleted, "gone") == 0);
  }
}

static void test_multi(CxxMapping_Multi m) {
  ENV;
  CORBA_long b = 5, c = 0, r;
  CORBA_char* s;
  CxxMapping_Base_BaseInfo info = {9}, gi;
  CxxMapping_Base base;
  CHECK(CORBA_Object_is_a(m, "IDL:rustorb.org/CxxMapping/Base:1.0", &ev));
  CHECK(CORBA_Object_is_a(m, "IDL:rustorb.org/CxxMapping/Other:1.0", &ev));
  CHECK(!CORBA_Object_is_a(m, "IDL:rustorb.org/CxxMapping/Types:1.0", &ev));
  OK(&ev);
  CHECK(CxxMapping_Multi__get_id(m, &ev) == 42);
  CxxMapping_Multi__set_label(m, "second", &ev);
  s = CxxMapping_Multi__get_label(m, &ev);
  if (OK(&ev)) CHECK(strcmp(s, "second") == 0);
  CORBA_free(s);
  /* The base's operation through the derived interface's name, and the
     base's own stub on the same reference. */
  CxxMapping_Multi_base_op(m, CORBA_FALSE, &ev);
  OK(&ev);
  CxxMapping_Base_base_op(m, CORBA_TRUE, &ev);
  CHECK(ev._major == CORBA_USER_EXCEPTION &&
        strcmp(((CxxMapping_Base_BaseError*)CORBA_exception_value(&ev))->why, "asked to") == 0);
  RAISED(&ev, ex_CxxMapping_Base_BaseError);
  r = CxxMapping_Multi_derived_op(m, 2, &b, &c, &ev);
  if (OK(&ev)) CHECK(r == 7 && b == 2 && c == 5);
  CxxMapping_Multi_notify(m, "hello", &ev);
  OK(&ev);
  base = CxxMapping_Multi_as_base(m, &ev);
  if (OK(&ev)) CHECK(CORBA_Object_is_equivalent(base, m, &ev));
  CORBA_Object_release(base, &ev);
  s = CxxMapping_Multi_other_op(m, &ev);
  if (OK(&ev)) CHECK(strcmp(s, "other") == 0);
  CORBA_free(s);
  gi = CxxMapping_Multi_multi_op(m, &info, &ev);
  if (OK(&ev)) CHECK(gi.id == 10);
}

static CORBA_any* any_of(CORBA_TypeCode tc, void* value) {
  CORBA_any* a = CORBA_any__alloc();
  a->_type = tc;
  a->_value = value;
  a->_release = CORBA_FALSE;
  return a;
}

/* Sends an any and checks that the same value comes back. */
static void round_trip(AnyTest_AnyEcho e, CORBA_any* a, int line) {
  ENV;
  CORBA_any* got = AnyTest_AnyEcho_echo(e, a, &ev);
  if (!ok(&ev, line)) {
    CORBA_free(a);
    return;
  }
  if (!CORBA_TypeCode_equal(got->_type, a->_type, &ev) || !CORBA_any_equal(got, a) ||
      (a->_value && !got->_value)) {
    failures++;
    fprintf(stderr, "%s:%d: the any came back different\n", __FILE__, line);
  }
  CORBA_free(got);
  CORBA_free(a);
}
#define ROUND_TRIP(e, a) round_trip((e), (a), __LINE__)

static void test_any(CORBA_ORB orb, AnyTest_AnyEcho e) {
  ENV;
  CORBA_long l = -7;
  CORBA_char* str = (CORBA_char*)"text";
  AnyTest_Point p = {1, 2.5};
  AnyTest_Choice ch;
  AnyTest_Shade shade = AnyTest_DIM;
  AnyTest_Money money = {5, 2, {0x00, 0x99, 0x9c}};
  AnyTest_ShortArray arr = {1, -2, 3};
  CORBA_double doubles[3] = {0.5, 1.5, -2};
  CORBA_sequence_double dseq = {3, 3, doubles, 0};
  AnyTest_Node kids[1] = {{(CORBA_char*)"kid", {0, 0, NULL, 0}}};
  AnyTest_Node node = {(CORBA_char*)"parent", {1, 1, kids, 0}};
  CORBA_any inner;
  AnyTest_NameValue nv[2];
  AnyTest_NVList list = {2, 2, nv, 0}, *gl;
  CORBA_any *a, *got, *out = NULL, io;
  CORBA_TypeCode seq_tc, tc;

  ROUND_TRIP(e, any_of(TC_long, &l));
  ROUND_TRIP(e, any_of(TC_string, &str));
  ROUND_TRIP(e, any_of(TC_AnyTest_Point, &p));
  ch._d = 3;
  ch._u.s = (CORBA_char*)"three";
  ROUND_TRIP(e, any_of(TC_AnyTest_Choice, &ch));
  ROUND_TRIP(e, any_of(TC_AnyTest_Shade, &shade));
  ROUND_TRIP(e, any_of(TC_AnyTest_Money, &money));
  ROUND_TRIP(e, any_of(TC_AnyTest_ShortArray, arr));
  ROUND_TRIP(e, any_of(TC_AnyTest_Node, &node));
  ROUND_TRIP(e, any_of(TC_AnyTest_Short5, &str));
  ROUND_TRIP(e, any_of(TC_null, NULL));
  ROUND_TRIP(e, any_of(TC_TypeCode, &(CORBA_TypeCode){TC_AnyTest_Node}));
  /* An anonymous sequence: the runtime makes its descriptor. */
  seq_tc = CORBA_ORB_create_sequence_tc(orb, 0, TC_double, &ev);
  OK(&ev);
  ROUND_TRIP(e, any_of(CORBA_Object_duplicate(seq_tc, &ev), &dseq));

  /* The value comes back as the C type. */
  a = any_of(TC_AnyTest_Point, &p);
  got = AnyTest_AnyEcho_echo(e, a, &ev);
  if (OK(&ev)) {
    AnyTest_Point* gp = (AnyTest_Point*)got->_value;
    CHECK(CORBA_TypeCode_equal(got->_type, TC_AnyTest_Point, &ev) && gp && gp->x == 1 &&
          gp->y == 2.5);
  }
  CORBA_free(got);
  CORBA_free(a);

  /* Anys in a struct in a sequence. */
  inner._type = TC_AnyTest_Point;
  inner._value = &p;
  inner._release = CORBA_FALSE;
  inner._rustorb = NULL;
  nv[0].name = (CORBA_char*)"point";
  nv[0].value = inner;
  nv[1].name = (CORBA_char*)"seq";
  nv[1].value._type = seq_tc;
  nv[1].value._value = &dseq;
  nv[1].value._release = CORBA_FALSE;
  nv[1].value._rustorb = NULL;
  gl = AnyTest_AnyEcho_echo_nvlist(e, &list, &ev);
  if (OK(&ev)) {
    CHECK(gl->_length == 2 && strcmp(gl->_buffer[1].name, "seq") == 0);
    CHECK(CORBA_any_equal(&gl->_buffer[0].value, &inner));
    CHECK(CORBA_any_equal(&gl->_buffer[1].value, &nv[1].value));
    CHECK(((CORBA_sequence_double*)gl->_buffer[1].value._value)->_buffer[2] == -2);
  }
  CORBA_free(gl);

  /* out and inout anys. */
  io._type = TC_string;
  io._value = &str;
  io._release = CORBA_FALSE;
  io._rustorb = NULL;
  AnyTest_AnyEcho_out_inout(e, "x", &out, &io, &ev);
  if (OK(&ev)) {
    CHECK(out && CORBA_TypeCode_equal(out->_type, TC_string, &ev) &&
          strcmp(*(CORBA_char**)out->_value, "text") == 0);
    CHECK(CORBA_TypeCode_equal(io._type, TC_long, &ev) && *(CORBA_long*)io._value == 7);
  }
  CORBA_free(out);
  rustorb_c_free_value(&CORBA_any__desc, &io);

  /* TypeCodes. */
  tc = AnyTest_AnyEcho_echo_tc(e, TC_AnyTest_NVList, &ev);
  if (OK(&ev)) CHECK(CORBA_TypeCode_equal(tc, TC_AnyTest_NVList, &ev));
  CORBA_Object_release(tc, &ev);

  /* An any without a value of its type cannot be sent. */
  a = any_of(TC_AnyTest_Point, NULL);
  got = AnyTest_AnyEcho_echo(e, a, &ev);
  RAISED(&ev, ex_CORBA_BAD_PARAM);
  CORBA_free(a);
  CORBA_Object_release(seq_tc, &ev);
}

static void test_memory(void) {
  /* CORBA_free frees what the values hold. */
  CxxMapping_Holder* h = CxxMapping_Holder__alloc();
  CxxMapping_Grid_slice* g = CxxMapping_Grid__alloc();
  CORBA_sequence_CxxMapping_Pair* s = CORBA_sequence_CxxMapping_Pair__alloc();
  CxxMapping_Tree* t = CxxMapping_Tree__alloc();
  CxxMapping_Tree* copy;
  h->tag = CORBA_string_dup("tag");
  h->nested._buffer = CORBA_sequence_CORBA_sequence_string_allocbuf(2);
  h->nested._length = h->nested._maximum = 2;
  h->nested._release = CORBA_TRUE;
  h->nested._buffer[0]._buffer = CORBA_sequence_string_allocbuf(1);
  h->nested._buffer[0]._buffer[0] = CORBA_string_dup("deep");
  h->nested._buffer[0]._length = 1;
  h->nested._buffer[0]._release = CORBA_TRUE;
  CORBA_free(h);
  g[1][2] = 3;
  CORBA_free(g);
  s->_buffer = CORBA_sequence_CxxMapping_Pair_allocbuf(4);
  s->_length = 4;
  s->_release = CORBA_TRUE;
  CORBA_free(s);
  t->label = CORBA_string_dup("root");
  t->children._buffer = CORBA_sequence_CxxMapping_Tree_allocbuf(1);
  t->children._length = 1;
  t->children._release = CORBA_TRUE;
  t->children._buffer[0].label = CORBA_string_dup("child");
  copy = (CxxMapping_Tree*)rustorb_c_copy(&CxxMapping_Tree__desc, t);
  CHECK(copy && tree_eq(copy, t) && copy->children._buffer != t->children._buffer);
  CORBA_free(copy);
  CORBA_free(t);
  CORBA_free(NULL);
}

/* Sequences of numbers: copied and freed at once, and large buffers kept
   for reuse come back zeroed from allocbuf. */
static const rustorb_c_type seq_octet_desc = {CORBA_tk_sequence, sizeof(CORBA_sequence_octet), "",
                                              "", &CORBA_octet__desc, 0, 0, NULL, NULL, -1, 0, 0,
                                              NULL};
static const rustorb_c_type seq_double_desc = {CORBA_tk_sequence, sizeof(CORBA_sequence_double), "",
                                               "", &CORBA_double__desc, 0, 0, NULL, NULL, -1, 0, 0,
                                               NULL};

static void test_large_sequences(void) {
  const CORBA_unsigned_long n = 1u << 20;
  CORBA_unsigned_long i;
  int round, zero = 1;
  CORBA_sequence_octet s = {n, n, NULL, CORBA_TRUE};
  CORBA_sequence_double d = {n / 8, n / 8, NULL, CORBA_TRUE};
  CORBA_sequence_octet* sc;
  CORBA_sequence_double* dc;
  for (round = 0; round < 3; round++) {
    CORBA_octet* b = CORBA_sequence_octet_allocbuf(n);
    CHECK(b != NULL);
    for (i = 0; i < n; i++)
      if (b[i]) zero = 0;
    memset(b, 0xab, n);
    CORBA_free(b);
  }
  CHECK(zero);
  s._buffer = CORBA_sequence_octet_allocbuf(n);
  for (i = 0; i < n; i++) s._buffer[i] = (CORBA_octet)(i * 7);
  sc = (CORBA_sequence_octet*)rustorb_c_copy(&seq_octet_desc, &s);
  CHECK(sc && sc->_length == n && sc->_buffer != s._buffer && sc->_release);
  CHECK(sc && memcmp(sc->_buffer, s._buffer, n) == 0);
  CORBA_free(sc);
  d._buffer = (CORBA_double*)rustorb_c_alloc(&CORBA_double__desc, n / 8);
  for (i = 0; i < n / 8; i++) d._buffer[i] = i * 0.5;
  dc = (CORBA_sequence_double*)rustorb_c_copy(&seq_double_desc, &d);
  CHECK(dc && dc->_length == n / 8 && memcmp(dc->_buffer, d._buffer, n) == 0);
  CORBA_free(dc);
  CORBA_free(s._buffer);
  CORBA_free(d._buffer);
}

int main(int argc, char** argv) {
  ENV;
  CORBA_ORB orb = CORBA_ORB_init(&argc, argv, "", &ev);
  PortableServer_POA poa;
  PortableServer_POAManager manager;
  CORBA_Object types, multi, anyecho;
  PortableServer_ObjectId* id;
  if (!OK(&ev)) return 1;
  poa = CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
  manager = PortableServer_POA__get_the_POAManager(poa, &ev);
  PortableServer_POAManager_activate(manager, &ev);
  if (!OK(&ev)) return 1;
  POA_CxxMapping_Types__init(&types_servant, &ev);
  POA_CxxMapping_Multi__init(&multi_servant, &ev);
  anyecho_epv.echo = any_echo;
  anyecho_epv.echo_tc = any_echo_tc;
  anyecho_epv.echo_nvlist = any_echo_nvlist;
  anyecho_epv.out_inout = any_out_inout;
  POA_AnyTest_AnyEcho__init(&anyecho_servant, &ev);
  if (!OK(&ev)) return 1;
  types = activate(poa, orb, &types_servant);
  multi = activate(poa, orb, &multi_servant);
  anyecho = activate(poa, orb, &anyecho_servant);

  test_constants();
  test_typecodes();
  test_memory();
  test_large_sequences();
  test_types(types);
  test_multi(multi);
  test_any(orb, anyecho);

  /* An operation the servant does not implement: NO_IMPLEMENT. */
  {
    CORBA_char* d = AnyTest_AnyEcho_describe(anyecho, &(CORBA_any){TC_long, &(CORBA_long){1}, 0, NULL}, &ev);
    CHECK(d == NULL);
    RAISED(&ev, ex_CORBA_NO_IMPLEMENT);
  }

  CORBA_Object_release(types, &ev);
  CORBA_Object_release(multi, &ev);
  CORBA_Object_release(anyecho, &ev);
  id = PortableServer_POA_servant_to_id(poa, &types_servant, &ev);
  PortableServer_POA_deactivate_object(poa, id, &ev);
  CORBA_free(id);
  OK(&ev);
  CORBA_Object_release(manager, &ev);
  CORBA_Object_release(poa, &ev);
  CORBA_ORB_destroy(orb, &ev);
  OK(&ev);
  CORBA_Object_release(orb, &ev);
  POA_CxxMapping_Types__fini(&types_servant, &ev);
  POA_CxxMapping_Multi__fini(&multi_servant, &ev);
  POA_AnyTest_AnyEcho__fini(&anyecho_servant, &ev);
  if (failures) {
    fprintf(stderr, "%d failures\n", failures);
    return 1;
  }
  printf("mapping: all checks passed\n");
  return 0;
}
