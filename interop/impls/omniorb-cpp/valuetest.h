// The valuetype tests (idl/ValueTest.idl) on omniORB C++: the concrete
// value classes and their factories, the ValueEcho servant and the client
// cases. The cases are those of impls/omniorbpy/valuetest.py.

#ifndef INTEROP_VALUETEST_H
#define INTEROP_VALUETEST_H

#include <functional>
#include <set>
#include <string>

#include "ValueTest.hh"

namespace valuetest {

using namespace ValueTest;

// The generated OBV_ classes of these valuetypes (which have no
// operations) are concrete, with reference counting. Point3Impl only makes
// the private member readable.
using PointImpl = OBV_ValueTest::Point;
using NodeImpl = OBV_ValueTest::Node;
using ShapeImpl = OBV_ValueTest::Shape;

class Point3Impl : public OBV_ValueTest::Point3 {
 public:
  Point3Impl() {}
  Point3Impl(CORBA::Long x, CORBA::Long y, CORBA::Long z, const char* label) {
    this->x(x);
    this->y(y);
    this->z(z);
    this->label(label);
  }
  std::string get_label() { return label(); }
};

template <typename T>
class Factory : public CORBA::ValueFactoryBase {
 public:
  CORBA::ValueBase* create_for_unmarshal() override { return new T; }
};

// Registers the factories of the valuetypes of ValueTest.idl.
inline void register_factories(CORBA::ORB_ptr orb) {
  auto reg = [&](const char* id, CORBA::ValueFactoryBase* f) {
    CORBA::ValueFactory old = orb->register_value_factory(id, f);
    if (old) old->_remove_ref();
    f->_remove_ref();
  };
  reg("IDL:ValueTest/Point:1.0", new Factory<PointImpl>);
  reg("IDL:ValueTest/Point3:1.0", new Factory<Point3Impl>);
  reg("IDL:ValueTest/Node:1.0", new Factory<NodeImpl>);
  reg("IDL:ValueTest/Shape:1.0", new Factory<ShapeImpl>);
}

inline Point* add_ref(Point* p) {
  if (p) p->_add_ref();
  return p;
}

class ValueEchoImpl : public POA_ValueTest::ValueEcho {
 public:
  explicit ValueEchoImpl(CORBA::ORB_ptr orb) : orb_(CORBA::ORB::_duplicate(orb)) {}

  Point* echo_point(Point* p) override { return add_ref(p); }
  Pair* echo_pair(const Pair& p) override { return new Pair(p); }
  Node* echo_node(Node* n) override {
    if (n) n->_add_ref();
    return n;
  }
  Shape* echo_shape(Shape* s) override {
    if (s) s->_add_ref();
    return s;
  }
  PointSeq* echo_points(const PointSeq& s) override { return new PointSeq(s); }
  CORBA::Boolean same(Point* a, Point* b) override { return a == b; }
  CORBA::Long count_nodes(Node* n) override {
    std::set<Node*> seen;
    while (n && seen.insert(n).second) n = n->next();
    return static_cast<CORBA::Long>(seen.size());
  }
  LongBox* echo_long_box(LongBox* b) override {
    if (b) b->_add_ref();
    return b;
  }
  StringBox* echo_string_box(StringBox* b) override {
    if (b) b->_add_ref();
    return b;
  }
  PairBox* echo_pair_box(PairBox* b) override {
    if (b) b->_add_ref();
    return b;
  }
  CORBA::Any* echo_any(const CORBA::Any& a) override { return new CORBA::Any(a); }
  void shutdown() override { orb_->shutdown(false); }

 private:
  CORBA::ORB_var orb_;
};

// Two nodes that refer to each other (never freed).
inline Node* ring() {
  Node* a = new NodeImpl;
  Node* b = new NodeImpl;
  a->id(1);
  a->next(b);
  b->id(2);
  b->next(a);
  b->_remove_ref();
  return a;
}

// A list of n nodes, ids 0 to n - 1.
inline Node* chain(int n) {
  Node_var head;
  for (int i = n - 1; i >= 0; --i) {
    Node* node = new NodeImpl;
    node->id(i);
    node->next(head.in());
    head = node;
  }
  return head._retn();
}

inline bool is_point(Point* p, CORBA::Long x, CORBA::Long y) {
  return p && !Point3::_downcast(p) && p->x() == x && p->y() == y;
}

inline bool is_point3(Point* p) {
  Point3Impl* q = dynamic_cast<Point3Impl*>(p);
  return q && q->x() == 1 && q->y() == 2 && q->z() == 3 && q->get_label() == "three";
}

// Runs the valuetype cases; `run(name, fn)` reports a case, `check(cond,
// what)` fails one.
template <typename Runner>
void run_cases(Runner& r, ValueEcho_ptr echo,
               const std::function<void(bool, const std::string&)>& check) {
  r.run("echo_point[plain]", [&]() {
    Point_var p = new PointImpl(1, -2);
    Point_var got = echo->echo_point(p);
    check(is_point(got, 1, -2), "wrong point");
  });
  r.run("echo_point[null]", [&]() {
    Point_var got = echo->echo_point(nullptr);
    check(got.in() == nullptr, "not null");
  });
  r.run("echo_point[derived]", [&]() {
    Point_var p = new Point3Impl(1, 2, 3, "three");
    Point_var got = echo->echo_point(p);
    check(is_point3(got), "wrong point3");
  });
  r.run("echo_pair[shared]", [&]() {
    Pair pair;
    pair.first = new PointImpl(5, 6);
    pair.second = add_ref(pair.first.in());
    Pair_var got = echo->echo_pair(pair);
    check(is_point(got->first, 5, 6), "wrong point");
    check(got->first.in() == got->second.in(), "not one instance");
  });
  r.run("echo_pair[distinct]", [&]() {
    Pair pair;
    pair.first = new PointImpl(5, 6);
    pair.second = new PointImpl(5, 6);
    Pair_var got = echo->echo_pair(pair);
    check(is_point(got->first, 5, 6) && is_point(got->second, 5, 6), "wrong points");
    check(got->first.in() != got->second.in(), "one instance");
  });
  r.run("echo_pair[null]", [&]() {
    Pair pair;
    pair.second = new Point3Impl(1, 2, 3, "three");
    Pair_var got = echo->echo_pair(pair);
    check(got->first.in() == nullptr, "not null");
    check(is_point3(got->second), "wrong point3");
  });
  r.run("same[shared]", [&]() {
    Point_var p = new PointImpl(0, 0);
    check(echo->same(p, p), "not the same");
  });
  r.run("same[distinct]", [&]() {
    Point_var p = new PointImpl(0, 0);
    Point_var q = new PointImpl(0, 0);
    check(!echo->same(p, q), "the same");
  });
  r.run("echo_node[list]", [&]() {
    Node_var n = chain(20);
    Node_var got = echo->echo_node(n);
    int i = 0;
    for (Node* m = got; m; m = m->next(), ++i) check(m->id() == i, "wrong id");
    check(i == 20, "wrong length");
  });
  r.run("echo_node[ring]", [&]() {
    Node_var n = ring();
    Node_var got = echo->echo_node(n);
    check(got->id() == 1 && got->next()->id() == 2, "wrong ids");
    check(got->next()->next() == got.in(), "the cycle is lost");
  });
  r.run("count_nodes[list]", [&]() {
    Node_var n = chain(20);
    check(echo->count_nodes(n) == 20, "wrong count");
  });
  r.run("count_nodes[ring]", [&]() {
    Node_var n = ring();
    check(echo->count_nodes(n) == 2, "wrong count");
  });
  r.run("echo_shape", [&]() {
    Point_var c = new PointImpl(0, 0);
    ShapeImpl* s = new ShapeImpl;
    Shape_var shape = s;
    s->name("square");
    PointSeq points;
    points.length(2);
    points[0] = new PointImpl(1, 1);
    points[1] = add_ref(c.in());
    s->points(points);
    s->center(c);
    s->area(1.5);
    Shape_var got = echo->echo_shape(shape);
    check(std::string(got->name()) == "square" && got->area() == 1.5, "wrong name or area");
    check(got->points().length() == 2, "wrong length");
    check(is_point(got->points()[0], 1, 1) && is_point(got->center(), 0, 0), "wrong points");
    check(got->points()[1].in() == got->center(), "not one instance");
  });
  r.run("echo_points", [&]() {
    Point_var p = new PointImpl(7, 8);
    Point_var q = new Point3Impl(1, 2, 3, "three");
    PointSeq s;
    s.length(5);
    s[0] = add_ref(p.in());
    s[1] = add_ref(q.in());
    s[2] = add_ref(p.in());
    s[4] = add_ref(q.in());
    PointSeq_var got = echo->echo_points(s);
    check(got->length() == 5, "wrong length");
    check(is_point(got[0], 7, 8) && is_point3(got[1]), "wrong points");
    check(got[0].in() == got[2].in() && got[1].in() == got[4].in(), "sharing lost");
    check(got[3].in() == nullptr, "not null");
  });
  r.run("echo_long_box[value]", [&]() {
    LongBox_var b = new LongBox(7);
    LongBox_var got = echo->echo_long_box(b);
    check(got.in() && got->_value() == 7, "wrong box");
  });
  r.run("echo_long_box[null]", [&]() {
    LongBox_var got = echo->echo_long_box(nullptr);
    check(got.in() == nullptr, "not null");
  });
  r.run("echo_string_box[value]", [&]() {
    StringBox_var b = new StringBox("boxed");
    StringBox_var got = echo->echo_string_box(b);
    check(got.in() && std::string(got->_value()) == "boxed", "wrong box");
  });
  r.run("echo_string_box[null]", [&]() {
    StringBox_var got = echo->echo_string_box(nullptr);
    check(got.in() == nullptr, "not null");
  });
  r.run("echo_pair_box", [&]() {
    Pair pair;
    pair.first = new PointImpl(1, 2);
    PairBox_var b = new PairBox(pair);
    PairBox_var got = echo->echo_pair_box(b);
    check(got.in() != nullptr, "null");
    check(is_point(got->_value().first, 1, 2), "wrong point");
    check(got->_value().second.in() == nullptr, "not null");
  });
  r.run("echo_any[ring]", [&]() {
    Node_var n = ring();
    CORBA::Any a;
    a <<= n.in();
    CORBA::Any_var got = echo->echo_any(a);
    check(got->type()->equal(_tc_Node), "wrong TypeCode");
    Node* m = nullptr;
    check(got >>= m, "not a Node");
    check(m && m->id() == 1 && m->next()->id() == 2 && m->next()->next() == m, "wrong ring");
  });
  r.run("echo_any[derived]", [&]() {
    Point_var p = new Point3Impl(1, 2, 3, "three");
    CORBA::Any a;
    a <<= p.in();
    CORBA::Any_var got = echo->echo_any(a);
    check(got->type()->equal(_tc_Point), "wrong TypeCode");
    Point* q = nullptr;
    check(got >>= q, "not a Point");
    check(is_point3(q), "wrong point3");
  });
  r.run("echo_any[box]", [&]() {
    StringBox_var b = new StringBox("in an any");
    CORBA::Any a;
    a <<= b.in();
    CORBA::Any_var got = echo->echo_any(a);
    check(got->type()->equal(_tc_StringBox), "wrong TypeCode");
    StringBox* c = nullptr;
    check(got >>= c, "not a StringBox");
    check(c && std::string(c->_value()) == "in an any", "wrong box");
  });
}

}  // namespace valuetest

#endif
