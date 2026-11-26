# frozen_string_literal: true

# Ruby program of the rustorb / omniORB comparison benchmark (interop/bench,
# docs/omniorb-comparison.md) on the rustorb Ruby binding: the same cases,
# options and output as the Rust, C++ and Python programs.
#
#   ruby bench.rb serve  [ORB options]
#   ruby bench.rb client --ior IOR [options] [ORB options]
#   ruby bench.rb inproc [options] [ORB options]
#
# The rustorb Ruby runtime and the stubs of Bench.idl (rustorb-idl -bruby)
# must be on the load path (run-bench.py sets RUBYLIB).

require "json"
require "corba"
require "BenchS"

DEFAULT_CASES = "ping,long,string,octets:64,octets:1024,octets:16384,octets:262144," \
                "octets:1048576,octets:8388608,put:1048576,get:1048576,samples:100,any," \
                "oneway,threads:4"

class TargetServant < POA::Bench::Target
  def initialize(orb)
    super()
    @orb = orb
    @lock = Mutex.new
    @posted = 0
  end

  def ping = nil
  def echo_long(v) = v
  def echo_string(v) = v
  def echo_octets(v) = v
  def put_octets(v) = v.size # an Array of Integer (a String when sent from Ruby in-process)
  def get_octets(n) = "\x07".b * n
  def echo_samples(v) = v
  def echo_any(v) = v
  def post(_v) = @lock.synchronize { @posted += 1 }

  def take_posted
    @lock.synchronize do
      n = @posted
      @posted = 0
      n
    end
  end

  def shutdown = Thread.new { @orb.shutdown(false) }
end

mode = ARGV.shift
opts = { "--ior" => "", "--cases" => DEFAULT_CASES, "--seconds" => "2", "--warmup" => "0.3" }
orb_args = []
until ARGV.empty?
  a = ARGV.shift
  if opts.key?(a) && !ARGV.empty?
    opts[a] = ARGV.shift
  else
    orb_args << a
  end
end
SECONDS = opts["--seconds"].to_f
WARMUP = opts["--warmup"].to_f

def now = Process.clock_gettime(Process::CLOCK_MONOTONIC)

def out(h)
  puts JSON.generate(h)
  $stdout.flush
end

def check(ok, what)
  raise "#{what} mismatch" unless ok
end

def timed(name, nbytes)
  warm_end = now + WARMUP
  warm = 0
  while now < warm_end
    yield
    warm += 1
  end
  lat = []
  start = now
  stop = start + SECONDS
  loop do
    t = now
    yield
    n = now
    lat << (n - t)
    break if n >= stop
  end
  took = now - start
  lat.sort!
  n = lat.size
  pct = ->(p) { (lat[((n - 1) * p).to_i] * 1e6).round(2) }
  rate = n / took
  out({ case: name, calls: n, seconds: took.round(3), rate: rate.round(1),
        mean_us: (lat.sum / n * 1e6).round(2), p50_us: pct.(0.5), p90_us: pct.(0.9),
        p99_us: pct.(0.99), max_us: pct.(1.0), mb_per_s: (rate * nbytes / 1e6).round(2),
        total_calls: n + warm })
end

def run_case(t, c)
  name, arg = c.split(":", 2)
  n = arg.to_i
  case name
  when "first"
    # The first call on a fresh reference: the connection is made.
    t0 = now
    t.ping
    us = ((now - t0) * 1e6).round(2)
    out({ case: "first", calls: 1, p50_us: us, mean_us: us })
  when "ping" then timed(c, 0) { t.ping }
  when "long" then timed(c, 0) { check(t.echo_long(12_345) == 12_345, "echo_long") }
  when "string"
    s = "x" * (n.zero? ? 100 : n)
    timed(c, 0) { check(t.echo_string(s).size == s.size, "echo_string") }
  when "octets"
    v = "\x01".b * n
    timed(c, 2 * n) { check(t.echo_octets(v).size == n, "echo_octets") }
  when "put"
    v = "\x01".b * n
    timed(c, n) { check(t.put_octets(v) == n, "put_octets") }
  when "get" then timed(c, n) { check(t.get_octets(n).size == n, "get_octets") }
  when "samples"
    v = (0...n).map { |i| Bench::Sample.new(i, 1.5, 2.5, 3.5, "sample-#{i}") }
    timed(c, 0) { check(t.echo_samples(v).size == n, "echo_samples") }
  when "any"
    a = CORBA::Any.to_any(Bench::Sample.new(1, 1.5, 2.5, 3.5, "sample"), Bench::Sample._tc)
    timed(c, 0) { check(t.echo_any(a).value.id == 1, "echo_any") }
  when "oneway"
    t.take_posted
    start = now
    stop = start + SECONDS
    sent = 0
    while now < stop
      64.times do
        t.post(sent)
        sent += 1
      end
    end
    send_took = now - start
    got = t.take_posted
    deadline = now + 10
    while got < sent && now < deadline
      sleep 0.001
      got += t.take_posted
    end
    took = now - start
    out({ case: c, calls: sent, seconds: took.round(3), rate: (sent / took).round(1),
          send_rate: (sent / send_took).round(1), received: got, total_calls: sent })
    check(got == sent, "oneway")
  when "threads"
    stop = false
    counts = Array.new([n, 1].max, 0)
    errors = []
    hs = counts.each_index.map do |w|
      Thread.new do
        until stop
          check(t.echo_long(w) == w, "echo_long")
          counts[w] += 1
        end
      rescue StandardError => e
        errors << e
      end
    end
    sleep WARMUP
    c0 = counts.sum
    start = now
    sleep SECONDS
    c1 = counts.sum
    took = now - start
    stop = true
    hs.each(&:join)
    raise errors.first unless errors.empty?

    out({ case: c, calls: c1 - c0, seconds: took.round(3), rate: ((c1 - c0) / took).round(1),
          total_calls: counts.sum })
  else
    raise "unknown case #{c}"
  end
end

orb = CORBA.ORB_init(orb_args)
target =
  if %w[serve inproc].include?(mode)
    poa = orb.resolve_initial_references("RootPOA")
    oid = poa.activate_object(TargetServant.new(orb))
    ref = poa.id_to_reference(oid)
    poa.the_POAManager.activate
    if mode == "serve"
      puts orb.object_to_string(ref)
      $stdout.flush
      orb.run
      orb.destroy
      exit 0
    end
    Bench::Target._narrow(ref)
  else
    Bench::Target._narrow(orb.string_to_object(opts["--ior"]))
  end
status = 0
opts["--cases"].split(",").reject(&:empty?).each do |c|
  run_case(target, c)
rescue StandardError, CORBA::Exception => e
  warn "bench.rb: #{c}: #{e.class}: #{e.message}"
  status = 1
  break
end
orb.destroy
exit status
