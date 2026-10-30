// The DynAny probe of interop/impls/omniorb-cpp/dynany_probe.cc on the
// rustorb C++ binding (<rustorb/dynany.h>): the same cases, whose output
// run-dynany-probe.sh compares with expected/dynany-probe-rustorb.txt.
#include <rustorb/cdr.h>

#include <iostream>
#include <string>

using namespace DynamicAny;

static IDL::traits<DynAnyFactory>::ref_type factory;

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
  auto i = static_cast<uint32_t>(k);
  return i < sizeof(names) / sizeof(*names) ? names[i] : "?";
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
    } catch (MustTruncate&) {                                          \
      std::cout << "MustTruncate";                                     \
    } catch (CORBA::SystemException& e) {                              \
      std::cout << e._name();                                          \
    }                                                                  \
    std::cout << "\n";                                                 \
  } while (0)

static void out(bool b) { std::cout << (b ? "true" : "false"); }
static void out(int32_t v) { std::cout << v; }
static void out(uint32_t v) { std::cout << v; }
static void out(const std::string& s) { std::cout << '"' << s << '"'; }
static void out(CORBA::TCKind k) { std::cout << kind_name(k); }

template <typename T> static typename IDL::traits<T>::ref_type narrow(const DynAny_ref& d) {
  return IDL::traits<T>::narrow(d);
}

template <typename T> static CORBA::Any any_of(T v) {
  CORBA::Any a;
  a <<= v;
  return a;
}

// The current position, found by counting the components after it (this
// moves the position to -1, so it comes last in a case).
static int32_t position(const DynAny_ref& d) {
  auto cur = d->current_component();
  if (!cur) return -1;
  int32_t after = 0;
  while (d->next()) after++;
  return static_cast<int32_t>(d->component_count()) - 1 - after;
}

static void describe(const DynAny_ref& d) { std::cout << kind_name(d->to_any().type()->kind()); }

int main(int argc, char** argv) {
  auto orb = CORBA::ORB_init(argc, argv);
  factory = IDL::traits<DynAnyFactory>::narrow(orb->resolve_initial_references("DynAnyFactory"));

  auto tc_long = CORBA::_tc_long;
  auto tc_string = CORBA::_tc_string;
  auto tc_principal = rustorb::detail::tc_simple(CORBA::TCKind::tk_Principal);
  auto tc_longdouble = rustorb::detail::tc_simple(CORBA::TCKind::tk_longdouble);
  auto count = orb->create_alias_tc("IDL:P/Count:1.0", "Count", tc_long);
  auto s_tc = orb->create_struct_tc("IDL:P/S:1.0", "S",
                                    {{"a", tc_long}, {"b", tc_string}, {"c", count}});
  auto e_tc = orb->create_exception_tc("IDL:P/E:1.0", "E", {});
  auto color = orb->create_enum_tc("IDL:P/Color:1.0", "Color", {"RED", "GREEN", "BLUE"});
  CORBA::Any deflt = any_of(uint8_t(0));
  // union U switch (long) { case 1: long x; case 2: case 3: string y;
  //                         default: boolean z; };
  auto u_tc = orb->create_union_tc("IDL:P/U:1.0", "U", tc_long,
                                   {{"x", any_of(int32_t(1)), tc_long},
                                    {"y", any_of(int32_t(2)), tc_string},
                                    {"y", any_of(int32_t(3)), tc_string},
                                    {"z", deflt, CORBA::_tc_boolean}});
  // union D switch (long) { default: boolean z; case 0: long x; };
  auto d_tc = orb->create_union_tc("IDL:P/D:1.0", "D", tc_long,
                                   {{"z", deflt, CORBA::_tc_boolean},
                                    {"x", any_of(int32_t(0)), tc_long}});
  // union N switch (short) { case 5: long x; case 2: string y; };
  auto n_tc = orb->create_union_tc("IDL:P/N:1.0", "N", CORBA::_tc_short,
                                   {{"x", any_of(int16_t(5)), tc_long},
                                    {"y", any_of(int16_t(2)), tc_string}});
  // union B switch (boolean) { case TRUE: long t; case FALSE: string f; };
  auto b_tc = orb->create_union_tc("IDL:P/B:1.0", "B", CORBA::_tc_boolean,
                                   {{"t", any_of(true), tc_long},
                                    {"f", any_of(false), tc_string}});
  auto seq_tc = orb->create_sequence_tc(0, tc_long);
  auto bseq_tc = orb->create_sequence_tc(2, tc_long);
  auto arr_tc = orb->create_array_tc(3, tc_long);
  auto money = orb->create_fixed_tc(5, 2);
  auto bstr = orb->create_string_tc(3);
  auto v_tc = orb->create_value_tc("IDL:P/V:1.0", "V", CORBA::VM_NONE, nullptr,
                                   {{"n", tc_long, CORBA::PUBLIC_MEMBER}});
  auto box_tc = orb->create_value_box_tc("IDL:P/LBox:1.0", "LBox", tc_long);

  // -- creation from TypeCodes and positions
  CASE("long.count", { auto d = factory->create_dyn_any_from_type_code(tc_long); out(d->component_count()); });
  CASE("long.get", { auto d = factory->create_dyn_any_from_type_code(tc_long); out(d->get_long()); });
  CASE("long.seek0", { auto d = factory->create_dyn_any_from_type_code(tc_long); out(d->seek(0)); });
  CASE("long.next", { auto d = factory->create_dyn_any_from_type_code(tc_long); out(d->next()); });
  CASE("long.current_component", { auto d = factory->create_dyn_any_from_type_code(tc_long); auto c = d->current_component(); out(!c); });
  CASE("long.get_short", { auto d = factory->create_dyn_any_from_type_code(tc_long); d->get_short(); });
  CASE("long.insert_short", { auto d = factory->create_dyn_any_from_type_code(tc_long); d->insert_short(1); });
  CASE("alias.insert_long", { auto d = factory->create_dyn_any_from_type_code(count); d->insert_long(7); out(d->get_long()); });
  CASE("alias.type", { auto d = factory->create_dyn_any_from_type_code(count); auto t = d->type(); out(t->kind()); });
  CASE("string.get", { auto d = factory->create_dyn_any_from_type_code(tc_string); std::string s = d->get_string(); out(s); });
  CASE("bstring.insert_long", { auto d = factory->create_dyn_any_from_type_code(bstr); d->insert_string("abcd"); });
  CASE("bstring.insert_ok", { auto d = factory->create_dyn_any_from_type_code(bstr); d->insert_string("abc"); out(true); });
  CASE("typecode.get", { auto d = factory->create_dyn_any_from_type_code(CORBA::_tc_TypeCode); auto t = d->get_typecode(); out(t->kind()); });
  CASE("any.get", { auto d = factory->create_dyn_any_from_type_code(CORBA::_tc_any); CORBA::Any a = d->get_any(); auto t = a.type(); out(t->kind()); });
  CASE("any.count", { auto d = factory->create_dyn_any_from_type_code(CORBA::_tc_any); out(d->component_count()); });
  CASE("objref.get", { auto d = factory->create_dyn_any_from_type_code(CORBA::_tc_Object); auto o = d->get_reference(); out(!o); });
  CASE("principal", { auto d = factory->create_dyn_any_from_type_code(tc_principal); describe(d); });
  CASE("longdouble", { auto d = factory->create_dyn_any_from_type_code(tc_longdouble); describe(d); });

  // -- get_dyn_any: shared with the any or a copy
  CASE("any.get_dyn_any_shared", {
    auto d = factory->create_dyn_any_from_type_code(CORBA::_tc_any);
    CORBA::Any a; a <<= (int32_t)5; d->insert_any(a);
    auto inner = d->get_dyn_any(); inner->insert_long(9);
    CORBA::Any b = d->get_any(); int32_t v = 0; b >>= v; out(v);
  });

  // -- struct
  CASE("struct.count", { auto d = factory->create_dyn_any_from_type_code(s_tc); out(d->component_count()); });
  CASE("struct.position", { auto d = factory->create_dyn_any_from_type_code(s_tc); out(position(d)); });
  CASE("struct.insert_long", { auto d = factory->create_dyn_any_from_type_code(s_tc); d->insert_long(3); auto s = narrow<DynStruct>(d); std::string n = s->current_member_name(); out(n); });
  CASE("struct.insert_long_moves", { auto d = factory->create_dyn_any_from_type_code(s_tc); d->insert_long(3); out(d->seek(0)); auto c = d->current_component(); out(c->get_long()); });
  CASE("struct.insert_string_at_0", { auto d = factory->create_dyn_any_from_type_code(s_tc); d->insert_string("x"); });
  CASE("struct.member_kind_alias", { auto d = factory->create_dyn_any_from_type_code(s_tc); d->seek(2); auto s = narrow<DynStruct>(d); out(s->current_member_kind()); });
  CASE("struct.member_name_end", { auto d = factory->create_dyn_any_from_type_code(s_tc); d->seek(5); auto s = narrow<DynStruct>(d); std::string n = s->current_member_name(); out(n); });
  CASE("struct.insert_at_end", { auto d = factory->create_dyn_any_from_type_code(s_tc); d->seek(5); d->insert_long(1); });
  CASE("struct.next_past_end", { auto d = factory->create_dyn_any_from_type_code(s_tc); d->seek(2); out(d->next()); auto c = d->current_component(); out(!c); });
  CASE("struct.next_from_minus1", { auto d = factory->create_dyn_any_from_type_code(s_tc); d->seek(-1); out(d->next()); out(position(d)); });
  CASE("struct.seek_minus1", { auto d = factory->create_dyn_any_from_type_code(s_tc); out(d->seek(-1)); });
  CASE("struct.component_shared", {
    auto d = factory->create_dyn_any_from_type_code(s_tc);
    auto c = d->current_component(); c->insert_long(11);
    auto s = narrow<DynStruct>(d); auto m = s->get_members();
    int32_t v = 0; m[0].value() >>= v; out(v);
  });
  CASE("struct.members_as_dyn_any_shared", {
    auto d = factory->create_dyn_any_from_type_code(s_tc);
    auto s = narrow<DynStruct>(d); auto m = s->get_members_as_dyn_any();
    m[0].value()->insert_long(12);
    auto c = d->current_component(); out(c->get_long());
  });
  CASE("struct.set_members_wrong_name", {
    auto d = factory->create_dyn_any_from_type_code(s_tc);
    auto s = narrow<DynStruct>(d); auto m = s->get_members();
    m[1].id("zz"); s->set_members(m);
  });
  CASE("struct.set_members_empty_names", {
    auto d = factory->create_dyn_any_from_type_code(s_tc);
    auto s = narrow<DynStruct>(d); auto m = s->get_members();
    for (uint32_t i = 0; i < m.size(); i++) m[i].id("");
    s->set_members(m); out(true);
  });
  CASE("struct.set_members_short", {
    auto d = factory->create_dyn_any_from_type_code(s_tc);
    auto s = narrow<DynStruct>(d); auto m = s->get_members();
    m.resize(2); s->set_members(m);
  });
  CASE("struct.set_members_wrong_type", {
    auto d = factory->create_dyn_any_from_type_code(s_tc);
    auto s = narrow<DynStruct>(d); auto m = s->get_members();
    m[0].value(any_of((int16_t)1)); s->set_members(m);
  });
  CASE("struct.set_members_unaliased", {
    auto d = factory->create_dyn_any_from_type_code(s_tc);
    auto s = narrow<DynStruct>(d); auto m = s->get_members();
    m[2].value(any_of((int32_t)4)); s->set_members(m); out(true);
  });
  CASE("struct.set_members_position", {
    auto d = factory->create_dyn_any_from_type_code(s_tc);
    d->seek(2);
    auto s = narrow<DynStruct>(d); auto m = s->get_members();
    s->set_members(m); out(position(d));
  });
  CASE("struct.from_any_wrong", { auto d = factory->create_dyn_any_from_type_code(s_tc); CORBA::Any a; a <<= (int32_t)1; d->from_any(a); });
  CASE("struct.from_any_position", {
    auto d = factory->create_dyn_any_from_type_code(s_tc);
    CORBA::Any a = d->to_any(); d->seek(2); d->from_any(a); out(position(d));
  });
  CASE("struct.assign_wrong", { auto d = factory->create_dyn_any_from_type_code(s_tc); auto e = factory->create_dyn_any_from_type_code(tc_long); d->assign(e); });
  CASE("struct.destroy_component", {
    auto d = factory->create_dyn_any_from_type_code(s_tc);
    auto c = d->current_component(); c->destroy(); c->insert_long(2); out(c->get_long());
  });
  CASE("struct.destroyed", { auto d = factory->create_dyn_any_from_type_code(s_tc); auto c = d->current_component(); d->destroy(); c->get_long(); });
  CASE("struct.destroyed_top", { auto d = factory->create_dyn_any_from_type_code(s_tc); d->destroy(); d->component_count(); });
  CASE("struct.copy_independent", {
    auto d = factory->create_dyn_any_from_type_code(s_tc);
    auto e = d->copy(); e->insert_long(5); auto c = d->current_component(); out(c->get_long());
  });
  CASE("struct.equal", { auto d = factory->create_dyn_any_from_type_code(s_tc); auto e = d->copy(); out(d->equal(e)); e->insert_long(1); out(d->equal(e)); });
  CASE("except.count", { auto d = factory->create_dyn_any_from_type_code(e_tc); out(d->component_count()); });
  CASE("except.current_component", { auto d = factory->create_dyn_any_from_type_code(e_tc); auto c = d->current_component(); out(!c); });
  CASE("except.member_name", { auto d = factory->create_dyn_any_from_type_code(e_tc); auto s = narrow<DynStruct>(d); std::string n = s->current_member_name(); out(n); });

  // -- enum
  CASE("enum.get", { auto d = factory->create_dyn_any_from_type_code(color); auto e = narrow<DynEnum>(d); std::string s = e->get_as_string(); out(s); });
  CASE("enum.set_as_string_bad", { auto d = factory->create_dyn_any_from_type_code(color); auto e = narrow<DynEnum>(d); e->set_as_string("PINK"); });
  CASE("enum.set_as_ulong_bad", { auto d = factory->create_dyn_any_from_type_code(color); auto e = narrow<DynEnum>(d); e->set_as_ulong(3); });
  CASE("enum.set_as_ulong", { auto d = factory->create_dyn_any_from_type_code(color); auto e = narrow<DynEnum>(d); e->set_as_ulong(2); std::string s = e->get_as_string(); out(s); });
  CASE("enum.current_component", { auto d = factory->create_dyn_any_from_type_code(color); auto c = d->current_component(); out(!c); });
  CASE("enum.count", { auto d = factory->create_dyn_any_from_type_code(color); out(d->component_count()); });
  CASE("enum.insert_ulong", { auto d = factory->create_dyn_any_from_type_code(color); d->insert_ulong(1); });

  // -- fixed
  CASE("fixed.get", { auto d = factory->create_dyn_any_from_type_code(money); auto f = narrow<DynFixed>(d); std::string s = f->get_value(); out(s); });
  CASE("fixed.set", { auto d = factory->create_dyn_any_from_type_code(money); auto f = narrow<DynFixed>(d); out(f->set_value("-12.5")); std::string s = f->get_value(); out(s); });
  CASE("fixed.set_d", { auto d = factory->create_dyn_any_from_type_code(money); auto f = narrow<DynFixed>(d); out(f->set_value("1.25d")); std::string s = f->get_value(); out(s); });
  CASE("fixed.set_blank", { auto d = factory->create_dyn_any_from_type_code(money); auto f = narrow<DynFixed>(d); out(f->set_value(" 1.25 ")); std::string s = f->get_value(); out(s); });
  CASE("fixed.set_truncates", { auto d = factory->create_dyn_any_from_type_code(money); auto f = narrow<DynFixed>(d); out(f->set_value("1.239")); std::string s = f->get_value(); out(s); });
  CASE("fixed.set_zero_tail", { auto d = factory->create_dyn_any_from_type_code(money); auto f = narrow<DynFixed>(d); out(f->set_value("1.230")); });
  CASE("fixed.set_too_big", { auto d = factory->create_dyn_any_from_type_code(money); auto f = narrow<DynFixed>(d); f->set_value("1234.5"); });
  CASE("fixed.set_bad", { auto d = factory->create_dyn_any_from_type_code(money); auto f = narrow<DynFixed>(d); f->set_value("abc"); });
  CASE("fixed.set_int", { auto d = factory->create_dyn_any_from_type_code(money); auto f = narrow<DynFixed>(d); out(f->set_value("7")); std::string s = f->get_value(); out(s); });

  // -- union
  CASE("union.initial", {
    auto d = factory->create_dyn_any_from_type_code(u_tc); auto u = narrow<DynUnion>(d);
    auto disc = u->get_discriminator(); out(disc->get_long()); std::cout << " ";
    out(u->has_no_active_member()); std::cout << " ";
    std::string n = u->member_name(); out(n);
  });
  CASE("union.count", { auto d = factory->create_dyn_any_from_type_code(u_tc); out(d->component_count()); });
  CASE("union.position", { auto d = factory->create_dyn_any_from_type_code(u_tc); out(position(d)); });
  CASE("union.default_first", {
    auto d = factory->create_dyn_any_from_type_code(d_tc); auto u = narrow<DynUnion>(d);
    auto disc = u->get_discriminator(); out(disc->get_long()); std::cout << " ";
    std::string n = u->member_name(); out(n);
  });
  CASE("union.no_default_initial", {
    auto d = factory->create_dyn_any_from_type_code(n_tc); auto u = narrow<DynUnion>(d);
    auto disc = u->get_discriminator(); std::cout << disc->get_short() << " ";
    std::string n = u->member_name(); out(n);
  });
  CASE("union.set_discriminator", {
    auto d = factory->create_dyn_any_from_type_code(u_tc); auto u = narrow<DynUnion>(d);
    auto disc = factory->create_dyn_any_from_type_code(tc_long); disc->insert_long(3);
    u->set_discriminator(disc); std::string n = u->member_name(); out(n); std::cout << " "; out(position(d));
  });
  CASE("union.set_discriminator_keeps", {
    auto d = factory->create_dyn_any_from_type_code(u_tc); auto u = narrow<DynUnion>(d);
    auto disc = factory->create_dyn_any_from_type_code(tc_long); disc->insert_long(2);
    u->set_discriminator(disc); auto m = u->member(); m->insert_string("kept");
    disc->insert_long(3); u->set_discriminator(disc); m = u->member(); std::string s = m->get_string(); out(s);
  });
  CASE("union.set_discriminator_default", {
    auto d = factory->create_dyn_any_from_type_code(u_tc); auto u = narrow<DynUnion>(d);
    auto disc = factory->create_dyn_any_from_type_code(tc_long); disc->insert_long(42);
    u->set_discriminator(disc); std::string n = u->member_name(); out(n);
  });
  CASE("union.set_discriminator_wrong", {
    auto d = factory->create_dyn_any_from_type_code(u_tc); auto u = narrow<DynUnion>(d);
    auto disc = factory->create_dyn_any_from_type_code(CORBA::_tc_short); u->set_discriminator(disc);
  });
  CASE("union.disc_component", {
    auto d = factory->create_dyn_any_from_type_code(u_tc); auto u = narrow<DynUnion>(d);
    auto disc = u->get_discriminator(); disc->insert_long(2);
    std::string n = u->member_name(); out(n); std::cout << " "; out(d->component_count());
  });
  CASE("union.disc_component_to_none", {
    auto d = factory->create_dyn_any_from_type_code(n_tc); auto u = narrow<DynUnion>(d);
    auto disc = u->get_discriminator(); disc->insert_short(9);
    out(u->has_no_active_member()); std::cout << " "; out(d->component_count());
  });
  CASE("union.set_to_default_member", {
    auto d = factory->create_dyn_any_from_type_code(u_tc); auto u = narrow<DynUnion>(d);
    u->set_to_default_member(); std::string n = u->member_name(); out(n); std::cout << " ";
    auto disc = u->get_discriminator(); out(disc->get_long()); std::cout << " "; out(position(d));
  });
  CASE("union.set_to_default_member_none", { auto d = factory->create_dyn_any_from_type_code(n_tc); auto u = narrow<DynUnion>(d); u->set_to_default_member(); });
  CASE("union.set_to_no_active_member", {
    auto d = factory->create_dyn_any_from_type_code(n_tc); auto u = narrow<DynUnion>(d);
    u->set_to_no_active_member(); out(u->has_no_active_member()); std::cout << " ";
    auto disc = u->get_discriminator(); std::cout << disc->get_short() << " "; out(d->component_count()); std::cout << " "; out(position(d));
  });
  CASE("union.set_to_no_active_member_default", { auto d = factory->create_dyn_any_from_type_code(u_tc); auto u = narrow<DynUnion>(d); u->set_to_no_active_member(); });
  CASE("union.set_to_no_active_member_full", { auto d = factory->create_dyn_any_from_type_code(b_tc); auto u = narrow<DynUnion>(d); u->set_to_no_active_member(); });
  CASE("union.member_none", { auto d = factory->create_dyn_any_from_type_code(n_tc); auto u = narrow<DynUnion>(d); u->set_to_no_active_member(); u->member(); });
  CASE("union.member_name_none", { auto d = factory->create_dyn_any_from_type_code(n_tc); auto u = narrow<DynUnion>(d); u->set_to_no_active_member(); u->member_name(); });
  CASE("union.discriminator_kind", { auto d = factory->create_dyn_any_from_type_code(n_tc); auto u = narrow<DynUnion>(d); out(u->discriminator_kind()); });
  CASE("union.member_kind", { auto d = factory->create_dyn_any_from_type_code(u_tc); auto u = narrow<DynUnion>(d); out(u->member_kind()); });
  CASE("union.insert_long", { auto d = factory->create_dyn_any_from_type_code(u_tc); d->insert_long(1); out(position(d)); });
  CASE("union.boolean_initial", {
    auto d = factory->create_dyn_any_from_type_code(b_tc); auto u = narrow<DynUnion>(d);
    auto disc = u->get_discriminator(); out(disc->get_boolean()); std::cout << " "; std::string n = u->member_name(); out(n);
  });

  // -- sequence and array
  CASE("seq.count", { auto d = factory->create_dyn_any_from_type_code(seq_tc); out(d->component_count()); });
  CASE("seq.current_component", { auto d = factory->create_dyn_any_from_type_code(seq_tc); auto c = d->current_component(); out(!c); });
  CASE("seq.insert_empty", { auto d = factory->create_dyn_any_from_type_code(seq_tc); d->insert_long(1); });
  CASE("seq.grow_position", { auto d = factory->create_dyn_any_from_type_code(seq_tc); auto s = narrow<DynSequence>(d); s->set_length(3); out(position(d)); });
  CASE("seq.grow_keeps_position", {
    auto d = factory->create_dyn_any_from_type_code(seq_tc); auto s = narrow<DynSequence>(d);
    s->set_length(2); d->seek(1); d->insert_long(4); s->set_length(4);
    auto c = d->current_component(); out(c->get_long());
  });
  CASE("seq.grow_from_minus1", {
    auto d = factory->create_dyn_any_from_type_code(seq_tc); auto s = narrow<DynSequence>(d);
    s->set_length(2); d->seek(-1); s->set_length(4); auto c = d->current_component();
    out(!c); std::cout << " "; d->insert_long(8); auto e = d->copy(); e->seek(2); out(e->get_long());
  });
  CASE("seq.shrink_position", {
    auto d = factory->create_dyn_any_from_type_code(seq_tc); auto s = narrow<DynSequence>(d);
    s->set_length(4); d->seek(3); s->set_length(2); auto c = d->current_component(); out(!c);
  });
  CASE("seq.shrink_keeps", {
    auto d = factory->create_dyn_any_from_type_code(seq_tc); auto s = narrow<DynSequence>(d);
    s->set_length(4); d->seek(1); d->insert_long(6); s->set_length(2); auto c = d->current_component(); out(c->get_long());
  });
  CASE("seq.shrink_to_0", {
    auto d = factory->create_dyn_any_from_type_code(seq_tc); auto s = narrow<DynSequence>(d);
    s->set_length(4); s->set_length(0); auto c = d->current_component(); out(!c);
  });
  CASE("seq.bound", { auto d = factory->create_dyn_any_from_type_code(bseq_tc); auto s = narrow<DynSequence>(d); s->set_length(3); });
  CASE("seq.set_elements_bound", {
    auto d = factory->create_dyn_any_from_type_code(bseq_tc); auto s = narrow<DynSequence>(d);
    AnySeq e(3); for (int i = 0; i < 3; i++) e[i] <<= (int32_t)i; s->set_elements(e);
  });
  CASE("seq.set_elements_type", {
    auto d = factory->create_dyn_any_from_type_code(seq_tc); auto s = narrow<DynSequence>(d);
    AnySeq e(1); e[0] <<= (int16_t)1; s->set_elements(e);
  });
  CASE("seq.set_elements_position", {
    auto d = factory->create_dyn_any_from_type_code(seq_tc); auto s = narrow<DynSequence>(d);
    AnySeq e(2); e[0] <<= (int32_t)1; e[1] <<= (int32_t)2; s->set_elements(e); out(position(d));
  });
  CASE("seq.insert_long_seq", {
    auto d = factory->create_dyn_any_from_type_code(seq_tc);
    CORBA::LongSeq v(2); v[0] = 1; v[1] = 2; d->insert_long_seq(v); out(d->component_count()); std::cout << " "; out(position(d));
  });
  CASE("seq.get_long_seq", {
    auto d = factory->create_dyn_any_from_type_code(seq_tc); auto s = narrow<DynSequence>(d);
    s->set_length(2); auto v = d->get_long_seq(); out((uint32_t)v.size());
  });
  CASE("seq.insert_short_seq", {
    auto d = factory->create_dyn_any_from_type_code(seq_tc);
    CORBA::ShortSeq v(1); v[0] = 1; d->insert_short_seq(v);
  });
  CASE("seq.insert_long_seq_bound", {
    auto d = factory->create_dyn_any_from_type_code(bseq_tc);
    CORBA::LongSeq v(3); d->insert_long_seq(v);
  });
  CASE("struct.insert_long_seq", {
    // A struct whose current component is a long: not a sequence.
    auto d = factory->create_dyn_any_from_type_code(s_tc);
    CORBA::LongSeq v(1); d->insert_long_seq(v);
  });
  CASE("arr.count", { auto d = factory->create_dyn_any_from_type_code(arr_tc); out(d->component_count()); std::cout << " "; out(position(d)); });
  CASE("arr.set_elements_count", {
    auto d = factory->create_dyn_any_from_type_code(arr_tc); auto a = narrow<DynArray>(d);
    AnySeq e(2); e[0] <<= (int32_t)1; e[1] <<= (int32_t)1; a->set_elements(e);
  });
  CASE("arr.insert_long_seq_count", {
    auto d = factory->create_dyn_any_from_type_code(arr_tc);
    CORBA::LongSeq v(2); d->insert_long_seq(v);
  });

  // -- valuetypes
  CASE("value.null", { auto d = factory->create_dyn_any_from_type_code(v_tc); auto v = narrow<DynValue>(d); out(v->is_null()); std::cout << " "; out(d->component_count()); });
  CASE("value.set_to_value", { auto d = factory->create_dyn_any_from_type_code(v_tc); auto v = narrow<DynValue>(d); v->set_to_value(); out(v->is_null()); std::cout << " "; out(d->component_count()); std::cout << " "; out(position(d)); });
  CASE("value.get_members_null", { auto d = factory->create_dyn_any_from_type_code(v_tc); auto v = narrow<DynValue>(d); auto m = v->get_members(); out((uint32_t)m.size()); });
  CASE("value.member_name_null", { auto d = factory->create_dyn_any_from_type_code(v_tc); auto v = narrow<DynValue>(d); std::string n = v->current_member_name(); out(n); });
  CASE("value.current_component_null", { auto d = factory->create_dyn_any_from_type_code(v_tc); auto c = d->current_component(); out(!c); });
  CASE("value.set_members", {
    auto d = factory->create_dyn_any_from_type_code(v_tc); auto v = narrow<DynValue>(d);
    NameValuePairSeq m(1); m[0].id("n"); m[0].value(any_of((int32_t)3));
    v->set_members(m); out(v->is_null()); std::cout << " "; auto c = d->current_component(); out(c->get_long());
  });
  CASE("box.null", { auto d = factory->create_dyn_any_from_type_code(box_tc); auto b = narrow<DynValueBox>(d); out(b->is_null()); std::cout << " "; out(d->component_count()); });
  CASE("box.get_null", { auto d = factory->create_dyn_any_from_type_code(box_tc); auto b = narrow<DynValueBox>(d); CORBA::Any a = b->get_boxed_value(); });
  CASE("box.set", {
    auto d = factory->create_dyn_any_from_type_code(box_tc); auto b = narrow<DynValueBox>(d);
    CORBA::Any a; a <<= (int32_t)4; b->set_boxed_value(a); out(b->is_null()); std::cout << " "; out(d->component_count()); std::cout << " "; out(position(d));
  });
  CASE("box.set_wrong", {
    auto d = factory->create_dyn_any_from_type_code(box_tc); auto b = narrow<DynValueBox>(d);
    CORBA::Any a; a <<= (int16_t)4; b->set_boxed_value(a);
  });
  CASE("box.set_to_value", { auto d = factory->create_dyn_any_from_type_code(box_tc); auto b = narrow<DynValueBox>(d); b->set_to_value(); auto c = d->current_component(); out(c->get_long()); });

  orb->destroy();
  return 0;
}
