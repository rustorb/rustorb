# frozen_string_literal: true

# The DII (CORBA::Request) against the static Echo servant of the interop
# server, and the DSI (PortableServer::DynamicImplementation) called by a
# static stub. test_interop runs every interop case through both.

require_relative "helper"
require "echo_servant"

class EchoDsi < PortableServer::DynamicImplementation
  def _primary_interface(_oid, _poa) = InteropTest::Echo::ID

  def invoke(req)
    case req.operation
    when "echo_long"
      nv = CORBA::NVList.new
      nv.add_value("v", CORBA::Any.to_any(nil, CORBA._tc_long), CORBA::ARG_IN)
      req.arguments(nv)
      req.set_result(CORBA::Any.to_any(nv.item(0).argument.value + 1, CORBA._tc_long))
    when "divide"
      nv = CORBA::NVList.new
      nv.add_value("a", CORBA::Any.to_any(nil, CORBA._tc_long), CORBA::ARG_IN)
      nv.add_value("b", CORBA::Any.to_any(nil, CORBA._tc_long), CORBA::ARG_IN)
      nv.add_value("remainder", CORBA::Any.to_any(nil, CORBA._tc_long), CORBA::ARG_OUT)
      req.arguments(nv)
      a, b = nv.first(2).map { |x| x.argument.value }
      if b.zero?
        req.set_exception(CORBA::Any.to_any(InteropTest::DivideByZero.new, InteropTest::DivideByZero._tc))
      else
        req.set_result(CORBA::Any.to_any(a / b, CORBA._tc_long))
        nv.item(2).argument = CORBA::Any.to_any(a % b, CORBA._tc_long)
      end
    when "fail"
      req.arguments(CORBA::NVList.new)
    else
      raise CORBA::BAD_OPERATION.new("", 0, CORBA::COMPLETED_NO)
    end
  end
end

class DiiTest < Minitest::Test
  def setup
    @echo = TestORB.ref(EchoServant.new(TestORB.orb), InteropTest::Echo)
  end

  def long(v) = CORBA::Any.to_any(v, CORBA._tc_long)

  def test_request
    r = @echo._request("divide")
    r.add_in_arg(long(17))
    r.add_in_arg(long(5))
    r.add_out_arg(CORBA._tc_long)
    r.set_return_type(CORBA._tc_long)
    r.add_exception(InteropTest::DivideByZero._tc)
    r.invoke
    assert_equal 3, r.return_value.value
    assert_equal 2, r.arguments.item(2).argument.value
    assert_equal "divide", r.operation
    assert_equal @echo, r.target

    z = @echo._request("divide")
    z.add_in_arg(long(1))
    z.add_in_arg(long(0))
    z.add_out_arg(CORBA._tc_long)
    z.set_return_type(CORBA._tc_long)
    assert_raises(CORBA::UNKNOWN) { z.invoke }
    z.add_exception(InteropTest::DivideByZero._tc)
    assert_raises(InteropTest::DivideByZero) { z.invoke }
  end

  def test_create_request_and_deferred
    args = TestORB.orb.create_list
    args.add_value("v", CORBA::Any.to_any("hi", CORBA._tc_string), CORBA::ARG_IN)
    result = CORBA::NamedValue.new("", CORBA::Any.to_any(nil, CORBA._tc_string), 0)
    r = @echo._create_request(nil, "echo_string", args, result)
    r.send_deferred
    r.get_response
    assert_equal "hi", r.return_value.value
    assert_raises(CORBA::Bounds) { args.item(3) }
    assert_raises(CORBA::BAD_INV_ORDER) { @echo._request("echo_long").get_response }
  end

  def test_system_exception
    r = @echo._request("no_such_operation")
    assert_raises(CORBA::BAD_OPERATION) { r.invoke }
  end

  def test_dsi
    dsi = TestORB.ref(EchoDsi.new, InteropTest::Echo)
    assert_equal 8, dsi.echo_long(7)
    assert_equal [3, 2], dsi.divide(17, 5)
    assert_raises(InteropTest::DivideByZero) { dsi.divide(1, 0) }
    assert dsi._is_a?(InteropTest::Echo::ID)
    assert_raises(CORBA::BAD_OPERATION) { dsi.echo_string("x") }
    # fail sets no result: a void operation's reply.
    assert_nil dsi.fail(1, "x")
  end
end
