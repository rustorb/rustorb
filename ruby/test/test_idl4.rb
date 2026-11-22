# frozen_string_literal: true

# The IDL 4 constructs of cpp/tests/idl/Idl4.idl (int8, bitmask, bitset,
# @default, @range / @min / @max) through rustorb-idl -bruby and the ORB.

require_relative "helper"
require "Idl4S"

class Idl4Servant < POA::Idl4::Echo
  def echo_settings(s) = s
  def echo_flags(f) = [f, Idl4::Wide::HIGH | Idl4::Wide::LOW]
  def echo_header(h) = h
  def echo_int8(v, b) = [-v - 1, b.reverse]
  def echo_any(a) = a
end

class Idl4Test < Minitest::Test
  def setup
    @echo = TestORB.ref(Idl4Servant.new, Idl4::Echo)
  end

  def settings(**kw)
    s = Idl4::Settings.new(1)
    s.level = 3
    s.step = 2
    s.offset = -5
    s.raw = 200
    s.flags = Idl4::Flags::READ | Idl4::Flags::EXEC
    s.wide = Idl4::Wide::HIGH
    s.header = Idl4::Header.new(5, -3, true)
    kw.each { |k, v| s.send("#{k}=", v) }
    s
  end

  def test_constants
    assert_equal(-128, Idl4::SMALLEST)
    assert_equal 8, Idl4::Flags::WRITE
    assert_equal 16, Idl4::Flags::EXEC
    assert_equal 1 << 39, Idl4::Wide::HIGH
  end

  def test_defaults
    s = Idl4::Settings.new
    assert_equal 7, s.retries
    assert_equal true, s.enabled
    assert_equal 2.5, s.ratio
    assert_nil s.id
    assert_equal 9, Idl4::Settings.new(1, 9).retries
  end

  def test_echo_settings
    s = settings
    got = @echo.echo_settings(s)
    assert_equal s, got
    assert_equal(-5, got.offset)
    assert_equal Idl4::Header.new(5, -3, true), got.header
  end

  def test_ranges
    assert_raises(CORBA::BAD_PARAM) { @echo.echo_settings(settings(level: 11)) }
    assert_raises(CORBA::BAD_PARAM) { @echo.echo_settings(settings(step: 0)) }
  end

  def test_flags_header_int8
    assert_equal [Idl4::Flags::WRITE, (1 << 39) | 1], @echo.echo_flags(Idl4::Flags::WRITE)
    h = Idl4::Header.new(7, -16, false)
    assert_equal h, @echo.echo_header(h)
    assert_equal [127, [3, -2, -128]], @echo.echo_int8(-128, [-128, -2, 3])
    assert_raises(CORBA::MARSHAL) { @echo.echo_int8(128, []) }
    assert_raises(CORBA::MARSHAL) { @echo.echo_header(Idl4::Header.new(8, 0, false)) }
  end

  def test_any
    s = settings
    got = @echo.echo_any(CORBA::Any.new(s, Idl4::Settings._tc))
    assert_equal s, got.value
  end
end
