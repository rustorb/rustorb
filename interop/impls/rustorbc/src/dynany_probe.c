/* The DynAny probe of interop/impls/omniorb-cpp/dynany_probe.cc on the
 * rustorb C binding (<rustorb/dynany.h>): the same cases, whose output
 * run-dynany-probe.sh compares with expected/dynany-probe-rustorb.txt.
 *
 * Licensed under MIT OR Apache-2.0, like the rest of the rustorb core. */

#include <stdio.h>
#include <string.h>

#include <rustorb/dynany.h>

static CORBA_Environment ev = {CORBA_NO_EXCEPTION, NULL, NULL};
static DynamicAny_DynAnyFactory factory;

typedef DynamicAny_DynAny D;

static const char* kind_name(CORBA_TCKind k) {
  static const char* names[] = {
      "tk_null", "tk_void", "tk_short", "tk_long", "tk_ushort", "tk_ulong",
      "tk_float", "tk_double", "tk_boolean", "tk_char", "tk_octet", "tk_any",
      "tk_TypeCode", "tk_Principal", "tk_objref", "tk_struct", "tk_union",
      "tk_enum", "tk_string", "tk_sequence", "tk_array", "tk_alias",
      "tk_except", "tk_longlong", "tk_ulonglong", "tk_longdouble", "tk_wchar",
      "tk_wstring", "tk_fixed", "tk_value", "tk_value_box", "tk_native",
      "tk_abstract_interface", "tk_local_interface", "tk_component",
      "tk_home", "tk_event"};
  unsigned i = (unsigned)k;
  return i < sizeof names / sizeof *names ? names[i] : "?";
}

/* A case: the body runs until an exception (X), whose name ends the line. */
#define CASE(name) printf("%s: ", (name)); do
#define END                                       \
  while (0);                                      \
  finish()
#define X                                         \
  if (ev._major != CORBA_NO_EXCEPTION) break

static void finish(void) {
  if (ev._major != CORBA_NO_EXCEPTION) {
    const char* id = ev._id;
    if (strcmp(id, ex_DynamicAny_DynAny_TypeMismatch) == 0) {
      printf("TypeMismatch");
    } else if (strcmp(id, ex_DynamicAny_DynAny_InvalidValue) == 0) {
      printf("InvalidValue");
    } else if (strcmp(id, ex_DynamicAny_DynAnyFactory_InconsistentTypeCode) == 0) {
      printf("InconsistentTypeCode");
    } else if (strcmp(id, ex_DynamicAny_MustTruncate) == 0) {
      printf("MustTruncate");
    } else {
      /* IDL:omg.org/CORBA/BAD_PARAM:1.0 -> BAD_PARAM */
      const char* s = strrchr(id, '/');
      const char* e;
      s = s ? s + 1 : id;
      e = strchr(s, ':');
      printf("%.*s", (int)(e ? e - s : (long)strlen(s)), s);
    }
    CORBA_exception_free(&ev);
  }
  printf("\n");
}

static void out_b(CORBA_boolean b) { printf("%s", b ? "true" : "false"); }
static void out_i(long v) { printf("%ld", v); }
static void out_s(const char* s) {
  printf("\"%s\"", s ? s : "");
}
static void out_k(CORBA_TCKind k) { printf("%s", kind_name(k)); }
static void sp(void) { printf(" "); }

static D mk(CORBA_TypeCode tc) {
  return DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(factory, tc, &ev);
}

/* The current position, found by counting the components after it (this
   moves the position to -1, so it comes last in a case). */
static long position(D d) {
  D cur = DynamicAny_DynAny_current_component(d, &ev);
  long after = 0;
  if (ev._major != CORBA_NO_EXCEPTION || !cur) return -1;
  CORBA_Object_release(cur, &ev);
  while (DynamicAny_DynAny_next(d, &ev)) after++;
  return (long)DynamicAny_DynAny_component_count(d, &ev) - 1 - after;
}

static void describe(D d) {
  CORBA_any* a = DynamicAny_DynAny_to_any(d, &ev);
  if (a) out_k(CORBA_TypeCode_kind(a->_type, &ev));
}

/* Anys of values kept in static storage. */
static CORBA_any any_of(CORBA_TypeCode tc, void* value) {
  CORBA_any a;
  a._type = tc;
  a._value = value;
  a._release = CORBA_FALSE;
  a._rustorb = NULL;
  return a;
}

static CORBA_long longs[8] = {0, 1, 2, 3, 4, 5, 6, 7};
static CORBA_short short_one = 1;
static CORBA_octet octet_zero = 0;
static CORBA_boolean true_ = CORBA_TRUE, false_ = CORBA_FALSE;
static CORBA_short shorts[] = {5, 2};

int main(int argc, char** argv) {
  CORBA_ORB orb = CORBA_ORB_init(&argc, argv, "", &ev);
  CORBA_TypeCode tc_long = TC_long, tc_string = TC_string;
  CORBA_TypeCode tc_principal = rustorb_c_tc_simple(CORBA_tk_Principal);
  CORBA_TypeCode tc_longdouble = TC_longdouble;
  CORBA_TypeCode count, s_tc, e_tc, color, u_tc, d_tc, n_tc, b_tc, seq_tc, bseq_tc, arr_tc,
      money, bstr, v_tc, box_tc;
  if (ev._major != CORBA_NO_EXCEPTION) return 1;
  factory = CORBA_ORB_resolve_initial_references(orb, "DynAnyFactory", &ev);
  count = CORBA_ORB_create_alias_tc(orb, "IDL:P/Count:1.0", "Count", tc_long, &ev);
  {
    CORBA_StructMember m[3] = {{"a", NULL, NULL}, {"b", NULL, NULL}, {"c", NULL, NULL}};
    CORBA_StructMemberSeq seq = {3, 3, NULL, CORBA_FALSE};
    m[0].type = tc_long;
    m[1].type = tc_string;
    m[2].type = count;
    seq._buffer = m;
    s_tc = CORBA_ORB_create_struct_tc(orb, "IDL:P/S:1.0", "S", &seq, &ev);
    seq._length = 0;
    e_tc = CORBA_ORB_create_exception_tc(orb, "IDL:P/E:1.0", "E", &seq, &ev);
  }
  {
    CORBA_char* names[3] = {"RED", "GREEN", "BLUE"};
    CORBA_EnumMemberSeq seq = {3, 3, NULL, CORBA_FALSE};
    seq._buffer = names;
    color = CORBA_ORB_create_enum_tc(orb, "IDL:P/Color:1.0", "Color", &seq, &ev);
  }
  {
    /* union U switch (long) { case 1: long x; case 2: case 3: string y;
                               default: boolean z; }; */
    CORBA_UnionMember m[4];
    CORBA_UnionMemberSeq seq = {4, 4, NULL, CORBA_FALSE};
    memset(m, 0, sizeof m);
    seq._buffer = m;
    m[0].name = "x"; m[0].label = any_of(tc_long, &longs[1]); m[0].type = tc_long;
    m[1].name = "y"; m[1].label = any_of(tc_long, &longs[2]); m[1].type = tc_string;
    m[2].name = "y"; m[2].label = any_of(tc_long, &longs[3]); m[2].type = tc_string;
    m[3].name = "z"; m[3].label = any_of(TC_octet, &octet_zero); m[3].type = TC_boolean;
    u_tc = CORBA_ORB_create_union_tc(orb, "IDL:P/U:1.0", "U", tc_long, &seq, &ev);
    /* union D switch (long) { default: boolean z; case 0: long x; }; */
    seq._length = 2;
    m[0].name = "z"; m[0].label = any_of(TC_octet, &octet_zero); m[0].type = TC_boolean;
    m[1].name = "x"; m[1].label = any_of(tc_long, &longs[0]); m[1].type = tc_long;
    d_tc = CORBA_ORB_create_union_tc(orb, "IDL:P/D:1.0", "D", tc_long, &seq, &ev);
    /* union N switch (short) { case 5: long x; case 2: string y; }; */
    m[0].name = "x"; m[0].label = any_of(TC_short, &shorts[0]); m[0].type = tc_long;
    m[1].name = "y"; m[1].label = any_of(TC_short, &shorts[1]); m[1].type = tc_string;
    n_tc = CORBA_ORB_create_union_tc(orb, "IDL:P/N:1.0", "N", TC_short, &seq, &ev);
    /* union B switch (boolean) { case TRUE: long t; case FALSE: string f; }; */
    m[0].name = "t"; m[0].label = any_of(TC_boolean, &true_); m[0].type = tc_long;
    m[1].name = "f"; m[1].label = any_of(TC_boolean, &false_); m[1].type = tc_string;
    b_tc = CORBA_ORB_create_union_tc(orb, "IDL:P/B:1.0", "B", TC_boolean, &seq, &ev);
  }
  seq_tc = CORBA_ORB_create_sequence_tc(orb, 0, tc_long, &ev);
  bseq_tc = CORBA_ORB_create_sequence_tc(orb, 2, tc_long, &ev);
  arr_tc = CORBA_ORB_create_array_tc(orb, 3, tc_long, &ev);
  money = CORBA_ORB_create_fixed_tc(orb, 5, 2, &ev);
  bstr = CORBA_ORB_create_string_tc(orb, 3, &ev);
  {
    CORBA_ValueMember m[1];
    CORBA_ValueMemberSeq seq = {1, 1, NULL, CORBA_FALSE};
    memset(m, 0, sizeof m);
    m[0].name = "n";
    m[0].type = tc_long;
    m[0].access = CORBA_PUBLIC_MEMBER;
    seq._buffer = m;
    v_tc = CORBA_ORB_create_value_tc(orb, "IDL:P/V:1.0", "V", CORBA_VM_NONE, CORBA_OBJECT_NIL,
                                     &seq, &ev);
  }
  box_tc = CORBA_ORB_create_value_box_tc(orb, "IDL:P/LBox:1.0", "LBox", tc_long, &ev);
  if (ev._major != CORBA_NO_EXCEPTION) {
    fprintf(stderr, "setup: %s\n", ev._id);
    return 1;
  }

  /* -- creation from TypeCodes and positions */
  CASE("long.count") { D d = mk(tc_long); X; out_i((long)DynamicAny_DynAny_component_count(d, &ev)); } END;
  CASE("long.get") { D d = mk(tc_long); CORBA_long v; X; v = DynamicAny_DynAny_get_long(d, &ev); X; out_i(v); } END;
  CASE("long.seek0") { D d = mk(tc_long); CORBA_boolean b; X; b = DynamicAny_DynAny_seek(d, 0, &ev); X; out_b(b); } END;
  CASE("long.next") { D d = mk(tc_long); CORBA_boolean b; X; b = DynamicAny_DynAny_next(d, &ev); X; out_b(b); } END;
  CASE("long.current_component") { D d = mk(tc_long); D c; X; c = DynamicAny_DynAny_current_component(d, &ev); X; out_b(!c); } END;
  CASE("long.get_short") { D d = mk(tc_long); X; DynamicAny_DynAny_get_short(d, &ev); } END;
  CASE("long.insert_short") { D d = mk(tc_long); X; DynamicAny_DynAny_insert_short(d, 1, &ev); } END;
  CASE("alias.insert_long") { D d = mk(count); CORBA_long v; X; DynamicAny_DynAny_insert_long(d, 7, &ev); X; v = DynamicAny_DynAny_get_long(d, &ev); X; out_i(v); } END;
  CASE("alias.type") { D d = mk(count); CORBA_TypeCode t; X; t = DynamicAny_DynAny_type(d, &ev); X; out_k(CORBA_TypeCode_kind(t, &ev)); } END;
  CASE("string.get") { D d = mk(tc_string); CORBA_char* s; X; s = DynamicAny_DynAny_get_string(d, &ev); X; out_s(s); CORBA_free(s); } END;
  CASE("bstring.insert_long") { D d = mk(bstr); X; DynamicAny_DynAny_insert_string(d, "abcd", &ev); } END;
  CASE("bstring.insert_ok") { D d = mk(bstr); X; DynamicAny_DynAny_insert_string(d, "abc", &ev); X; out_b(1); } END;
  CASE("typecode.get") { D d = mk(TC_TypeCode); CORBA_TypeCode t; X; t = DynamicAny_DynAny_get_typecode(d, &ev); X; out_k(CORBA_TypeCode_kind(t, &ev)); } END;
  CASE("any.get") { D d = mk(TC_any); CORBA_any* a; X; a = DynamicAny_DynAny_get_any(d, &ev); X; out_k(CORBA_TypeCode_kind(a->_type, &ev)); CORBA_free(a); } END;
  CASE("any.count") { D d = mk(TC_any); X; out_i((long)DynamicAny_DynAny_component_count(d, &ev)); } END;
  CASE("objref.get") { D d = mk(TC_Object); CORBA_Object o; X; o = DynamicAny_DynAny_get_reference(d, &ev); X; out_b(o == CORBA_OBJECT_NIL); } END;
  CASE("principal") { D d = mk(tc_principal); X; describe(d); } END;
  CASE("longdouble") { D d = mk(tc_longdouble); X; describe(d); } END;

  /* -- get_dyn_any: shared with the any or a copy */
  CASE("any.get_dyn_any_shared") {
    D d = mk(TC_any), inner;
    CORBA_any a = any_of(tc_long, &longs[5]), *b;
    X;
    DynamicAny_DynAny_insert_any(d, &a, &ev); X;
    inner = DynamicAny_DynAny_get_dyn_any(d, &ev); X;
    DynamicAny_DynAny_insert_long(inner, 9, &ev); X;
    b = DynamicAny_DynAny_get_any(d, &ev); X;
    out_i(*(CORBA_long*)b->_value);
    CORBA_free(b);
  } END;

  /* -- struct */
  CASE("struct.count") { D d = mk(s_tc); X; out_i((long)DynamicAny_DynAny_component_count(d, &ev)); } END;
  CASE("struct.position") { D d = mk(s_tc); X; out_i(position(d)); } END;
  CASE("struct.insert_long") { D d = mk(s_tc); CORBA_char* n; X; DynamicAny_DynAny_insert_long(d, 3, &ev); X; n = DynamicAny_DynStruct_current_member_name(d, &ev); X; out_s(n); CORBA_free(n); } END;
  CASE("struct.insert_long_moves") { D d = mk(s_tc); D c; CORBA_boolean b; X; DynamicAny_DynAny_insert_long(d, 3, &ev); X; b = DynamicAny_DynAny_seek(d, 0, &ev); X; out_b(b); c = DynamicAny_DynAny_current_component(d, &ev); X; out_i(DynamicAny_DynAny_get_long(c, &ev)); } END;
  CASE("struct.insert_string_at_0") { D d = mk(s_tc); X; DynamicAny_DynAny_insert_string(d, "x", &ev); } END;
  CASE("struct.member_kind_alias") { D d = mk(s_tc); CORBA_TCKind k; X; DynamicAny_DynAny_seek(d, 2, &ev); X; k = DynamicAny_DynStruct_current_member_kind(d, &ev); X; out_k(k); } END;
  CASE("struct.member_name_end") { D d = mk(s_tc); CORBA_char* n; X; DynamicAny_DynAny_seek(d, 5, &ev); X; n = DynamicAny_DynStruct_current_member_name(d, &ev); X; out_s(n); } END;
  CASE("struct.insert_at_end") { D d = mk(s_tc); X; DynamicAny_DynAny_seek(d, 5, &ev); X; DynamicAny_DynAny_insert_long(d, 1, &ev); } END;
  CASE("struct.next_past_end") { D d = mk(s_tc); D c; CORBA_boolean b; X; DynamicAny_DynAny_seek(d, 2, &ev); X; b = DynamicAny_DynAny_next(d, &ev); X; out_b(b); c = DynamicAny_DynAny_current_component(d, &ev); X; out_b(!c); } END;
  CASE("struct.next_from_minus1") { D d = mk(s_tc); CORBA_boolean b; X; DynamicAny_DynAny_seek(d, -1, &ev); X; b = DynamicAny_DynAny_next(d, &ev); X; out_b(b); out_i(position(d)); } END;
  CASE("struct.seek_minus1") { D d = mk(s_tc); CORBA_boolean b; X; b = DynamicAny_DynAny_seek(d, -1, &ev); X; out_b(b); } END;
  CASE("struct.component_shared") {
    D d = mk(s_tc), c;
    DynamicAny_NameValuePairSeq* m;
    X;
    c = DynamicAny_DynAny_current_component(d, &ev); X;
    DynamicAny_DynAny_insert_long(c, 11, &ev); X;
    m = DynamicAny_DynStruct_get_members(d, &ev); X;
    out_i(*(CORBA_long*)m->_buffer[0].value._value);
    CORBA_free(m);
  } END;
  CASE("struct.members_as_dyn_any_shared") {
    D d = mk(s_tc), c;
    DynamicAny_NameDynAnyPairSeq* m;
    X;
    m = DynamicAny_DynStruct_get_members_as_dyn_any(d, &ev); X;
    DynamicAny_DynAny_insert_long(m->_buffer[0].value, 12, &ev); X;
    c = DynamicAny_DynAny_current_component(d, &ev); X;
    out_i(DynamicAny_DynAny_get_long(c, &ev));
    CORBA_free(m);
  } END;
  CASE("struct.set_members_wrong_name") {
    D d = mk(s_tc);
    DynamicAny_NameValuePairSeq* m;
    X;
    m = DynamicAny_DynStruct_get_members(d, &ev); X;
    CORBA_free(m->_buffer[1].id);
    m->_buffer[1].id = CORBA_string_dup("zz");
    DynamicAny_DynStruct_set_members(d, m, &ev);
    CORBA_free(m);
  } END;
  CASE("struct.set_members_empty_names") {
    D d = mk(s_tc);
    DynamicAny_NameValuePairSeq* m;
    CORBA_unsigned_long i;
    X;
    m = DynamicAny_DynStruct_get_members(d, &ev); X;
    for (i = 0; i < m->_length; i++) {
      CORBA_free(m->_buffer[i].id);
      m->_buffer[i].id = CORBA_string_dup("");
    }
    DynamicAny_DynStruct_set_members(d, m, &ev);
    CORBA_free(m); X;
    out_b(1);
  } END;
  CASE("struct.set_members_short") {
    D d = mk(s_tc);
    DynamicAny_NameValuePairSeq* m;
    X;
    m = DynamicAny_DynStruct_get_members(d, &ev); X;
    m->_length = 2; /* the third stays in the buffer, freed with it */
    DynamicAny_DynStruct_set_members(d, m, &ev);
    m->_length = 3;
    CORBA_free(m);
  } END;
  CASE("struct.set_members_wrong_type") {
    D d = mk(s_tc);
    DynamicAny_NameValuePairSeq* m;
    CORBA_any a = any_of(TC_short, &short_one);
    X;
    m = DynamicAny_DynStruct_get_members(d, &ev); X;
    rustorb_c_free_value(&CORBA_any__desc, &m->_buffer[0].value);
    rustorb_c_copy_value(&CORBA_any__desc, &m->_buffer[0].value, &a);
    DynamicAny_DynStruct_set_members(d, m, &ev);
    CORBA_free(m);
  } END;
  CASE("struct.set_members_unaliased") {
    D d = mk(s_tc);
    DynamicAny_NameValuePairSeq* m;
    CORBA_any a = any_of(tc_long, &longs[4]);
    X;
    m = DynamicAny_DynStruct_get_members(d, &ev); X;
    rustorb_c_free_value(&CORBA_any__desc, &m->_buffer[2].value);
    rustorb_c_copy_value(&CORBA_any__desc, &m->_buffer[2].value, &a);
    DynamicAny_DynStruct_set_members(d, m, &ev);
    CORBA_free(m); X;
    out_b(1);
  } END;
  CASE("struct.set_members_position") {
    D d = mk(s_tc);
    DynamicAny_NameValuePairSeq* m;
    X;
    DynamicAny_DynAny_seek(d, 2, &ev); X;
    m = DynamicAny_DynStruct_get_members(d, &ev); X;
    DynamicAny_DynStruct_set_members(d, m, &ev);
    CORBA_free(m); X;
    out_i(position(d));
  } END;
  CASE("struct.from_any_wrong") { D d = mk(s_tc); CORBA_any a = any_of(tc_long, &longs[1]); X; DynamicAny_DynAny_from_any(d, &a, &ev); } END;
  CASE("struct.from_any_position") {
    D d = mk(s_tc);
    CORBA_any* a;
    X;
    a = DynamicAny_DynAny_to_any(d, &ev); X;
    DynamicAny_DynAny_seek(d, 2, &ev); X;
    DynamicAny_DynAny_from_any(d, a, &ev);
    CORBA_free(a); X;
    out_i(position(d));
  } END;
  CASE("struct.assign_wrong") { D d = mk(s_tc); D e; X; e = mk(tc_long); X; DynamicAny_DynAny_assign(d, e, &ev); } END;
  CASE("struct.destroy_component") {
    D d = mk(s_tc), c;
    X;
    c = DynamicAny_DynAny_current_component(d, &ev); X;
    DynamicAny_DynAny_destroy(c, &ev); X;
    DynamicAny_DynAny_insert_long(c, 2, &ev); X;
    out_i(DynamicAny_DynAny_get_long(c, &ev));
  } END;
  CASE("struct.destroyed") { D d = mk(s_tc); D c; X; c = DynamicAny_DynAny_current_component(d, &ev); X; DynamicAny_DynAny_destroy(d, &ev); X; DynamicAny_DynAny_get_long(c, &ev); } END;
  CASE("struct.destroyed_top") { D d = mk(s_tc); X; DynamicAny_DynAny_destroy(d, &ev); X; DynamicAny_DynAny_component_count(d, &ev); } END;
  CASE("struct.copy_independent") {
    D d = mk(s_tc), e, c;
    X;
    e = DynamicAny_DynAny_copy(d, &ev); X;
    DynamicAny_DynAny_insert_long(e, 5, &ev); X;
    c = DynamicAny_DynAny_current_component(d, &ev); X;
    out_i(DynamicAny_DynAny_get_long(c, &ev));
  } END;
  CASE("struct.equal") {
    D d = mk(s_tc), e;
    CORBA_boolean b;
    X;
    e = DynamicAny_DynAny_copy(d, &ev); X;
    b = DynamicAny_DynAny_equal(d, e, &ev); X;
    out_b(b);
    DynamicAny_DynAny_insert_long(e, 1, &ev); X;
    b = DynamicAny_DynAny_equal(d, e, &ev); X;
    out_b(b);
  } END;
  CASE("except.count") { D d = mk(e_tc); X; out_i((long)DynamicAny_DynAny_component_count(d, &ev)); } END;
  CASE("except.current_component") { D d = mk(e_tc); D c; X; c = DynamicAny_DynAny_current_component(d, &ev); X; out_b(!c); } END;
  CASE("except.member_name") { D d = mk(e_tc); CORBA_char* n; X; n = DynamicAny_DynStruct_current_member_name(d, &ev); X; out_s(n); } END;

  /* -- enum */
  CASE("enum.get") { D d = mk(color); CORBA_char* s; X; s = DynamicAny_DynEnum_get_as_string(d, &ev); X; out_s(s); CORBA_free(s); } END;
  CASE("enum.set_as_string_bad") { D d = mk(color); X; DynamicAny_DynEnum_set_as_string(d, "PINK", &ev); } END;
  CASE("enum.set_as_ulong_bad") { D d = mk(color); X; DynamicAny_DynEnum_set_as_ulong(d, 3, &ev); } END;
  CASE("enum.set_as_ulong") { D d = mk(color); CORBA_char* s; X; DynamicAny_DynEnum_set_as_ulong(d, 2, &ev); X; s = DynamicAny_DynEnum_get_as_string(d, &ev); X; out_s(s); CORBA_free(s); } END;
  CASE("enum.current_component") { D d = mk(color); D c; X; c = DynamicAny_DynAny_current_component(d, &ev); X; out_b(!c); } END;
  CASE("enum.count") { D d = mk(color); X; out_i((long)DynamicAny_DynAny_component_count(d, &ev)); } END;
  CASE("enum.insert_ulong") { D d = mk(color); X; DynamicAny_DynAny_insert_ulong(d, 1, &ev); } END;

  /* -- fixed */
#define FIXED_SET(name, text, show)                                                    \
  CASE(name) {                                                                          \
    D d = mk(money);                                                                    \
    CORBA_boolean b;                                                                    \
    CORBA_char* s;                                                                      \
    X;                                                                                  \
    b = DynamicAny_DynFixed_set_value(d, text, &ev); X;                                 \
    out_b(b);                                                                           \
    if (show) {                                                                         \
      s = DynamicAny_DynFixed_get_value(d, &ev); X;                                     \
      out_s(s);                                                                         \
      CORBA_free(s);                                                                    \
    }                                                                                   \
  } END
  CASE("fixed.get") { D d = mk(money); CORBA_char* s; X; s = DynamicAny_DynFixed_get_value(d, &ev); X; out_s(s); CORBA_free(s); } END;
  FIXED_SET("fixed.set", "-12.5", 1);
  FIXED_SET("fixed.set_d", "1.25d", 1);
  FIXED_SET("fixed.set_blank", " 1.25 ", 1);
  FIXED_SET("fixed.set_truncates", "1.239", 1);
  FIXED_SET("fixed.set_zero_tail", "1.230", 0);
  CASE("fixed.set_too_big") { D d = mk(money); X; DynamicAny_DynFixed_set_value(d, "1234.5", &ev); } END;
  CASE("fixed.set_bad") { D d = mk(money); X; DynamicAny_DynFixed_set_value(d, "abc", &ev); } END;
  FIXED_SET("fixed.set_int", "7", 1);

  /* -- union */
  CASE("union.initial") {
    D d = mk(u_tc), disc;
    CORBA_boolean none;
    CORBA_char* n;
    X;
    disc = DynamicAny_DynUnion_get_discriminator(d, &ev); X;
    out_i(DynamicAny_DynAny_get_long(disc, &ev)); X;
    sp();
    none = DynamicAny_DynUnion_has_no_active_member(d, &ev); X;
    out_b(none);
    sp();
    n = DynamicAny_DynUnion_member_name(d, &ev); X;
    out_s(n);
    CORBA_free(n);
  } END;
  CASE("union.count") { D d = mk(u_tc); X; out_i((long)DynamicAny_DynAny_component_count(d, &ev)); } END;
  CASE("union.position") { D d = mk(u_tc); X; out_i(position(d)); } END;
  CASE("union.default_first") {
    D d = mk(d_tc), disc;
    CORBA_char* n;
    X;
    disc = DynamicAny_DynUnion_get_discriminator(d, &ev); X;
    out_i(DynamicAny_DynAny_get_long(disc, &ev)); X;
    sp();
    n = DynamicAny_DynUnion_member_name(d, &ev); X;
    out_s(n);
    CORBA_free(n);
  } END;
  CASE("union.no_default_initial") {
    D d = mk(n_tc), disc;
    CORBA_char* n;
    X;
    disc = DynamicAny_DynUnion_get_discriminator(d, &ev); X;
    out_i(DynamicAny_DynAny_get_short(disc, &ev)); X;
    sp();
    n = DynamicAny_DynUnion_member_name(d, &ev); X;
    out_s(n);
    CORBA_free(n);
  } END;
  CASE("union.set_discriminator") {
    D d = mk(u_tc), disc;
    CORBA_char* n;
    X;
    disc = mk(tc_long); X;
    DynamicAny_DynAny_insert_long(disc, 3, &ev); X;
    DynamicAny_DynUnion_set_discriminator(d, disc, &ev); X;
    n = DynamicAny_DynUnion_member_name(d, &ev); X;
    out_s(n);
    CORBA_free(n);
    sp();
    out_i(position(d));
  } END;
  CASE("union.set_discriminator_keeps") {
    D d = mk(u_tc), disc, m;
    CORBA_char* s;
    X;
    disc = mk(tc_long); X;
    DynamicAny_DynAny_insert_long(disc, 2, &ev); X;
    DynamicAny_DynUnion_set_discriminator(d, disc, &ev); X;
    m = DynamicAny_DynUnion_member(d, &ev); X;
    DynamicAny_DynAny_insert_string(m, "kept", &ev); X;
    DynamicAny_DynAny_insert_long(disc, 3, &ev); X;
    DynamicAny_DynUnion_set_discriminator(d, disc, &ev); X;
    m = DynamicAny_DynUnion_member(d, &ev); X;
    s = DynamicAny_DynAny_get_string(m, &ev); X;
    out_s(s);
    CORBA_free(s);
  } END;
  CASE("union.set_discriminator_default") {
    D d = mk(u_tc), disc;
    CORBA_char* n;
    X;
    disc = mk(tc_long); X;
    DynamicAny_DynAny_insert_long(disc, 42, &ev); X;
    DynamicAny_DynUnion_set_discriminator(d, disc, &ev); X;
    n = DynamicAny_DynUnion_member_name(d, &ev); X;
    out_s(n);
    CORBA_free(n);
  } END;
  CASE("union.set_discriminator_wrong") { D d = mk(u_tc); D disc; X; disc = mk(TC_short); X; DynamicAny_DynUnion_set_discriminator(d, disc, &ev); } END;
  CASE("union.disc_component") {
    D d = mk(u_tc), disc;
    CORBA_char* n;
    X;
    disc = DynamicAny_DynUnion_get_discriminator(d, &ev); X;
    DynamicAny_DynAny_insert_long(disc, 2, &ev); X;
    n = DynamicAny_DynUnion_member_name(d, &ev); X;
    out_s(n);
    CORBA_free(n);
    sp();
    out_i((long)DynamicAny_DynAny_component_count(d, &ev));
  } END;
  CASE("union.disc_component_to_none") {
    D d = mk(n_tc), disc;
    CORBA_boolean none;
    X;
    disc = DynamicAny_DynUnion_get_discriminator(d, &ev); X;
    DynamicAny_DynAny_insert_short(disc, 9, &ev); X;
    none = DynamicAny_DynUnion_has_no_active_member(d, &ev); X;
    out_b(none);
    sp();
    out_i((long)DynamicAny_DynAny_component_count(d, &ev));
  } END;
  CASE("union.set_to_default_member") {
    D d = mk(u_tc), disc;
    CORBA_char* n;
    X;
    DynamicAny_DynUnion_set_to_default_member(d, &ev); X;
    n = DynamicAny_DynUnion_member_name(d, &ev); X;
    out_s(n);
    CORBA_free(n);
    sp();
    disc = DynamicAny_DynUnion_get_discriminator(d, &ev); X;
    out_i(DynamicAny_DynAny_get_long(disc, &ev)); X;
    sp();
    out_i(position(d));
  } END;
  CASE("union.set_to_default_member_none") { D d = mk(n_tc); X; DynamicAny_DynUnion_set_to_default_member(d, &ev); } END;
  CASE("union.set_to_no_active_member") {
    D d = mk(n_tc), disc;
    CORBA_boolean none;
    X;
    DynamicAny_DynUnion_set_to_no_active_member(d, &ev); X;
    none = DynamicAny_DynUnion_has_no_active_member(d, &ev); X;
    out_b(none);
    sp();
    disc = DynamicAny_DynUnion_get_discriminator(d, &ev); X;
    out_i(DynamicAny_DynAny_get_short(disc, &ev)); X;
    sp();
    out_i((long)DynamicAny_DynAny_component_count(d, &ev)); X;
    sp();
    out_i(position(d));
  } END;
  CASE("union.set_to_no_active_member_default") { D d = mk(u_tc); X; DynamicAny_DynUnion_set_to_no_active_member(d, &ev); } END;
  CASE("union.set_to_no_active_member_full") { D d = mk(b_tc); X; DynamicAny_DynUnion_set_to_no_active_member(d, &ev); } END;
  CASE("union.member_none") { D d = mk(n_tc); X; DynamicAny_DynUnion_set_to_no_active_member(d, &ev); X; DynamicAny_DynUnion_member(d, &ev); } END;
  CASE("union.member_name_none") { D d = mk(n_tc); X; DynamicAny_DynUnion_set_to_no_active_member(d, &ev); X; DynamicAny_DynUnion_member_name(d, &ev); } END;
  CASE("union.discriminator_kind") { D d = mk(n_tc); CORBA_TCKind k; X; k = DynamicAny_DynUnion_discriminator_kind(d, &ev); X; out_k(k); } END;
  CASE("union.member_kind") { D d = mk(u_tc); CORBA_TCKind k; X; k = DynamicAny_DynUnion_member_kind(d, &ev); X; out_k(k); } END;
  CASE("union.insert_long") { D d = mk(u_tc); X; DynamicAny_DynAny_insert_long(d, 1, &ev); X; out_i(position(d)); } END;
  CASE("union.boolean_initial") {
    D d = mk(b_tc), disc;
    CORBA_char* n;
    X;
    disc = DynamicAny_DynUnion_get_discriminator(d, &ev); X;
    out_b(DynamicAny_DynAny_get_boolean(disc, &ev)); X;
    sp();
    n = DynamicAny_DynUnion_member_name(d, &ev); X;
    out_s(n);
    CORBA_free(n);
  } END;

  /* -- sequence and array */
  CASE("seq.count") { D d = mk(seq_tc); X; out_i((long)DynamicAny_DynAny_component_count(d, &ev)); } END;
  CASE("seq.current_component") { D d = mk(seq_tc); D c; X; c = DynamicAny_DynAny_current_component(d, &ev); X; out_b(!c); } END;
  CASE("seq.insert_empty") { D d = mk(seq_tc); X; DynamicAny_DynAny_insert_long(d, 1, &ev); } END;
  CASE("seq.grow_position") { D d = mk(seq_tc); X; DynamicAny_DynSequence_set_length(d, 3, &ev); X; out_i(position(d)); } END;
  CASE("seq.grow_keeps_position") {
    D d = mk(seq_tc), c;
    X;
    DynamicAny_DynSequence_set_length(d, 2, &ev); X;
    DynamicAny_DynAny_seek(d, 1, &ev); X;
    DynamicAny_DynAny_insert_long(d, 4, &ev); X;
    DynamicAny_DynSequence_set_length(d, 4, &ev); X;
    c = DynamicAny_DynAny_current_component(d, &ev); X;
    out_i(DynamicAny_DynAny_get_long(c, &ev));
  } END;
  CASE("seq.grow_from_minus1") {
    D d = mk(seq_tc), c, e;
    X;
    DynamicAny_DynSequence_set_length(d, 2, &ev); X;
    DynamicAny_DynAny_seek(d, -1, &ev); X;
    DynamicAny_DynSequence_set_length(d, 4, &ev); X;
    c = DynamicAny_DynAny_current_component(d, &ev); X;
    out_b(!c);
    sp();
    DynamicAny_DynAny_insert_long(d, 8, &ev); X;
    e = DynamicAny_DynAny_copy(d, &ev); X;
    DynamicAny_DynAny_seek(e, 2, &ev); X;
    out_i(DynamicAny_DynAny_get_long(e, &ev));
  } END;
  CASE("seq.shrink_position") {
    D d = mk(seq_tc), c;
    X;
    DynamicAny_DynSequence_set_length(d, 4, &ev); X;
    DynamicAny_DynAny_seek(d, 3, &ev); X;
    DynamicAny_DynSequence_set_length(d, 2, &ev); X;
    c = DynamicAny_DynAny_current_component(d, &ev); X;
    out_b(!c);
  } END;
  CASE("seq.shrink_keeps") {
    D d = mk(seq_tc), c;
    X;
    DynamicAny_DynSequence_set_length(d, 4, &ev); X;
    DynamicAny_DynAny_seek(d, 1, &ev); X;
    DynamicAny_DynAny_insert_long(d, 6, &ev); X;
    DynamicAny_DynSequence_set_length(d, 2, &ev); X;
    c = DynamicAny_DynAny_current_component(d, &ev); X;
    out_i(DynamicAny_DynAny_get_long(c, &ev));
  } END;
  CASE("seq.shrink_to_0") {
    D d = mk(seq_tc), c;
    X;
    DynamicAny_DynSequence_set_length(d, 4, &ev); X;
    DynamicAny_DynSequence_set_length(d, 0, &ev); X;
    c = DynamicAny_DynAny_current_component(d, &ev); X;
    out_b(!c);
  } END;
  CASE("seq.bound") { D d = mk(bseq_tc); X; DynamicAny_DynSequence_set_length(d, 3, &ev); } END;
  {
    CORBA_any e[3];
    DynamicAny_AnySeq elems = {3, 3, NULL, CORBA_FALSE};
    elems._buffer = e;
    e[0] = any_of(tc_long, &longs[0]);
    e[1] = any_of(tc_long, &longs[1]);
    e[2] = any_of(tc_long, &longs[2]);
    CASE("seq.set_elements_bound") { D d = mk(bseq_tc); X; DynamicAny_DynSequence_set_elements(d, &elems, &ev); } END;
    CASE("seq.set_elements_type") {
      D d = mk(seq_tc);
      DynamicAny_AnySeq one = {1, 1, NULL, CORBA_FALSE};
      CORBA_any s = any_of(TC_short, &short_one);
      one._buffer = &s;
      X;
      DynamicAny_DynSequence_set_elements(d, &one, &ev);
    } END;
    CASE("seq.set_elements_position") {
      D d = mk(seq_tc);
      DynamicAny_AnySeq two = {2, 2, NULL, CORBA_FALSE};
      two._buffer = e + 1;
      X;
      DynamicAny_DynSequence_set_elements(d, &two, &ev); X;
      out_i(position(d));
    } END;
    CASE("seq.insert_long_seq") {
      D d = mk(seq_tc);
      CORBA_LongSeq v = {2, 2, NULL, CORBA_FALSE};
      v._buffer = longs + 1;
      X;
      DynamicAny_DynAny_insert_long_seq(d, &v, &ev); X;
      out_i((long)DynamicAny_DynAny_component_count(d, &ev));
      sp();
      out_i(position(d));
    } END;
    CASE("seq.get_long_seq") {
      D d = mk(seq_tc);
      CORBA_LongSeq* v;
      X;
      DynamicAny_DynSequence_set_length(d, 2, &ev); X;
      v = DynamicAny_DynAny_get_long_seq(d, &ev); X;
      out_i((long)v->_length);
      CORBA_free(v);
    } END;
    CASE("seq.insert_short_seq") {
      D d = mk(seq_tc);
      CORBA_ShortSeq v = {1, 1, NULL, CORBA_FALSE};
      v._buffer = &short_one;
      X;
      DynamicAny_DynAny_insert_short_seq(d, &v, &ev);
    } END;
    CASE("seq.insert_long_seq_bound") {
      D d = mk(bseq_tc);
      CORBA_LongSeq v = {3, 3, NULL, CORBA_FALSE};
      v._buffer = longs;
      X;
      DynamicAny_DynAny_insert_long_seq(d, &v, &ev);
    } END;
    /* A struct whose current component is a long: not a sequence. */
    CASE("struct.insert_long_seq") {
      D d = mk(s_tc);
      CORBA_LongSeq v = {1, 1, NULL, CORBA_FALSE};
      v._buffer = longs;
      X;
      DynamicAny_DynAny_insert_long_seq(d, &v, &ev);
    } END;
    CASE("arr.count") { D d = mk(arr_tc); X; out_i((long)DynamicAny_DynAny_component_count(d, &ev)); sp(); out_i(position(d)); } END;
    CASE("arr.set_elements_count") {
      D d = mk(arr_tc);
      DynamicAny_AnySeq two = {2, 2, NULL, CORBA_FALSE};
      two._buffer = e + 1;
      X;
      DynamicAny_DynArray_set_elements(d, &two, &ev);
    } END;
    CASE("arr.insert_long_seq_count") {
      D d = mk(arr_tc);
      CORBA_LongSeq v = {2, 2, NULL, CORBA_FALSE};
      v._buffer = longs;
      X;
      DynamicAny_DynAny_insert_long_seq(d, &v, &ev);
    } END;
  }

  /* -- valuetypes */
  CASE("value.null") { D d = mk(v_tc); CORBA_boolean b; X; b = DynamicAny_DynValue_is_null(d, &ev); X; out_b(b); sp(); out_i((long)DynamicAny_DynAny_component_count(d, &ev)); } END;
  CASE("value.set_to_value") {
    D d = mk(v_tc);
    CORBA_boolean b;
    X;
    DynamicAny_DynValue_set_to_value(d, &ev); X;
    b = DynamicAny_DynValue_is_null(d, &ev); X;
    out_b(b);
    sp();
    out_i((long)DynamicAny_DynAny_component_count(d, &ev)); X;
    sp();
    out_i(position(d));
  } END;
  CASE("value.get_members_null") { D d = mk(v_tc); DynamicAny_NameValuePairSeq* m; X; m = DynamicAny_DynValue_get_members(d, &ev); X; out_i((long)m->_length); CORBA_free(m); } END;
  CASE("value.member_name_null") { D d = mk(v_tc); CORBA_char* n; X; n = DynamicAny_DynValue_current_member_name(d, &ev); X; out_s(n); } END;
  CASE("value.current_component_null") { D d = mk(v_tc); D c; X; c = DynamicAny_DynAny_current_component(d, &ev); X; out_b(!c); } END;
  CASE("value.set_members") {
    D d = mk(v_tc), c;
    DynamicAny_NameValuePair p;
    DynamicAny_NameValuePairSeq m = {1, 1, NULL, CORBA_FALSE};
    CORBA_boolean b;
    p.id = "n";
    p.value = any_of(tc_long, &longs[3]);
    m._buffer = &p;
    X;
    DynamicAny_DynValue_set_members(d, &m, &ev); X;
    b = DynamicAny_DynValue_is_null(d, &ev); X;
    out_b(b);
    sp();
    c = DynamicAny_DynAny_current_component(d, &ev); X;
    out_i(DynamicAny_DynAny_get_long(c, &ev));
  } END;
  CASE("box.null") { D d = mk(box_tc); CORBA_boolean b; X; b = DynamicAny_DynValueBox_is_null(d, &ev); X; out_b(b); sp(); out_i((long)DynamicAny_DynAny_component_count(d, &ev)); } END;
  CASE("box.get_null") { D d = mk(box_tc); X; DynamicAny_DynValueBox_get_boxed_value(d, &ev); } END;
  CASE("box.set") {
    D d = mk(box_tc);
    CORBA_any a = any_of(tc_long, &longs[4]);
    CORBA_boolean b;
    X;
    DynamicAny_DynValueBox_set_boxed_value(d, &a, &ev); X;
    b = DynamicAny_DynValueBox_is_null(d, &ev); X;
    out_b(b);
    sp();
    out_i((long)DynamicAny_DynAny_component_count(d, &ev)); X;
    sp();
    out_i(position(d));
  } END;
  CASE("box.set_wrong") {
    D d = mk(box_tc);
    CORBA_short four = 4;
    CORBA_any a = any_of(TC_short, &four);
    X;
    DynamicAny_DynValueBox_set_boxed_value(d, &a, &ev);
  } END;
  CASE("box.set_to_value") {
    D d = mk(box_tc), c;
    X;
    DynamicAny_DynValueBox_set_to_value(d, &ev); X;
    c = DynamicAny_DynAny_current_component(d, &ev); X;
    out_i(DynamicAny_DynAny_get_long(c, &ev));
  } END;

  fflush(stdout);
  CORBA_ORB_destroy(orb, &ev);
  return 0;
}
