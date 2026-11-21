# frozen_string_literal: true

# The InteropTest::Echo servant of the rustorb Ruby implementation
# (rustorbrb), the same as the other implementations' (see the comments of
# interop/idl/InteropTest.idl). ruby/test/test_interop.rb uses it in
# process too.
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

require "InteropTestS"

class EchoServant < POA::InteropTest::Echo
  def initialize(orb)
    super()
    @orb = orb
    @lock = Mutex.new
    @posted = []
    @counter = 0
    @callback = nil
  end

  # --- basic and constructed types: each returns its argument ---
  %w[boolean octet char wchar short ushort long ulong longlong ulonglong float double string
     wstring color point record records value shade long_array matrix long_seq string_seq
     octets].each do |t|
    define_method("echo_#{t}") { |v| v }
  end

  # --- parameter modes ---
  def swap(a, b) = [a + b, b, a]

  def divide(a, b)
    raise InteropTest::DivideByZero if b.zero?

    q = a.abs / b.abs
    q = -q if (a.negative?) != (b.negative?)
    [q, a - q * b]
  end

  def make(x)
    p = InteropTest::Point.new(x.to_f, -x.to_f)
    [p, InteropTest::Record.new(x, "r#{x}", InteropTest::BLUE, p, [x, x + 1], true)]
  end

  # --- exceptions ---
  def fail(code, reason)
    raise InteropTest::Oops.new(code, reason)
  end

  def fail_system
    raise CORBA::BAD_PARAM.new("", 42, CORBA::COMPLETED_NO)
  end

  # --- attributes ---
  def counter = @lock.synchronize { @counter }

  def counter=(v)
    @lock.synchronize { @counter = v }
  end

  def name = "rustorb-interop-echo"

  # --- oneway ---
  def post(message)
    @lock.synchronize { @posted << message }
  end

  def posted
    @lock.synchronize do
      r = @posted
      @posted = []
      r
    end
  end

  # --- object references ---
  def self_ref = _this
  def echo_ref(other) = other.nil? ? _this : other
  def is_nil(other) = other.nil?

  # --- callbacks ---
  def call_back(cb, message) = cb.notify(message)

  def register_callback(cb)
    @lock.synchronize { @callback = cb }
  end

  def trigger(message)
    cb = @lock.synchronize { @callback }
    raise InteropTest::Oops.new(1, "no callback") if cb.nil?

    cb.notify(message)
  end

  # --- lifecycle ---
  def shutdown
    Thread.new { @orb.shutdown(false) }
  end
end
