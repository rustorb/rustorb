// The stage 2 tests (idl/AnyTest.idl) on omniORB C++: the table of values,
// their comparison, the AnyEcho servant and the client cases. The values
// and cases are those of impls/omniorbpy/anytest.py.

#ifndef INTEROP_ANYTEST_H
#define INTEROP_ANYTEST_H

#include <functional>
#include <string>
#include <vector>

#include "AnyTest.hh"
#ifdef ANYTEST_HIDDEN
#include "AnyTestHidden.hh"
#endif

namespace anytest {

using namespace AnyTest;

inline const std::vector<std::string>& names() {
  static const std::vector<std::string> n = {
      "long", "short", "ulonglong", "float", "double", "longdouble", "boolean", "char", "octet",
      "string", "bounded", "count", "money", "fixed", "point", "shade", "choice_l", "choice_s",
      "choice_default", "longseq", "octets", "shortarray", "node", "nvlist", "any",
      "typecode", "objref", "nil", "oops", "empty"};
  return n;
}

inline Node node(const char* name, std::vector<Node> children = {}) {
  Node n;
  n.name = name;
  n.children.length(static_cast<CORBA::ULong>(children.size()));
  for (CORBA::ULong i = 0; i < children.size(); ++i) n.children[i] = children[i];
  return n;
}

inline Choice choice(const std::string& name) {
  Choice c;
  if (name == "choice_l") {
    c.l(10);
  } else if (name == "choice_s") {
    c.s("three");
    c._d(3);
  } else {
    c.b(true);
    c._d(9);
  }
  return c;
}

// The value named `name` of the table in AnyTest.idl. `self` is the AnyEcho
// object (for "objref").
inline CORBA::Any* make(const std::string& name, AnyEcho_ptr self) {
  CORBA::Any* a = new CORBA::Any;
  if (name == "long") *a <<= static_cast<CORBA::Long>(42);
  else if (name == "short") *a <<= static_cast<CORBA::Short>(-7);
  else if (name == "ulonglong") *a <<= static_cast<CORBA::ULongLong>(18446744073709551615ULL);
  else if (name == "float") *a <<= static_cast<CORBA::Float>(0.25);
  else if (name == "double") *a <<= static_cast<CORBA::Double>(1.5);
  else if (name == "longdouble") *a <<= static_cast<CORBA::LongDouble>(0.1);
  else if (name == "boolean") *a <<= CORBA::Any::from_boolean(1);
  else if (name == "char") *a <<= CORBA::Any::from_char('x');
  else if (name == "octet") *a <<= CORBA::Any::from_octet(255);
  else if (name == "string") *a <<= "hello any";
  else if (name == "bounded") {
    *a <<= CORBA::Any::from_string("abc", 5);
    a->type(_tc_Short5);
  } else if (name == "count") {
    *a <<= static_cast<CORBA::Long>(7);
    a->type(_tc_Count);
  } else if (name == "money") {
    *a <<= CORBA::Any::from_fixed(CORBA::Fixed("-123.45"), 5, 2);
    a->type(_tc_Money);
  } else if (name == "fixed") {
    *a <<= CORBA::Any::from_fixed(CORBA::Fixed("123456789012345678901234567.8901"), 31, 4);
  } else if (name == "point") {
    Point p;
    p.x = 1;
    p.y = 2.5;
    *a <<= p;
  } else if (name == "shade") *a <<= DIM;
  else if (name.rfind("choice_", 0) == 0) *a <<= choice(name);
  else if (name == "longseq") {
    LongSeq s;
    s.length(3);
    s[0] = 1;
    s[1] = 2;
    s[2] = 3;
    *a <<= s;
  } else if (name == "octets") {
    Octets s;
    s.length(3);
    s[0] = 0;
    s[1] = 1;
    s[2] = 255;
    *a <<= s;
  } else if (name == "shortarray") {
    ShortArray v = {1, -2, 3};
    *a <<= ShortArray_forany(v);
  } else if (name == "node") {
    *a <<= node("root", {node("a"), node("b", {node("c")})});
  } else if (name == "nvlist") {
    NVList l;
    l.length(3);
    l[0].name = "a";
    l[0].value <<= static_cast<CORBA::Long>(1);
    l[1].name = "b";
    l[1].value <<= "x";
    l[2].name = "p";
    Point p;
    p.x = 3;
    p.y = -0.5;
    l[2].value <<= p;
    *a <<= l;
  } else if (name == "any") {
    CORBA::Any inner;
    inner <<= static_cast<CORBA::Long>(5);
    *a <<= inner;
  } else if (name == "typecode") *a <<= _tc_Node;
  else if (name == "objref") *a <<= self;
  else if (name == "nil") *a <<= AnyEcho::_nil();
  else if (name == "oops") *a <<= Oops("bad", 3);
  else if (name == "empty") {
  }
#ifdef ANYTEST_HIDDEN
  else if (name == "secret") {
    AnyTestHidden::Secret s;
    s.text = "s";
    s.points.length(1);
    s.points[0].x = 1;
    s.points[0].y = 2.0;
    s.shade = DARK;
    *a <<= s;
  } else if (name == "pick") {
    AnyTestHidden::Secret s;
    s.text = "p";
    s.shade = LIGHT;
    AnyTestHidden::Pick p;
    p.s(s);
    *a <<= p;
  } else if (name == "moods") {
    AnyTestHidden::Moods m;
    m.length(2);
    m[0] = AnyTestHidden::SAD;
    m[1] = AnyTestHidden::HAPPY;
    *a <<= m;
  }
#endif
  else {
    delete a;
    throw CORBA::BAD_PARAM(0, CORBA::COMPLETED_NO);
  }
  return a;
}

inline bool same(const Point& a, const Point& b) { return a.x == b.x && a.y == b.y; }

inline bool same(const Node& a, const Node& b) {
  if (std::string(a.name.in()) != b.name.in() || a.children.length() != b.children.length())
    return false;
  for (CORBA::ULong i = 0; i < a.children.length(); ++i)
    if (!same(a.children[i], b.children[i])) return false;
  return true;
}

template <typename T>
bool same_seq(const T& a, const T& b) {
  if (a.length() != b.length()) return false;
  for (CORBA::ULong i = 0; i < a.length(); ++i)
    if (!(a[i] == b[i])) return false;
  return true;
}

// Whether `got` is the value named `name`: an equal TypeCode and the same
// value.
inline bool same(const std::string& name, const CORBA::Any& got, AnyEcho_ptr self) {
  CORBA::Any_var want = make(name, self);
  CORBA::TypeCode_var got_tc = got.type();
  CORBA::TypeCode_var want_tc = want->type();
  if (!got_tc->equal(want_tc)) return false;
  if (name == "long" || name == "count") {
    CORBA::Long v, w;
    return (got >>= v) && (want.in() >>= w) && v == w;
  }
  if (name == "short") {
    CORBA::Short v;
    return (got >>= v) && v == -7;
  }
  if (name == "ulonglong") {
    CORBA::ULongLong v;
    return (got >>= v) && v == 18446744073709551615ULL;
  }
  if (name == "float") {
    CORBA::Float v;
    return (got >>= v) && v == 0.25;
  }
  if (name == "double") {
    CORBA::Double v;
    return (got >>= v) && v == 1.5;
  }
  if (name == "longdouble") {
    CORBA::LongDouble v;
    return (got >>= v) && v == static_cast<CORBA::LongDouble>(0.1);
  }
  if (name == "boolean") {
    CORBA::Boolean v;
    return (got >>= CORBA::Any::to_boolean(v)) && v;
  }
  if (name == "char") {
    CORBA::Char v;
    return (got >>= CORBA::Any::to_char(v)) && v == 'x';
  }
  if (name == "octet") {
    CORBA::Octet v;
    return (got >>= CORBA::Any::to_octet(v)) && v == 255;
  }
  if (name == "string") {
    const char* v;
    return (got >>= v) && std::string(v) == "hello any";
  }
  if (name == "bounded") {
    const char* v;
    return (got >>= CORBA::Any::to_string(v, 5)) && std::string(v) == "abc";
  }
  if (name == "money") {
    CORBA::Fixed v;
    return (got >>= CORBA::Any::to_fixed(v, 5, 2)) && v == CORBA::Fixed("-123.45");
  }
  if (name == "fixed") {
    CORBA::Fixed v;
    return (got >>= CORBA::Any::to_fixed(v, 31, 4)) &&
           v == CORBA::Fixed("123456789012345678901234567.8901");
  }
  if (name == "point") {
    const Point* v;
    const Point* w;
    return (got >>= v) && (want.in() >>= w) && same(*v, *w);
  }
  if (name == "shade") {
    Shade v;
    return (got >>= v) && v == DIM;
  }
  if (name.rfind("choice_", 0) == 0) {
    const Choice* v;
    Choice w = choice(name);
    if (!(got >>= v) || v->_d() != w._d()) return false;
    switch (w._d()) {
      case 1: return v->l() == w.l();
      case 2:
      case 3: return std::string(v->s()) == w.s();
      default: return v->b() == w.b();
    }
  }
  if (name == "longseq") {
    const LongSeq* v;
    const LongSeq* w;
    return (got >>= v) && (want.in() >>= w) && same_seq(*v, *w);
  }
  if (name == "octets") {
    const Octets* v;
    const Octets* w;
    return (got >>= v) && (want.in() >>= w) && same_seq(*v, *w);
  }
  if (name == "shortarray") {
    ShortArray_forany v;
    return (got >>= v) && v[0] == 1 && v[1] == -2 && v[2] == 3;
  }
  if (name == "node") {
    const Node* v;
    const Node* w;
    return (got >>= v) && (want.in() >>= w) && same(*v, *w);
  }
  if (name == "nvlist") {
    const NVList* v;
    if (!(got >>= v) || v->length() != 3) return false;
    const NVList& l = *v;
    CORBA::Long n;
    const char* s;
    const Point* p;
    CORBA::TypeCode_var t0 = l[0].value.type(), t1 = l[1].value.type(), t2 = l[2].value.type();
    return std::string(l[0].name.in()) == "a" && t0->equal(CORBA::_tc_long) &&
           (l[0].value >>= n) && n == 1 && std::string(l[1].name.in()) == "b" &&
           t1->equal(CORBA::_tc_string) && (l[1].value >>= s) && std::string(s) == "x" &&
           std::string(l[2].name.in()) == "p" && t2->equal(_tc_Point) &&
           (l[2].value >>= p) && p->x == 3 && p->y == -0.5;
  }
  if (name == "any") {
    const CORBA::Any* inner;
    CORBA::Long n;
    if (!(got >>= inner)) return false;
    CORBA::TypeCode_var t = inner->type();
    return t->equal(CORBA::_tc_long) && (*inner >>= n) && n == 5;
  }
  if (name == "typecode") {
    CORBA::TypeCode_ptr t;
    return (got >>= t) && t->equal(_tc_Node);
  }
  if (name == "objref") {
    AnyEcho_ptr v;
    return (got >>= v) && !CORBA::is_nil(v) && v->_is_equivalent(self);
  }
  if (name == "nil") {
    AnyEcho_ptr v;
    return (got >>= v) && CORBA::is_nil(v);
  }
  if (name == "oops") {
    const Oops* v;
    return (got >>= v) && std::string(v->why.in()) == "bad" && v->code == 3;
  }
  if (name == "empty") return got_tc->kind() == CORBA::tk_null;
#ifdef ANYTEST_HIDDEN
  if (name == "secret") {
    const AnyTestHidden::Secret* v;
    return (got >>= v) && std::string(v->text.in()) == "s" && v->points.length() == 1 &&
           v->points[0].x == 1 && v->points[0].y == 2.0 && v->shade == DARK;
  }
  if (name == "pick") {
    const AnyTestHidden::Pick* v;
    return (got >>= v) && v->_d() == 'b' && std::string(v->s().text.in()) == "p" &&
           v->s().points.length() == 0 && v->s().shade == LIGHT;
  }
  if (name == "moods") {
    const AnyTestHidden::Moods* v;
    return (got >>= v) && v->length() == 2 && (*v)[0] == AnyTestHidden::SAD &&
           (*v)[1] == AnyTestHidden::HAPPY;
  }
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

class AnyEchoImpl : public POA_AnyTest::AnyEcho {
 public:
  CORBA::Any* echo(const CORBA::Any& a) override { return new CORBA::Any(a); }

  CORBA::Any* make(const char* name) override {
    AnyTest::AnyEcho_var self = _this();
    return anytest::make(name, self);
  }

  CORBA::Boolean check(const char* name, const CORBA::Any& a) override {
    AnyTest::AnyEcho_var self = _this();
    return anytest::same(name, a, self);
  }

  void out_inout(const char* name, CORBA::Any_out b, CORBA::Any& c) override {
    AnyTest::AnyEcho_var self = _this();
    if (!anytest::same(name, c, self)) throw CORBA::BAD_PARAM(0, CORBA::COMPLETED_NO);
    b = anytest::make(name, self);
    CORBA::Any_var l = anytest::make("long", self);
    c = l.in();
  }

  CORBA::TypeCode_ptr echo_tc(CORBA::TypeCode_ptr tc) override {
    return CORBA::TypeCode::_duplicate(tc);
  }

  CORBA::TypeCode_ptr make_tc(const char* name) override {
    AnyTest::AnyEcho_var self = _this();
    CORBA::Any_var a = anytest::make(name, self);
    return a->type();
  }

  CORBA::Boolean check_tc(const char* name, CORBA::TypeCode_ptr tc) override {
    AnyTest::AnyEcho_var self = _this();
    CORBA::Any_var a = anytest::make(name, self);
    CORBA::TypeCode_var t = a->type();
    return t->equal(tc);
  }

  NVList* echo_nvlist(const NVList& l) override { return new NVList(l); }

  char* describe(const CORBA::Any& a) override {
    CORBA::TypeCode_var tc = a.type();
    std::string id;
    try {
      id = tc->id();
    } catch (CORBA::TypeCode::BadKind&) {
    }
    return CORBA::string_dup((std::string(kind_name(tc->kind())) + " " + id).c_str());
  }
};

#ifdef ANYTEST_HIDDEN
// Runs the stage 2 cases; `run(name, fn)` reports a case, `check(cond,
// what)` fails one.
template <typename Runner>
void run_cases(Runner& r, AnyEcho_ptr echo,
               const std::function<void(bool, const std::string&)>& check) {
  for (const std::string& n : names()) {
    r.run("make[" + n + "]", [&]() {
      CORBA::Any_var got = echo->make(n.c_str());
      check(same(n, got.in(), echo), "wrong value");
    });
    r.run("check[" + n + "]", [&]() {
      CORBA::Any_var v = make(n, echo);
      check(echo->check(n.c_str(), v.in()), "server says no");
    });
    r.run("echo[" + n + "]", [&]() {
      CORBA::Any_var v = make(n, echo);
      CORBA::Any_var got = echo->echo(v.in());
      check(same(n, got.in(), echo), "wrong value");
    });
  }
  for (const std::string& n : names()) {
    r.run("make_tc[" + n + "]", [&]() {
      CORBA::Any_var v = make(n, echo);
      CORBA::TypeCode_var want = v->type();
      CORBA::TypeCode_var got = echo->make_tc(n.c_str());
      check(got->equal(want), "wrong TypeCode");
    });
    r.run("check_tc[" + n + "]", [&]() {
      CORBA::Any_var v = make(n, echo);
      CORBA::TypeCode_var t = v->type();
      check(echo->check_tc(n.c_str(), t), "server says no");
    });
    r.run("echo_tc[" + n + "]", [&]() {
      CORBA::Any_var v = make(n, echo);
      CORBA::TypeCode_var t = v->type();
      CORBA::TypeCode_var got = echo->echo_tc(t);
      check(got->equal(t), "wrong TypeCode");
    });
  }
  for (const char* n : {"point", "nvlist", "node"}) {
    r.run(std::string("out_inout[") + n + "]", [&]() {
      CORBA::Any_var b;
      CORBA::Any_var c = make(n, echo);
      echo->out_inout(n, b.out(), c.inout());
      check(same(n, b.in(), echo), "wrong out value");
      check(same("long", c.in(), echo), "wrong inout value");
    });
  }
  r.run("echo_nvlist", [&]() {
    CORBA::Any_var v = make("nvlist", echo);
    const NVList* l;
    v.in() >>= l;
    NVList_var got = echo->echo_nvlist(*l);
    CORBA::Any a;
    a <<= got.in();
    check(same("nvlist", a, echo), "wrong value");
  });
  const std::vector<std::pair<std::string, std::string>> hidden = {
      {"secret", "struct IDL:AnyTestHidden/Secret:1.0"},
      {"pick", "union IDL:AnyTestHidden/Pick:1.0"},
      {"moods", "alias IDL:AnyTestHidden/Moods:1.0"}};
  for (const auto& h : hidden) {
    r.run("echo[" + h.first + "]", [&]() {
      CORBA::Any_var v = make(h.first, echo);
      CORBA::Any_var got = echo->echo(v.in());
      check(same(h.first, got.in(), echo), "wrong value");
    });
    r.run("describe[" + h.first + "]", [&]() {
      CORBA::Any_var v = make(h.first, echo);
      CORBA::String_var d = echo->describe(v.in());
      check(h.second == d.in(), d.in());
    });
  }
  r.run("describe[long]", [&]() {
    CORBA::Any_var v = make("long", echo);
    CORBA::String_var d = echo->describe(v.in());
    check(std::string(d.in()) == "long ", d.in());
  });
  r.run("describe[node]", [&]() {
    CORBA::Any_var v = make("node", echo);
    CORBA::String_var d = echo->describe(v.in());
    check(std::string(d.in()) == "struct IDL:AnyTest/Node:1.0", d.in());
  });
  r.run("check_wrong_type", [&]() {
    CORBA::Any a;
    a <<= static_cast<CORBA::Short>(42);
    check(!echo->check("long", a), "server says yes");
  });
  r.run("check_unaliased", [&]() {
    CORBA::Any a;
    a <<= static_cast<CORBA::Long>(7);
    check(!echo->check("count", a), "server says yes");
  });
  r.run("check_tc_unaliased",
        [&]() { check(!echo->check_tc("count", CORBA::_tc_long), "server says yes"); });
  r.run("out_inout_wrong", [&]() {
    CORBA::Any_var b;
    CORBA::Any_var c = make("long", echo);
    try {
      echo->out_inout("point", b.out(), c.inout());
    } catch (CORBA::BAD_PARAM&) {
      return;
    }
    check(false, "no BAD_PARAM");
  });
}
#endif

}  // namespace anytest

#endif
