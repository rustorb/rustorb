"""Python program of the rustorb / omniORB comparison benchmark
(interop/bench, docs/omniorb-comparison.md): the same cases, options and
output as the Rust and C++ programs, on the standard Python mapping, so the
one script runs on rustorbpy and on omniORBpy.

    bench.py serve  [--orb rustorb|omniorb] [ORB options]
    bench.py client --ior IOR [options] [ORB options]
    bench.py inproc [options] [ORB options]

The stubs of Bench.idl must be importable (rustorb-idl -bpython for
rustorbpy, omniidl -bpython for omniORBpy; run-bench.py generates them).
"""

import json
import sys
import threading
import time

DEFAULT_CASES = ("ping,long,string,octets:64,octets:1024,octets:16384,octets:262144,"
                 "octets:1048576,octets:8388608,put:1048576,get:1048576,samples:100,any,"
                 "oneway,threads:4")

args = sys.argv[1:]
if not args:
    sys.exit("usage: bench.py serve|client|inproc [options]")
mode = args.pop(0)
opts = {"--ior": "", "--cases": DEFAULT_CASES, "--seconds": "2", "--warmup": "0.3",
        "--orb": "rustorb"}
orb_args = [sys.argv[0]]
i = 0
while i < len(args):
    if args[i] in opts and i + 1 < len(args):
        opts[args[i]] = args[i + 1]
        i += 2
    else:
        orb_args.append(args[i])
        i += 1
SECONDS = float(opts["--seconds"])
WARMUP = float(opts["--warmup"])

if opts["--orb"] == "omniorb":
    from omniORB import CORBA, PortableServer
else:
    from rustorb import CORBA, PortableServer
import Bench
import Bench__POA


class TargetImpl(Bench__POA.Target):
    def __init__(self, orb):
        self.orb = orb
        self.posted = 0
        self.lock = threading.Lock()

    def ping(self):
        pass

    def echo_long(self, v):
        return v

    def echo_string(self, v):
        return v

    def echo_octets(self, v):
        return v

    def put_octets(self, v):
        return len(v)

    def get_octets(self, n):
        return b"\x07" * n

    def echo_samples(self, v):
        return v

    def echo_any(self, v):
        return v

    def post(self, v):
        with self.lock:
            self.posted += 1

    def take_posted(self):
        with self.lock:
            n, self.posted = self.posted, 0
        return n

    def shutdown(self):
        threading.Thread(target=self.orb.shutdown, args=(False,)).start()


def out(d):
    print(json.dumps(d), flush=True)


def report(case, lat, took, nbytes, warm):
    lat.sort()
    n = len(lat)

    def pct(p):
        return round(lat[int((n - 1) * p)] * 1e6, 2)

    rate = n / took
    out({"case": case, "calls": n, "seconds": round(took, 3), "rate": round(rate, 1),
         "mean_us": round(sum(lat) / n * 1e6, 2), "p50_us": pct(0.5), "p90_us": pct(0.9),
         "p99_us": pct(0.99), "max_us": pct(1.0), "mb_per_s": round(rate * nbytes / 1e6, 2),
         "total_calls": n + warm})


def timed(case, nbytes, call):
    clock = time.perf_counter
    warm_end = clock() + WARMUP
    warm = 0
    while clock() < warm_end:
        call()
        warm += 1
    lat = []
    push = lat.append
    start = clock()
    end = start + SECONDS
    while True:
        t = clock()
        call()
        now = clock()
        push(now - t)
        if now >= end:
            break
    report(case, lat, clock() - start, nbytes, warm)


def check(ok, what):
    if not ok:
        raise RuntimeError(what + " mismatch")


def run_case(t, case):
    name, _, arg = case.partition(":")
    n = int(arg) if arg else 0
    if name == "first":
        # The first call on a fresh reference: the connection is made.
        t0 = time.perf_counter()
        t.ping()
        us = round((time.perf_counter() - t0) * 1e6, 2)
        out({"case": "first", "calls": 1, "p50_us": us, "mean_us": us})
    elif name == "ping":
        timed(case, 0, t.ping)
    elif name == "long":
        def call():
            check(t.echo_long(12345) == 12345, "echo_long")
        timed(case, 0, call)
    elif name == "string":
        s = "x" * (n or 100)

        def call():
            check(len(t.echo_string(s)) == len(s), "echo_string")
        timed(case, 0, call)
    elif name == "octets":
        v = b"\x01" * n

        def call():
            check(len(t.echo_octets(v)) == n, "echo_octets")
        timed(case, 2 * n, call)
    elif name == "put":
        v = b"\x01" * n

        def call():
            check(t.put_octets(v) == n, "put_octets")
        timed(case, n, call)
    elif name == "get":
        def call():
            check(len(t.get_octets(n)) == n, "get_octets")
        timed(case, n, call)
    elif name == "samples":
        v = [Bench.Sample(i, 1.5, 2.5, 3.5, "sample-%d" % i) for i in range(n)]

        def call():
            check(len(t.echo_samples(v)) == n, "echo_samples")
        timed(case, 0, call)
    elif name == "any":
        a = CORBA.Any(Bench._tc_Sample, Bench.Sample(1, 1.5, 2.5, 3.5, "sample"))

        def call():
            r = t.echo_any(a)
            check(r.value().id == 1, "echo_any")
        timed(case, 0, call)
    elif name == "oneway":
        t.take_posted()
        start = time.perf_counter()
        end = start + SECONDS
        sent = 0
        post = t.post
        while time.perf_counter() < end:
            for _ in range(64):
                post(sent)
                sent += 1
        send_took = time.perf_counter() - start
        got = t.take_posted()
        deadline = time.perf_counter() + 10
        while got < sent and time.perf_counter() < deadline:
            time.sleep(0.001)
            got += t.take_posted()
        took = time.perf_counter() - start
        out({"case": case, "calls": sent, "seconds": round(took, 3),
             "rate": round(sent / took, 1), "send_rate": round(sent / send_took, 1),
             "received": got, "total_calls": sent})
        check(got == sent, "oneway")
    elif name == "threads":
        stop = threading.Event()
        counts = [0] * max(n, 1)
        errors = []

        def worker(w):
            try:
                while not stop.is_set():
                    check(t.echo_long(w) == w, "echo_long")
                    counts[w] += 1
            except Exception as e:  # noqa: BLE001
                errors.append(e)

        hs = [threading.Thread(target=worker, args=(w,)) for w in range(max(n, 1))]
        for h in hs:
            h.start()
        time.sleep(WARMUP)
        c0 = sum(counts)
        start = time.perf_counter()
        time.sleep(SECONDS)
        c1 = sum(counts)
        took = time.perf_counter() - start
        stop.set()
        for h in hs:
            h.join()
        if errors:
            raise errors[0]
        out({"case": case, "calls": c1 - c0, "seconds": round(took, 3),
             "rate": round((c1 - c0) / took, 1), "total_calls": sum(counts)})
    else:
        raise RuntimeError("unknown case " + case)


def main():
    orb = CORBA.ORB_init(orb_args, CORBA.ORB_ID)
    if mode in ("serve", "inproc"):
        poa = orb.resolve_initial_references("RootPOA")
        servant = TargetImpl(orb)
        oid = poa.activate_object(servant)
        ref = poa.id_to_reference(oid)
        poa._get_the_POAManager().activate()
        target = ref._narrow(Bench.Target)
        if mode == "serve":
            print(orb.object_to_string(ref), flush=True)
            orb.run()
            return 0
    elif mode == "client":
        target = orb.string_to_object(opts["--ior"])._narrow(Bench.Target)
    else:
        sys.exit("unknown mode " + mode)
    for case in [c for c in opts["--cases"].split(",") if c]:
        try:
            run_case(target, case)
        except Exception as e:  # noqa: BLE001
            print("bench.py: %s: %r" % (case, e), file=sys.stderr)
            return 1
    orb.destroy()
    return 0


sys.exit(main())
