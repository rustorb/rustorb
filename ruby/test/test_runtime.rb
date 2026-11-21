# frozen_string_literal: true

# The runtime on its own: the hand-written stubs and skeletons of
# test/demo, in the shape rustorb-idl -bruby writes them.

require_relative "helper"
require_relative "demo/DemoS"

class CalcImpl < POA::Demo::Calc
  attr_reader :pings

  def initialize
    @label = "calc"
    @pings = Queue.new
  end

  def add(a, b) = a + b
  def split(p, y) = [p.x, p.y + y]
  attr_accessor :label

  def fail(why)
    raise Demo::Oops.new(why) unless why == "sys"

    raise CORBA::NO_PERMISSION.new("no", 7, CORBA::COMPLETED_YES)
  end

  def ping = @pings << :ping
  def echo(a) = a
  def me = _this
end

class RuntimeTest < Minitest::Test
  def setup
    @impl = CalcImpl.new
    @calc = TestORB.ref(@impl, Demo::Calc)
  end

  def echo_any(value, tc = nil)
    @calc.echo(CORBA::Any.to_any(value, tc))
  end

  def test_calls
    assert_kind_of Demo::Calc, @calc
    assert_equal 5, @calc.add(2, 3)
    assert_equal [1, 12], @calc.split(Demo::Point.new(1, 2), 10)
    assert_equal "calc", @calc.label
    @calc.label = "x"
    assert_equal "x", @calc.label
    @calc.ping
    assert_equal :ping, @impl.pings.pop
  end

  def test_exceptions
    e = assert_raises(Demo::Oops) { @calc.fail("why") }
    assert_equal "why", e.why
    e = assert_raises(CORBA::NO_PERMISSION) { @calc.fail("sys") }
    assert_equal 7, e.minor
    assert_equal CORBA::COMPLETED_YES, e.completed
    assert_raises(CORBA::MARSHAL) { @calc.add(2**40, 1) }
  end

  def test_any_and_objref
    a = @calc.echo(CORBA::Any.to_any(Demo::Point.new(3, 4)))
    assert_equal Demo::Point.new(3, 4), a.value
    assert_equal 42, @calc.echo(CORBA::Any.to_any(42)).value
    me = @calc.me
    assert_kind_of Demo::Calc, me
    assert me._is_equivalent?(@calc)
    assert @calc._is_a?("IDL:Demo/Calc:1.0")
    refute @calc._non_existent?
  end

  def test_any_values
    assert_equal "text", echo_any("text").value
    assert_equal 2**40, echo_any(2**40).value
    assert_equal 0.5, echo_any(0.5).value
    assert_equal true, echo_any(true).value
    assert_nil echo_any(nil).value
    assert_equal [1, 2, 3], echo_any([1, 2, 3], CORBA::TypeCode.create_sequence_tc(0, CORBA._tc_short)).value
    assert_equal [0x3042, 0x61], echo_any([0x3042, 0x61], CORBA._tc_wstring).value
    assert_equal BigDecimal("-1.25"), echo_any(BigDecimal("-1.25")).value
    inner = echo_any(CORBA::Any.to_any(7)).value
    assert_equal 7, inner.value
    tc = echo_any(Demo::Point._tc).value
    assert tc.equal?(Demo::Point._tc)
    ref = echo_any(@calc).value
    assert ref._is_equivalent?(@calc)
    assert_kind_of Demo::Calc, ref
    assert_raises(CORBA::BAD_PARAM) { CORBA::Any.to_any(Object.new) }
    assert_raises(CORBA::MARSHAL) { echo_any(300, CORBA._tc_octet) }
    assert_equal CORBA::Any.to_any(5), CORBA::Any.to_any(5)
  end

  def test_poa
    root = TestORB.root_poa
    assert_equal "RootPOA", root.the_name
    assert_nil root.the_parent
    poa = root.create_POA("RuntimeTest", root.the_POAManager, [
      root.create_id_assignment_policy(PortableServer::USER_ID)
    ])
    assert_equal root, poa.the_parent
    assert_equal poa, root.find_POA("RuntimeTest", false)
    assert_raises(PortableServer::POA::AdapterAlreadyExists) do
      root.create_POA("RuntimeTest", root.the_POAManager, [])
    end
    assert_raises(PortableServer::POA::AdapterNonExistent) { root.find_POA("nope", false) }
    e = assert_raises(PortableServer::POA::InvalidPolicy) do
      root.create_POA("Bad", nil, [root.create_lifespan_policy(PortableServer::PERSISTENT),
                                   PortableServer::Policy.new(PortableServer::LIFESPAN_POLICY_ID, 9)])
    end
    assert_equal 1, e.index
    impl = CalcImpl.new
    poa.activate_object_with_id("calc-1".b, impl)
    assert_raises(PortableServer::POA::ObjectAlreadyActive) { poa.activate_object_with_id("calc-1".b, CalcImpl.new) }
    assert_same impl, poa.id_to_servant("calc-1".b)
    ref = poa.id_to_reference("calc-1".b)
    assert_kind_of CORBA::Object, ref
    assert_equal "calc-1".b, poa.reference_to_id(ref)
    assert_equal 3, Demo::Calc._narrow(ref).add(1, 2)
    poa.deactivate_object("calc-1".b)
    assert_raises(PortableServer::POA::ObjectNotActive) { poa.id_to_servant("calc-1".b) }
    assert_raises(CORBA::OBJECT_NOT_EXIST) { Demo::Calc._narrow(ref) }
    current = Class.new(CalcImpl) do
      def label
        c = PortableServer::Current.instance
        "#{c.get_POA.the_name}:#{c.get_object_id}:#{c.get_servant.equal?(self)}"
      end
    end.new
    poa.activate_object_with_id("calc-3".b, current)
    ref3 = Demo::Calc._narrow(poa.id_to_reference("calc-3".b))
    assert_equal "RuntimeTest:calc-3:true", ref3.label
    assert ref3.me._is_equivalent?(ref3), "_this in a request is the reference called"
    later = poa.create_reference_with_id("calc-2".b, Demo::Calc::ID)
    assert_kind_of Demo::Calc, later
    poa.destroy(false, true)
    assert_raises(CORBA::ORB::InvalidName) { TestORB.orb.resolve_initial_references("NoSuchService") }
    assert_equal PortableServer::POAManager::ACTIVE, root.the_POAManager.get_state
  end

  def test_current
    assert_raises(PortableServer::Current::NoContext) { PortableServer::Current.instance.get_object_id }
    current = TestORB.orb.resolve_initial_references("POACurrent")
    assert_same PortableServer::Current.instance, current
  end

  def test_threads
    ts = 8.times.map { |i| Thread.new { 20.times.map { |j| @calc.add(i, j) } } }
    assert_equal 8.times.map { |i| 20.times.map { |j| i + j } }, ts.map(&:value)
  end
end
