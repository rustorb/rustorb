# frozen_string_literal: true

# The rustorb Ruby side of run-ami-probe.sh: callback AMI (docs/ami.md)
# against the Echo server of any implementation. Each case sends one
# sendc_ request and prints what the AMI_EchoHandler servant got, in the
# lines that run-ami-probe.sh compares (the same in every probe), as
# impls/rustorbpy/ami_probe.py.
#
#   ami_probe.rb REFERENCE [ORB options]
#
# Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

here = File.expand_path("..", __dir__)
$LOAD_PATH.unshift(File.join(here, "gen"), File.join(here, "lib"), File.join(here, "rt"))

require "corba"
require "InteropTestS"

WAIT = 10
COMPLETED = { CORBA::COMPLETED_YES => "COMPLETED_YES", CORBA::COMPLETED_NO => "COMPLETED_NO",
              CORBA::COMPLETED_MAYBE => "COMPLETED_MAYBE" }.freeze

# What raise_exception raises, as a probe line.
def describe(holder)
  holder.raise_exception
  "nothing raised"
rescue InteropTest::Oops => e
  "Oops #{e.code} #{e.reason}"
rescue InteropTest::DivideByZero
  "DivideByZero"
rescue CORBA::SystemException => e
  "#{e.class.name.split('::').last} #{e.minor} #{COMPLETED.fetch(e.completed, '?')}"
rescue StandardError => e
  "unexpected #{e.inspect}"
end

# Keeps one line for each call.
class Handler < POA::InteropTest::AMI_EchoHandler
  def initialize
    super
    @lines = []
    @lock = Mutex.new
  end

  def put(line) = @lock.synchronize { @lines << line }

  # The next line, or nil after WAIT seconds.
  def take
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + WAIT
    loop do
      line = @lock.synchronize { @lines.shift }
      return line if line
      return nil if Process.clock_gettime(Process::CLOCK_MONOTONIC) > deadline

      sleep 0.01
    end
  end

  def echo_long(ami_return_val) = put("echo_long: #{ami_return_val}")
  def echo_string(ami_return_val) = put("echo_string: #{ami_return_val}")
  def swap(ami_return_val, a, b) = put("swap: #{ami_return_val} #{a} #{b}")
  def divide(ami_return_val, remainder) = put("divide: #{ami_return_val} #{remainder}")
  def divide_excep(excep_holder) = put("divide_excep: #{describe(excep_holder)}")
  def fail_excep(excep_holder) = put("fail_excep: #{describe(excep_holder)}")
  def fail_system_excep(excep_holder) = put("fail_system_excep: #{describe(excep_holder)}")
  def set_counter = put("set_counter")
  def get_counter(ami_return_val) = put("get_counter: #{ami_return_val}")
  def get_name(ami_return_val) = put("get_name: #{ami_return_val}")
  def trigger_excep(excep_holder) = put("trigger_excep: #{describe(excep_holder)}")
end

# Answers after 2 s, longer than the call timeout of the last case.
class SlowCallback < POA::InteropTest::Callback
  def notify(message)
    sleep 2
    "ack:#{message}"
  end
end

abort "usage: ami_probe.rb REFERENCE [ORB options]" if ARGV.empty?
ref = ARGV[0]
orb = CORBA.ORB_init(["-ORBendPoint", "giop:tcp:127.0.0.1:"] + ARGV[1..])
poa = orb.resolve_initial_references("RootPOA")
poa.the_POAManager.activate
echo = InteropTest::Echo._narrow(orb.string_to_object(ref))
handler = Handler.new
h = InteropTest::AMI_EchoHandler._narrow(poa.id_to_reference(poa.activate_object(handler)))
$status = 0

show = lambda do |what|
  line = handler.take
  if line
    puts line
  else
    puts "#{what}: no reply"
    $status = 1
  end
  $stdout.flush
end

echo.sendc_echo_long(h, 42)
show.call("echo_long")
echo.sendc_echo_string(h, "hello AMI")
show.call("echo_string")
echo.sendc_swap(h, 1, 2)
show.call("swap")
echo.sendc_divide(h, 7, 2)
show.call("divide")
echo.sendc_divide(h, 1, 0)
show.call("divide_excep")
echo.sendc_fail(h, 7, "bad")
show.call("fail_excep")
echo.sendc_fail_system(h)
show.call("fail_system_excep")
echo.sendc_set_counter(h, 5)
show.call("set_counter")
echo.sendc_get_counter(h)
show.call("get_counter")
echo.sendc_get_name(h)
show.call("get_name")
# A nil handler: the request is sent, the reply dropped.
echo.sendc_set_counter(nil, 9)
deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + WAIT
sleep 0.01 while echo.counter != 9 && Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline
puts "nil handler: counter #{echo.counter}"
$stdout.flush
# The server calls the slow callback; the call times out first.
cb = InteropTest::Callback._narrow(poa.id_to_reference(poa.activate_object(SlowCallback.new)))
echo.register_callback(cb)
echo._call_timeout = 0.5
echo.sendc_trigger(h, "late")
echo._call_timeout = nil
show.call("trigger_excep")
# Let the server's call of the callback finish before going away.
sleep 2
orb.destroy
exit($status)
