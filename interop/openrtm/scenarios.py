"""Runs examples of OpenRTM-aist-Python with each RTC on rustorb (through
the omniORBpy compatibility package) or on omniORBpy, and checks that data
and service calls go through.

    scenarios.py --openrtm DIR --rustorb DIR --compat DIR --rustorb-stubs DIR
                 --omniorbpy DIR --omniorbpy-stubs DIR --out DIR
                 [--python PY] [--port N] [--only SCENARIO] [--log-level LEVEL]

Each scenario starts two RTCs of an example, A and B, connects their
ports and activates them with connect.py (on B's ORB), and reads B's
output. It runs for the four combinations of ORBs. omniNames must be on
the PATH; it is started on --port.
"""

import argparse
import os
import re
import signal
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))


def simpleio_ok(out):
    return len(re.findall(r"^Received:\s+\d+$", out, re.M)) >= 5


def seqio_ok(out):
    return out.count("Sequence Data") >= 3 and re.search(r"^\s+9\s+\d+\[", out, re.M)


SERVICE_INPUT = [
    "set_value 1.5",
    "set_value 2.5",
    "get_value",
    "get_value_history",
    "echo hello",
    "get_echo_history",
    "get_value",
]


def service_ok(out):
    return all(
        re.search(p, out)
        for p in (
            r"Set remote value:\s+1\.5",
            r"Current remote value:\s+2\.5",
            r"0 :\s+1\.5\s+1 :\s+2\.5",
            r"0 :\s+hello",
        )
    )


SCENARIOS = {
    # Data port (TimedLong), A's port "out" to B's port "in".
    "simpleio": dict(
        dir="SimpleIO", a="ConsoleIn.py", b="ConsoleOut.py",
        names=("ConsoleIn0.rtc", "ConsoleOut0.rtc"), pairs=["out:in"],
        a_input="".join("%d\n" % i for i in range(1, 100001)),
        check=simpleio_ok, secs=3,
    ),
    # Ten data ports of basic types and sequences.
    "seqio": dict(
        dir="SeqIO", a="SeqOut.py", b="SeqIn.py",
        names=("SequenceOutComponent0.rtc", "SequenceInComponent0.rtc"), pairs=[],
        check=seqio_ok, secs=3,
    ),
    # Service port: B calls A's MyService.
    "simpleservice": dict(
        dir="SimpleService", a="MyServiceProvider.py", b="MyServiceConsumer.py",
        names=("MyServiceProvider0.rtc", "MyServiceConsumer0.rtc"), pairs=[],
        b_input=SERVICE_INPUT, check=service_ok, secs=2,
    ),
}

IMPLS = ("rustorb", "omniorbpy")


class Runner(object):
    def __init__(self, args):
        self.args = args
        self.python = args.python
        openrtm = os.path.abspath(args.openrtm)
        self.examples = os.path.join(openrtm, "OpenRTM_aist", "examples")
        common = [openrtm, os.path.join(openrtm, "OpenRTM_aist"),
                  os.path.join(openrtm, "OpenRTM_aist", "utils")]
        self.paths = {
            "rustorb": [args.compat, args.rustorb, args.rustorb_stubs] + common,
            "omniorbpy": [args.omniorbpy, args.omniorbpy_stubs] + common,
        }
        self.nameserver = "localhost:%d" % args.port
        self.out = os.path.abspath(args.out)
        os.makedirs(os.path.join(self.out, "logs"), exist_ok=True)
        self.conf = os.path.join(self.out, "rtc.conf")
        with open(self.conf, "w") as f:
            f.write("corba.nameservers: %s\n" % self.nameserver)
            f.write("naming.formats: %n.rtc\n")
            if args.log_level:
                # OpenRTM-aist's own log, into the output of each RTC.
                f.write("logger.enable: YES\nlogger.file_name: STDOUT\n")
                f.write("logger.log_level: %s\n" % args.log_level)
            else:
                f.write("logger.enable: NO\n")

    def env(self, impl):
        env = dict(os.environ)
        env["PYTHONPATH"] = os.pathsep.join(os.path.abspath(p) for p in self.paths[impl])
        env["PYTHONUNBUFFERED"] = "1"
        return env

    def start(self, impl, cwd, script, log, stdin):
        return subprocess.Popen(
            [self.python, script, "-f", self.conf], cwd=cwd, env=self.env(impl),
            stdin=stdin, stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
        )

    def run(self, name, impl_a, impl_b):
        sc = SCENARIOS[name]
        cwd = os.path.join(self.examples, sc["dir"])
        tag = "%s-%s-%s" % (name, impl_a, impl_b)
        logs = os.path.join(self.out, "logs")
        a_log = open(os.path.join(logs, tag + "-a.log"), "w")
        b_log = open(os.path.join(logs, tag + "-b.log"), "w")
        procs = []
        try:
            a_in = subprocess.DEVNULL
            if "a_input" in sc:
                path = os.path.join(self.out, name + "-a.in")
                with open(path, "w") as f:
                    f.write(sc["a_input"])
                a_in = open(path)
            procs.append(self.start(impl_a, cwd, sc["a"], a_log, a_in))
            b_in = subprocess.PIPE if "b_input" in sc else subprocess.DEVNULL
            b = self.start(impl_b, cwd, sc["b"], b_log, b_in)
            procs.append(b)
            conn = subprocess.run(
                [self.python, os.path.join(HERE, "connect.py"), self.nameserver]
                + list(sc["names"]) + sc["pairs"],
                env=self.env(impl_b), stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                timeout=60, universal_newlines=True,
            )
            with open(os.path.join(logs, tag + "-connect.log"), "w") as f:
                f.write(conn.stdout)
            if conn.returncode != 0:
                return False, "connect failed"
            for line in sc.get("b_input", []):
                time.sleep(0.5)
                b.stdin.write((line + "\n").encode())
                b.stdin.flush()
            time.sleep(sc["secs"])
        finally:
            for p in procs:
                try:
                    os.killpg(p.pid, signal.SIGTERM)
                except OSError:
                    pass
            for p in procs:
                try:
                    p.wait(10)
                except subprocess.TimeoutExpired:
                    os.killpg(p.pid, signal.SIGKILL)
                    p.wait()
            a_log.close()
            b_log.close()
        with open(os.path.join(logs, tag + "-b.log"), errors="replace") as f:
            out = f.read()
        if not sc["check"](out):
            return False, "unexpected output of %s (logs/%s-b.log)" % (sc["b"], tag)
        return True, ""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--openrtm", required=True)
    ap.add_argument("--rustorb", required=True)
    ap.add_argument("--compat", required=True)
    ap.add_argument("--rustorb-stubs", required=True)
    ap.add_argument("--omniorbpy", required=True)
    ap.add_argument("--omniorbpy-stubs", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--python", default=sys.executable)
    ap.add_argument("--port", type=int, default=2809)
    ap.add_argument("--only", action="append", choices=sorted(SCENARIOS))
    ap.add_argument("--log-level", help="enable the log of OpenRTM-aist (DEBUG, TRACE...)")
    args = ap.parse_args()

    runner = Runner(args)
    naming_log = open(os.path.join(runner.out, "logs", "omniNames.log"), "w")
    naming_dir = os.path.join(runner.out, "naming")
    os.makedirs(naming_dir, exist_ok=True)
    for f in os.listdir(naming_dir):
        os.remove(os.path.join(naming_dir, f))
    naming = subprocess.Popen(
        ["omniNames", "-start", str(args.port), "-logdir", naming_dir],
        stdout=naming_log, stderr=subprocess.STDOUT,
    )
    failed = 0
    try:
        time.sleep(1)
        for name in args.only or sorted(SCENARIOS):
            for impl_a in IMPLS:
                for impl_b in IMPLS:
                    ok, why = runner.run(name, impl_a, impl_b)
                    print("%-6s %-14s A=%-9s B=%-9s %s" % (
                        "ok" if ok else "FAIL", name, impl_a, impl_b, why))
                    sys.stdout.flush()
                    failed += not ok
    finally:
        naming.terminate()
        naming.wait()
    print("\n%d failed" % failed)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
