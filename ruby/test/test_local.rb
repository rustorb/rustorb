# frozen_string_literal: true

# Local interfaces (LocalTest.idl): implementations derive from
# CORBA::LocalObject and include the interface's module.

require_relative "helper"
require "LocalTestC"
require "echo_servant"

class Greeter < CORBA::LocalObject
  include LocalTest::Loud

  attr_accessor :count

  def initialize
    super
    @count = 0
  end

  def greet(who)
    raise LocalTest::Greeter::Busy.new("too many") if @count >= LocalTest::Greeter::N

    @count += 1
    "hello #{who}"
  end

  def make = [LocalTest::Greeter::Msg.new("made"), @count]
  def shout = nil
end

class Factory < CORBA::LocalObject
  include LocalTest::Factory

  def make_greeter(like) = like
end

class LocalTest_ < Minitest::Test
  def test_calls
    g = Greeter.new
    assert_equal "hello a", g.greet("a")
    msg, n = g.make
    assert_equal "made", msg.text
    assert_equal 1, n
    g.greet("b")
    g.greet("c")
    e = assert_raises(LocalTest::Greeter::Busy) { g.greet("d") }
    assert_equal "too many", e.why
    assert_same g, Factory.new.make_greeter(g)
  end

  def test_object_operations
    g = Greeter.new
    assert g._is_a?("IDL:LocalTest/Loud:1.0")
    assert g._is_a?("IDL:LocalTest/Greeter:1.0")
    assert g._is_a?("IDL:omg.org/CORBA/LocalObject:1.0")
    assert g._is_a?("IDL:omg.org/CORBA/Object:1.0")
    refute g._is_a?("IDL:LocalTest/Factory:1.0")
    assert_equal "IDL:LocalTest/Loud:1.0", g._repository_id
    refute g._non_existent?
    assert g._is_equivalent?(g)
    refute g._is_equivalent?(Greeter.new)
    assert_kind_of CORBA::Object, g
  end

  def test_narrow
    g = Greeter.new
    assert_same g, LocalTest::Greeter._narrow(g)
    assert_same g, LocalTest::Loud._narrow(g)
    assert_nil LocalTest::Greeter._narrow(nil)
    assert_raises(CORBA::BAD_PARAM) { LocalTest::Factory._narrow(g) }
  end

  def test_typecode
    tc = LocalTest::Greeter._tc
    assert_equal CORBA::TK_LOCAL_INTERFACE, tc.kind
    assert_equal "IDL:LocalTest/Greeter:1.0", tc.id
    assert_equal "Greeter", tc.name
  end

  # A local object cannot leave the process.
  def test_not_sent
    echo = TestORB.ref(EchoServant.new(TestORB.orb), InteropTest::Echo)
    e = assert_raises(CORBA::MARSHAL) { echo.is_nil(Greeter.new) }
    assert_equal 4, e.minor
    assert_raises(CORBA::MARSHAL) { TestORB.orb.object_to_string(Greeter.new) }
    assert_raises(CORBA::MARSHAL) { CORBA::Any.to_any(Greeter.new, LocalTest::Greeter._tc)._native }
  end
end
