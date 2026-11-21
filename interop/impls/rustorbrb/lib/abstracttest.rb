# frozen_string_literal: true

# The abstract interface tests (idl/AbstractTest.idl) of the rustorb Ruby
# client and server (rustorbrb): the Person servant, the Card
# implementation, the AbstractEcho servant and the client cases, the same as
# impls/rustorbpy/abstracttest.py.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

require "AbstractTestS"
require "cases"

module AbstractCases
  A = AbstractTest

  # AbstractTest::Person: a Named object.
  class PersonServant < POA::AbstractTest::Person
    def initialize(name, age)
      super()
      @name = name
      @age = age
    end

    def name = @name
    def age = @age
  end

  # AbstractTest::Card: a Named value, whose name is its title.
  class CardImpl < A::Card
    def name = title
  end

  class AbstractEchoServant < POA::AbstractTest::AbstractEcho
    def initialize(orb)
      super()
      @orb = orb
    end

    def echo_named(n) = n
    def name_of(n) = n.name
    def is_object(n) = n.is_a?(CORBA::Object)
    def echo_pair(p) = p
    def echo_seq(s) = s
    def make_person(name) = PersonServant.new(name, 42)._this
    def echo_any(a) = a
    def shutdown = @orb.shutdown(false)
  end

  module_function

  # The value factory of Card.
  def register(orb)
    orb.register_value_factory(A::Card::ID, CardImpl)
  end

  def check(cond, msg = "check failed") = Cases.check(cond, msg)

  def card?(v, title)
    check(v.is_a?(CardImpl), "got #{v.inspect}")
    check(v.title == title && v.name == title, "got #{v.title.inspect}")
  end

  def alice?(v)
    check(v.is_a?(CORBA::Object), "got #{v.inspect}")
    check(v.name == "alice", "wrong name")
  end

  # The client's RootPOA must be active: the Person it sends is activated
  # there.
  def run(r, echo)
    alice = PersonServant.new("alice", 30)._this
    r.case("echo_named[object]") { alice?(echo.echo_named(alice)) }
    r.case("echo_named[value]") { card?(echo.echo_named(CardImpl.new("ace")), "ace") }
    r.case("echo_named[null]") { check(echo.echo_named(nil).nil?, "not null") }
    r.case("name_of[object]") { check(echo.name_of(alice) == "alice", "wrong name") }
    r.case("name_of[value]") { check(echo.name_of(CardImpl.new("ace")) == "ace", "wrong name") }
    r.case("is_object[object]") { check(echo.is_object(alice), "not an object") }
    r.case("is_object[value]") { check(!echo.is_object(CardImpl.new("ace")), "an object") }
    r.case("echo_pair[shared]") do
      c = CardImpl.new("king")
      got = echo.echo_pair(A::NamedPair.new(c, c))
      card?(got.first, "king")
      check(got.first.equal?(got.second), "not one instance")
    end
    r.case("echo_pair[mixed]") do
      got = echo.echo_pair(A::NamedPair.new(alice, CardImpl.new("queen")))
      alice?(got.first)
      card?(got.second, "queen")
    end
    r.case("echo_seq") do
      c = CardImpl.new("jack")
      got = echo.echo_seq([c, alice, nil, c])
      check(got.size == 4, "wrong length")
      card?(got[0], "jack")
      alice?(got[1])
      check(got[2].nil?, "not null")
      check(got[0].equal?(got[3]), "not one instance")
    end
    r.case("make_person") do
      p = echo.make_person("bob")
      check(p.name == "bob" && p.age == 42, "wrong person")
    end
    r.case("echo_any[value]") do
      got = echo.echo_any(CORBA::Any.to_any(CardImpl.new("joker"), A::Named._tc))
      check(got._tc.equal?(A::Named._tc), "got #{got._tc.inspect}")
      card?(got.value, "joker")
    end
    r.case("echo_any[object]") do
      got = echo.echo_any(CORBA::Any.to_any(alice, A::Named._tc))
      check(got._tc.equal?(A::Named._tc), "got #{got._tc.inspect}")
      alice?(got.value)
    end
  end
end
