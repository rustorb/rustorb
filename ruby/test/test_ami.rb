# frozen_string_literal: true

# Callback AMI (InteropTest.idl with -Wbami): sendc_ requests to the Echo
# servant, the replies at an AMI_EchoHandler servant. run-ami-probe.sh runs
# the same against other ORBs.

require_relative "helper"
require "echo_servant"
require "InteropTestS"

class AmiTest < Minitest::Test
  class Handler < POA::InteropTest::AMI_EchoHandler
    def initialize
      super
      @queue = Queue.new
    end

    def take(timeout = 10)
      deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + timeout
      while @queue.empty?
        raise "no reply" if Process.clock_gettime(Process::CLOCK_MONOTONIC) > deadline

        sleep 0.005
      end
      @queue.pop
    end

    def echo_long(r) = @queue << [:echo_long, r]
    def echo_string(r) = @queue << [:echo_string, r]
    def swap(r, a, b) = @queue << [:swap, r, a, b]
    def divide(r, rem) = @queue << [:divide, r, rem]
    def divide_excep(h) = @queue << [:divide_excep, h]
    def fail_excep(h) = @queue << [:fail_excep, h]
    def fail_system_excep(h) = @queue << [:fail_system_excep, h]
    def set_counter = @queue << [:set_counter]
    def get_counter(r) = @queue << [:get_counter, r]

    def echo_point(_r)
      raise "a handler that raises"
    end
  end

  def setup
    @echo = TestORB.ref(EchoServant.new(TestORB.orb), InteropTest::Echo)
    @handler = Handler.new
    @h = InteropTest::AMI_EchoHandler._narrow(@handler._this)
  end

  def test_replies
    @echo.sendc_echo_long(@h, 42)
    assert_equal [:echo_long, 42], @handler.take
    @echo.sendc_swap(@h, 1, 2)
    assert_equal [:swap, 3, 2, 1], @handler.take
    @echo.sendc_divide(@h, 7, 2)
    assert_equal [:divide, 3, 1], @handler.take
    @echo.sendc_set_counter(@h, 5)
    assert_equal [:set_counter], @handler.take
    @echo.sendc_get_counter(@h)
    assert_equal [:get_counter, 5], @handler.take
  end

  def test_exceptions
    @echo.sendc_divide(@h, 1, 0)
    op, holder = @handler.take
    assert_equal :divide_excep, op
    assert_raises(InteropTest::DivideByZero) { holder.raise_exception }
    @echo.sendc_fail(@h, 7, "bad")
    _, holder = @handler.take
    e = assert_raises(InteropTest::Oops) { holder.raise_exception }
    assert_equal [7, "bad"], [e.code, e.reason]
    e = assert_raises(CORBA::UNKNOWN) { holder.raise_exception_with_list([InteropTest::DivideByZero._tc]) }
    assert_equal 0x4f4d0001, e.minor
    assert_raises(InteropTest::Oops) { holder.raise_exception_with_list([InteropTest::Oops._tc]) }
    @echo.sendc_fail_system(@h)
    _, holder = @handler.take
    e = assert_raises(CORBA::BAD_PARAM) { holder.raise_exception }
    assert_equal [42, CORBA::COMPLETED_NO], [e.minor, e.completed]
  end

  # A holder from another process has only its state.
  def test_holder_state
    @echo.sendc_fail(@h, 3, "three")
    _, holder = @handler.take
    copy = Messaging::ExceptionHolder.new(holder.is_system_exception, holder.byte_order,
                                          holder.marshaled_exception)
    e = assert_raises(InteropTest::Oops) { copy.raise_exception }
    assert_equal [3, "three"], [e.code, e.reason]
    sys = Messaging::ExceptionHolder._of(CORBA::TRANSIENT.new("", 2, CORBA::COMPLETED_MAYBE))
    copy = Messaging::ExceptionHolder.new(sys.is_system_exception, sys.byte_order, sys.marshaled_exception)
    e = assert_raises(CORBA::TRANSIENT) { copy.raise_exception }
    assert_equal [2, CORBA::COMPLETED_MAYBE], [e.minor, e.completed]
    unknown = Messaging::ExceptionHolder.new(false, holder.byte_order,
                                             Rustorb::Native.cdr_encode(holder.byte_order, ->(o) { o.write_string("IDL:x/Y:1.0") }))
    assert_raises(CORBA::UNKNOWN) { unknown.raise_exception }
  end

  def test_nil_handler_and_raising_handler
    @echo.sendc_set_counter(nil, 9)
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 10
    sleep 0.01 while @echo.counter != 9 && Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline
    assert_equal 9, @echo.counter
    @echo.sendc_echo_point(@h, InteropTest::Point.new(1, 2))
    @echo.sendc_echo_long(@h, 1)
    assert_equal [:echo_long, 1], @handler.take
  end

  def test_concurrent
    50.times { |i| @echo.sendc_echo_long(@h, i) }
    got = Array.new(50) { @handler.take[1] }
    assert_equal (0...50).to_a, got.sort
  end

  def test_arguments_copied
    s = +"before"
    @echo.sendc_echo_string(@h, s)
    s.replace("after")
    assert_equal [:echo_string, "before"], @handler.take
  end
end
