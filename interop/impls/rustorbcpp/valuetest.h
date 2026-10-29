// The valuetype tests (idl/ValueTest.idl) on the rustorb C++ binding: the
// ValueEcho servant and the client cases. The cases are those of
// impls/omniorbpy/valuetest.py (and of impls/omniorb-cpp/valuetest.h, in
// the IDL to C++11 mapping).

#ifndef INTEROP_RUSTORBCPP_VALUETEST_H
#define INTEROP_RUSTORBCPP_VALUETEST_H

#include <functional>
#include <set>
#include <string>

#include "ValueTest.hh"

namespace valuetest {

using namespace ValueTest;

template <typename T> using ref = typename IDL::traits<T>::ref_type;

// The generated OBV_ classes are concrete, and generated factories make
// them; Point3Impl only makes the private member readable, so it gets a
// factory of its own.
class Point3Impl : public OBV_ValueTest::Point3 {
 public:
  Point3Impl() = default;
  Point3Impl(int32_t x, int32_t y, int32_t z, const std::string& label)
      : OBV_ValueTest::Point3(x, y, z, label) {}
  std::string get_label() const { return label(); }
};

class Point3Factory : public CORBA::ValueFactoryBase {
 public:
  ref<CORBA::ValueBase> create_for_unmarshal() override {
    return CORBA::make_reference<Point3Impl>();
  }
};

inline void register_factories(ref<CORBA::ORB> orb) {
  orb->register_value_factory(Point3::_rorb_repository_id,
                              CORBA::make_reference<Point3Factory>());
}

class ValueEchoImpl : public CORBA::servant_traits<ValueEcho>::base_type {
 public:
  explicit ValueEchoImpl(ref<CORBA::ORB> orb) : orb_(std::move(orb)) {}

  ref<Point> echo_point(ref<Point> p) override { return p; }
  Pair echo_pair(const Pair& p) override { return p; }
  ref<Node> echo_node(ref<Node> n) override { return n; }
  ref<Shape> echo_shape(ref<Shape> s) override { return s; }
  PointSeq echo_points(const PointSeq& s) override { return s; }
  bool same(ref<Point> a, ref<Point> b) override { return a == b; }
  int32_t count_nodes(ref<Node> n) override {
    std::set<Node*> seen;
    for (Node* m = n ? &*n : nullptr; m && seen.insert(m).second;) {
      m = m->next() ? &*m->next() : nullptr;
    }
    return static_cast<int32_t>(seen.size());
  }
  ref<LongBox> echo_long_box(ref<LongBox> b) override { return b; }
  ref<StringBox> echo_string_box(ref<StringBox> b) override { return b; }
  ref<PairBox> echo_pair_box(ref<PairBox> b) override { return b; }
  CORBA::Any echo_any(const CORBA::Any& a) override { return a; }
  void shutdown() override { orb_->shutdown(false); }

 private:
  ref<CORBA::ORB> orb_;
};

inline ref<Point> point(int32_t x, int32_t y) {
  return CORBA::make_reference<OBV_ValueTest::Point>(x, y);
}

inline ref<Point> point3() { return CORBA::make_reference<Point3Impl>(1, 2, 3, "three"); }

inline ref<Node> node(int32_t id, ref<Node> next) {
  return CORBA::make_reference<OBV_ValueTest::Node>(id, next);
}

// Two nodes that refer to each other (never freed: the cycle keeps them).
inline ref<Node> ring() {
  auto a = node(1, nullptr);
  a->next(node(2, a));
  return a;
}

// A list of n nodes, ids 0 to n - 1.
inline ref<Node> chain(int n) {
  ref<Node> head;
  for (int i = n - 1; i >= 0; --i) head = node(i, head);
  return head;
}

inline bool is_point(const ref<Point>& p, int32_t x, int32_t y) {
  return p && !IDL::traits<Point3>::narrow(p) && p->x() == x && p->y() == y;
}

inline bool is_point3(const ref<Point>& p) {
  auto q = std::dynamic_pointer_cast<Point3Impl>(p._rorb_shared());
  return q && q->x() == 1 && q->y() == 2 && q->z() == 3 && q->get_label() == "three";
}

// Runs the valuetype cases; `run(name, fn)` reports a case, `check(cond,
// what)` fails one.
template <typename Runner>
void run_cases(Runner& r, ref<ValueEcho> echo,
               const std::function<void(bool, const std::string&)>& check) {
  r.run("echo_point[plain]", [&]() { check(is_point(echo->echo_point(point(1, -2)), 1, -2), "wrong point"); });
  r.run("echo_point[null]", [&]() { check(echo->echo_point(nullptr) == nullptr, "not null"); });
  r.run("echo_point[derived]", [&]() { check(is_point3(echo->echo_point(point3())), "wrong point3"); });
  r.run("echo_pair[shared]", [&]() {
    auto p = point(5, 6);
    Pair got = echo->echo_pair(Pair(p, p));
    check(is_point(got.first(), 5, 6), "wrong point");
    check(got.first() == got.second(), "not one instance");
  });
  r.run("echo_pair[distinct]", [&]() {
    Pair got = echo->echo_pair(Pair(point(5, 6), point(5, 6)));
    check(is_point(got.first(), 5, 6) && is_point(got.second(), 5, 6), "wrong points");
    check(got.first() != got.second(), "one instance");
  });
  r.run("echo_pair[null]", [&]() {
    Pair got = echo->echo_pair(Pair(nullptr, point3()));
    check(got.first() == nullptr, "not null");
    check(is_point3(got.second()), "wrong point3");
  });
  r.run("same[shared]", [&]() {
    auto p = point(0, 0);
    check(echo->same(p, p), "not the same");
  });
  r.run("same[distinct]", [&]() { check(!echo->same(point(0, 0), point(0, 0)), "the same"); });
  r.run("echo_node[list]", [&]() {
    auto got = echo->echo_node(chain(20));
    int i = 0;
    for (auto m = got; m; m = m->next(), ++i) check(m->id() == i, "wrong id");
    check(i == 20, "wrong length");
  });
  r.run("echo_node[ring]", [&]() {
    auto n = ring();
    auto got = echo->echo_node(n);
    check(got->id() == 1 && got->next()->id() == 2, "wrong ids");
    check(got->next()->next() == got, "the cycle is lost");
    got->next(nullptr);
    n->next(nullptr);
  });
  r.run("count_nodes[list]", [&]() { check(echo->count_nodes(chain(20)) == 20, "wrong count"); });
  r.run("count_nodes[ring]", [&]() {
    auto n = ring();
    check(echo->count_nodes(n) == 2, "wrong count");
    n->next(nullptr);
  });
  r.run("echo_shape", [&]() {
    auto c = point(0, 0);
    auto shape = CORBA::make_reference<OBV_ValueTest::Shape>("square", PointSeq{point(1, 1), c}, c, 1.5);
    auto got = echo->echo_shape(shape);
    check(got->name() == "square" && got->area() == 1.5, "wrong name or area");
    check(got->points().size() == 2, "wrong length");
    check(is_point(got->points()[0], 1, 1) && is_point(got->center(), 0, 0), "wrong points");
    check(got->points()[1] == got->center(), "not one instance");
  });
  r.run("echo_points", [&]() {
    auto p = point(7, 8);
    auto q = point3();
    PointSeq got = echo->echo_points(PointSeq{p, q, p, nullptr, q});
    check(got.size() == 5, "wrong length");
    check(is_point(got[0], 7, 8) && is_point3(got[1]), "wrong points");
    check(got[0] == got[2] && got[1] == got[4], "sharing lost");
    check(got[3] == nullptr, "not null");
  });
  r.run("echo_long_box[value]", [&]() {
    auto got = echo->echo_long_box(CORBA::make_reference<LongBox>(7));
    check(got && got->_value() == 7, "wrong box");
  });
  r.run("echo_long_box[null]", [&]() { check(echo->echo_long_box(nullptr) == nullptr, "not null"); });
  r.run("echo_string_box[value]", [&]() {
    auto got = echo->echo_string_box(CORBA::make_reference<StringBox>("boxed"));
    check(got && got->_value() == "boxed", "wrong box");
  });
  r.run("echo_string_box[null]", [&]() {
    check(echo->echo_string_box(nullptr) == nullptr, "not null");
  });
  r.run("echo_pair_box", [&]() {
    auto got = echo->echo_pair_box(CORBA::make_reference<PairBox>(Pair(point(1, 2), nullptr)));
    check(got != nullptr, "null");
    check(is_point(got->_value().first(), 1, 2), "wrong point");
    check(got->_value().second() == nullptr, "not null");
  });
  r.run("echo_any[ring]", [&]() {
    auto n = ring();
    CORBA::Any a;
    a <<= n;
    CORBA::Any got = echo->echo_any(a);
    check(got.type()->equal(_tc_Node), "wrong TypeCode");
    ref<Node> m;
    check(got >>= m, "not a Node");
    check(m && m->id() == 1 && m->next()->id() == 2 && m->next()->next() == m, "wrong ring");
    if (m) m->next(nullptr);
    n->next(nullptr);
  });
  r.run("echo_any[derived]", [&]() {
    CORBA::Any a;
    a <<= point3();
    CORBA::Any got = echo->echo_any(a);
    check(got.type()->equal(_tc_Point), "wrong TypeCode");
    ref<Point> q;
    check(got >>= q, "not a Point");
    check(is_point3(q), "wrong point3");
  });
  r.run("echo_any[box]", [&]() {
    CORBA::Any a;
    a <<= CORBA::make_reference<StringBox>("in an any");
    CORBA::Any got = echo->echo_any(a);
    check(got.type()->equal(_tc_StringBox), "wrong TypeCode");
    ref<StringBox> c;
    check(got >>= c, "not a StringBox");
    check(c && c->_value() == "in an any", "wrong box");
  });
}

}  // namespace valuetest

#endif
