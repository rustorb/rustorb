#!/usr/bin/env python3
"""Runs the load, soak and leak tests natively on this machine, without
Docker, and writes the results with the machine's details (issue #63).

    run-load-local.py [--tests LIST] [--seconds S] [--threads N]
                      [--connections N] [--servers LIST] [--clients LIST]
                      [--stress-scale N] [--soak-secs S] [--leak-rounds N]
                      [--out DIR] [--no-build]
    run-load-local.py --serve --host ADDR [--servers LIST]
    run-load-local.py --server-host ADDR [--clients LIST] [--servers LIST]

--tests picks what runs (comma separated; default load,stress,soak):

  load         each load client against each Echo server, one server at a
               time, with rustorb-names as the naming service: the phases of
               impls/rustorb/src/load_client.rs (small, string, connections,
               large; `connections` from the Rust client only)
  stress       crates/rustorb-core/tests/stress.rs (RUSTORB_STRESS_SCALE)
  soak         crates/rustorb-core/tests/soak.rs (RUSTORB_SOAK_SECS)
  leak-python  python/tests/test_leak.py (RUSTORB_LEAK_ROUNDS)
  leak-cpp     cpp/tests/test_leak.cc with AddressSanitizer / LeakSanitizer
  leak-java    java LeakTest
  leak         the three leak checks; all: everything

The servers and clients are rustorb (Rust), rustorbpy, rustorbcpp and
rustorbjava, and omniorb-cpp and omniorbpy (servers; omniorbpy also as a
client) when omniORB is installed: omniidl on the PATH, and the omniORB
Python package importable for omniorbpy. An implementation whose tools are
missing is skipped and listed as such. The Rust ones need only cargo; the
others are built by their impls/*/build.sh, which need bash and idl_parser
(importable, or --idl-parser DIR / IDL_PARSER_DIR).

Across two machines: on the server machine, `--serve --host ADDR` starts
rustorb-names and every server, published at ADDR (the address the other
machine reaches), and waits until interrupted; on the client machine,
`--server-host ADDR` runs the load clients against them (servers that are
not bound there are skipped).

The results go to DIR (default interop/out/load-local/DATE-TIME):
result.json, result.md (the table docs/load-testing.md takes) and the logs.
A failed call or test makes the exit status 1; the rates are only recorded.
"""

import argparse
import datetime
import json
import os
import platform
import re
import shutil
import signal
import socket
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
IMPLS = os.path.join(HERE, "impls")
IDL = os.path.join(HERE, "idl")
WINDOWS = os.name == "nt"
EXE = ".exe" if WINDOWS else ""

SERVERS = ["rustorb", "rustorbpy", "rustorbcpp", "rustorbjava", "omniorb-cpp", "omniorbpy"]
CLIENTS = ["rustorb", "rustorbpy", "rustorbcpp", "rustorbjava", "omniorbpy"]
PHASES = ["small", "string", "connections", "large"]
TESTS = ["load", "stress", "soak", "leak-python", "leak-cpp", "leak-java"]
NAMING_PORT = 2809
SERVER_PORT = 2810


def parse_args():
    p = argparse.ArgumentParser(
        description=__doc__.split("\n\n")[0],
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="\n\n".join(__doc__.split("\n\n")[2:]))
    p.add_argument("--tests", default="load,stress,soak")
    p.add_argument("--seconds", type=float, default=5, help="length of each load phase")
    p.add_argument("--threads", type=int, default=8)
    p.add_argument("--connections", type=int, default=16)
    p.add_argument("--servers", help="comma separated (default: every one available)")
    p.add_argument("--clients", help="comma separated (default: every one available)")
    p.add_argument("--stress-scale", type=int, default=1, help="RUSTORB_STRESS_SCALE")
    p.add_argument("--soak-secs", type=int, default=60, help="RUSTORB_SOAK_SECS")
    p.add_argument("--leak-rounds", type=int, default=1, help="RUSTORB_LEAK_ROUNDS")
    p.add_argument("--serve", action="store_true",
                   help="start the naming service and the servers only, and wait")
    p.add_argument("--host", help="address the servers listen on and publish (--serve)")
    p.add_argument("--server-host", help="run the load clients against the servers there")
    p.add_argument("--naming-port", type=int, default=NAMING_PORT)
    p.add_argument("--idl-parser", help="a checkout of idl_parser (else IDL_PARSER_DIR)")
    p.add_argument("--out", help="results directory")
    p.add_argument("--no-build", action="store_true", help="use what is built already")
    a = p.parse_args()
    tests = []
    for t in a.tests.split(","):
        t = t.strip()
        if t == "all":
            tests += TESTS
        elif t == "leak":
            tests += ["leak-python", "leak-cpp", "leak-java"]
        elif t in TESTS:
            tests.append(t)
        elif t:
            p.error("unknown test %r" % t)
    a.tests = [t for t in TESTS if t in tests]
    if a.serve and not a.host:
        p.error("--serve needs --host")
    if a.serve and a.server_host:
        p.error("--serve and --server-host do not go together")
    a.servers = names(p, a.servers, SERVERS)
    a.clients = names(p, a.clients, CLIENTS)
    return a


def names(parser, value, known):
    if not value:
        return None
    out = [v.strip() for v in value.split(",") if v.strip()]
    for v in out:
        if v not in known:
            parser.error("unknown implementation %r (known: %s)" % (v, ", ".join(known)))
    return out


def log(msg):
    print(msg, flush=True)


def run(cmd, **kw):
    """Runs cmd, echoed, and raises if it fails."""
    log("$ " + " ".join(cmd))
    subprocess.run(cmd, check=True, **kw)


def capture(cmd):
    try:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=30).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""


# ---------------------------------------------------------------------------
# The implementations: what each needs, how to build it and how to start its
# server and its load client.

class Impls:
    def __init__(self, args, env, logs):
        self.args = args
        self.env = env
        self.logs = logs
        self.skipped = {}  # implementation -> why
        self.built = set()

    def bin(self, name):
        return os.path.join(ROOT, "target", "release", name + EXE)

    def need(self, impl, *tools):
        for t in tools:
            if not shutil.which(t):
                self.skipped[impl] = "%s not found" % t
                return False
        return True

    def idl_parser_ok(self, impl):
        code = "import idl_parser"
        r = subprocess.run([sys.executable, "-c", code], env=self.env, capture_output=True)
        if r.returncode != 0:
            self.skipped[impl] = "idl_parser not importable (--idl-parser DIR)"
            return False
        return True

    def omniorb_prefix(self):
        omniidl = shutil.which("omniidl")
        return os.path.dirname(os.path.dirname(omniidl)) if omniidl else None

    def build(self, impl):
        """Builds impl if its tools are there. Returns whether it can run."""
        if impl in self.built:
            return True
        if impl in self.skipped:
            return False
        try:
            ok = self._build(impl)
        except subprocess.CalledProcessError as e:
            self.skipped[impl] = "build failed (%s)" % " ".join(e.cmd[:2])
            return False
        if ok:
            self.built.add(impl)
        return ok

    def _build(self, impl):
        a = self.args
        bash = shutil.which("bash")
        if impl == "rustorb":
            if not a.no_build:
                run(["cargo", "build", "--release", "--locked", "-p", "rustorb-interop",
                     "--bin", "rustorb-interop-server", "--bin", "rustorb-load-client"], cwd=ROOT)
            return True
        if impl in ("rustorbpy", "rustorbcpp", "rustorbjava"):
            if WINDOWS:
                self.skipped[impl] = "its build script is for Linux and macOS"
                return False
            tools = {"rustorbpy": ["bash"], "rustorbcpp": ["bash"],
                     "rustorbjava": ["bash", "javac", "java"]}[impl]
            if impl == "rustorbcpp":
                tools.append("c++" if platform.system() == "Darwin" else
                             os.environ.get("CXX", "g++"))
            if not self.need(impl, *tools):
                return False
            if a.no_build:
                return True
            if not self.idl_parser_ok(impl):
                return False
            run([bash, os.path.join(IMPLS, impl, "build.sh")], env=self.env)
            return True
        if impl == "omniorbpy":
            if not self.need(impl, "omniidl"):
                return False
            r = subprocess.run([sys.executable, "-c", "import omniORB"], env=self.env,
                               capture_output=True)
            if r.returncode != 0:
                self.skipped[impl] = "the omniORB Python package is not importable"
                return False
            stubs = os.path.join(IMPLS, "omniorbpy", "stubs")
            if not a.no_build or not os.path.isdir(stubs):
                os.makedirs(stubs, exist_ok=True)
                run(["omniidl", "-bpython", "-C", stubs, "-I" + IDL,
                     os.path.join(IDL, "InteropTest.idl")], env=self.env)
            return True
        if impl == "omniorb-cpp":
            if not self.need(impl, "omniidl", "make"):
                return False
            if not a.no_build:
                run(["make", "-C", os.path.join(IMPLS, "omniorb-cpp"),
                     "OMNIORB=" + self.omniorb_prefix(), "IDLDIR=" + IDL, "server"],
                    env=self.env)
            return True
        raise AssertionError(impl)

    def server_cmd(self, impl):
        if impl == "rustorb":
            return [self.bin("rustorb-interop-server")]
        if impl in ("rustorbpy", "omniorbpy"):
            return [sys.executable, os.path.join(IMPLS, impl, "server.py")]
        if impl == "rustorbcpp":
            return [os.path.join(IMPLS, impl, "bin", "server")]
        if impl == "rustorbjava":
            d = os.path.join(IMPLS, impl)
            return [os.environ.get("RUSTORB_JAVA", "java"), "--enable-native-access=ALL-UNNAMED",
                    "-Djava.library.path=" + os.path.join(d, "lib"),
                    "-cp", os.path.join(d, "classes"), "interop.Server"]
        if impl == "omniorb-cpp":
            return [os.path.join(IMPLS, impl, "server")]
        raise AssertionError(impl)

    def client_cmd(self, impl):
        if impl == "rustorb":
            return [self.bin("rustorb-load-client"), "--connections", str(self.args.connections)]
        if impl in ("rustorbpy", "omniorbpy"):
            return [sys.executable, os.path.join(HERE, "scripts", "load-client"), "--impl", impl]
        if impl == "rustorbcpp":
            return [os.path.join(IMPLS, impl, "bin", "load_client")]
        if impl == "rustorbjava":
            d = os.path.join(IMPLS, impl)
            return [os.environ.get("RUSTORB_JAVA", "java"), "--enable-native-access=ALL-UNNAMED",
                    "-Djava.library.path=" + os.path.join(d, "lib"),
                    "-cp", os.path.join(d, "classes"), "interop.LoadClient"]
        raise AssertionError(impl)


def omniorb_env(env):
    """omniORB's libraries and Python package next to its omniidl."""
    omniidl = shutil.which("omniidl")
    if not omniidl:
        return env
    prefix = os.path.dirname(os.path.dirname(omniidl))
    env = dict(env)
    lib = os.path.join(prefix, "lib")
    var = "DYLD_LIBRARY_PATH" if platform.system() == "Darwin" else "LD_LIBRARY_PATH"
    if os.path.isdir(lib):
        env[var] = os.pathsep.join(filter(None, [lib, env.get(var)]))
    for py in (os.path.join(prefix, "python"),
               os.path.join(prefix, "lib", "python%d.%d" % sys.version_info[:2], "site-packages")):
        if os.path.isdir(py):
            env["PYTHONPATH"] = os.pathsep.join(filter(None, [env.get("PYTHONPATH"), py]))
    return env


# ---------------------------------------------------------------------------
# Processes

class Background:
    """A process that runs until stop(), with its output in a log file."""

    def __init__(self, cmd, logfile, env):
        log("$ " + " ".join(cmd) + " &")
        self.logfile = logfile
        self.out = open(logfile, "w", encoding="utf-8")
        kw = {}
        if WINDOWS:
            kw["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
        else:
            kw["start_new_session"] = True
        self.proc = subprocess.Popen(cmd, stdout=self.out, stderr=subprocess.STDOUT, env=env,
                                     cwd=HERE, **kw)

    def alive(self):
        return self.proc.poll() is None

    def stop(self):
        if self.alive():
            if WINDOWS:
                self.proc.terminate()
            else:
                os.killpg(self.proc.pid, signal.SIGTERM)
            try:
                self.proc.wait(10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()
        self.out.close()


def wait_for_port(host, port, proc, seconds=30):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if proc is not None and not proc.alive():
            return False
        try:
            with socket.create_connection((host, port), timeout=1):
                return True
        except OSError:
            time.sleep(0.2)
    return False


def start_naming(args, host, out, env):
    port = args.naming_port
    names = os.path.join(ROOT, "target", "release", "rustorb-names" + EXE)
    if not args.no_build:
        run(["cargo", "build", "--release", "--locked", "-p", "rustorb-naming",
             "--bin", "rustorb-names"], cwd=ROOT)
    naming = Background([names, "-memory", "-start", str(port), "-ignoreport",
                         "-ORBendPoint", "giop:tcp:%s:%d" % (host, port)],
                        os.path.join(out, "naming.log"), env)
    if not wait_for_port(host, port, naming):
        naming.stop()
        raise SystemExit("rustorb-names did not start; see %s" % naming.logfile)
    return naming


def start_server(impls, impl, host, port, naming, out, env):
    return Background(impls.server_cmd(impl) + [
        "--naming", naming, "--name", "load/" + impl, "--host", host, "--port", str(port)],
        os.path.join(out, "server-%s.log" % impl), env)


LINE = re.compile(r"^(\w+) calls=(\d+) errors=(\d+) rate=(\d+)/s")


def run_client(impls, client, server, naming, args, out, env, timeout):
    """One load client against one server: returns the result entry."""
    cmd = impls.client_cmd(client) + [
        "--naming", naming, "--name", "load/" + server, "--seconds", "%g" % args.seconds,
        "--threads", str(args.threads), "--timeout", str(timeout)]
    logfile = os.path.join(out, "%s-to-%s.txt" % (client, server))
    log("== %s -> %s" % (client, server))
    log("$ " + " ".join(cmd))
    # Every phase plus the start of the JVM or Python, with room to spare.
    limit = timeout + 4 * args.seconds * 3 + 120
    try:
        r = subprocess.run(cmd, capture_output=True, encoding="utf-8", errors="replace", env=env,
                           cwd=HERE, timeout=limit)
        code, stdout, stderr = r.returncode, r.stdout, r.stderr
    except subprocess.TimeoutExpired as e:
        text = [v.decode(errors="replace") if isinstance(v, bytes) else v or ""
                for v in (e.stdout, e.stderr)]
        code, stdout, stderr = None, text[0], text[1] + "\ntimed out"
    with open(logfile, "w", encoding="utf-8") as f:
        f.write(stdout)
        if stderr:
            f.write("\n--- stderr\n" + stderr)
    sys.stdout.write(stdout)
    phases = {}
    for line in stdout.splitlines():
        m = LINE.match(line)
        if m:
            phases[m.group(1)] = {"calls": int(m.group(2)), "errors": int(m.group(3)),
                                  "rate": int(m.group(4))}
    status = {0: "ok", 2: "not-found"}.get(code, "failed")
    if status != "ok":
        sys.stdout.write(stderr[-2000:])
    return {"client": client, "server": server, "status": status, "exit": code,
            "phases": phases, "log": os.path.relpath(logfile, out)}


# ---------------------------------------------------------------------------
# The tests

def load_local(args, impls, out, env):
    host = "127.0.0.1"
    naming = "corbaloc::%s:%d/NameService" % (host, args.naming_port)
    servers = [s for s in (args.servers or SERVERS) if impls.build(s)]
    clients = [c for c in (args.clients or CLIENTS) if impls.build(c)]
    results = []
    ns = start_naming(args, host, out, env)
    try:
        for server in servers:
            proc = start_server(impls, server, host, SERVER_PORT, naming, out, env)
            try:
                if not wait_for_port(host, SERVER_PORT, proc, 60):
                    log("server %s did not start; see %s" % (server, proc.logfile))
                    results += [{"client": c, "server": server, "status": "failed",
                                 "exit": None, "phases": {}, "log": None,
                                 "note": "the server did not start"} for c in clients]
                    continue
                for client in clients:
                    results.append(run_client(impls, client, server, naming, args, out, env, 60))
            finally:
                proc.stop()
    finally:
        ns.stop()
    return results


def load_remote(args, impls, out, env):
    host = args.server_host
    naming = "corbaloc::%s:%d/NameService" % (host, args.naming_port)
    clients = [c for c in (args.clients or CLIENTS) if impls.build(c)]
    results = []
    for server in args.servers or SERVERS:
        missing = False
        for i, client in enumerate(clients):
            # The first client finds out whether the server is there.
            r = run_client(impls, client, server, naming, args, out, env, 10 if i == 0 else 30)
            if r["status"] == "not-found" and i == 0:
                log("load/%s is not bound at %s; skipped" % (server, host))
                missing = True
                break
            results.append(r)
        if missing:
            impls.skipped["%s server" % server] = "not bound on the server machine"
    return results


def serve(args, impls, out, env):
    host = args.host
    naming = "corbaloc::%s:%d/NameService" % (host, args.naming_port)
    servers = [s for s in (args.servers or SERVERS) if impls.build(s)]
    procs = [start_naming(args, host, out, env)]
    try:
        for i, server in enumerate(servers):
            procs.append(start_server(impls, server, host, SERVER_PORT + i, naming, out, env))
        for server, proc in zip(servers, procs[1:]):
            port = SERVER_PORT + servers.index(server)
            log("%s: %s" % (server, "listening on %s:%d" % (host, port)
                            if wait_for_port(host, port, proc, 60) else
                            "did not start; see " + proc.logfile))
        for impl, why in impls.skipped.items():
            log("%s: skipped (%s)" % (impl, why))
        log("naming service %s; on the client machine run\n"
            "  %s --server-host %s\nCtrl-C stops the servers." % (naming, sys.argv[0], host))
        while all(p.alive() for p in procs[:1]):
            time.sleep(1)
    except KeyboardInterrupt:
        pass
    finally:
        for p in reversed(procs):
            p.stop()


def cargo_test(name, var, value, out, env):
    """One of the rustorb-core load tests in release, with its output."""
    env = dict(env, **{var: str(value)})
    cmd = ["cargo", "test", "--release", "--locked", "-p", "rustorb-core", "--test", name,
           "--", "--nocapture", "--test-threads=1"]
    return run_test(name, cmd, out, env, ROOT, notes=notes_of_cargo_test)


def notes_of_cargo_test(text):
    """What the tests print, without cargo's own lines."""
    notes = []
    for line in text.splitlines():
        if line.startswith("test ") and " ... " in line:
            # With --nocapture, the first line a test prints follows its name.
            line = line.split(" ... ", 1)[1]
        if line in ("ok", "FAILED", "ignored"):
            continue
        if line.strip() and not line.startswith((
                "running ", "test result", "     Running", "   Compiling", "    Finished",
                "warning", "   Doc-tests")):
            notes.append(line)
    return notes


def run_test(name, cmd, out, env, cwd, notes=None):
    logfile = os.path.join(out, "%s.log" % name)
    log("== %s" % name)
    log("$ " + " ".join(cmd))
    start = time.monotonic()
    with open(logfile, "w") as f:
        r = subprocess.run(cmd, stdout=f, stderr=subprocess.STDOUT, env=env, cwd=cwd)
    took = time.monotonic() - start
    with open(logfile, encoding="utf-8", errors="replace") as f:
        text = f.read()
    status = "ok" if r.returncode == 0 else "failed"
    log("%s: %s in %.0f s" % (name, status, took))
    if status != "ok":
        sys.stdout.write(text[-3000:])
    return {"test": name, "status": status, "seconds": round(took, 1),
            "notes": notes(text) if notes else [], "log": os.path.relpath(logfile, out)}


def leak_test(name, args, out, env):
    env = dict(env, RUSTORB_LEAK_ROUNDS=str(args.leak_rounds))
    if name == "leak-python":
        if WINDOWS or not shutil.which("bash"):
            return {"test": name, "status": "skipped", "why": "needs bash"}
        run(["bash", os.path.join(ROOT, "python", "build-native.sh"), "--release"], env=env)
        env["RUSTORB_REQUIRE_NATIVE"] = "1"
        return run_test(name, [sys.executable, "-m", "unittest", "discover", "-s",
                               os.path.join(ROOT, "python", "tests"), "-p", "test_leak.py", "-v"],
                        out, env, ROOT)
    if name == "leak-cpp":
        if WINDOWS or not shutil.which("cmake"):
            return {"test": name, "status": "skipped", "why": "needs cmake (Linux or macOS)"}
        build = os.path.join(ROOT, "build", "cpp-leak")
        cfg = ["cmake", "-S", os.path.join(ROOT, "cpp"), "-B", build,
               "-DRUSTORB_SANITIZE=address"]
        if env.get("IDL_PARSER_DIR"):
            cfg.append("-DIDL_PARSER_DIR=" + env["IDL_PARSER_DIR"])
        run(cfg, env=env)
        run(["cmake", "--build", build, "--target", "test_leak"], env=env)
        return run_test(name, ["ctest", "--test-dir", build, "-R", "leak", "--output-on-failure"],
                        out, env, ROOT)
    if name == "leak-java":
        if not shutil.which("mvn"):
            return {"test": name, "status": "skipped", "why": "needs mvn"}
        cmd = ["mvn", "-B", "-f", os.path.join(ROOT, "java", "pom.xml"),
               "-Dpython=" + sys.executable, "-Dtest=LeakTest",
               "-Dsurefire.failIfNoSpecifiedTests=false", "verify"]
        if env.get("IDL_PARSER_DIR"):
            cmd.insert(-1, "-Didl.parser.dir=" + env["IDL_PARSER_DIR"])
        return run_test(name, cmd, out, env, ROOT)
    raise AssertionError(name)


# ---------------------------------------------------------------------------
# The machine and the report

def machine():
    system = platform.system()
    cpu = platform.processor()
    memory = None
    if system == "Linux":
        try:
            with open("/proc/cpuinfo") as f:
                for line in f:
                    if line.startswith(("model name", "Model", "Hardware")):
                        cpu = line.split(":", 1)[1].strip()
                        break
            with open("/proc/meminfo") as f:
                memory = int(f.readline().split()[1]) * 1024
        except OSError:
            pass
    elif system == "Darwin":
        cpu = capture(["sysctl", "-n", "machdep.cpu.brand_string"]) or cpu
        memory = int(capture(["sysctl", "-n", "hw.memsize"]) or 0) or None
    elif system == "Windows":
        import ctypes

        class MemoryStatus(ctypes.Structure):
            _fields_ = [("dwLength", ctypes.c_ulong), ("dwMemoryLoad", ctypes.c_ulong),
                        ("ullTotalPhys", ctypes.c_ulonglong)] + [
                            (n, ctypes.c_ulonglong) for n in (
                                "ullAvailPhys", "ullTotalPageFile", "ullAvailPageFile",
                                "ullTotalVirtual", "ullAvailVirtual", "ullAvailExtendedVirtual")]
        status = MemoryStatus()
        status.dwLength = ctypes.sizeof(status)
        if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status)):
            memory = status.ullTotalPhys
    commit = capture(["git", "-C", ROOT, "rev-parse", "HEAD"])
    dirty = bool(capture(["git", "-C", ROOT, "status", "--porcelain", "--untracked-files=no"]))
    return {
        "hostname": socket.gethostname(),
        "os": platform.platform(),
        "cpu": cpu,
        "logical_cpus": os.cpu_count(),
        "memory_bytes": memory,
        "rustc": capture(["rustc", "-V"]),
        "python": platform.python_version(),
        "java": java_version(),
        "commit": commit + ("-dirty" if dirty else ""),
    }


def java_version():
    if not shutil.which("java"):
        return ""
    r = subprocess.run(["java", "-version"], capture_output=True, text=True)
    # The first line that is not a note of the JVM's (JAVA_TOOL_OPTIONS).
    return next((line for line in r.stderr.splitlines() if not line.startswith("Picked up")), "")


def markdown(result):
    m, p = result["machine"], result["parameters"]
    mem = "%.0f GiB" % (m["memory_bytes"] / 2**30) if m.get("memory_bytes") else "?"
    lines = [
        "### %s (%s)" % (m["cpu"] or "?", result["date"]),
        "",
        "- OS: %s; %s logical CPUs; %s memory" % (m["os"], m["logical_cpus"], mem),
        "- rustorb %s; %s" % (m["commit"][:12] or "?", m["rustc"]),
    ]
    if result["load"]:
        lines.append("- load: %s; %d threads, %d connections, %g s per phase" % (
            "servers on %s" % p["server_host"] if p["server_host"] else "one machine",
            p["threads"], p["connections"], p["seconds"]))
    if result["tests"]:
        lines.append("- RUSTORB_STRESS_SCALE=%d, RUSTORB_SOAK_SECS=%d, RUSTORB_LEAK_ROUNDS=%d" % (
            p["stress_scale"], p["soak_secs"], p["leak_rounds"]))
    lines.append("")
    if result["load"]:
        lines += ["| client | server | small | string | connections | large (1 MiB) |",
                  "| --- | --- | ---: | ---: | ---: | ---: |"]
        for r in result["load"]:
            if r["status"] == "ok":
                cells = [str(r["phases"][ph]["rate"]) if ph in r["phases"] else "-"
                         for ph in PHASES]
            else:
                cells = [r["status"]] + [""] * 3
            lines.append("| %s | %s | %s |" % (r["client"], r["server"], " | ".join(cells)))
        lines.append("")
    if result["tests"]:
        lines += ["| test | result | seconds |", "| --- | --- | ---: |"]
        for t in result["tests"]:
            lines.append("| %s | %s | %s |" % (
                t["test"], t["status"] + (" (%s)" % t["why"] if t.get("why") else ""),
                t.get("seconds", "")))
        lines.append("")
    if result["skipped"]:
        lines.append("Skipped: " + "; ".join("%s (%s)" % kv for kv in result["skipped"].items()))
        lines.append("")
    return "\n".join(lines)


def main():
    args = parse_args()
    # The logs echoed may hold what the console's code page cannot show.
    sys.stdout.reconfigure(errors="replace")
    if not WINDOWS:
        # Stopped with kill as with Ctrl-C: the servers it started stop too.
        signal.signal(signal.SIGTERM, signal.default_int_handler)
    env = dict(os.environ)
    if args.idl_parser:
        env["IDL_PARSER_DIR"] = os.path.abspath(args.idl_parser)
    if env.get("IDL_PARSER_DIR"):
        env["PYTHONPATH"] = os.pathsep.join(filter(None, [env["IDL_PARSER_DIR"],
                                                          env.get("PYTHONPATH")]))
    env = omniorb_env(env)
    env.setdefault("PYTHONUNBUFFERED", "1")
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    out = os.path.abspath(args.out or os.path.join(HERE, "out", "load-local", stamp))
    os.makedirs(out, exist_ok=True)
    impls = Impls(args, env, out)

    if args.serve:
        serve(args, impls, out, env)
        return 0

    result = {
        "date": datetime.datetime.now().astimezone().isoformat(timespec="seconds"),
        "machine": machine(),
        "parameters": {
            "seconds": args.seconds, "threads": args.threads, "connections": args.connections,
            "server_host": args.server_host, "stress_scale": args.stress_scale,
            "soak_secs": args.soak_secs, "leak_rounds": args.leak_rounds,
        },
        "load": [],
        "tests": [],
        "skipped": {},
    }
    if "load" in args.tests:
        if args.server_host:
            result["load"] = load_remote(args, impls, out, env)
        else:
            result["load"] = load_local(args, impls, out, env)
    if not args.server_host:
        if "stress" in args.tests:
            result["tests"].append(
                cargo_test("stress", "RUSTORB_STRESS_SCALE", args.stress_scale, out, env))
        if "soak" in args.tests:
            result["tests"].append(cargo_test("soak", "RUSTORB_SOAK_SECS", args.soak_secs, out, env))
        for t in args.tests:
            if t.startswith("leak-"):
                try:
                    result["tests"].append(leak_test(t, args, out, env))
                except subprocess.CalledProcessError as e:
                    result["tests"].append({"test": t, "status": "failed",
                                            "why": "build failed: " + " ".join(e.cmd[:3])})
    result["skipped"] = impls.skipped

    with open(os.path.join(out, "result.json"), "w", encoding="utf-8") as f:
        json.dump(result, f, indent=2)
        f.write("\n")
    md = markdown(result)
    with open(os.path.join(out, "result.md"), "w", encoding="utf-8") as f:
        f.write(md)
    log("\n" + md)
    log("results in %s" % out)
    failed = [r for r in result["load"] if r["status"] != "ok"] + \
             [t for t in result["tests"] if t["status"] == "failed"]
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
