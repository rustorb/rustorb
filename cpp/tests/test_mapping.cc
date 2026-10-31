// The IDL to C++11 mapping of rustorb-idl -bc++11 (cpp/tests/idl/
// CxxMapping.idl): constants, the generated types, and every type of the
// file through stubs and skeletons, between two ORBs in one process.

#include <cstdint>
#include <iostream>
#include <limits>
#include <sstream>
#include <string>
#include <thread>
#include <type_traits>

#include "CxxMapping.hh"

static int failures = 0;

#define CHECK(cond)                                                              \
  do {                                                                           \
    if (!(cond)) {                                                               \
      std::cout << "FAIL line " << __LINE__ << ": " #cond << std::endl;          \
      ++failures;                                                                \
    }                                                                            \
  } while (0)

template <typename E, typename F> static bool raises(F fn) {
  try {
    fn();
  } catch (const E&) {
    return true;
  } catch (const CORBA::Exception& e) {
    std::cout << "unexpected exception: " << e << std::endl;
  }
  return false;
}

using namespace CxxMapping;

// In a servant class, the names of the interface and its bases find the
// skeleton classes (injected class names), so the types are qualified.

class LaterImpl : public CORBA::servant_traits<Later>::base_type {
 public:
  explicit LaterImpl(int n) : n_(n) {}
  IDL::traits<CxxMapping::Later>::ref_type next() override {
    if (n_ == 0) return nullptr;
    return CORBA::make_reference<LaterImpl>(n_ - 1)->_this();
  }

 private:
  int n_;
};

class TypesImpl : public CORBA::servant_traits<Types>::base_type {
 public:
  Tree echo_tree(const Tree& t) override { return t; }
  Node echo_node(const Node& n) override { return n; }
  ByBool echo_by_bool(const ByBool& u) override { return u; }
  ByChar echo_by_char(const ByChar& u) override { return u; }
  ByEnum echo_by_enum(const ByEnum& u) override { return u; }
  ByULongLong echo_by_ulonglong(const ByULongLong& u) override { return u; }
  WithArray echo_with_array(const WithArray& u) override { return u; }
  Holder echo_holder(const Holder& h) override { return h; }
  Outer echo_outer(const Outer& o) override { return o; }
  Keywords echo_keywords(const Keywords& k) override { return k; }
  GlobalPoint echo_global(const GlobalPoint& p) override { return p; }
  ShortString echo_short_string(const ShortString& s) override {
    // Longer than the bound: the reply cannot be marshalled.
    if (s == "grow") return ShortString("much too long");
    return s;
  }
  Money echo_money(const Money& m, Money& total, IDL::Fixed<3, 0>& whole) override {
    if (total != Money(1)) throw CORBA::BAD_PARAM(0, CORBA::CompletionStatus::COMPLETED_NO);
    total = m;
    std::string text = m.to_string();
    whole = IDL::Fixed<3, 0>(text.substr(0, text.find('.')));
    return m;
  }
  Price echo_price(const Price& p) override { return p; }
  Quad echo_quad(Quad q, long double& twice, long double& negated) override {
    twice = twice * 2;
    negated = -q;
    return q;
  }
  Measure echo_measure(const Measure& m) override { return m; }
  void fail(int32_t code) override {
    if (code == 0) throw Empty();
    if (code < 0) throw CORBA::NO_PERMISSION(42, CORBA::CompletionStatus::COMPLETED_YES);
    throw Failure(code, "failed", Fruit::CHERRY);
  }
  LaterSeq laters(int32_t n) override {
    LaterSeq seq;
    for (int32_t i = 0; i < n; ++i) seq.push_back(CORBA::make_reference<LaterImpl>(i)->_this());
    return seq;
  }
  void _cxx_delete(const std::string& what) override { deleted = what; }
  std::string deleted;
};

class MultiImpl : public CORBA::servant_traits<Multi>::base_type {
 public:
  int32_t id() override { return 5; }
  std::string label() override { return label_; }
  void label(const std::string& v) override { label_ = v; }
  void base_op(bool fail) override {
    if (fail) throw CxxMapping::Base::BaseError("because");
  }
  int32_t derived_op(int32_t a, int32_t& b, int32_t& c) override {
    if (a < 0) throw Failure(a, "negative", Fruit::APPLE);
    c = b * 2;
    b = b + a;
    return a + 1;
  }
  void notify(const std::string& msg) override { label_ = msg; }
  IDL::traits<CxxMapping::Base>::ref_type as_base() override { return _this(); }
  std::string other_op() override { return "other"; }
  CxxMapping::Base::BaseInfo multi_op(const CxxMapping::Base::BaseInfo& info) override {
    return CxxMapping::Base::BaseInfo(info.id() + 1);
  }

 private:
  std::string label_ = "start";
};

static void check_constants() {
  CHECK(GLOBAL_LIMIT == 10);
  CHECK(C_SHORT == -3);
  CHECK(C_ULONGLONG == 18000000000000000000ULL);
  CHECK(C_DOUBLE == 0.25);
  CHECK(C_TRUE);
  CHECK(C_CHAR == 'q');
  CHECK(C_WCHAR == L'w');
  CHECK(std::string(C_STRING) == "abcd");
  CHECK(std::wstring(C_WSTRING) == L"wide");
  CHECK(C_OCTET == 255);
  CHECK(C_EXPR == (1 << 21) - 3);
  CHECK(C_ENUM == Fruit::BANANA);
  CHECK(Base::BASE_CONST == 7);
  CHECK(REOPENED == 1);
  static_assert(std::is_same<std::underlying_type<Fruit>::type, uint32_t>::value, "enum");
  static_assert(std::is_same<Count, int32_t>::value, "typedef");
  static_assert(ShortString::bound() == 8, "bounded string");
  static_assert(std::tuple_size<Grid>::value == 2, "array");
}

static void check_fixed() {
  CHECK(C_FIXED.to_string() == "12.50");
  static_assert(decltype(C_FIXED)::fixed_digits() == 4 && decltype(C_FIXED)::fixed_scale() == 2,
                "fixed constant");
  CHECK(C_MONEY.to_string() == "-1.50");
  static_assert(std::is_same<Money, IDL::Fixed<5, 2>>::value, "fixed typedef");
  CHECK(Money().to_string() == "0.00");
  CHECK(Money(7).to_string() == "7.00");
  CHECK(Money(-999).to_string() == "-999.00");
  CHECK(Money("0.05").to_string() == "0.05");
  CHECK(Money("+.5").to_string() == "0.50");
  CHECK(Money("12d").to_string() == "12.00");
  CHECK(Money("-0.00").to_string() == "0.00");
  CHECK(Money("-0") == Money(0));
  CHECK(Money("001.230").to_string() == "1.23");
  CHECK(Money(2.675).to_string() == "2.67" || Money(2.675).to_string() == "2.68");
  CHECK(Money(-3.5).to_string() == "-3.50");
  CHECK(static_cast<double>(Money("-12.25")) == -12.25);
  CHECK((IDL::Fixed<3, 3>("0.125").to_string() == "0.125"));
  CHECK((IDL::Fixed<2, 0>(99).to_string() == "99"));
  CHECK((IDL::Fixed<6, 3>(Money("1.5")).to_string() == "1.500"));
  for (const char* bad : {"1000", "1.234", "", ".", "-", "1.2.3", "1e3", "12x"}) {
    CHECK(raises<CORBA::DATA_CONVERSION>([&] { Money m(bad); (void)m; }));
  }
  CHECK(raises<CORBA::DATA_CONVERSION>([] { Money m(1000); (void)m; }));
  CHECK(raises<CORBA::DATA_CONVERSION>([] { Money m(1e300); (void)m; }));
  CHECK(raises<CORBA::DATA_CONVERSION>([] { Money m(std::numeric_limits<double>::infinity()); (void)m; }));
  CHECK(raises<CORBA::DATA_CONVERSION>([] { IDL::Fixed<2, 1> f(Money("12.5")); (void)f; }));
  CHECK(Money("-1") < Money("0.01"));
  CHECK(Money("-2") < Money("-1"));
  CHECK(Money("1.5") > Money("1.49"));
  CHECK(Money("1.5") >= Money("1.50") && Money("1.5") <= Money("1.50"));
  CHECK(-Money("1.5") == Money("-1.5"));
  CHECK(-Money(0) == Money(0));
  std::ostringstream os;
  os << Money("-4.2");
  CHECK(os.str() == "-4.20");

  // Arithmetic: the result types of the IDL spec, cut to 31 digits.
  using F31 = IDL::Fixed<31, 0>;
  using Tiny = IDL::Fixed<31, 31>;
  static_assert(std::is_same<decltype(Money() + Money()), IDL::Fixed<6, 2>>::value, "+");
  static_assert(std::is_same<decltype(Money() - IDL::Fixed<3, 3>()), IDL::Fixed<7, 3>>::value, "-");
  static_assert(std::is_same<decltype(Money() * IDL::Fixed<4, 1>()), IDL::Fixed<9, 3>>::value, "*");
  static_assert(std::is_same<decltype(Money() / IDL::Fixed<4, 1>()), IDL::Fixed<31, 27>>::value, "/");
  static_assert(std::is_same<decltype(F31() + F31()), F31>::value, "+ over 31");
  static_assert(std::is_same<decltype(Tiny() + Tiny()), IDL::Fixed<31, 30>>::value, "+ cuts fraction");
  static_assert(std::is_same<decltype(F31() * Tiny()), IDL::Fixed<31, 0>>::value, "* cuts fraction");
  static_assert(std::is_same<decltype(Money() + 1), IDL::Fixed<23, 2>>::value, "integer operand");
  CHECK((Money("999.99") + Money("0.01")).to_string() == "1000.00");
  CHECK((Money("1.25") + Money("-3.5")).to_string() == "-2.25");
  CHECK((Money("-1.25") + Money("1.25")).to_string() == "0.00");
  CHECK((Money("1.25") - Money("-3.5")).to_string() == "4.75");
  CHECK((Money("1.5") - IDL::Fixed<3, 3>("0.125")).to_string() == "1.375");
  CHECK((Money("-12.5") * IDL::Fixed<4, 1>("-0.3")).to_string() == "3.750");
  CHECK((Money("999.99") * Money("999.99")).to_string() == "999980.0001");
  CHECK((Money("1") / Money("3")).to_string() == "0." + std::string(26, '3'));
  CHECK((Money("-2") / Money("3")).to_string() == "-0." + std::string(26, '6'));
  CHECK((Money("10") / IDL::Fixed<3, 3>("0.004")).to_string().substr(0, 9) == "2500.0000");
  CHECK((Money("1") / Money("-0.01") == IDL::Fixed<31, 26>(-100)));
  CHECK(raises<CORBA::DATA_CONVERSION>([] { (void)(Money(1) / Money(0)); }));
  CHECK((Money("2.5") * 2).to_string() == "5.00");
  CHECK((7 - Money("2.5")).to_string() == "4.50");
  CHECK((Money("7") / 2).to_string().substr(0, 6) == "3.5000");
  F31 big("9999999999999999999999999999999");
  CHECK(raises<CORBA::DATA_CONVERSION>([&] { (void)(big + F31(1)); }));
  CHECK(raises<CORBA::DATA_CONVERSION>([&] { (void)(big * F31(10)); }));
  CHECK((big - big) == F31(0));
  CHECK((Tiny("0.0000000000000000000000000000001") + Tiny("0.0000000000000000000000000000009"))
            .to_string() == "0.000000000000000000000000000001");

  // Compound assignment keeps the type and cuts the fraction.
  Money m("10.00");
  m += Money("0.5");
  CHECK(m.to_string() == "10.50");
  m -= 1;
  CHECK(m.to_string() == "9.50");
  m *= IDL::Fixed<3, 3>("0.333");
  CHECK(m.to_string() == "3.16");
  m /= 3;
  CHECK(m.to_string() == "1.05");
  CHECK((++m).to_string() == "2.05" && (m--).to_string() == "2.05" && m.to_string() == "1.05");
  Money full("999.00");
  CHECK(raises<CORBA::DATA_CONVERSION>([&] { full += 1; }));
  CHECK(full.to_string() == "999.00");

  // round (half away from zero) and truncate.
  CHECK(Money("1.25").round(1).to_string() == "1.30");
  CHECK(Money("1.24").round(1).to_string() == "1.20");
  CHECK(Money("-1.25").round(1).to_string() == "-1.30");
  CHECK(Money("-1.25").truncate(1).to_string() == "-1.20");
  CHECK(Money("-0.05").truncate(1).to_string() == "0.00");
  CHECK(Money("-0.05").round(1).to_string() == "-0.10");
  CHECK(Money("9.99").round(0).to_string() == "10.00");
  CHECK(Money("1.99").round(2) == Money("1.99"));
  CHECK(Money("1.99").truncate(5) == Money("1.99"));
  CHECK(raises<CORBA::DATA_CONVERSION>([] { (void)Money("999.50").round(0); }));
  CHECK(raises<CORBA::DATA_CONVERSION>([] { (void)IDL::Fixed<2, 2>("0.5").round(0); }));

  CORBA::Any a;
  a <<= Money("123.45");
  CHECK(a.type()->kind() == CORBA::TCKind::tk_fixed);
  CHECK(a.type()->fixed_digits() == 5 && a.type()->fixed_scale() == 2);
  CHECK(a.type()->equal(_tc_Money->content_type()));
  Money back;
  CHECK(a >>= back);
  CHECK(back == Money("123.45"));
  IDL::Fixed<5, 1> other;
  CHECK(!(a >>= other));
  Price p(Money("-0.99"), IDL::Fixed<31, 31>("0.0000000000000000000000000000001"), {Money(1), Money("-2.5")});
  CORBA::Any pa;
  pa <<= p;
  Price p2;
  CHECK(pa >>= p2);
  CHECK(p2 == p);
}

// long double <-> binary128 bits, whatever the format of long double.
static void check_long_double() {
  namespace ld = rustorb::cdr::long_double;
  auto bits = [](long double v) { return ld::to_bits(v); };
  auto same = [](ld::u128 a, uint64_t hi, uint64_t lo) { return a.hi == hi && a.lo == lo; };
  CHECK(same(bits(1.0L), 0x3fff000000000000u, 0));
  CHECK(same(bits(-2.0L), 0xc000000000000000u, 0));
  CHECK(same(bits(1.5L), 0x3fff800000000000u, 0));
  CHECK(same(bits(0.0L), 0, 0));
  CHECK(same(bits(-0.0L), 0x8000000000000000u, 0));
  CHECK(same(bits(std::numeric_limits<long double>::infinity()), 0x7fff000000000000u, 0));
  CHECK(std::isnan(ld::from_bits(bits(std::numeric_limits<long double>::quiet_NaN()))));
  CHECK(std::signbit(ld::from_bits({0x8000000000000000u, 0})));
  // 0.1 as a double is exact in binary128.
  CHECK(same(bits(static_cast<long double>(0.1)), 0x3ffb999999999999u, 0xa000000000000000u));
  for (long double v : {1.0L, -1.0L, 0.1L, 1e300L, -1e-300L, 3.141592653589793238462643383279L,
                        std::numeric_limits<long double>::max(),
                        std::numeric_limits<long double>::min(),
                        std::numeric_limits<long double>::denorm_min(),
                        std::numeric_limits<long double>::epsilon()}) {
    CHECK(ld::from_bits(bits(v)) == v);
  }
  // Rounding of the 112 fraction bits to the digits of long double.
  if (std::numeric_limits<long double>::digits < 113) {
    int drop = 113 - std::numeric_limits<long double>::digits;  // dropped bits
    auto with_bit = [](int n) {  // 1 + 2^-(112 - n)
      return n >= 64 ? ld::u128{0x3fff000000000000u | (uint64_t{1} << (n - 64)), 0}
                     : ld::u128{0x3fff000000000000u, uint64_t{1} << n};
    };
    long double ulp = std::numeric_limits<long double>::epsilon();
    // Half an ulp above 1: ties to even, 1.
    CHECK(ld::from_bits(with_bit(drop - 1)) == 1.0L);
    // One ulp above 1 is exact.
    CHECK(ld::from_bits(with_bit(drop)) == 1.0L + ulp);
    // All fraction bits set: rounds up to 2.
    CHECK(ld::from_bits({0x3fffffffffffffffu, ~uint64_t{0}}) == 2.0L);
  }
  // Out of range for a long double that is a double: infinity and zero.
  if (std::numeric_limits<long double>::max_exponent < 16384) {
    CHECK(std::isinf(ld::from_bits({0x7ffe000000000000u, 0})));
    CHECK(ld::from_bits({0x0001000000000000u, 0}) == 0.0L);
  }
  CHECK(C_QUAD == 2.5L);
  static_assert(std::is_same<Quad, long double>::value, "long double typedef");
  CORBA::Any a;
  a <<= 0.25L;
  CHECK(a.type()->kind() == CORBA::TCKind::tk_longdouble);
  long double back = 0;
  CHECK(a >>= back);
  CHECK(back == 0.25L);
  double d = 0;
  CHECK(!(a >>= d));
  Measure m(-7.5L, {1.0L, 0.1L, -1e100L});
  CORBA::Any ma;
  ma <<= m;
  Measure m2;
  CHECK(ma >>= m2);
  CHECK(m2 == m);
}

static void check_unions() {
  ByEnum e;
  CHECK(e._d() == Fruit::BANANA);  // the default member
  e.cherry_name("bing");
  CHECK(e._d() == Fruit::CHERRY);
  CHECK(e.cherry_name() == "bing");
  CHECK(raises<CORBA::BAD_PARAM>([&] { (void)e.apple_count(); }));
  CHECK(raises<CORBA::BAD_PARAM>([&] { e._d(Fruit::APPLE); }));
  ByChar c;
  c.bc("x", 'c');
  CHECK(c._d() == 'c');
  c._d('b');  // the same member
  CHECK(c._d() == 'b');
  ByULongLong u;
  u._default();
  CHECK(u._d() != 7);
  CHECK(raises<CORBA::BAD_PARAM>([&] { (void)u.seven(); }));
}

static void check_calls(IDL::traits<CORBA::ORB>::ref_type orb, const std::string& types_ior,
                        const std::string& multi_ior, TypesImpl& types_servant) {
  auto types = IDL::traits<Types>::narrow(orb->string_to_object(types_ior));
  CHECK(types != nullptr);

  Tree t("root", {Tree("a", {}), Tree("b", {Tree("c", {})})});
  CHECK(types->echo_tree(t) == t);

  Node leaf;
  leaf.leaf(3);
  Node branch;
  branch.branch({leaf, leaf});
  Node n;
  n.branch({branch, leaf});
  CHECK(types->echo_node(n) == n);

  ByBool bb;
  bb.no("nope");
  CHECK(types->echo_by_bool(bb) == bb);
  bb.yes(9);
  CHECK(types->echo_by_bool(bb) == bb);

  ByChar bc;
  bc.a(1);
  CHECK(types->echo_by_char(bc) == bc);
  bc.bc("bc", 'b');
  CHECK(types->echo_by_char(bc).bc() == "bc");
  CHECK(types->echo_by_char(bc)._d() == 'b');

  for (Fruit f : {Fruit::APPLE, Fruit::BANANA, Fruit::CHERRY}) {
    ByEnum be;
    if (f == Fruit::APPLE) be.apple_count(4);
    if (f == Fruit::BANANA) be.other(true);
    if (f == Fruit::CHERRY) be.cherry_name("red");
    CHECK(types->echo_by_enum(be) == be);
  }

  ByULongLong bu;
  bu.seven(7.5);
  CHECK(types->echo_by_ulonglong(bu) == bu);
  bu._default();
  CHECK(types->echo_by_ulonglong(bu) == bu);

  WithArray wa;
  wa.cells(Grid{{{{1, 2, 3}}, {{4, 5, 6}}}});
  CHECK(types->echo_with_array(wa) == wa);
  wa.two(Pair(1, 2));
  CHECK(types->echo_with_array(wa).two().second() == 2);

  Holder h;
  h.numbers({{1, -2, 3}});
  h.pairs({Pair(1, 2), Pair(3, 4)});
  h.cells(Grid{{{{7, 8, 9}}, {{10, 11, 12}}}});
  h.tag("eight ch");
  h.bounded({1, 2, 3, 4, 5});
  h.wide(L"wé");
  h.flags({true, false, true});
  h.wchars({L'a', L'é'});
  h.nested({{"a", "b"}, {}, {"c"}});
  h.target(types);
  h.choice().apple_count(1);
  Holder back = types->echo_holder(h);
  CHECK(back.numbers() == h.numbers());
  CHECK(back.pairs() == h.pairs());
  CHECK(back.cells() == h.cells());
  CHECK(back.tag() == h.tag());
  CHECK(back.bounded() == h.bounded());
  CHECK(back.wide() == h.wide());
  CHECK(back.flags() == h.flags());
  CHECK(back.wchars() == h.wchars());
  CHECK(back.nested() == h.nested());
  CHECK(back.target() && back.target()->_is_equivalent(types));
  CHECK(back.choice() == h.choice());
  h.target(nullptr);
  CHECK(!types->echo_holder(h).target());

  // Bounds are checked when marshalling (BAD_PARAM, as in the Python
  // binding), on either side.
  Holder too_long = h;
  too_long.tag("nine chars");
  CHECK(raises<CORBA::BAD_PARAM>([&] { types->echo_holder(too_long); }));
  Holder too_many = h;
  too_many.bounded().push_back(6);
  CHECK(raises<CORBA::BAD_PARAM>([&] { types->echo_holder(too_many); }));
  CHECK(types->echo_short_string("short") == "short");

  for (const char* text : {"123.45", "-0.5", "0", "999.99", "-999.99", "0.01"}) {
    Money total(1);
    IDL::Fixed<3, 0> whole(5);
    CHECK(types->echo_money(Money(text), total, whole) == Money(text));
    CHECK(total == Money(text));
    std::string s = Money(text).to_string();
    CHECK((whole == IDL::Fixed<3, 0>(s.substr(0, s.find('.')))));
  }
  Price price(Money("-12.34"), IDL::Fixed<31, 31>("0.9999999999999999999999999999999"),
              {Money(1), Money("2.5"), Money("-0.01")});
  CHECK(types->echo_price(price) == price);
  {
    long double twice = 1.25L, negated = 0;
    long double third = 1.0L / 3;
    CHECK(types->echo_quad(third, twice, negated) == third);
    CHECK(twice == 2.5L && negated == -third);
    Measure m(std::numeric_limits<long double>::min(), {third, -0.0L, 1e300L});
    CHECK(types->echo_measure(m) == m);
  }
  try {
    types->echo_short_string("grow");
    CHECK(false);
  } catch (const CORBA::BAD_PARAM& e) {
    CHECK(e.completed() == CORBA::CompletionStatus::COMPLETED_YES);
  }

  Outer o(Outer::Inner(3), Outer::Mode::OFF);
  CHECK(types->echo_outer(o) == o);
  Keywords k(1, "ns");
  CHECK(types->echo_keywords(k)._cxx_namespace() == "ns");
  CHECK(types->echo_global(GlobalPoint(1, 2)) == GlobalPoint(1, 2));

  try {
    types->fail(3);
    CHECK(false);
  } catch (const Failure& f) {
    CHECK(f.code() == 3);
    CHECK(f.message() == "failed");
    CHECK(f.kind() == Fruit::CHERRY);
  }
  CHECK(raises<Empty>([&] { types->fail(0); }));
  try {
    types->fail(-1);
    CHECK(false);
  } catch (const CORBA::NO_PERMISSION& e) {
    CHECK(e.minor() == 42);
    CHECK(e.completed() == CORBA::CompletionStatus::COMPLETED_YES);
  }

  LaterSeq laters = types->laters(3);
  CHECK(laters.size() == 3);
  CHECK(laters[2]->next()->next() != nullptr);
  CHECK(laters[2]->next()->next()->next() == nullptr);

  types->_cxx_delete("it");
  CHECK(types_servant.deleted == "it");

  // Inheritance: narrowing a reference to each base, attributes, inout and
  // out arguments, and oneway.
  auto obj = orb->string_to_object(multi_ior);
  auto multi = IDL::traits<Multi>::narrow(obj);
  CHECK(multi != nullptr);
  CHECK(IDL::traits<Types>::narrow(obj) == nullptr);
  auto base = IDL::traits<Base>::narrow(obj);
  auto other = IDL::traits<Other>::narrow(obj);
  CHECK(base && other);
  CHECK(base->id() == 5);
  base->label("changed");
  CHECK(multi->label() == "changed");
  CHECK(other->other_op() == "other");
  CHECK(raises<Base::BaseError>([&] { base->base_op(true); }));
  base->base_op(false);
  int32_t b = 10, c = 0;
  CHECK(multi->derived_op(1, b, c) == 2);
  CHECK(b == 11 && c == 20);
  CHECK(raises<Failure>([&] { multi->derived_op(-1, b, c); }));
  CHECK(multi->multi_op(Base::BaseInfo(1)).id() == 2);
  CHECK(multi->as_base()->_is_equivalent(multi));
  CHECK(IDL::traits<Multi>::narrow(multi->as_base()) != nullptr);
  multi->notify("posted");
  for (int i = 0; i < 50 && multi->label() != "posted"; ++i)
    std::this_thread::sleep_for(std::chrono::milliseconds(20));
  CHECK(multi->label() == "posted");
  CHECK(multi->_is_a("IDL:rustorb.org/CxxMapping/Other:1.0"));
  CHECK(!multi->_is_a("IDL:rustorb.org/CxxMapping/Types:1.0"));
}

int main() {
  try {
    check_constants();
    check_fixed();
    check_long_double();
    check_unions();

    int sargc = 3;
    char sarg0[] = "server", sarg1[] = "-ORBendPoint", sarg2[] = "giop:tcp:127.0.0.1:";
    char* sargv[] = {sarg0, sarg1, sarg2, nullptr};
    auto server_orb = CORBA::ORB_init(sargc, sargv);
    auto root = IDL::traits<PortableServer::POA>::narrow(
        server_orb->resolve_initial_references("RootPOA"));
    root->the_POAManager()->activate();
    auto types_servant = CORBA::make_reference<TypesImpl>();
    auto multi_servant = CORBA::make_reference<MultiImpl>();
    std::string types_ior = server_orb->object_to_string(types_servant->_this());
    std::string multi_ior = server_orb->object_to_string(multi_servant->_this());
    std::thread server([&] { server_orb->run(); });

    int cargc = 1;
    char carg0[] = "client";
    char* cargv[] = {carg0, nullptr};
    auto client_orb = CORBA::ORB_init(cargc, cargv);
    check_calls(client_orb, types_ior, multi_ior, *types_servant);
    // The same calls on the server ORB: colocated.
    check_calls(server_orb, types_ior, multi_ior, *types_servant);

    server_orb->shutdown(true);
    server.join();
    server_orb->destroy();
    client_orb->destroy();
  } catch (const CORBA::Exception& e) {
    std::cout << "CORBA exception: " << e << std::endl;
    return 1;
  }
  std::cout << (failures ? "FAILED" : "ok") << std::endl;
  return failures ? 1 : 0;
}
