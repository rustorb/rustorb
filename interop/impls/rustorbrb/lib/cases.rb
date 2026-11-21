# frozen_string_literal: true

# The InteropTest cases of the rustorb Ruby client (rustorbrb), the same as
# the other clients' (interop/impls/rustorbpy/client.py). ruby/test runs
# them in process too.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

require "InteropTestS"

class CallbackServant < POA::InteropTest::Callback
  attr_reader :received

  def initialize
    super
    @received = []
  end

  def notify(message)
    @received << message
    "ack:#{message}"
  end
end

class Runner
  attr_reader :passed, :failed

  def initialize(out = $stdout)
    @out = out
    @passed = 0
    @failed = []
  end

  def case(name)
    yield
  rescue Exception => e # rubocop:disable Lint/RescueException - every failure is reported
    raise if e.is_a?(Interrupt) || e.is_a?(SystemExit)

    @failed << name
    @out.puts "FAIL #{name}: #{e.class}: #{e.message}"
    @out.puts e.backtrace.first(5).map { |l| "    #{l}" }
  else
    @passed += 1
    @out.puts "ok   #{name}"
  ensure
    @out.flush
  end

  def summary
    @out.puts
    @out.puts "#{@passed} passed, #{@failed.size} failed"
    @out.puts "failed: #{@failed.join(', ')}" unless @failed.empty?
    @out.flush
  end
end

module Cases
  class Failure < StandardError; end

  LARGE_RECORDS = 400

  module_function

  def check(cond, msg = "check failed")
    raise Failure, msg unless cond
  end

  def echo(echo, op, value, expected = value, &cmp)
    got = echo.public_send(op, value)
    ok = cmp ? cmp.call(got, expected) : got == expected
    check(ok, "sent #{value.inspect[0, 200]}, got #{got.inspect[0, 200]}")
  end

  def expect_raises(klass)
    yield
  rescue klass => e
    e
  else
    raise Failure, "#{klass} was not raised"
  end

  def float_eq(a, b) = [a].pack("g") == [b].pack("g")

  def wstr(s) = s.encode("UTF-16LE").unpack("v*")

  def value(member, v, disc = nil) = InteropTest::Value.new._put(member, v, disc)

  def run(r, echo, poa)
    t = InteropTest

    # --- basic types ---
    [
      ["echo_boolean", [true, false]],
      ["echo_octet", [0, 0x7F, 0xFF]],
      ["echo_char", ["a", "\x00", "~"]],
      ["echo_wchar", ["a".ord, 0x3042, 0xE9]],
      ["echo_short", [0, -32_768, 32_767]],
      ["echo_ushort", [0, 65_535]],
      ["echo_long", [0, -2_147_483_648, 2_147_483_647, t::MAGIC]],
      ["echo_ulong", [0, 4_294_967_295]],
      ["echo_longlong", [0, -(2**63), 2**63 - 1]],
      ["echo_ulonglong", [0, 2**64 - 1]],
      ["echo_double", [0.0, -1.5, 1e300, 2.0**-1074]],
      ["echo_string", ["", "hello", "x" * 10_000, "café"]],
      ["echo_wstring", ["", "hello", "こんにちは", "\u{1F980}"].map { |s| wstr(s) }]
    ].each do |op, values|
      values.each_with_index { |v, i| r.case("#{op}[#{i}]") { echo(echo, op, v) } }
    end
    [0.0, -1.5, 3.4028234663852886e38, 1.1754943508222875e-38].each_with_index do |v, i|
      r.case("echo_float[#{i}]") { echo(echo, "echo_float", v) { |a, b| float_eq(a, b) } }
    end

    # --- constructed types ---
    p = t::Point.new(1.25, -2.5)
    rec = t::Record.new(7, "seven", t::GREEN, p, [1, -2, 3], true)
    rec2 = t::Record.new(-1, "", t::RED, t::Point.new(0.0, 0.0), [], false)
    r.case("echo_color") { [t::RED, t::GREEN, t::BLUE].each { |c| echo(echo, "echo_color", c) } }
    r.case("echo_point") { echo(echo, "echo_point", p) }
    r.case("echo_record") { echo(echo, "echo_record", rec) }
    r.case("echo_records") { echo(echo, "echo_records", [rec, rec2, rec]) }
    r.case("echo_records[empty]") { echo(echo, "echo_records", []) }
    large = Array.new(LARGE_RECORDS) do |i|
      t::Record.new(i, "r#{i}", [t::RED, t::GREEN, t::BLUE][i % 3], t::Point.new(i + 0.5, -0.25 * i),
                    [i, i + 1], i.even?)
    end
    r.case("echo_records[large]") { echo(echo, "echo_records", large) }
    [
      value("l", -5), value("s", "union"), value("p", p), value("seq", [1, 2], 4),
      value("seq", [], 5), value("flag", true, 99), value("flag", false, 0)
    ].each_with_index { |v, i| r.case("echo_value[#{i}]") { echo(echo, "echo_value", v) } }
    [
      t::Shade.new.tap { |s| s.level = 200 }, t::Shade.new.tap { |s| s.label = "leaf" },
      t::Shade.new._default(t::BLUE)
    ].each_with_index { |v, i| r.case("echo_shade[#{i}]") { echo(echo, "echo_shade", v) } }
    r.case("echo_long_array") { echo(echo, "echo_long_array", [1, -1, 2_147_483_647, 0]) }
    r.case("echo_matrix") { echo(echo, "echo_matrix", [[1.0, 2.0, 3.0], [-4.0, 0.5, 1e-9]]) }
    r.case("echo_long_seq") { echo(echo, "echo_long_seq", (-500...500).to_a) }
    r.case("echo_long_seq[empty]") { echo(echo, "echo_long_seq", []) }
    r.case("echo_string_seq") { echo(echo, "echo_string_seq", ["a", "", "ccc"]) }
    octets = (0...256).to_a.pack("C*")
    r.case("echo_octets") { echo(echo, "echo_octets", octets, octets.bytes) }
    big = Array.new(300_000) { |i| i % 251 }.pack("C*")
    r.case("echo_octets[large]") { echo(echo, "echo_octets", big, big.bytes) }

    # --- parameter modes ---
    r.case("swap") { check(echo.swap(3, -10) == [-7, -10, 3]) }
    r.case("divide") do
      check(echo.divide(17, 5) == [3, 2])
      check(echo.divide(-17, 5) == [-3, -2])
      expect_raises(t::DivideByZero) { echo.divide(1, 0) }
    end
    r.case("make") do
      pt, rc = echo.make(12)
      check(pt == t::Point.new(12.0, -12.0))
      check(rc == t::Record.new(12, "r12", t::BLUE, t::Point.new(12.0, -12.0), [12, 13], true))
    end

    # --- exceptions ---
    r.case("fail") do
      e = expect_raises(t::Oops) { echo.fail(7, "boom") }
      check(e.code == 7 && e.reason == "boom", e.inspect)
    end
    r.case("fail_system") do
      e = expect_raises(CORBA::BAD_PARAM) { echo.fail_system }
      check(e.minor == 42 && e.completed == CORBA::COMPLETED_NO, e.inspect)
    end

    # --- attributes ---
    r.case("attribute counter") do
      echo.counter = 41
      check(echo.counter == 41)
      echo.counter = -1
      check(echo.counter == -1)
    end
    r.case("attribute name") { check(echo.name == "rustorb-interop-echo") }

    # --- object references ---
    r.case("self_ref") do
      ref = echo.self_ref
      check(ref._is_equivalent?(echo))
      check(ref.echo_long(5) == 5)
    end
    r.case("echo_ref") do
      check(echo.echo_ref(echo)._is_equivalent?(echo))
      check(echo.echo_ref(nil)._is_equivalent?(echo))
    end
    r.case("is_nil") { check(echo.is_nil(nil) && !echo.is_nil(echo)) }

    # --- callbacks ---
    cb_servant = CallbackServant.new
    poa.activate_object_with_id("Callback".b, cb_servant)
    cb = t::Callback._narrow(poa.id_to_reference("Callback".b))
    r.case("call_back") { check(echo.call_back(cb, "hi") == "ack:hi") }
    r.case("register_callback/trigger") do
      echo.register_callback(cb)
      check(echo.trigger("later") == "ack:later")
      check(cb_servant.received.last == "later")
    end

    # --- oneway (last: see rustorbpy/client.py) ---
    r.case("oneway post") do
      echo.posted
      sent = %w[one two three]
      sent.each { |m| echo.post(m) }
      got = []
      deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 5
      while got.size < sent.size && Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline
        got += echo.posted
        sleep 0.05
      end
      check(got.sort == sent.sort, "got #{got.inspect}")
    end
  end
end
