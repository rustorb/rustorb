// The abstract interface tests (idl/AbstractTest.idl) on the rustorb C++
// binding: the Person servant, the Card implementation, the AbstractEcho
// servant and the client cases, those of impls/rustorbpy/abstracttest.py.

#ifndef INTEROP_RUSTORBCPP_ABSTRACTTEST_H
#define INTEROP_RUSTORBCPP_ABSTRACTTEST_H

#include <functional>
#include <string>

#include "AbstractTest.hh"

namespace abstracttest {

using namespace AbstractTest;

template <typename T> using ref = typename IDL::traits<T>::ref_type;

// AbstractTest::Person: a Named object.
class PersonImpl : public CORBA::servant_traits<Person>::base_type {
 public:
  PersonImpl(std::string name, int32_t age) : name_(std::move(name)), age_(age) {}
  std::string name() override { return name_; }
  int32_t age() override { return age_; }

 private:
  std::string name_;
  int32_t age_;
};

// AbstractTest::Card: a Named value, whose name is its title.
class CardImpl : public OBV_AbstractTest::Card {
 public:
  CardImpl() = default;
  explicit CardImpl(const std::string& title) : OBV_AbstractTest::Card(title) {}
  std::string name() override { return title(); }
  ref<CORBA::ValueBase> _copy_value() const override {
    return CORBA::make_reference<CardImpl>(title());
  }
};

class CardFactory : public CORBA::ValueFactoryBase {
 public:
  ref<CORBA::ValueBase> create_for_unmarshal() override {
    return CORBA::make_reference<CardImpl>();
  }
};

inline void register_factories(ref<CORBA::ORB> orb) {
  orb->register_value_factory(Card::_rorb_repository_id, CORBA::make_reference<CardFactory>());
}

class AbstractEchoImpl : public CORBA::servant_traits<AbstractEcho>::base_type {
 public:
  explicit AbstractEchoImpl(ref<CORBA::ORB> orb) : orb_(std::move(orb)) {}

  ref<Named> echo_named(ref<Named> n) override { return n; }
  std::string name_of(ref<Named> n) override { return n->name(); }
  bool is_object(ref<Named> n) override {
    return dynamic_cast<CORBA::Object*>(n._rorb_shared().get()) != nullptr;
  }
  NamedPair echo_pair(const NamedPair& p) override { return p; }
  NamedSeq echo_seq(const NamedSeq& s) override { return s; }
  ref<Person> make_person(const std::string& name) override {
    return CORBA::make_reference<PersonImpl>(name, 42)->_this();
  }
  CORBA::Any echo_any(const CORBA::Any& a) override { return a; }
  void shutdown() override { orb_->shutdown(false); }

 private:
  ref<CORBA::ORB> orb_;
};

inline ref<Named> card(const std::string& title) {
  return CORBA::make_reference<CardImpl>(title);
}

inline bool is_card(const ref<Named>& v, const std::string& title) {
  auto c = std::dynamic_pointer_cast<CardImpl>(v._rorb_shared());
  return c && c->title() == title && c->name() == title;
}

inline bool is_object(const ref<Named>& v) {
  return v && std::dynamic_pointer_cast<CORBA::Object>(v._rorb_shared()) != nullptr;
}

// Runs the abstract interface cases; `run(name, fn)` reports a case,
// `check(cond, what)` fails one. The client's RootPOA must be active: the
// Person it sends is activated there.
template <typename Runner>
void run_cases(Runner& r, ref<AbstractEcho> echo,
               const std::function<void(bool, const std::string&)>& check) {
  ref<Person> alice = CORBA::make_reference<PersonImpl>("alice", 30)->_this();
  auto is_alice = [&](const ref<Named>& v) {
    check(is_object(v), "not an object");
    check(v->name() == "alice", "wrong name");
  };
  r.run("echo_named[object]", [&]() { is_alice(echo->echo_named(alice)); });
  r.run("echo_named[value]", [&]() { check(is_card(echo->echo_named(card("ace")), "ace"), "wrong card"); });
  r.run("echo_named[null]", [&]() { check(echo->echo_named(nullptr) == nullptr, "not null"); });
  r.run("name_of[object]", [&]() { check(echo->name_of(alice) == "alice", "wrong name"); });
  r.run("name_of[value]", [&]() { check(echo->name_of(card("ace")) == "ace", "wrong name"); });
  r.run("is_object[object]", [&]() { check(echo->is_object(alice), "not an object"); });
  r.run("is_object[value]", [&]() { check(!echo->is_object(card("ace")), "an object"); });
  r.run("echo_pair[shared]", [&]() {
    auto c = card("king");
    NamedPair got = echo->echo_pair(NamedPair(c, c));
    check(is_card(got.first(), "king"), "wrong card");
    check(got.first() == got.second(), "not one instance");
  });
  r.run("echo_pair[mixed]", [&]() {
    NamedPair got = echo->echo_pair(NamedPair(alice, card("queen")));
    is_alice(got.first());
    check(is_card(got.second(), "queen"), "wrong card");
  });
  r.run("echo_seq", [&]() {
    auto c = card("jack");
    NamedSeq got = echo->echo_seq(NamedSeq{c, alice, nullptr, c});
    check(got.size() == 4, "wrong length");
    check(is_card(got[0], "jack"), "wrong card");
    is_alice(got[1]);
    check(got[2] == nullptr, "not null");
    check(got[0] == got[3], "not one instance");
  });
  r.run("make_person", [&]() {
    auto p = echo->make_person("bob");
    check(p && p->name() == "bob" && p->age() == 42, "wrong person");
  });
  r.run("echo_any[value]", [&]() {
    CORBA::Any a;
    a <<= card("joker");
    CORBA::Any got = echo->echo_any(a);
    check(got.type()->equal(_tc_Named), "wrong TypeCode");
    ref<Named> n;
    check(got >>= n, "not a Named");
    check(is_card(n, "joker"), "wrong card");
  });
  r.run("echo_any[object]", [&]() {
    CORBA::Any a;
    a <<= ref<Named>(alice);
    CORBA::Any got = echo->echo_any(a);
    check(got.type()->equal(_tc_Named), "wrong TypeCode");
    ref<Named> n;
    check(got >>= n, "not a Named");
    is_alice(n);
  });
}

}  // namespace abstracttest

#endif
