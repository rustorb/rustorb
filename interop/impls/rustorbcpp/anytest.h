// The stage 2 tests (idl/AnyTest.idl) on the rustorb C++ binding: the
// table of values, their comparison, the AnyEcho servant and the client
// cases. The values and cases are those of impls/omniorbpy/anytest.py, on
// the IDL to C++11 mapping.

#ifndef RUSTORBCPP_ANYTEST_H
#define RUSTORBCPP_ANYTEST_H

#include <functional>
#include <string>
#include <utility>
#include <vector>

#include "AnyTest.hh"
#ifdef ANYTEST_HIDDEN
#include "AnyTestHidden.hh"
#endif

namespace anytest {

using namespace AnyTest;
using EchoRef = IDL::traits<AnyEcho>::ref_type;
using TypeCodeRef = IDL::traits<CORBA::TypeCode>::ref_type;

inline const std::vector<std::string>& names() {
  static const std::vector<std::string> n = {
      "long", "short", "ulonglong", "float", "double", "longdouble", "boolean", "char", "octet",
      "string", "bounded", "count", "money", "fixed", "point", "shade", "choice_l", "choice_s",
      "choice_default", "longseq", "octets", "shortarray", "node", "nvlist", "any",
      "typecode", "objref", "nil", "oops", "empty"};
  return n;
}

inline Node node(const std::string& name, std::vector<Node> children = {}) {
  return Node(name, std::move(children));
}

inline Choice choice(const std::string& name) {
  Choice c;
  if (name == "choice_l") {
    c.l(10);
  } else if (name == "choice_s") {
    c.s("three", 3);
  } else {
    c.b(true, 9);
  }
  return c;
}

// The value named `name` of the table in AnyTest.idl. `self` is the AnyEcho
// object (for "objref").
inline CORBA::Any make(const std::string& name, const EchoRef& self) {
  CORBA::Any a;
  if (name == "long") a <<= int32_t{42};
  else if (name == "short") a <<= int16_t{-7};
  else if (name == "ulonglong") a <<= uint64_t{18446744073709551615ULL};
  else if (name == "float") a <<= 0.25f;
  else if (name == "double") a <<= 1.5;
  else if (name == "longdouble") a <<= static_cast<long double>(0.1);
  else if (name == "boolean") a <<= true;
  else if (name == "char") a <<= 'x';
  else if (name == "octet") a <<= uint8_t{255};
  else if (name == "string") a <<= "hello any";
  else if (name == "bounded") {
    a <<= Short5("abc");
    a.type(_tc_Short5);
  } else if (name == "count") {
    a <<= Count{7};
    a.type(_tc_Count);
  } else if (name == "money") {
    a <<= Money("-123.45");
    a.type(_tc_Money);
  } else if (name == "fixed") {
    a <<= IDL::Fixed<31, 4>("123456789012345678901234567.8901");
  } else if (name == "point") a <<= Point(1, 2.5);
  else if (name == "shade") a <<= Shade::DIM;
  else if (name.rfind("choice_", 0) == 0) a <<= choice(name);
  else if (name == "longseq") {
    a <<= LongSeq{1, 2, 3};
    a.type(_tc_LongSeq);
  } else if (name == "octets") {
    a <<= Octets{0, 1, 255};
    a.type(_tc_Octets);
  } else if (name == "shortarray") {
    a <<= ShortArray{1, -2, 3};
    a.type(_tc_ShortArray);
  } else if (name == "node") {
    a <<= node("root", {node("a"), node("b", {node("c")})});
  } else if (name == "nvlist") {
    NVList l(3);
    l[0].name("a");
    l[0].value() <<= int32_t{1};
    l[1].name("b");
    l[1].value() <<= "x";
    l[2].name("p");
    l[2].value() <<= Point(3, -0.5);
    a <<= l;
    a.type(_tc_NVList);
  } else if (name == "any") {
    CORBA::Any inner;
    inner <<= int32_t{5};
    a <<= inner;
  } else if (name == "typecode") a <<= _tc_Node;
  else if (name == "objref") a <<= self;
  else if (name == "nil") a <<= EchoRef();
  else if (name == "oops") a <<= Oops("bad", 3);
  else if (name == "empty") {
  }
#ifdef ANYTEST_HIDDEN
  else if (name == "secret") {
    a <<= AnyTestHidden::Secret("s", {Point(1, 2.0)}, Shade::DARK);
  } else if (name == "pick") {
    AnyTestHidden::Pick p;
    p.s(AnyTestHidden::Secret("p", {}, Shade::LIGHT));
    a <<= p;
  } else if (name == "moods") {
    a <<= AnyTestHidden::Moods{AnyTestHidden::Mood::SAD, AnyTestHidden::Mood::HAPPY};
    a.type(AnyTestHidden::_tc_Moods);
  }
#endif
  else {
    throw CORBA::BAD_PARAM(0, CORBA::CompletionStatus::COMPLETED_NO);
  }
  return a;
}

inline bool same_nodes(const Node& a, const Node& b) {
  if (a.name() != b.name() || a.children().size() != b.children().size()) return false;
  for (size_t i = 0; i < a.children().size(); ++i)
    if (!same_nodes(a.children()[i], b.children()[i])) return false;
  return true;
}

// Whether value is extracted from `got` as type T and equals `want`.
template <typename T> bool holds(const CORBA::Any& got, const T& want) {
  T v{};
  return (got >>= v) && v == want;
}

// Whether `got` is the value named `name`: an equal TypeCode and the same
// value.
inline bool same(const std::string& name, const CORBA::Any& got, const EchoRef& self) {
  CORBA::Any want = make(name, self);
  if (!got.type()->equal(want.type())) return false;
  if (name == "long") return holds(got, int32_t{42});
  if (name == "count") return holds(got, int32_t{7});
  if (name == "short") return holds(got, int16_t{-7});
  if (name == "ulonglong") return holds(got, uint64_t{18446744073709551615ULL});
  if (name == "float") return holds(got, 0.25f);
  if (name == "double") return holds(got, 1.5);
  if (name == "longdouble") return holds(got, static_cast<long double>(0.1));
  if (name == "boolean") return holds(got, true);
  if (name == "char") return holds(got, 'x');
  if (name == "octet") return holds(got, uint8_t{255});
  if (name == "string") return holds(got, std::string("hello any"));
  if (name == "bounded") return holds(got, Short5("abc"));
  if (name == "money") return holds(got, Money("-123.45"));
  if (name == "fixed") return holds(got, IDL::Fixed<31, 4>("123456789012345678901234567.8901"));
  if (name == "point") return holds(got, Point(1, 2.5));
  if (name == "shade") return holds(got, Shade::DIM);
  if (name.rfind("choice_", 0) == 0) return holds(got, choice(name));
  if (name == "longseq") return holds(got, LongSeq{1, 2, 3});
  if (name == "octets") return holds(got, Octets{0, 1, 255});
  if (name == "shortarray") return holds(got, ShortArray{1, -2, 3});
  if (name == "node") {
    Node v, w;
    return (got >>= v) && (want >>= w) && same_nodes(v, w);
  }
  if (name == "nvlist") {
    NVList l;
    if (!(got >>= l) || l.size() != 3) return false;
    return l[0].name() == "a" && l[0].value().type()->equal(CORBA::_tc_long) &&
           holds(l[0].value(), int32_t{1}) && l[1].name() == "b" &&
           l[1].value().type()->equal(CORBA::_tc_string) &&
           holds(l[1].value(), std::string("x")) && l[2].name() == "p" &&
           l[2].value().type()->equal(_tc_Point) && holds(l[2].value(), Point(3, -0.5));
  }
  if (name == "any") {
    CORBA::Any inner;
    return (got >>= inner) && inner.type()->equal(CORBA::_tc_long) &&
           holds(inner, int32_t{5});
  }
  if (name == "typecode") {
    TypeCodeRef t;
    return (got >>= t) && t && t->equal(_tc_Node);
  }
  if (name == "objref") {
    EchoRef v;
    return (got >>= v) && v && v->_is_equivalent(self);
  }
  if (name == "nil") {
    EchoRef v;
    return (got >>= v) && !v;
  }
  if (name == "oops") return holds(got, Oops("bad", 3));
  if (name == "empty") return got.type()->kind() == CORBA::TCKind::tk_null;
#ifdef ANYTEST_HIDDEN
  if (name == "secret")
    return holds(got, AnyTestHidden::Secret("s", {Point(1, 2.0)}, Shade::DARK));
  if (name == "pick") {
    AnyTestHidden::Pick p;
    p.s(AnyTestHidden::Secret("p", {}, Shade::LIGHT));
    return holds(got, p);
  }
  if (name == "moods")
    return holds(got, AnyTestHidden::Moods{AnyTestHidden::Mood::SAD,
                                           AnyTestHidden::Mood::HAPPY});
#endif
  return false;
}

inline const char* kind_name(CORBA::TCKind k) {
  static const char* const names[] = {
      "null", "void", "short", "long", "ushort", "ulong", "float", "double", "boolean",
      "char", "octet", "any", "TypeCode", "Principal", "objref", "struct", "union",
      "enum", "string", "sequence", "array", "alias", "except", "longlong",
      "ulonglong", "longdouble", "wchar", "wstring", "fixed", "value", "value_box",
      "native", "abstract_interface", "local_interface"};
  unsigned i = static_cast<unsigned>(k);
  return i < sizeof(names) / sizeof(names[0]) ? names[i] : "?";
}

// The servant of the servers, which are built without AnyTestHidden.
class AnyEchoImpl : public CORBA::servant_traits<AnyEcho>::base_type {
 public:
  CORBA::Any echo(const CORBA::Any& a) override { return a; }

  CORBA::Any make(const std::string& name) override { return anytest::make(name, _this()); }

  bool check(const std::string& name, const CORBA::Any& a) override {
    return anytest::same(name, a, _this());
  }

  void out_inout(const std::string& name, CORBA::Any& b, CORBA::Any& c) override {
    if (!anytest::same(name, c, _this()))
      throw CORBA::BAD_PARAM(0, CORBA::CompletionStatus::COMPLETED_NO);
    b = anytest::make(name, _this());
    c = anytest::make("long", _this());
  }

  TypeCodeRef echo_tc(TypeCodeRef tc) override { return tc; }

  TypeCodeRef make_tc(const std::string& name) override {
    return anytest::make(name, _this()).type();
  }

  bool check_tc(const std::string& name, TypeCodeRef tc) override {
    return anytest::make(name, _this()).type()->equal(tc);
  }

  NVList echo_nvlist(const NVList& l) override { return l; }

  std::string describe(const CORBA::Any& a) override {
    TypeCodeRef tc = a.type();
    std::string id;
    try {
      id = tc->id();
    } catch (const CORBA::TypeCode::BadKind&) {
    }
    return std::string(kind_name(tc->kind())) + " " + id;
  }
};

#ifdef ANYTEST_HIDDEN
// The DynAny of src copied into dst, a DynAny of the same type made from
// its TypeCode alone, component by component.
inline void dyn_copy(const DynamicAny::DynAny_ref& src, const DynamicAny::DynAny_ref& dst);

// `value` taken apart with a DynAny and built again in a DynAny made from
// its TypeCode alone (docs/dynany.md).
inline CORBA::Any dyn_rebuild(const CORBA::Any& value) {
  static const auto factory = IDL::traits<DynamicAny::DynAnyFactory>::ref_type(
      std::make_shared<DynamicAny::DynAnyFactory>());
  auto src = factory->create_dyn_any(value);
  auto dst = factory->create_dyn_any_from_type_code(value.type());
  dyn_copy(src, dst);
  CORBA::Any rebuilt = dst->to_any();
  src->destroy();
  dst->destroy();
  return rebuilt;
}

inline void dyn_copy(const DynamicAny::DynAny_ref& src, const DynamicAny::DynAny_ref& dst) {
  using namespace DynamicAny;
  using K = CORBA::TCKind;
  auto narrow = [](const DynAny_ref& d, auto* tag) {
    using T = std::remove_pointer_t<decltype(tag)>;
    return IDL::traits<T>::narrow(d);
  };
  TypeCodeRef tc = src->type();
  while (tc->kind() == K::tk_alias) tc = tc->content_type();
  switch (tc->kind()) {
    case K::tk_sequence:
    case K::tk_value:
    case K::tk_value_box:
    case K::tk_struct:
    case K::tk_except:
    case K::tk_array: {
      if (tc->kind() == K::tk_sequence) {
        narrow(dst, static_cast<DynSequence*>(nullptr))
            ->set_length(narrow(src, static_cast<DynSequence*>(nullptr))->get_length());
      } else if (tc->kind() == K::tk_value || tc->kind() == K::tk_value_box) {
        auto s = narrow(src, static_cast<DynValueCommon*>(nullptr));
        auto d = narrow(dst, static_cast<DynValueCommon*>(nullptr));
        if (s->is_null()) return d->set_to_null();
        d->set_to_value();
      }
      for (uint32_t i = 0; i < src->component_count(); ++i) {
        src->seek(static_cast<int32_t>(i));
        dst->seek(static_cast<int32_t>(i));
        dyn_copy(src->current_component(), dst->current_component());
      }
      return;
    }
    case K::tk_union: {
      auto s = narrow(src, static_cast<DynUnion*>(nullptr));
      auto d = narrow(dst, static_cast<DynUnion*>(nullptr));
      d->set_discriminator(s->get_discriminator());
      if (!s->has_no_active_member()) dyn_copy(s->member(), d->member());
      return;
    }
    case K::tk_enum:
      return narrow(dst, static_cast<DynEnum*>(nullptr))
          ->set_as_ulong(narrow(src, static_cast<DynEnum*>(nullptr))->get_as_ulong());
    case K::tk_fixed:
      narrow(dst, static_cast<DynFixed*>(nullptr))
          ->set_value(narrow(src, static_cast<DynFixed*>(nullptr))->get_value());
      return;
    case K::tk_any: return dst->insert_any(dyn_rebuild(src->get_any()));
    case K::tk_long: return dst->insert_long(src->get_long());
    case K::tk_short: return dst->insert_short(src->get_short());
    case K::tk_ulong: return dst->insert_ulong(src->get_ulong());
    case K::tk_ushort: return dst->insert_ushort(src->get_ushort());
    case K::tk_longlong: return dst->insert_longlong(src->get_longlong());
    case K::tk_ulonglong: return dst->insert_ulonglong(src->get_ulonglong());
    case K::tk_float: return dst->insert_float(src->get_float());
    case K::tk_double: return dst->insert_double(src->get_double());
    case K::tk_longdouble: return dst->insert_longdouble(src->get_longdouble());
    case K::tk_boolean: return dst->insert_boolean(src->get_boolean());
    case K::tk_char: return dst->insert_char(src->get_char());
    case K::tk_wchar: return dst->insert_wchar(src->get_wchar());
    case K::tk_octet: return dst->insert_octet(src->get_octet());
    case K::tk_string: return dst->insert_string(src->get_string());
    case K::tk_wstring: return dst->insert_wstring(src->get_wstring());
    case K::tk_TypeCode: return dst->insert_typecode(src->get_typecode());
    case K::tk_objref: return dst->insert_reference(src->get_reference());
    default: return dst->from_any(src->to_any());  // tk_null and tk_void
  }
}

// Runs the stage 2 cases; `r.run(name, fn)` reports a case, `check(cond,
// what)` fails one.
template <typename Runner>
void run_cases(Runner& r, const EchoRef& echo,
               const std::function<void(bool, const std::string&)>& check) {
  for (const std::string& n : names()) {
    r.run("make[" + n + "]", [&]() { check(same(n, echo->make(n), echo), "wrong value"); });
    r.run("check[" + n + "]", [&]() { check(echo->check(n, make(n, echo)), "server says no"); });
    r.run("echo[" + n + "]",
          [&]() { check(same(n, echo->echo(make(n, echo)), echo), "wrong value"); });
  }
  for (const std::string& n : names()) {
    r.run("make_tc[" + n + "]", [&]() {
      check(echo->make_tc(n)->equal(make(n, echo).type()), "wrong TypeCode");
    });
    r.run("check_tc[" + n + "]",
          [&]() { check(echo->check_tc(n, make(n, echo).type()), "server says no"); });
    r.run("echo_tc[" + n + "]", [&]() {
      TypeCodeRef t = make(n, echo).type();
      check(echo->echo_tc(t)->equal(t), "wrong TypeCode");
    });
  }
  for (const char* n : {"point", "nvlist", "node"}) {
    r.run(std::string("out_inout[") + n + "]", [&]() {
      CORBA::Any b;
      CORBA::Any c = make(n, echo);
      echo->out_inout(n, b, c);
      check(same(n, b, echo), "wrong out value");
      check(same("long", c, echo), "wrong inout value");
    });
  }
  r.run("echo_nvlist", [&]() {
    NVList l;
    make("nvlist", echo) >>= l;
    CORBA::Any a;
    a <<= echo->echo_nvlist(l);
    a.type(_tc_NVList);
    check(same("nvlist", a, echo), "wrong value");
  });
  const std::vector<std::pair<std::string, std::string>> hidden = {
      {"secret", "struct IDL:AnyTestHidden/Secret:1.0"},
      {"pick", "union IDL:AnyTestHidden/Pick:1.0"},
      {"moods", "alias IDL:AnyTestHidden/Moods:1.0"}};
  for (const auto& h : hidden) {
    r.run("echo[" + h.first + "]",
          [&]() { check(same(h.first, echo->echo(make(h.first, echo)), echo), "wrong value"); });
    r.run("describe[" + h.first + "]", [&]() {
      std::string d = echo->describe(make(h.first, echo));
      check(d == h.second, d);
    });
  }
  // The values taken apart and built again with DynAny, from their
  // TypeCodes alone.
  for (const std::string& n : names()) {
    r.run("dyn_check[" + n + "]",
          [&]() { check(echo->check(n, dyn_rebuild(make(n, echo))), "server says no"); });
    r.run("dyn_make[" + n + "]",
          [&]() { check(same(n, dyn_rebuild(echo->make(n)), echo), "wrong value"); });
  }
  for (const auto& h : hidden) {
    r.run("dyn_echo[" + h.first + "]", [&]() {
      check(same(h.first, echo->echo(dyn_rebuild(make(h.first, echo))), echo), "wrong value");
    });
  }
  r.run("describe[long]", [&]() {
    std::string d = echo->describe(make("long", echo));
    check(d == "long ", d);
  });
  r.run("describe[node]", [&]() {
    std::string d = echo->describe(make("node", echo));
    check(d == "struct IDL:AnyTest/Node:1.0", d);
  });
  r.run("check_wrong_type", [&]() {
    CORBA::Any a;
    a <<= int16_t{42};
    check(!echo->check("long", a), "server says yes");
  });
  r.run("check_unaliased", [&]() {
    CORBA::Any a;
    a <<= int32_t{7};
    check(!echo->check("count", a), "server says yes");
  });
  r.run("check_tc_unaliased",
        [&]() { check(!echo->check_tc("count", CORBA::_tc_long), "server says yes"); });
  r.run("out_inout_wrong", [&]() {
    CORBA::Any b;
    CORBA::Any c = make("long", echo);
    try {
      echo->out_inout("point", b, c);
    } catch (const CORBA::BAD_PARAM&) {
      return;
    }
    check(false, "no BAD_PARAM");
  });
}
#endif

}  // namespace anytest

#endif
