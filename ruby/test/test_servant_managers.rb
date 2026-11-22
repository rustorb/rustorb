# frozen_string_literal: true

# Servant activators, servant locators and adapter activators, called by
# the POA through references made with create_reference_with_id. The POA
# probe (interop/run-poa-probe.sh rustorbrb) records the rest.

require_relative "helper"
require "echo_servant"

class TestActivator < PortableServer::ServantActivator
  attr_reader :events

  def initialize(servant, forward = nil)
    super()
    @servant = servant
    @forward = forward
    @events = Queue.new
  end

  def incarnate(oid, adapter)
    @events << [:incarnate, oid, adapter.the_name]
    case oid
    when "forward" then raise PortableServer::ForwardRequest.new(@forward)
    when "missing" then raise CORBA::OBJECT_NOT_EXIST.new("", 7, CORBA::COMPLETED_NO)
    when "nil" then nil
    else @servant
    end
  end

  def etherealize(oid, _adapter, servant, cleanup, remaining)
    @events << [:etherealize, oid, servant.equal?(@servant), cleanup, remaining]
  end
end

class TestLocator < PortableServer::ServantLocator
  attr_reader :events

  def initialize(servant)
    super()
    @servant = servant
    @events = Queue.new
  end

  def preinvoke(oid, _adapter, operation)
    @events << [:preinvoke, oid, operation]
    raise CORBA::TRANSIENT.new("", 3, CORBA::COMPLETED_NO) if oid == "busy"

    [@servant, { oid: oid }]
  end

  def postinvoke(oid, _adapter, operation, cookie, servant)
    @events << [:postinvoke, oid, operation, cookie, servant.equal?(@servant)]
  end
end

class TestAdapterActivator < PortableServer::AdapterActivator
  def initialize(servant)
    super()
    @servant = servant
  end

  def unknown_adapter(parent, name)
    return false unless name == "lazy"

    poa = parent.create_POA(name, parent.the_POAManager, [
      parent.create_id_assignment_policy(PortableServer::USER_ID)
    ])
    poa.activate_object_with_id("x", @servant)
    true
  end
end

class ServantManagersTest < Minitest::Test
  ROOT = TestORB.root_poa
  ECHO = InteropTest::Echo::ID

  def poa(name, *extra)
    ROOT.create_POA(name, ROOT.the_POAManager, [
      ROOT.create_id_assignment_policy(PortableServer::USER_ID),
      ROOT.create_request_processing_policy(PortableServer::USE_SERVANT_MANAGER)
    ] + extra)
  end

  def ref(poa, oid)
    ior = TestORB.orb.object_to_string(poa.create_reference_with_id(oid, ECHO))
    InteropTest::Echo._narrow(TestORB.orb.string_to_object(ior))
  end

  def drain(q) = Array.new(q.size) { q.pop }

  def test_activator
    servant = EchoServant.new(TestORB.orb)
    target = TestORB.ref(EchoServant.new(TestORB.orb), InteropTest::Echo)
    a = TestActivator.new(servant, target)
    p = poa("activator")
    p.set_servant_manager(a)
    assert_same a, p.get_servant_manager

    assert_equal 5, ref(p, "one").echo_long(5)
    assert_equal 6, ref(p, "one").echo_long(6)
    assert_equal [[:incarnate, "one", "activator"]], drain(a.events)

    assert_equal 7, ref(p, "forward").echo_long(7)
    assert_raises(CORBA::OBJECT_NOT_EXIST) { ref(p, "missing").echo_long(1) }
    assert_raises(CORBA::OBJ_ADAPTER) { ref(p, "nil").echo_long(1) }
    drain(a.events)

    p.deactivate_object("one")
    sleep 0.05 until a.events.size.positive?
    assert_equal [[:etherealize, "one", true, false, false]], drain(a.events)
  ensure
    p&.destroy(true, true)
  end

  def test_locator
    servant = EchoServant.new(TestORB.orb)
    l = TestLocator.new(servant)
    p = poa("locator", ROOT.create_servant_retention_policy(PortableServer::NON_RETAIN))
    p.set_servant_manager(l)

    a = ref(p, "a") # _narrow asks _is_a
    drain(l.events)
    assert_equal 3, a.echo_long(3)
    assert_equal [[:preinvoke, "a", "echo_long"], [:postinvoke, "a", "echo_long", { oid: "a" }, true]],
                 drain(l.events)
    assert_raises(CORBA::TRANSIENT) { ref(p, "busy") }
    assert_equal [[:preinvoke, "busy", "_is_a"]], drain(l.events)
  ensure
    p&.destroy(false, true)
  end

  def test_the_wrong_manager
    p = poa("wrong")
    assert_raises(CORBA::BAD_PARAM) { p.set_servant_manager(Object.new) }
    assert_raises(CORBA::BAD_PARAM) { p.the_activator = Object.new }
  ensure
    p&.destroy(false, true)
  end

  def test_adapter_activator
    ROOT.the_activator = TestAdapterActivator.new(EchoServant.new(TestORB.orb))
    lazy = ROOT.find_POA("lazy", true)
    assert_equal "lazy", lazy.the_name
    assert_equal 4, ref(lazy, "x").echo_long(4)
    assert_raises(PortableServer::POA::AdapterNonExistent) { ROOT.find_POA("other", true) }
  ensure
    ROOT.the_activator = nil
    lazy&.destroy(false, true)
  end
end
