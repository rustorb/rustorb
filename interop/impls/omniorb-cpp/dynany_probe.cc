// Records how omniORB's DynAny behaves where the specification leaves
// room or where rustorb had to choose (docs/dynany.md): the initial values
// of DynAnys made from TypeCodes, current positions, the union and
// sequence operations, DynFixed strings and the exceptions raised.
//
// Each case prints its name and what the call returned or raised. The
// rustorb tests (crates/rustorb-core/src/dynany/tests.rs) check the same
// cases against expected/dynany-probe-omniorb-cpp.txt.
#include <omniORB4/CORBA.h>
#include <omniORB4/omniORB.h>

#include <iostream>
#include <string>

using namespace DynamicAny;

static CORBA::ORB_var orb;
static DynAnyFactory_var factory;

static const char* kind_name(CORBA::TCKind k) {
  static const char* names[] = {
      "tk_null", "tk_void", "tk_short", "tk_long", "tk_ushort", "tk_ulong",
      "tk_float", "tk_double", "tk_boolean", "tk_char", "tk_octet", "tk_any",
      "tk_TypeCode", "tk_Principal", "tk_objref", "tk_struct", "tk_union",
      "tk_enum", "tk_string", "tk_sequence", "tk_array", "tk_alias",
      "tk_except", "tk_longlong", "tk_ulonglong", "tk_longdouble", "tk_wchar",
      "tk_wstring", "tk_fixed", "tk_value", "tk_value_box", "tk_native",
      "tk_abstract_interface", "tk_local_interface", "tk_component",
      "tk_home", "tk_event"};
  return (unsigned)k < sizeof(names) / sizeof(*names) ? names[k] : "?";
}

#define CASE(name, body)                                               \
  do {                                                                 \
    std::cout << name << ": ";                                         \
    try {                                                              \
      body;                                                            \
    } catch (DynAny::TypeMismatch&) {                                  \
      std::cout << "TypeMismatch";                                     \
    } catch (DynAny::InvalidValue&) {                                  \
      std::cout << "InvalidValue";                                     \
    } catch (DynAnyFactory::InconsistentTypeCode&) {                   \
      std::cout << "InconsistentTypeCode";                             \
    } catch (CORBA::SystemException& e) {                              \
      std::cout << e._name();                                          \
    }                                                                  \
    std::cout << "\n";                                                 \
  } while (0)

static void out(bool b) { std::cout << (b ? "true" : "false"); }
static void out(CORBA::Long v) { std::cout << v; }
static void out(CORBA::ULong v) { std::cout << v; }
static void out(const char* s) { std::cout << '"' << s << '"'; }
static void out(CORBA::TCKind k) { std::cout << kind_name(k); }

// The current position, found by counting the components after it (this
// moves the position to -1, so it comes last in a case).
static CORBA::Long position(DynAny_ptr d) {
  DynAny_var cur = d->current_component();
  if (CORBA::is_nil(cur)) return -1;
  CORBA::Long after = 0;
  while (d->next()) after++;
  return (CORBA::Long)d->component_count() - 1 - after;
}

static void describe(DynAny_ptr d) {
  CORBA::Any_var a = d->to_any();
  CORBA::TypeCode_var tc = a->type();
  std::cout << kind_name(tc->kind());
}

int main(int argc, char** argv) {
  orb = CORBA::ORB_init(argc, argv);
  CORBA::Object_var obj = orb->resolve_initial_references("DynAnyFactory");
  factory = DynAnyFactory::_narrow(obj);

  CORBA::TypeCode_var tc_long = CORBA::TypeCode::_duplicate(CORBA::_tc_long);
  CORBA::TypeCode_var tc_string = CORBA::TypeCode::_duplicate(CORBA::_tc_string);
  CORBA::TypeCode_var count = orb->create_alias_tc("IDL:P/Count:1.0", "Count", tc_long);

  CORBA::StructMemberSeq sm;
  sm.length(3);
  sm[0].name = CORBA::string_dup("a");
  sm[0].type = CORBA::TypeCode::_duplicate(tc_long);
  sm[1].name = CORBA::string_dup("b");
  sm[1].type = CORBA::TypeCode::_duplicate(tc_string);
  sm[2].name = CORBA::string_dup("c");
  sm[2].type = CORBA::TypeCode::_duplicate(count);
  CORBA::TypeCode_var s_tc = orb->create_struct_tc("IDL:P/S:1.0", "S", sm);

  CORBA::StructMemberSeq none;
  CORBA::TypeCode_var e_tc = orb->create_exception_tc("IDL:P/E:1.0", "E", none);

  CORBA::EnumMemberSeq em;
  em.length(3);
  em[0] = CORBA::string_dup("RED");
  em[1] = CORBA::string_dup("GREEN");
  em[2] = CORBA::string_dup("BLUE");
  CORBA::TypeCode_var color = orb->create_enum_tc("IDL:P/Color:1.0", "Color", em);

  // union U switch (long) { case 1: long x; case 2: case 3: string y;
  //                         default: boolean z; };
  CORBA::UnionMemberSeq um;
  um.length(4);
  um[0].name = CORBA::string_dup("x");
  um[0].label <<= (CORBA::Long)1;
  um[0].type = CORBA::TypeCode::_duplicate(tc_long);
  um[1].name = CORBA::string_dup("y");
  um[1].label <<= (CORBA::Long)2;
  um[1].type = CORBA::TypeCode::_duplicate(tc_string);
  um[2].name = CORBA::string_dup("y");
  um[2].label <<= (CORBA::Long)3;
  um[2].type = CORBA::TypeCode::_duplicate(tc_string);
  um[3].name = CORBA::string_dup("z");
  um[3].label <<= CORBA::Any::from_octet(0);
  um[3].type = CORBA::TypeCode::_duplicate(CORBA::_tc_boolean);
  CORBA::TypeCode_var u_tc = orb->create_union_tc("IDL:P/U:1.0", "U", tc_long, um);

  // union D switch (long) { default: boolean z; case 4: long x; };
  CORBA::UnionMemberSeq dm;
  dm.length(2);
  dm[0].name = CORBA::string_dup("z");
  dm[0].label <<= CORBA::Any::from_octet(0);
  dm[0].type = CORBA::TypeCode::_duplicate(CORBA::_tc_boolean);
  dm[1].name = CORBA::string_dup("x");
  dm[1].label <<= (CORBA::Long)0;
  dm[1].type = CORBA::TypeCode::_duplicate(tc_long);
  CORBA::TypeCode_var d_tc = orb->create_union_tc("IDL:P/D:1.0", "D", tc_long, dm);

  // union N switch (short) { case 5: long x; case 2: string y; };
  CORBA::UnionMemberSeq nm;
  nm.length(2);
  nm[0].name = CORBA::string_dup("x");
  nm[0].label <<= (CORBA::Short)5;
  nm[0].type = CORBA::TypeCode::_duplicate(tc_long);
  nm[1].name = CORBA::string_dup("y");
  nm[1].label <<= (CORBA::Short)2;
  nm[1].type = CORBA::TypeCode::_duplicate(tc_string);
  CORBA::TypeCode_var n_tc = orb->create_union_tc("IDL:P/N:1.0", "N", CORBA::_tc_short, nm);

  // union B switch (boolean) { case TRUE: long t; case FALSE: string f; };
  CORBA::UnionMemberSeq bm;
  bm.length(2);
  bm[0].name = CORBA::string_dup("t");
  bm[0].label <<= CORBA::Any::from_boolean(1);
  bm[0].type = CORBA::TypeCode::_duplicate(tc_long);
  bm[1].name = CORBA::string_dup("f");
  bm[1].label <<= CORBA::Any::from_boolean(0);
  bm[1].type = CORBA::TypeCode::_duplicate(tc_string);
  CORBA::TypeCode_var b_tc = orb->create_union_tc("IDL:P/B:1.0", "B", CORBA::_tc_boolean, bm);

  CORBA::TypeCode_var seq_tc = orb->create_sequence_tc(0, tc_long);
  CORBA::TypeCode_var bseq_tc = orb->create_sequence_tc(2, tc_long);
  CORBA::TypeCode_var arr_tc = orb->create_array_tc(3, tc_long);
  CORBA::TypeCode_var money = orb->create_fixed_tc(5, 2);
  CORBA::TypeCode_var bstr = orb->create_string_tc(3);

  CORBA::ValueMemberSeq vm;
  vm.length(1);
  vm[0].name = CORBA::string_dup("n");
  vm[0].type = CORBA::TypeCode::_duplicate(tc_long);
  vm[0].access = CORBA::PUBLIC_MEMBER;
  CORBA::TypeCode_var v_tc =
      orb->create_value_tc("IDL:P/V:1.0", "V", CORBA::VM_NONE, CORBA::_tc_null, vm);
  CORBA::TypeCode_var box_tc = orb->create_value_box_tc("IDL:P/LBox:1.0", "LBox", tc_long);

  // -- creation from TypeCodes and positions
  CASE("long.count", { DynAny_var d = factory->create_dyn_any_from_type_code(tc_long); out(d->component_count()); });
  CASE("long.get", { DynAny_var d = factory->create_dyn_any_from_type_code(tc_long); out(d->get_long()); });
  CASE("long.seek0", { DynAny_var d = factory->create_dyn_any_from_type_code(tc_long); out(d->seek(0)); });
  CASE("long.next", { DynAny_var d = factory->create_dyn_any_from_type_code(tc_long); out(d->next()); });
  CASE("long.current_component", { DynAny_var d = factory->create_dyn_any_from_type_code(tc_long); DynAny_var c = d->current_component(); out(CORBA::is_nil(c)); });
  CASE("long.get_short", { DynAny_var d = factory->create_dyn_any_from_type_code(tc_long); d->get_short(); });
  CASE("long.insert_short", { DynAny_var d = factory->create_dyn_any_from_type_code(tc_long); d->insert_short(1); });
  CASE("alias.insert_long", { DynAny_var d = factory->create_dyn_any_from_type_code(count); d->insert_long(7); out(d->get_long()); });
  CASE("alias.type", { DynAny_var d = factory->create_dyn_any_from_type_code(count); CORBA::TypeCode_var t = d->type(); out(t->kind()); });
  CASE("string.get", { DynAny_var d = factory->create_dyn_any_from_type_code(tc_string); CORBA::String_var s = d->get_string(); out(s.in()); });
  CASE("bstring.insert_long", { DynAny_var d = factory->create_dyn_any_from_type_code(bstr); d->insert_string("abcd"); });
  CASE("bstring.insert_ok", { DynAny_var d = factory->create_dyn_any_from_type_code(bstr); d->insert_string("abc"); out(true); });
  CASE("typecode.get", { DynAny_var d = factory->create_dyn_any_from_type_code(CORBA::_tc_TypeCode); CORBA::TypeCode_var t = d->get_typecode(); out(t->kind()); });
  CASE("any.get", { DynAny_var d = factory->create_dyn_any_from_type_code(CORBA::_tc_any); CORBA::Any_var a = d->get_any(); CORBA::TypeCode_var t = a->type(); out(t->kind()); });
  CASE("any.count", { DynAny_var d = factory->create_dyn_any_from_type_code(CORBA::_tc_any); out(d->component_count()); });
  CASE("objref.get", { DynAny_var d = factory->create_dyn_any_from_type_code(CORBA::_tc_Object); CORBA::Object_var o = d->get_reference(); out(CORBA::is_nil(o)); });
  CASE("principal", { DynAny_var d = factory->create_dyn_any_from_type_code(CORBA::_tc_Principal); describe(d); });
  CASE("longdouble", { DynAny_var d = factory->create_dyn_any_from_type_code(CORBA::_tc_longdouble); describe(d); });

  // -- get_dyn_any: shared with the any or a copy
  CASE("any.get_dyn_any_shared", {
    DynAny_var d = factory->create_dyn_any_from_type_code(CORBA::_tc_any);
    CORBA::Any a; a <<= (CORBA::Long)5; d->insert_any(a);
    DynAny_var inner = d->get_dyn_any(); inner->insert_long(9);
    CORBA::Any_var b = d->get_any(); CORBA::Long v = 0; b.in() >>= v; out(v);
  });

  // -- struct
  CASE("struct.count", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); out(d->component_count()); });
  CASE("struct.position", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); out(position(d)); });
  CASE("struct.insert_long", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); d->insert_long(3); DynStruct_var s = DynStruct::_narrow(d); CORBA::String_var n = s->current_member_name(); out(n.in()); });
  CASE("struct.insert_long_moves", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); d->insert_long(3); out(d->seek(0)); DynAny_var c = d->current_component(); out(c->get_long()); });
  CASE("struct.insert_string_at_0", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); d->insert_string("x"); });
  CASE("struct.member_kind_alias", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); d->seek(2); DynStruct_var s = DynStruct::_narrow(d); out(s->current_member_kind()); });
  CASE("struct.member_name_end", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); d->seek(5); DynStruct_var s = DynStruct::_narrow(d); CORBA::String_var n = s->current_member_name(); out(n.in()); });
  CASE("struct.insert_at_end", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); d->seek(5); d->insert_long(1); });
  CASE("struct.next_past_end", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); d->seek(2); out(d->next()); DynAny_var c = d->current_component(); out(CORBA::is_nil(c)); });
  CASE("struct.next_from_minus1", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); d->seek(-1); out(d->next()); out(position(d)); });
  CASE("struct.seek_minus1", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); out(d->seek(-1)); });
  CASE("struct.component_shared", {
    DynAny_var d = factory->create_dyn_any_from_type_code(s_tc);
    DynAny_var c = d->current_component(); c->insert_long(11);
    DynStruct_var s = DynStruct::_narrow(d); NameValuePairSeq_var m = s->get_members();
    CORBA::Long v = 0; m[0].value >>= v; out(v);
  });
  CASE("struct.members_as_dyn_any_shared", {
    DynAny_var d = factory->create_dyn_any_from_type_code(s_tc);
    DynStruct_var s = DynStruct::_narrow(d); NameDynAnyPairSeq_var m = s->get_members_as_dyn_any();
    m[0].value->insert_long(12);
    DynAny_var c = d->current_component(); out(c->get_long());
  });
  CASE("struct.set_members_wrong_name", {
    DynAny_var d = factory->create_dyn_any_from_type_code(s_tc);
    DynStruct_var s = DynStruct::_narrow(d); NameValuePairSeq_var m = s->get_members();
    m[1].id = CORBA::string_dup("zz"); s->set_members(m);
  });
  CASE("struct.set_members_empty_names", {
    DynAny_var d = factory->create_dyn_any_from_type_code(s_tc);
    DynStruct_var s = DynStruct::_narrow(d); NameValuePairSeq_var m = s->get_members();
    for (CORBA::ULong i = 0; i < m->length(); i++) m[i].id = CORBA::string_dup("");
    s->set_members(m); out(true);
  });
  CASE("struct.set_members_short", {
    DynAny_var d = factory->create_dyn_any_from_type_code(s_tc);
    DynStruct_var s = DynStruct::_narrow(d); NameValuePairSeq_var m = s->get_members();
    m->length(2); s->set_members(m);
  });
  CASE("struct.set_members_wrong_type", {
    DynAny_var d = factory->create_dyn_any_from_type_code(s_tc);
    DynStruct_var s = DynStruct::_narrow(d); NameValuePairSeq_var m = s->get_members();
    m[0].value <<= (CORBA::Short)1; s->set_members(m);
  });
  CASE("struct.set_members_unaliased", {
    DynAny_var d = factory->create_dyn_any_from_type_code(s_tc);
    DynStruct_var s = DynStruct::_narrow(d); NameValuePairSeq_var m = s->get_members();
    m[2].value <<= (CORBA::Long)4; s->set_members(m); out(true);
  });
  CASE("struct.set_members_position", {
    DynAny_var d = factory->create_dyn_any_from_type_code(s_tc);
    d->seek(2);
    DynStruct_var s = DynStruct::_narrow(d); NameValuePairSeq_var m = s->get_members();
    s->set_members(m); out(position(d));
  });
  CASE("struct.from_any_wrong", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); CORBA::Any a; a <<= (CORBA::Long)1; d->from_any(a); });
  CASE("struct.from_any_position", {
    DynAny_var d = factory->create_dyn_any_from_type_code(s_tc);
    CORBA::Any_var a = d->to_any(); d->seek(2); d->from_any(a.in()); out(position(d));
  });
  CASE("struct.assign_wrong", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); DynAny_var e = factory->create_dyn_any_from_type_code(tc_long); d->assign(e); });
  CASE("struct.destroy_component", {
    DynAny_var d = factory->create_dyn_any_from_type_code(s_tc);
    DynAny_var c = d->current_component(); c->destroy(); c->insert_long(2); out(c->get_long());
  });
  CASE("struct.destroyed", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); DynAny_var c = d->current_component(); d->destroy(); c->get_long(); });
  CASE("struct.destroyed_top", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); d->destroy(); d->component_count(); });
  CASE("struct.copy_independent", {
    DynAny_var d = factory->create_dyn_any_from_type_code(s_tc);
    DynAny_var e = d->copy(); e->insert_long(5); DynAny_var c = d->current_component(); out(c->get_long());
  });
  CASE("struct.equal", { DynAny_var d = factory->create_dyn_any_from_type_code(s_tc); DynAny_var e = d->copy(); out(d->equal(e)); e->insert_long(1); out(d->equal(e)); });
  CASE("except.count", { DynAny_var d = factory->create_dyn_any_from_type_code(e_tc); out(d->component_count()); });
  CASE("except.current_component", { DynAny_var d = factory->create_dyn_any_from_type_code(e_tc); DynAny_var c = d->current_component(); out(CORBA::is_nil(c)); });
  CASE("except.member_name", { DynAny_var d = factory->create_dyn_any_from_type_code(e_tc); DynStruct_var s = DynStruct::_narrow(d); CORBA::String_var n = s->current_member_name(); out(n.in()); });

  // -- enum
  CASE("enum.get", { DynAny_var d = factory->create_dyn_any_from_type_code(color); DynEnum_var e = DynEnum::_narrow(d); CORBA::String_var s = e->get_as_string(); out(s.in()); });
  CASE("enum.set_as_string_bad", { DynAny_var d = factory->create_dyn_any_from_type_code(color); DynEnum_var e = DynEnum::_narrow(d); e->set_as_string("PINK"); });
  CASE("enum.set_as_ulong_bad", { DynAny_var d = factory->create_dyn_any_from_type_code(color); DynEnum_var e = DynEnum::_narrow(d); e->set_as_ulong(3); });
  CASE("enum.set_as_ulong", { DynAny_var d = factory->create_dyn_any_from_type_code(color); DynEnum_var e = DynEnum::_narrow(d); e->set_as_ulong(2); CORBA::String_var s = e->get_as_string(); out(s.in()); });
  CASE("enum.current_component", { DynAny_var d = factory->create_dyn_any_from_type_code(color); DynAny_var c = d->current_component(); out(CORBA::is_nil(c)); });
  CASE("enum.count", { DynAny_var d = factory->create_dyn_any_from_type_code(color); out(d->component_count()); });
  CASE("enum.insert_ulong", { DynAny_var d = factory->create_dyn_any_from_type_code(color); d->insert_ulong(1); });

  // -- fixed
  CASE("fixed.get", { DynAny_var d = factory->create_dyn_any_from_type_code(money); DynFixed_var f = DynFixed::_narrow(d); CORBA::String_var s = f->get_value(); out(s.in()); });
  CASE("fixed.set", { DynAny_var d = factory->create_dyn_any_from_type_code(money); DynFixed_var f = DynFixed::_narrow(d); out(f->set_value("-12.5")); CORBA::String_var s = f->get_value(); out(s.in()); });
  CASE("fixed.set_d", { DynAny_var d = factory->create_dyn_any_from_type_code(money); DynFixed_var f = DynFixed::_narrow(d); out(f->set_value("1.25d")); CORBA::String_var s = f->get_value(); out(s.in()); });
  CASE("fixed.set_blank", { DynAny_var d = factory->create_dyn_any_from_type_code(money); DynFixed_var f = DynFixed::_narrow(d); out(f->set_value(" 1.25 ")); CORBA::String_var s = f->get_value(); out(s.in()); });
  CASE("fixed.set_truncates", { DynAny_var d = factory->create_dyn_any_from_type_code(money); DynFixed_var f = DynFixed::_narrow(d); out(f->set_value("1.239")); CORBA::String_var s = f->get_value(); out(s.in()); });
  CASE("fixed.set_zero_tail", { DynAny_var d = factory->create_dyn_any_from_type_code(money); DynFixed_var f = DynFixed::_narrow(d); out(f->set_value("1.230")); });
  CASE("fixed.set_too_big", { DynAny_var d = factory->create_dyn_any_from_type_code(money); DynFixed_var f = DynFixed::_narrow(d); f->set_value("1234.5"); });
  CASE("fixed.set_bad", { DynAny_var d = factory->create_dyn_any_from_type_code(money); DynFixed_var f = DynFixed::_narrow(d); f->set_value("abc"); });
  CASE("fixed.set_int", { DynAny_var d = factory->create_dyn_any_from_type_code(money); DynFixed_var f = DynFixed::_narrow(d); out(f->set_value("7")); CORBA::String_var s = f->get_value(); out(s.in()); });

  // -- union
  CASE("union.initial", {
    DynAny_var d = factory->create_dyn_any_from_type_code(u_tc); DynUnion_var u = DynUnion::_narrow(d);
    DynAny_var disc = u->get_discriminator(); out(disc->get_long()); std::cout << " ";
    out(u->has_no_active_member()); std::cout << " ";
    CORBA::String_var n = u->member_name(); out(n.in());
  });
  CASE("union.count", { DynAny_var d = factory->create_dyn_any_from_type_code(u_tc); out(d->component_count()); });
  CASE("union.position", { DynAny_var d = factory->create_dyn_any_from_type_code(u_tc); out(position(d)); });
  CASE("union.default_first", {
    DynAny_var d = factory->create_dyn_any_from_type_code(d_tc); DynUnion_var u = DynUnion::_narrow(d);
    DynAny_var disc = u->get_discriminator(); out(disc->get_long()); std::cout << " ";
    CORBA::String_var n = u->member_name(); out(n.in());
  });
  CASE("union.no_default_initial", {
    DynAny_var d = factory->create_dyn_any_from_type_code(n_tc); DynUnion_var u = DynUnion::_narrow(d);
    DynAny_var disc = u->get_discriminator(); std::cout << disc->get_short() << " ";
    CORBA::String_var n = u->member_name(); out(n.in());
  });
  CASE("union.set_discriminator", {
    DynAny_var d = factory->create_dyn_any_from_type_code(u_tc); DynUnion_var u = DynUnion::_narrow(d);
    DynAny_var disc = factory->create_dyn_any_from_type_code(tc_long); disc->insert_long(3);
    u->set_discriminator(disc); CORBA::String_var n = u->member_name(); out(n.in()); std::cout << " "; out(position(d));
  });
  CASE("union.set_discriminator_keeps", {
    DynAny_var d = factory->create_dyn_any_from_type_code(u_tc); DynUnion_var u = DynUnion::_narrow(d);
    DynAny_var disc = factory->create_dyn_any_from_type_code(tc_long); disc->insert_long(2);
    u->set_discriminator(disc); DynAny_var m = u->member(); m->insert_string("kept");
    disc->insert_long(3); u->set_discriminator(disc); m = u->member(); CORBA::String_var s = m->get_string(); out(s.in());
  });
  CASE("union.set_discriminator_default", {
    DynAny_var d = factory->create_dyn_any_from_type_code(u_tc); DynUnion_var u = DynUnion::_narrow(d);
    DynAny_var disc = factory->create_dyn_any_from_type_code(tc_long); disc->insert_long(42);
    u->set_discriminator(disc); CORBA::String_var n = u->member_name(); out(n.in());
  });
  CASE("union.set_discriminator_wrong", {
    DynAny_var d = factory->create_dyn_any_from_type_code(u_tc); DynUnion_var u = DynUnion::_narrow(d);
    DynAny_var disc = factory->create_dyn_any_from_type_code(CORBA::_tc_short); u->set_discriminator(disc);
  });
  CASE("union.disc_component", {
    DynAny_var d = factory->create_dyn_any_from_type_code(u_tc); DynUnion_var u = DynUnion::_narrow(d);
    DynAny_var disc = u->get_discriminator(); disc->insert_long(2);
    CORBA::String_var n = u->member_name(); out(n.in()); std::cout << " "; out(d->component_count());
  });
  CASE("union.disc_component_to_none", {
    DynAny_var d = factory->create_dyn_any_from_type_code(n_tc); DynUnion_var u = DynUnion::_narrow(d);
    DynAny_var disc = u->get_discriminator(); disc->insert_short(9);
    out(u->has_no_active_member()); std::cout << " "; out(d->component_count());
  });
  CASE("union.set_to_default_member", {
    DynAny_var d = factory->create_dyn_any_from_type_code(u_tc); DynUnion_var u = DynUnion::_narrow(d);
    u->set_to_default_member(); CORBA::String_var n = u->member_name(); out(n.in()); std::cout << " ";
    DynAny_var disc = u->get_discriminator(); out(disc->get_long()); std::cout << " "; out(position(d));
  });
  CASE("union.set_to_default_member_none", { DynAny_var d = factory->create_dyn_any_from_type_code(n_tc); DynUnion_var u = DynUnion::_narrow(d); u->set_to_default_member(); });
  CASE("union.set_to_no_active_member", {
    DynAny_var d = factory->create_dyn_any_from_type_code(n_tc); DynUnion_var u = DynUnion::_narrow(d);
    u->set_to_no_active_member(); out(u->has_no_active_member()); std::cout << " ";
    DynAny_var disc = u->get_discriminator(); std::cout << disc->get_short() << " "; out(d->component_count()); std::cout << " "; out(position(d));
  });
  CASE("union.set_to_no_active_member_default", { DynAny_var d = factory->create_dyn_any_from_type_code(u_tc); DynUnion_var u = DynUnion::_narrow(d); u->set_to_no_active_member(); });
  CASE("union.set_to_no_active_member_full", { DynAny_var d = factory->create_dyn_any_from_type_code(b_tc); DynUnion_var u = DynUnion::_narrow(d); u->set_to_no_active_member(); });
  CASE("union.member_none", { DynAny_var d = factory->create_dyn_any_from_type_code(n_tc); DynUnion_var u = DynUnion::_narrow(d); u->set_to_no_active_member(); u->member(); });
  CASE("union.member_name_none", { DynAny_var d = factory->create_dyn_any_from_type_code(n_tc); DynUnion_var u = DynUnion::_narrow(d); u->set_to_no_active_member(); u->member_name(); });
  CASE("union.discriminator_kind", { DynAny_var d = factory->create_dyn_any_from_type_code(n_tc); DynUnion_var u = DynUnion::_narrow(d); out(u->discriminator_kind()); });
  CASE("union.member_kind", { DynAny_var d = factory->create_dyn_any_from_type_code(u_tc); DynUnion_var u = DynUnion::_narrow(d); out(u->member_kind()); });
  CASE("union.insert_long", { DynAny_var d = factory->create_dyn_any_from_type_code(u_tc); d->insert_long(1); out(position(d)); });
  CASE("union.boolean_initial", {
    DynAny_var d = factory->create_dyn_any_from_type_code(b_tc); DynUnion_var u = DynUnion::_narrow(d);
    DynAny_var disc = u->get_discriminator(); out(disc->get_boolean()); std::cout << " "; CORBA::String_var n = u->member_name(); out(n.in());
  });

  // -- sequence and array
  CASE("seq.count", { DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc); out(d->component_count()); });
  CASE("seq.current_component", { DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc); DynAny_var c = d->current_component(); out(CORBA::is_nil(c)); });
  CASE("seq.insert_empty", { DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc); d->insert_long(1); });
  CASE("seq.grow_position", { DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc); DynSequence_var s = DynSequence::_narrow(d); s->set_length(3); out(position(d)); });
  CASE("seq.grow_keeps_position", {
    DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc); DynSequence_var s = DynSequence::_narrow(d);
    s->set_length(2); d->seek(1); d->insert_long(4); s->set_length(4);
    DynAny_var c = d->current_component(); out(c->get_long());
  });
  CASE("seq.grow_from_minus1", {
    DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc); DynSequence_var s = DynSequence::_narrow(d);
    s->set_length(2); d->seek(-1); s->set_length(4); DynAny_var c = d->current_component();
    out(CORBA::is_nil(c)); std::cout << " "; d->insert_long(8); DynAny_var e = d->copy(); e->seek(2); out(e->get_long());
  });
  CASE("seq.shrink_position", {
    DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc); DynSequence_var s = DynSequence::_narrow(d);
    s->set_length(4); d->seek(3); s->set_length(2); DynAny_var c = d->current_component(); out(CORBA::is_nil(c));
  });
  CASE("seq.shrink_keeps", {
    DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc); DynSequence_var s = DynSequence::_narrow(d);
    s->set_length(4); d->seek(1); d->insert_long(6); s->set_length(2); DynAny_var c = d->current_component(); out(c->get_long());
  });
  CASE("seq.shrink_to_0", {
    DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc); DynSequence_var s = DynSequence::_narrow(d);
    s->set_length(4); s->set_length(0); DynAny_var c = d->current_component(); out(CORBA::is_nil(c));
  });
  CASE("seq.bound", { DynAny_var d = factory->create_dyn_any_from_type_code(bseq_tc); DynSequence_var s = DynSequence::_narrow(d); s->set_length(3); });
  CASE("seq.set_elements_bound", {
    DynAny_var d = factory->create_dyn_any_from_type_code(bseq_tc); DynSequence_var s = DynSequence::_narrow(d);
    AnySeq e; e.length(3); for (int i = 0; i < 3; i++) e[i] <<= (CORBA::Long)i; s->set_elements(e);
  });
  CASE("seq.set_elements_type", {
    DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc); DynSequence_var s = DynSequence::_narrow(d);
    AnySeq e; e.length(1); e[0] <<= (CORBA::Short)1; s->set_elements(e);
  });
  CASE("seq.set_elements_position", {
    DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc); DynSequence_var s = DynSequence::_narrow(d);
    AnySeq e; e.length(2); e[0] <<= (CORBA::Long)1; e[1] <<= (CORBA::Long)2; s->set_elements(e); out(position(d));
  });
  CASE("seq.insert_long_seq", {
    DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc);
    CORBA::LongSeq v; v.length(2); v[0] = 1; v[1] = 2; d->insert_long_seq(v); out(d->component_count()); std::cout << " "; out(position(d));
  });
  CASE("seq.get_long_seq", {
    DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc); DynSequence_var s = DynSequence::_narrow(d);
    s->set_length(2); CORBA::LongSeq_var v = d->get_long_seq(); out((CORBA::ULong)v->length());
  });
  CASE("seq.insert_short_seq", {
    DynAny_var d = factory->create_dyn_any_from_type_code(seq_tc);
    CORBA::ShortSeq v; v.length(1); v[0] = 1; d->insert_short_seq(v);
  });
  CASE("seq.insert_long_seq_bound", {
    DynAny_var d = factory->create_dyn_any_from_type_code(bseq_tc);
    CORBA::LongSeq v; v.length(3); d->insert_long_seq(v);
  });
  CASE("struct.insert_long_seq", {
    // A struct whose current component is a long: not a sequence.
    DynAny_var d = factory->create_dyn_any_from_type_code(s_tc);
    CORBA::LongSeq v; v.length(1); d->insert_long_seq(v);
  });
  CASE("arr.count", { DynAny_var d = factory->create_dyn_any_from_type_code(arr_tc); out(d->component_count()); std::cout << " "; out(position(d)); });
  CASE("arr.set_elements_count", {
    DynAny_var d = factory->create_dyn_any_from_type_code(arr_tc); DynArray_var a = DynArray::_narrow(d);
    AnySeq e; e.length(2); e[0] <<= (CORBA::Long)1; e[1] <<= (CORBA::Long)1; a->set_elements(e);
  });
  CASE("arr.insert_long_seq_count", {
    DynAny_var d = factory->create_dyn_any_from_type_code(arr_tc);
    CORBA::LongSeq v; v.length(2); d->insert_long_seq(v);
  });

  // -- valuetypes
  CASE("value.null", { DynAny_var d = factory->create_dyn_any_from_type_code(v_tc); DynValue_var v = DynValue::_narrow(d); out(v->is_null()); std::cout << " "; out(d->component_count()); });
  CASE("value.set_to_value", { DynAny_var d = factory->create_dyn_any_from_type_code(v_tc); DynValue_var v = DynValue::_narrow(d); v->set_to_value(); out(v->is_null()); std::cout << " "; out(d->component_count()); std::cout << " "; out(position(d)); });
  CASE("value.get_members_null", { DynAny_var d = factory->create_dyn_any_from_type_code(v_tc); DynValue_var v = DynValue::_narrow(d); NameValuePairSeq_var m = v->get_members(); out((CORBA::ULong)m->length()); });
  CASE("value.member_name_null", { DynAny_var d = factory->create_dyn_any_from_type_code(v_tc); DynValue_var v = DynValue::_narrow(d); CORBA::String_var n = v->current_member_name(); out(n.in()); });
  CASE("value.current_component_null", { DynAny_var d = factory->create_dyn_any_from_type_code(v_tc); DynAny_var c = d->current_component(); out(CORBA::is_nil(c)); });
  CASE("value.set_members", {
    DynAny_var d = factory->create_dyn_any_from_type_code(v_tc); DynValue_var v = DynValue::_narrow(d);
    NameValuePairSeq m; m.length(1); m[0].id = CORBA::string_dup("n"); m[0].value <<= (CORBA::Long)3;
    v->set_members(m); out(v->is_null()); std::cout << " "; DynAny_var c = d->current_component(); out(c->get_long());
  });
  CASE("box.null", { DynAny_var d = factory->create_dyn_any_from_type_code(box_tc); DynValueBox_var b = DynValueBox::_narrow(d); out(b->is_null()); std::cout << " "; out(d->component_count()); });
  CASE("box.get_null", { DynAny_var d = factory->create_dyn_any_from_type_code(box_tc); DynValueBox_var b = DynValueBox::_narrow(d); CORBA::Any_var a = b->get_boxed_value(); });
  CASE("box.set", {
    DynAny_var d = factory->create_dyn_any_from_type_code(box_tc); DynValueBox_var b = DynValueBox::_narrow(d);
    CORBA::Any a; a <<= (CORBA::Long)4; b->set_boxed_value(a); out(b->is_null()); std::cout << " "; out(d->component_count()); std::cout << " "; out(position(d));
  });
  CASE("box.set_wrong", {
    DynAny_var d = factory->create_dyn_any_from_type_code(box_tc); DynValueBox_var b = DynValueBox::_narrow(d);
    CORBA::Any a; a <<= (CORBA::Short)4; b->set_boxed_value(a);
  });
  CASE("box.set_to_value", { DynAny_var d = factory->create_dyn_any_from_type_code(box_tc); DynValueBox_var b = DynValueBox::_narrow(d); b->set_to_value(); DynAny_var c = d->current_component(); out(c->get_long()); });

  orb->destroy();
  return 0;
}
