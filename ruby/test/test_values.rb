# frozen_string_literal: true

# The value factories, values in anys and what the stream keeps
# (ValueTest.idl). test_interop runs the value, abstract and custom suites.

require_relative "helper"
require "valuetest"

class ValuesTest < Minitest::Test
  V = ValueTest

  def setup
    @echo = TestORB.ref(ValueCases::ValueEchoServant.new(TestORB.orb), V::ValueEcho)
  end

  def test_factories
    orb = TestORB.orb
    assert_equal V::Point, orb.lookup_value_factory(V::Point::ID)
    assert_nil orb.lookup_value_factory("IDL:nothing:1.0")
    mine = Class.new(V::Point)
    old = orb.register_value_factory(V::Point::ID) { mine.new }
    begin
      got = @echo.echo_point(V::Point.new(1, 2))
      assert_instance_of mine, got
      assert_equal [1, 2], [got.x, got.y]
    ensure
      orb.unregister_value_factory(V::Point::ID)
      orb.register_value_factory(V::Point::ID, old) if old
    end
    assert_equal V::Point, orb.lookup_value_factory(V::Point::ID)
    assert_raises(CORBA::BAD_PARAM) { orb.unregister_value_factory(V::Point::ID) }
    assert_raises(CORBA::BAD_PARAM) { orb.register_value_factory(V::Point::ID, Object.new) }
  end

  # A valuetype with operations has no factory of its own.
  def test_no_factory
    orb = TestORB.orb
    old = orb.lookup_value_factory(V::Account::ID)
    orb.unregister_value_factory(V::Account::ID) if old
    impl = Class.new(V::Account) { def doubled = balance * 2 }
    assert_raises(CORBA::MARSHAL) { @echo.account_doubled(impl.new(1)) }
  ensure
    orb.register_value_factory(V::Account::ID, old) if old
  end

  def test_not_a_value
    assert_raises(CORBA::BAD_PARAM) { @echo.echo_point(Object.new) }
  end

  def test_new_and_inspect
    p = V::Point3.new(1, 2)
    assert_equal [1, 2, nil, nil], [p.x, p.y, p.z, p.label]
    assert_raises(ArgumentError) { V::Point.new(1, 2, 3) }
    ring = ValueCases.ring
    assert_match(/#<ValueTest::Node id=1, r_next=#<ValueTest::Node id=2, r_next=#<ValueTest::Node \.\.\.>>>/, ring.inspect)
    assert_equal V::Point3._tc, p._tc
    assert_equal ["IDL:ValueTest/Point3:1.0", "IDL:ValueTest/Point:1.0"], p._truncatable_ids
  end

  def test_typecodes
    tc = V::Point3._tc
    assert_equal CORBA::TK_VALUE, tc.kind
    assert_equal CORBA::VM_TRUNCATABLE, tc.type_modifier
    assert_equal V::Point._tc, tc.concrete_base_type
    assert_equal CORBA::PRIVATE_MEMBER, tc.member_visibility(1)
    assert_nil V::Point._tc.concrete_base_type
    assert_equal CORBA::TK_VALUE_BOX, V::LongBox._tc.kind
  end

  # The instance in an any is the one put in, in this process.
  def test_any_keeps_the_instance
    p = V::Point.new(3, 4)
    a = CORBA::Any.to_any(p, V::Point._tc)
    native = a._native
    assert_same p, CORBA::Any._from_native(native).value
    b = CORBA::Any.to_any(p, V::Point._tc)
    assert_same CORBA::Any._from_native(b._native).value, CORBA::Any._from_native(native).value
  end

  # What the streams keep is released when they end.
  def test_released
    GC.start
    before = Rustorb::Native.kept_values
    10.times { @echo.echo_node(ValueCases.chain(50)) }
    # (Anys of other tests the GC frees meanwhile release theirs.)
    assert_operator Rustorb::Native.kept_values, :<=, before
  end
end
