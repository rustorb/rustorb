// The abstract interface tests (idl/AbstractTest.idl) on omniORB C++: the
// Person servant, the Card implementation, the AbstractEcho servant and
// the client cases, those of impls/omniorbpy/abstracttest.py.

#ifndef INTEROP_ABSTRACTTEST_H
#define INTEROP_ABSTRACTTEST_H

#include <functional>
#include <string>

#include "AbstractTest.hh"

namespace abstracttest {

using namespace AbstractTest;

// AbstractTest::Person: a Named object.
class PersonImpl : public POA_AbstractTest::Person {
 public:
  PersonImpl(const char* name, CORBA::Long age) : name_(name), age_(age) {}
  char* name() override { return CORBA::string_dup(name_.c_str()); }
  CORBA::Long age() override { return age_; }

 private:
  std::string name_;
  CORBA::Long age_;
};

// AbstractTest::Card: a Named value, whose name is its title.
class CardImpl : public virtual OBV_AbstractTest::Card,
                 public virtual CORBA::DefaultValueRefCountBase {
 public:
  CardImpl() {}
  explicit CardImpl(const char* title) { this->title(title); }
  char* name() override { return CORBA::string_dup(title()); }
  AbstractTest::Card* _copy_value() override { return new CardImpl(title()); }
};

class CardFactory : public CORBA::ValueFactoryBase {
 public:
  CORBA::ValueBase* create_for_unmarshal() override { return new CardImpl; }
};

inline void register_factories(CORBA::ORB_ptr orb) {
  CORBA::ValueFactoryBase* f = new CardFactory;
  CORBA::ValueFactory old = orb->register_value_factory("IDL:AbstractTest/Card:1.0", f);
  if (old) old->_remove_ref();
  f->_remove_ref();
}

// A nil reference read is a nil object reference (not a null pointer),
// a null value a null pointer.
inline bool is_null(Named_ptr n) { return !n || Named_Helper::is_nil(n); }

inline bool is_object(Named_ptr n) {
  if (is_null(n)) return false;
  CORBA::Object_var o = n->_to_object();
  return !CORBA::is_nil(o);
}

class AbstractEchoImpl : public POA_AbstractTest::AbstractEcho {
 public:
  explicit AbstractEchoImpl(CORBA::ORB_ptr orb) : orb_(CORBA::ORB::_duplicate(orb)) {}

  Named_ptr echo_named(Named_ptr n) override { return Named::_duplicate(n); }
  char* name_of(Named_ptr n) override { return n->name(); }
  CORBA::Boolean is_object(Named_ptr n) override { return abstracttest::is_object(n); }
  NamedPair* echo_pair(const NamedPair& p) override { return new NamedPair(p); }
  NamedSeq* echo_seq(const NamedSeq& s) override { return new NamedSeq(s); }
  Person_ptr make_person(const char* name) override {
    PersonImpl* servant = new PersonImpl(name, 42);
    Person_ptr p = servant->_this();
    servant->_remove_ref();
    return p;
  }
  CORBA::Any* echo_any(const CORBA::Any& a) override { return new CORBA::Any(a); }
  void shutdown() override { orb_->shutdown(false); }

 private:
  CORBA::ORB_var orb_;
};

inline bool is_card(Named_ptr v, const char* title) {
  CardImpl* c = dynamic_cast<CardImpl*>(Card::_downcast(v));
  if (!c) return false;
  CORBA::String_var n = c->name();
  return std::string(c->title()) == title && std::string(n.in()) == title;
}

inline bool is_name(Named_ptr v, const char* name) {
  CORBA::String_var n = v->name();
  return std::string(n.in()) == name;
}

// Runs the abstract interface cases; `run(name, fn)` reports a case,
// `check(cond, what)` fails one. The client's RootPOA must be active: the
// Person it sends is activated there.
template <typename Runner>
void run_cases(Runner& r, AbstractEcho_ptr echo,
               const std::function<void(bool, const std::string&)>& check) {
  PersonImpl* servant = new PersonImpl("alice", 30);
  Person_var alice = servant->_this();
  servant->_remove_ref();
  auto is_alice = [&](Named_ptr v) {
    check(is_object(v), "not an object");
    check(is_name(v, "alice"), "wrong name");
  };
  auto card = [](const char* title) { return Card_var(new CardImpl(title)); };
  r.run("echo_named[object]", [&]() {
    Named_var got = echo->echo_named(alice.in());
    is_alice(got.in());
  });
  r.run("echo_named[value]", [&]() {
    Card_var c = card("ace");
    Named_var got = echo->echo_named(c.in());
    check(is_card(got.in(), "ace"), "wrong card");
  });
  r.run("echo_named[null]", [&]() {
    Named_var got = echo->echo_named(Named::_nil());
    check(is_null(got.in()), "not null");
  });
  r.run("name_of[object]", [&]() {
    CORBA::String_var n = echo->name_of(alice.in());
    check(std::string(n.in()) == "alice", "wrong name");
  });
  r.run("name_of[value]", [&]() {
    Card_var c = card("ace");
    CORBA::String_var n = echo->name_of(c.in());
    check(std::string(n.in()) == "ace", "wrong name");
  });
  r.run("is_object[object]", [&]() { check(echo->is_object(alice.in()), "not an object"); });
  r.run("is_object[value]", [&]() {
    Card_var c = card("ace");
    check(!echo->is_object(c.in()), "an object");
  });
  r.run("echo_pair[shared]", [&]() {
    Card_var c = card("king");
    NamedPair p;
    p.first = Named::_duplicate(c.in());
    p.second = Named::_duplicate(c.in());
    NamedPair_var got = echo->echo_pair(p);
    check(is_card(got->first.in(), "king"), "wrong card");
    check(got->first.in() == got->second.in(), "not one instance");
  });
  r.run("echo_pair[mixed]", [&]() {
    Card_var c = card("queen");
    NamedPair p;
    p.first = Named::_duplicate(alice.in());
    p.second = Named::_duplicate(c.in());
    NamedPair_var got = echo->echo_pair(p);
    is_alice(got->first.in());
    check(is_card(got->second.in(), "queen"), "wrong card");
  });
  r.run("echo_seq", [&]() {
    Card_var c = card("jack");
    NamedSeq s;
    s.length(4);
    s[0] = Named::_duplicate(c.in());
    s[1] = Named::_duplicate(alice.in());
    s[2] = Named::_nil();
    s[3] = Named::_duplicate(c.in());
    NamedSeq_var got = echo->echo_seq(s);
    check(got->length() == 4, "wrong length");
    check(is_card(got[0].in(), "jack"), "wrong card");
    is_alice(got[1].in());
    check(is_null(got[2].in()), "not null");
    check(got[0].in() == got[3].in(), "not one instance");
  });
  r.run("make_person", [&]() {
    Person_var p = echo->make_person("bob");
    CORBA::String_var n = p->name();
    check(std::string(n.in()) == "bob" && p->age() == 42, "wrong person");
  });
  r.run("echo_any[value]", [&]() {
    Card_var c = card("joker");
    CORBA::Any a;
    a <<= static_cast<Named_ptr>(c.in());
    CORBA::Any_var got = echo->echo_any(a);
    CORBA::TypeCode_var tc = got->type();
    check(tc->equal(_tc_Named), "wrong TypeCode");
    Named_ptr n;
    check(got.in() >>= n, "not a Named");
    check(is_card(n, "joker"), "wrong card");
  });
  r.run("echo_any[object]", [&]() {
    CORBA::Any a;
    a <<= static_cast<Named_ptr>(alice.in());
    CORBA::Any_var got = echo->echo_any(a);
    CORBA::TypeCode_var tc = got->type();
    check(tc->equal(_tc_Named), "wrong TypeCode");
    Named_ptr n;
    check(got.in() >>= n, "not a Named");
    is_alice(n);
  });
}

}  // namespace abstracttest

#endif
