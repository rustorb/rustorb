#!/usr/bin/env python3
"""Runs the rustorb / omniORB comparison benchmark (docs/omniorb-comparison.md)
on this machine and writes the results with the machine's details.

    run-bench.py [--impls LIST] [--modes LIST] [--pairs same|all|C:S,...]
                 [--cases LIST] [--inproc-cases LIST] [--seconds S] [--warmup S]
                 [--repeat N] [--transports tcp,unix] [--out DIR] [--no-build]
                 [--quick] [--stats]

Every implementation is one program with three modes (README.md): `inproc`
(the servant and the caller in one ORB), `serve` and `client`. The
implementations:

  rustorb      Rust       impls/rustorb/src/bench.rs (cargo)
  rustorbcpp   C++11      rustorb-cpp/bench.cc on the rustorb C++ binding
  rustorbpy    Python     python/bench.py on rustorbpy
  omniorbcpp   C++        omniorb-cpp/bench.cc on omniORB (omniidl, headers, libraries)
  rustorbjava  Java       java/BenchMain.java on the rustorb Java binding (JDK 22+)
  rustorbcs    C#         csharp/Program.cs on the rustorb .NET binding (.NET 8 SDK)
  rustorbc     C          c/bench.c on the rustorb C binding (cmake)
  rustorbrb    Ruby       ruby/bench.rb on the rustorb Ruby binding (ruby with headers)

--modes picks what runs (default inproc,pairs): `inproc` runs each case in
one process; `pairs` starts each server once per transport and runs each
client against it, one client process per case. --pairs picks the pairs:
`same` (a client against the server of its own implementation), `all` (every
client against every server, the default) or a list of CLIENT:SERVER.

The cases (--cases, default all of them) are those of the programs, plus
`procs:N` (pairs only): N client processes run `long` at once, the rate is
their sum. Each run is repeated --repeat times (default 3, a fresh server
each time); the tables show the median.

For each case in pairs mode the script also records the CPU time the client
and the server used (Linux: /proc and wait4; macOS: ps and wait4), and the
server's resident memory after start and at its peak (the peak on Linux only).

omniORB: OMNIIDL (default `omniidl`, e.g. "python3.12 /usr/bin/omniidl" when
the omniidl script's python is not the default one) and OMNIORB_PREFIX
(default: the directory above omniidl's bin). omniORBpy must be importable
(PYTHONPATH, or PREFIX/python, PREFIX/lib/pythonX.Y/site-packages).

--quick is the short set for comparing before and after a change
(docs/perf-plan.md): rustorb and omniorbcpp (those of them that can be
built), each against itself, the cases ping, any, oneway, threads:4 and
get:1048576, and ping in one process. --impls, --cases, --inproc-cases and
--pairs given with it win. compare-bench.py makes the table of the
differences between two such results.

Context switches per call (voluntary and not, client and server) are
recorded on Linux and, for the client only, on macOS. --stats also records
rustorb's internal counters per call (rustorb_core::stats: follow, claims,
reads that found nothing, the upcall pool, ...) for the rustorb client and
server.

The results go to DIR (default interop/out/bench/DATE-TIME): result.json
(every run and the medians), result.md (the tables) and the logs.
"""

import argparse
import datetime
import json
import os
import platform
import shlex
import shutil
import signal
import socket
import statistics
import subprocess
import sys
import tempfile
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
INTEROP = os.path.dirname(HERE)
ROOT = os.path.dirname(INTEROP)
IDL = os.path.join(HERE, "Bench.idl")
BUILD = os.path.join(INTEROP, "out", "bench-build")
LINUX = platform.system() == "Linux"

IMPLS = ["rustorb", "rustorbcpp", "rustorbpy", "rustorbjava", "rustorbcs", "rustorbc",
         "rustorbrb", "omniorbcpp", "omniorbpy"]
CASES = ["first", "ping", "long", "string", "octets:64", "octets:1024", "octets:16384",
         "octets:262144", "octets:1048576", "octets:8388608", "put:1048576",
         "get:1048576", "samples:100", "any", "oneway", "threads:4", "procs:4"]
QUICK_IMPLS = ["rustorb", "omniorbcpp"]
QUICK_CASES = ["ping", "any", "oneway", "threads:4", "get:1048576"]
QUICK_INPROC_CASES = ["ping"]
# Every ORB takes messages up to 64 MiB (omniORB's default is 2 MiB).
ORB_ARGS = ["-ORBgiopMaxMsgSize", "67108864"]
FIRST_RUNS = 10
CLK_TCK = os.sysconf("SC_CLK_TCK") if hasattr(os, "sysconf") else 100


def log(msg):
    print(msg, file=sys.stderr, flush=True)


def run(cmd, **kw):
    log("$ " + " ".join(cmd))
    subprocess.run(cmd, check=True, **kw)


def capture(cmd):
    try:
        return subprocess.run(cmd, capture_output=True, text=True).stdout.strip()
    except OSError:
        return ""


# ---------------------------------------------------------------------------
# Implementations

class Impls:
    def __init__(self, args):
        self.args = args
        self.skipped = {}
        self.env = dict(os.environ)
        self.omniidl = shlex.split(os.environ.get("OMNIIDL", "omniidl"))
        self.prefix = os.environ.get("OMNIORB_PREFIX")
        if not self.prefix and shutil.which(self.omniidl[-1]):
            self.prefix = os.path.dirname(os.path.dirname(shutil.which(self.omniidl[-1])))

    def dir(self, impl):
        return os.path.join(BUILD, impl)

    def build(self, impl):
        try:
            return self._build(impl)
        except (subprocess.CalledProcessError, OSError) as e:
            self.skipped[impl] = "build failed: %s" % e
            return False

    def _build(self, impl):
        nb = self.args.no_build
        d = self.dir(impl)
        if impl == "rustorb":
            if not nb:
                run(["cargo", "build", "--release", "--locked", "-p", "rustorb-interop",
                     "--bin", "rustorb-bench"], cwd=ROOT)
            return True
        if impl == "rustorbcpp":
            if nb:
                return True
            run(["cargo", "build", "--release", "--locked", "-p", "rustorb-ffi"], cwd=ROOT)
            os.makedirs(d, exist_ok=True)
            run([sys.executable, "-m", "rustorb_idl", "-bc++11", "-C", d, IDL])
            cxx = os.environ.get("CXX", "c++" if platform.system() == "Darwin" else "g++")
            libs = (["-framework", "CoreFoundation", "-framework", "Security", "-lpthread", "-lm"]
                    if platform.system() == "Darwin" else
                    ["-static-libstdc++", "-static-libgcc", "-lpthread", "-ldl", "-lm"])
            run([cxx, "-std=c++17", "-O2", "-I" + os.path.join(ROOT, "cpp", "include"),
                 "-I" + os.path.join(ROOT, "crates", "rustorb-ffi", "include"), "-I" + d,
                 os.path.join(HERE, "rustorb-cpp", "bench.cc"), os.path.join(d, "Bench.cc"),
                 os.path.join(ROOT, "target", "release", "librustorb_ffi.a")] + libs +
                ["-o", os.path.join(d, "bench")])
            return True
        if impl == "rustorbpy":
            lib = os.path.join(d, "lib")
            if not nb:
                env = dict(self.env, PYO3_BUILD_EXTENSION_MODULE="1")
                run(["cargo", "build", "--release", "--locked", "-p", "rustorb-python"],
                    cwd=ROOT, env=env)
                shutil.rmtree(d, ignore_errors=True)
                os.makedirs(os.path.join(d, "stubs"))
                shutil.copytree(os.path.join(ROOT, "python", "rustorb"),
                                os.path.join(lib, "rustorb"),
                                ignore=shutil.ignore_patterns("_native*", "__pycache__"))
                native = "lib_native.dylib" if platform.system() == "Darwin" else "lib_native.so"
                shutil.copy(os.path.join(ROOT, "target", "release", native),
                            os.path.join(lib, "rustorb", "_native.abi3.so"))
                run([sys.executable, "-m", "rustorb_idl", "-bpython", "-C",
                     os.path.join(d, "stubs"), IDL])
            return True
        if impl == "rustorbjava":
            javac = os.path.join(os.environ["JAVA_HOME"], "bin", "javac") \
                if os.environ.get("JAVA_HOME") else shutil.which("javac")
            if not javac:
                self.skipped[impl] = "javac not found"
                return False
            if nb:
                return True
            run(["cargo", "build", "--release", "--locked", "-p", "rustorb-ffi"], cwd=ROOT)
            shutil.rmtree(d, ignore_errors=True)
            gen, classes = os.path.join(d, "gen"), os.path.join(d, "classes")
            os.makedirs(gen)
            run([sys.executable, "-m", "rustorb_idl", "-bjava", "-C", gen, IDL])
            sources = []
            for top in (os.path.join(ROOT, "java", "src", "main", "java"), gen,
                        os.path.join(HERE, "java")):
                for dirpath, _, files in os.walk(top):
                    sources += [os.path.join(dirpath, f) for f in files if f.endswith(".java")]
            with open(os.path.join(gen, "sources.txt"), "w") as f:
                f.write("\n".join(sources))
            run([javac, "--release", "22", "-encoding", "UTF-8", "-nowarn", "-d", classes,
                 "@" + os.path.join(gen, "sources.txt")])
            return True
        if impl == "rustorbcs":
            if not shutil.which("dotnet"):
                self.skipped[impl] = "dotnet not found"
                return False
            if nb:
                return True
            run(["cargo", "build", "--release", "--locked", "-p", "rustorb-ffi"], cwd=ROOT)
            shutil.rmtree(d, ignore_errors=True)
            os.makedirs(d)
            for f in ("bench.csproj", "Program.cs"):
                shutil.copy(os.path.join(HERE, "csharp", f), d)
            run([sys.executable, "-m", "rustorb_idl", "-bcsharp", "-C", d, IDL])
            run(["dotnet", "publish", os.path.join(d, "bench.csproj"), "-c", "Release",
                 "-o", os.path.join(d, "out"), "--nologo", "-v", "quiet",
                 "-p:RustorbRoot=" + ROOT])
            shutil.copy(os.path.join(ROOT, "target", "release", self.ffi_lib()),
                        os.path.join(d, "out"))
            return True
        if impl == "rustorbc":
            if not shutil.which("cmake"):
                self.skipped[impl] = "cmake not found"
                return False
            if nb:
                return True
            run(["cargo", "build", "--release", "--locked", "-p", "rustorb-ffi"], cwd=ROOT)
            os.makedirs(d, exist_ok=True)
            run(["cmake", "-S", os.path.join(ROOT, "c"), "-B", os.path.join(d, "build"),
                 "-DCMAKE_BUILD_TYPE=Release", "-DRUSTORB_BUILD_TESTS=OFF"],
                stdout=subprocess.DEVNULL)
            run(["cmake", "--build", os.path.join(d, "build")], stdout=subprocess.DEVNULL)
            run([sys.executable, "-m", "rustorb_idl", "-bc", "-C", d, IDL])
            cc = os.environ.get("CC", "cc")
            libs = (["-framework", "CoreFoundation", "-framework", "Security", "-lpthread", "-lm"]
                    if platform.system() == "Darwin" else
                    ["-static-libgcc", "-lpthread", "-ldl", "-lm"])
            run([cc, "-std=c99", "-O2", "-I" + os.path.join(ROOT, "c", "include"),
                 "-I" + os.path.join(ROOT, "crates", "rustorb-ffi", "include"), "-I" + d,
                 os.path.join(HERE, "c", "bench.c"), os.path.join(d, "Bench.c"),
                 os.path.join(d, "build", "librustorb_c.a"),
                 os.path.join(ROOT, "target", "release", "librustorb_ffi.a")] + libs +
                ["-o", os.path.join(d, "bench")])
            return True
        if impl == "rustorbrb":
            if not shutil.which("ruby"):
                self.skipped[impl] = "ruby not found"
                return False
            if nb:
                return True
            run(["cargo", "build", "--release", "--locked", "-p", "rustorb-ffi"], cwd=ROOT)
            shutil.rmtree(d, ignore_errors=True)
            rt, gen, ext = (os.path.join(d, x) for x in ("rt", "gen", "ext"))
            shutil.copytree(os.path.join(ROOT, "ruby", "lib"), rt)
            os.makedirs(gen)
            os.makedirs(ext)
            run([sys.executable, "-m", "rustorb_idl", "-bruby", "-C", gen, IDL])
            env = dict(self.env,
                       RUSTORB_FFI_LIBRARY=os.path.join(ROOT, "target", "release",
                                                        "librustorb_ffi.a"),
                       RUSTORB_FFI_INCLUDE=os.path.join(ROOT, "crates", "rustorb-ffi", "include"))
            run(["ruby", os.path.join(ROOT, "ruby", "ext", "rustorb_ext", "extconf.rb")],
                cwd=ext, env=env, stdout=subprocess.DEVNULL)
            run(["make"], cwd=ext, env=env, stdout=subprocess.DEVNULL)
            dlext = capture(["ruby", "-rrbconfig", "-e", "print RbConfig::CONFIG['DLEXT']"])
            shutil.copy(os.path.join(ext, "rustorb_ext." + dlext), os.path.join(rt, "rustorb"))
            return True
        if impl in ("omniorbcpp", "omniorbpy"):
            if not self.prefix:
                self.skipped[impl] = "omniidl not found (OMNIIDL, OMNIORB_PREFIX)"
                return False
            if impl == "omniorbpy":
                r = subprocess.run([sys.executable, "-c", "import omniORB.CORBA"],
                                   env=self.env_of(impl), capture_output=True)
                if r.returncode != 0:
                    self.skipped[impl] = "omniORBpy is not importable"
                    return False
            if nb:
                return True
            os.makedirs(d, exist_ok=True)
            if impl == "omniorbpy":
                run(self.omniidl + ["-bpython", "-C", d, IDL])
                return True
            run(self.omniidl + ["-bcxx", "-Wba", "-Wbh=.hh", "-Wbs=SK.cc", "-Wbd=DynSK.cc",
                                IDL], cwd=d)
            cxx = os.environ.get("CXX", "c++" if platform.system() == "Darwin" else "g++")
            run([cxx, "-std=c++17", "-O2", "-I" + d, "-I" + os.path.join(self.prefix, "include"),
                 os.path.join(HERE, "omniorb-cpp", "bench.cc"), os.path.join(d, "BenchSK.cc"),
                 os.path.join(d, "BenchDynSK.cc"), "-L" + os.path.join(self.prefix, "lib"),
                 "-lomniORB4", "-lomniDynamic4", "-lomnithread", "-lpthread",
                 "-o", os.path.join(d, "bench")])
            return True
        raise AssertionError(impl)

    @staticmethod
    def ffi_lib():
        return "librustorb_ffi.dylib" if platform.system() == "Darwin" else "librustorb_ffi.so"

    def env_of(self, impl):
        env = dict(self.env)
        if impl.startswith("omniorb") and self.prefix:
            lib = os.path.join(self.prefix, "lib")
            var = "DYLD_LIBRARY_PATH" if platform.system() == "Darwin" else "LD_LIBRARY_PATH"
            env[var] = os.pathsep.join(filter(None, [lib, env.get(var)]))
        if impl == "rustorbpy":
            d = self.dir(impl)
            env["PYTHONPATH"] = os.pathsep.join([os.path.join(d, "lib"), os.path.join(d, "stubs")])
        if impl == "rustorbrb":
            d = self.dir(impl)
            env["RUBYLIB"] = os.pathsep.join([os.path.join(d, "rt"), os.path.join(d, "gen")])
        if impl == "omniorbpy":
            paths = [self.dir(impl)]
            for py in (os.path.join(self.prefix, "python"),
                       os.path.join(self.prefix, "lib", "python%d.%d" % sys.version_info[:2],
                                    "site-packages")):
                if os.path.isdir(py):
                    paths.append(py)
            env["PYTHONPATH"] = os.pathsep.join(filter(None, paths + [env.get("PYTHONPATH")]))
        return env

    def cmd(self, impl, mode):
        if impl == "rustorb":
            return [os.path.join(ROOT, "target", "release", "rustorb-bench"), mode]
        if impl in ("rustorbcpp", "omniorbcpp"):
            return [os.path.join(self.dir(impl), "bench"), mode]
        if impl in ("rustorbpy", "omniorbpy"):
            return [sys.executable, os.path.join(HERE, "python", "bench.py"), mode,
                    "--orb", "omniorb" if impl == "omniorbpy" else "rustorb"]
        if impl == "rustorbjava":
            java = os.path.join(os.environ["JAVA_HOME"], "bin", "java") \
                if os.environ.get("JAVA_HOME") else "java"
            return [java, "--enable-native-access=ALL-UNNAMED",
                    "-Djava.library.path=" + os.path.join(ROOT, "target", "release"),
                    "-cp", os.path.join(self.dir(impl), "classes"), "BenchMain", mode]
        if impl == "rustorbcs":
            return [os.path.join(self.dir(impl), "out", "bench"), mode]
        if impl == "rustorbc":
            return [os.path.join(self.dir(impl), "bench"), mode]
        if impl == "rustorbrb":
            return ["ruby", os.path.join(HERE, "ruby", "bench.rb"), mode]
        raise AssertionError(impl)


# ---------------------------------------------------------------------------
# Processes

def proc_cpu(pid):
    """CPU seconds (user + system) of a running process, from /proc."""
    try:
        with open("/proc/%d/stat" % pid) as f:
            fields = f.read().rsplit(")", 1)[1].split()
        return (int(fields[11]) + int(fields[12])) / CLK_TCK
    except (OSError, IndexError, ValueError):
        pass
    if platform.system() == "Darwin":
        # No /proc: ps prints the CPU time as [[dd-]hh:]mm:ss.cc.
        try:
            t = capture(["ps", "-o", "time=", "-p", str(pid)])
            days, _, t = t.rpartition("-")
            secs = 0.0
            for part in t.split(":"):
                secs = secs * 60 + float(part)
            return secs + int(days or 0) * 86400
        except ValueError:
            pass
    return None


def proc_csw(pid):
    """Context switches (voluntary and not) of the threads of a running
    process, by thread name, from /proc (Linux only; threads that ended are
    not counted)."""
    by_name = {}
    try:
        tasks = os.listdir("/proc/%d/task" % pid)
    except OSError:
        return None
    for tid in tasks:
        try:
            n = 0
            with open("/proc/%d/task/%s/status" % (pid, tid)) as f:
                for line in f:
                    if line.startswith("Name:"):
                        name = line.split(None, 1)[1].strip()
                    elif line.startswith(("voluntary_ctxt_switches",
                                          "nonvoluntary_ctxt_switches")):
                        n += int(line.split()[1])
            by_name[name] = by_name.get(name, 0) + n
        except (OSError, ValueError, UnboundLocalError):
            pass
    return by_name


def read_stats(path):
    """The counts a rustorb server wrote to path (--stats-file), or None."""
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def stats_since(after, before):
    if after is None or before is None:
        return None
    return {k: v - before.get(k, 0) for k, v in after.items()}


def proc_mem(pid, key):
    try:
        with open("/proc/%d/status" % pid) as f:
            for line in f:
                if line.startswith(key + ":"):
                    return int(line.split()[1]) * 1024
    except OSError:
        pass
    if platform.system() == "Darwin" and key == "VmRSS":
        # No /proc (and no peak): the current resident size from ps, in KiB.
        try:
            return int(capture(["ps", "-o", "rss=", "-p", str(pid)])) * 1024
        except ValueError:
            pass
    return None


class Server:
    def __init__(self, cmd, env, logfile, stats_file=None):
        self.stats_file = stats_file
        log("$ " + " ".join(cmd) + " &")
        self.err = open(logfile, "w")
        self.proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=self.err, env=env,
                                     text=True, start_new_session=True)
        self.ior = None
        got = []
        t = threading.Thread(target=lambda: got.append(self.proc.stdout.readline()))
        t.daemon = True
        t.start()
        t.join(60)
        if got and got[0].startswith("IOR:"):
            self.ior = got[0].strip()
        self.rss_start = proc_mem(self.proc.pid, "VmRSS")

    def cpu(self):
        return proc_cpu(self.proc.pid)

    def csw(self):
        return proc_csw(self.proc.pid) if LINUX else None

    def stats(self):
        return read_stats(self.stats_file) if self.stats_file else None

    def peak(self):
        return proc_mem(self.proc.pid, "VmHWM")

    def stop(self):
        if self.stats_file and os.path.exists(self.stats_file):
            os.unlink(self.stats_file)
        if self.proc.poll() is None:
            os.killpg(self.proc.pid, signal.SIGTERM)
            try:
                self.proc.wait(10)
            except subprocess.TimeoutExpired:
                os.killpg(self.proc.pid, signal.SIGKILL)
                self.proc.wait()
        self.err.close()


class Client:
    """What run_client found: the case lines, the --stats lines (case ->
    counts), CPU seconds, context switches, peak RSS, and whether it
    succeeded."""

    def __init__(self, lines, stats, cpu, csw, rss, ok):
        self.lines, self.stats, self.cpu, self.csw, self.rss, self.ok = \
            lines, stats, cpu, csw, rss, ok


def run_client(cmd, env, logfile, timeout):
    """Runs cmd to the end. Returns a Client."""
    log("$ " + " ".join(cmd))
    with tempfile.TemporaryFile("w+") as out, open(logfile, "a") as err:
        p = subprocess.Popen(cmd, stdout=out, stderr=err, env=env, start_new_session=True)
        timer = threading.Timer(timeout, lambda: os.killpg(p.pid, signal.SIGKILL))
        timer.start()
        try:
            if hasattr(os, "wait4"):
                _, status, ru = os.wait4(p.pid, 0)
                p.returncode = os.waitstatus_to_exitcode(status)
                cpu = ru.ru_utime + ru.ru_stime
                csw = ru.ru_nvcsw + ru.ru_nivcsw
                rss = ru.ru_maxrss * (1 if platform.system() == "Darwin" else 1024)
            else:
                p.wait()
                cpu = csw = rss = None
        finally:
            timer.cancel()
        out.seek(0)
        lines, stats = [], {}
        for line in out:
            line = line.strip()
            if line.startswith("{"):
                obj = json.loads(line)
                if "stats_case" in obj:
                    stats[obj["stats_case"]] = obj["stats"]
                else:
                    lines.append(obj)
            elif line:
                err.write(line + "\n")
    return Client(lines, stats, cpu, csw, rss, p.returncode == 0)


# ---------------------------------------------------------------------------
# Running

def client_args(args, case, impl=None):
    extra = ["--stats"] if args.stats and impl == "rustorb" else []
    return ["--cases", case, "--seconds", str(args.seconds), "--warmup", str(args.warmup)] + extra


def endpoint(transport, tag):
    if transport == "unix":
        path = os.path.join(tempfile.gettempdir(), "rustorb-bench-%d-%s.sock" % (os.getpid(), tag))
        if os.path.exists(path):
            os.unlink(path)
        return ["-ORBendPoint", "giop:unix:" + path]
    return ["-ORBendPoint", "giop:tcp:127.0.0.1:"]


def baseline(impls, impl, args, logdir):
    """CPU seconds and context switches a client process takes without any
    case (start and end)."""
    ior = "corbaloc::127.0.0.1:1/none"
    c = run_client(impls.cmd(impl, "client") + ["--ior", ior, "--cases", ""] + ORB_ARGS,
                   impls.env_of(impl), os.path.join(logdir, "baseline-%s.log" % impl), 60)
    return {"cpu": c.cpu or 0.0, "csw": c.csw or 0}


def inproc(impls, impl, args, logdir, rep):
    rows = []
    for case in args.inproc_cases:
        if case.startswith("procs:") or case == "first":
            continue
        c = run_client(
            impls.cmd(impl, "inproc") + client_args(args, case, impl) + ORB_ARGS,
            impls.env_of(impl), os.path.join(logdir, "inproc-%s.log" % impl), args.timeout)
        row = {"mode": "inproc", "impl": impl, "case": case, "rep": rep,
               "ok": c.ok and bool(c.lines)}
        if c.lines:
            row.update(c.lines[0])
            row["process_cpu_s"] = c.cpu
            row["process_csw"] = c.csw
            row["process_peak_rss"] = c.rss
        if case in c.stats:
            row["client_stats"] = c.stats[case]
        rows.append(row)
    return rows


def pair(impls, client, server, transport, args, logdir, rep, base):
    tag = "%s-%s-%s-%d" % (client, server, transport, rep)
    stats_file, extra = None, []
    if args.stats and server == "rustorb":
        stats_file = os.path.join(logdir, "stats-%s.json" % tag)
        extra = ["--stats-file", stats_file]
    srv = Server(impls.cmd(server, "serve") + endpoint(transport, tag) + ORB_ARGS + extra,
                 impls.env_of(server), os.path.join(logdir, "server-%s.log" % tag), stats_file)
    rows = []
    try:
        if not srv.ior:
            return [{"mode": "pair", "client": client, "server": server, "transport": transport,
                     "rep": rep, "case": c, "ok": False, "error": "no IOR"} for c in args.cases]
        for case in args.cases:
            row = {"mode": "pair", "client": client, "server": server, "transport": transport,
                   "case": case, "rep": rep}
            s0, w0, st0 = srv.cpu(), srv.csw(), srv.stats()
            if case == "first":
                # A new client process each time: the first call makes the
                # connection. FIRST_RUNS of them; their median.
                firsts, walls = [], []
                for _ in range(FIRST_RUNS):
                    t0 = time.monotonic()
                    c = run_client(
                        impls.cmd(client, "client") + ["--ior", srv.ior, "--cases", "first"] +
                        ORB_ARGS, impls.env_of(client),
                        os.path.join(logdir, "client-%s.log" % tag), args.timeout)
                    if not c.ok or not c.lines:
                        break
                    walls.append((time.monotonic() - t0) * 1e3)
                    firsts.append(c.lines[0]["p50_us"])
                row["ok"] = len(firsts) == FIRST_RUNS
                if firsts:
                    row["p50_us"] = statistics.median(firsts)
                    row["process_wall_ms"] = round(statistics.median(walls), 1)
            elif case.startswith("procs:"):
                n = int(case.split(":")[1])
                results = [None] * n

                def one(i):
                    results[i] = run_client(
                        impls.cmd(client, "client") + ["--ior", srv.ior] +
                        client_args(args, "long", client) + ORB_ARGS, impls.env_of(client),
                        os.path.join(logdir, "client-%s.log" % tag), args.timeout)
                ts = [threading.Thread(target=one, args=(i,)) for i in range(n)]
                for t in ts:
                    t.start()
                for t in ts:
                    t.join()
                ok = all(r and r.ok and r.lines for r in results)
                row["ok"] = ok
                if ok:
                    row["rate"] = round(sum(r.lines[0]["rate"] for r in results), 1)
                    row["calls"] = sum(r.lines[0]["calls"] for r in results)
                    row["total_calls"] = sum(r.lines[0].get("total_calls", 0) for r in results)
                    row["p50_us"] = statistics.median(r.lines[0]["p50_us"] for r in results)
                    row["client_cpu_s"] = sum((r.cpu or 0) - base["cpu"] for r in results)
                    if all(r.csw is not None for r in results):
                        row["client_csw"] = sum(r.csw - base["csw"] for r in results)
            else:
                c = run_client(
                    impls.cmd(client, "client") + ["--ior", srv.ior] +
                    client_args(args, case, client) + ORB_ARGS, impls.env_of(client),
                    os.path.join(logdir, "client-%s.log" % tag), args.timeout)
                row["ok"] = c.ok and bool(c.lines)
                if c.lines:
                    row.update(c.lines[0])
                    row["client_cpu_s"] = None if c.cpu is None else max(c.cpu - base["cpu"], 0.0)
                    row["client_csw"] = None if c.csw is None else max(c.csw - base["csw"], 0)
                    row["client_peak_rss"] = c.rss
                if case in c.stats:
                    row["client_stats"] = c.stats[case]
            if stats_file:
                # The server writes its counts every 50 ms.
                time.sleep(0.15)
            s1, w1, st1 = srv.cpu(), srv.csw(), srv.stats()
            if s0 is not None and s1 is not None:
                row["server_cpu_s"] = round(s1 - s0, 3)
            if w0 is not None and w1 is not None:
                row["server_csw"] = sum(w1.values()) - sum(w0.values())
                row["server_csw_threads"] = {
                    k: v - w0.get(k, 0) for k, v in w1.items() if v - w0.get(k, 0)}
            st = stats_since(st1, st0)
            if st is not None:
                row["server_stats"] = st
            rows.append(row)
            if srv.proc.poll() is not None:
                break
        peak = srv.peak()
        for r in rows:
            r["server_rss_start"] = srv.rss_start
            r["server_peak_rss"] = peak
    finally:
        srv.stop()
    return rows


def pairs_of(args, available):
    if args.pairs == "same":
        return [(i, i) for i in available]
    if args.pairs == "all":
        return [(c, s) for s in available for c in available]
    out = []
    for p in args.pairs.split(","):
        c, s = p.split(":")
        if c in available and s in available:
            out.append((c, s))
    return out


# ---------------------------------------------------------------------------
# Results

METRICS = ["rate", "mean_us", "p50_us", "p90_us", "p99_us", "max_us", "mb_per_s", "send_rate",
           "cpu_us_per_call", "client_cpu_us_per_call", "server_cpu_us_per_call",
           "csw_per_call", "client_csw_per_call", "server_csw_per_call",
           "server_rss_start", "server_peak_rss", "process_peak_rss", "client_peak_rss",
           "process_wall_ms"]


def per_call(rows):
    for r in rows:
        n = r.get("total_calls") or r.get("calls")
        if not r.get("ok") or not n:
            continue
        if r.get("process_cpu_s") is not None:
            r["cpu_us_per_call"] = round(r["process_cpu_s"] / n * 1e6, 3)
        if r.get("client_cpu_s") is not None:
            r["client_cpu_us_per_call"] = round(r["client_cpu_s"] / n * 1e6, 3)
        if r.get("server_cpu_s") is not None:
            r["server_cpu_us_per_call"] = round(r["server_cpu_s"] / n * 1e6, 3)
        for k in ("process", "client", "server"):
            if r.get(k + "_csw") is not None:
                key = "csw_per_call" if k == "process" else k + "_csw_per_call"
                r[key] = round(r[k + "_csw"] / n, 3)
        counters = {}
        for side in ("client", "server"):
            for name, v in (r.get(side + "_stats") or {}).items():
                counters["%s.%s" % (side, name)] = round(v / n, 4)
        for name, v in (r.get("server_csw_threads") or {}).items():
            counters["server.csw[%s]" % name] = round(v / n, 4)
        if counters:
            r["counters_per_call"] = counters


def medians(rows):
    groups = {}
    for r in rows:
        key = (r["mode"], r.get("impl") or r["client"], r.get("server", ""),
               r.get("transport", ""), r["case"])
        groups.setdefault(key, []).append(r)
    out = []
    for (mode, a, b, tr, case), rs in groups.items():
        m = {"mode": mode, "case": case, "runs": len(rs), "ok": all(r.get("ok") for r in rs)}
        if mode == "inproc":
            m["impl"] = a
        else:
            m.update(client=a, server=b, transport=tr)
        for k in METRICS:
            vals = [r[k] for r in rs if r.get("ok") and r.get(k) is not None]
            if vals:
                m[k] = statistics.median(vals)
        names = []
        for r in rs:
            for name in r.get("counters_per_call") or {}:
                if name not in names:
                    names.append(name)
        counters = {}
        for name in names:
            vals = [r["counters_per_call"][name] for r in rs
                    if r.get("ok") and name in (r.get("counters_per_call") or {})]
            if vals:
                counters[name] = statistics.median(vals)
        if counters:
            m["counters_per_call"] = counters
        out.append(m)
    return out


def machine():
    cpu, memory = platform.processor(), None
    if LINUX:
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
    elif platform.system() == "Darwin":
        cpu = capture(["sysctl", "-n", "machdep.cpu.brand_string"]) or cpu
        try:
            memory = int(capture(["sysctl", "-n", "hw.memsize"]))
        except (TypeError, ValueError):
            pass
    commit = capture(["git", "-C", ROOT, "rev-parse", "--short", "HEAD"])
    dirty = bool(capture(["git", "-C", ROOT, "status", "--porcelain", "--untracked-files=no"]))
    return {"hostname": socket.gethostname(), "os": platform.platform(), "cpu": cpu,
            "logical_cpus": os.cpu_count(), "memory_bytes": memory,
            "rustc": capture(["rustc", "-V"]), "python": platform.python_version(),
            "commit": commit + ("-dirty" if dirty else "")}


def fmt(v, digits=None):
    if v is None:
        return "-"
    if digits is None:
        digits = 0 if v >= 100 else (1 if v >= 10 else 2)
    return "{:,.{}f}".format(v, digits)


LATENCY = ["ping", "long", "string", "samples:100", "any"]
RATES = ["oneway", "threads:4", "procs:4"]


def markdown(result):
    m, p = result["machine"], result["parameters"]
    med = result["medians"]
    mem = "%.0f GiB" % (m["memory_bytes"] / 2**30) if m.get("memory_bytes") else "?"
    L = ["### %s (%s)" % (m["cpu"] or "?", result["date"]), "",
         "%s, %s logical CPUs, %s, %s, Python %s, rustorb %s. Each case %s s (+%s s warm-up), "
         "median of %d runs." % (m["os"], m["logical_cpus"], mem, m["rustc"], m["python"],
                                 m["commit"], p["seconds"], p["warmup"], p["repeat"]), ""]
    if result["skipped"]:
        L += ["Skipped: " + "; ".join("%s (%s)" % kv for kv in result["skipped"].items()), ""]

    def cell(r, key, digits=None):
        if r is None:
            return ""
        if not r["ok"]:
            return "FAIL"
        return fmt(r.get(key), digits)

    def table(title, cols, rowkeys, get, key, digits=None):
        if not rowkeys:
            return
        L.extend(["#### " + title, "", "| case | " + " | ".join(cols) + " |",
                  "| --- |" + " ---: |" * len(cols)])
        for rk in rowkeys:
            L.append("| %s | %s |" % (rk, " | ".join(cell(get(rk, c), key, digits) for c in cols)))
        L.append("")

    inproc = {(r["impl"], r["case"]): r for r in med if r["mode"] == "inproc"}
    impls_in = [i for i in result["impls"] if any(k[0] == i for k in inproc)]
    cases = list(result["parameters"].get("inproc_cases") or result["parameters"]["cases"])
    if inproc:
        g = lambda case, impl: inproc.get((impl, case))  # noqa: E731
        table("In-process: latency p50 (µs)", impls_in, [c for c in cases if c in LATENCY or
              c.startswith(("octets", "put", "get"))], g, "p50_us")
        table("In-process: throughput (MB/s)", impls_in,
              [c for c in cases if c.startswith(("octets", "put", "get"))], g, "mb_per_s")
        table("In-process: calls/s", impls_in, [c for c in cases if c in RATES or c in LATENCY],
              g, "rate")
        table("In-process: CPU per call (µs, whole process)", impls_in,
              [c for c in cases if c in LATENCY or c.startswith(("octets", "oneway"))], g,
              "cpu_us_per_call")
    cases = list(result["parameters"]["cases"])
    prs = [r for r in med if r["mode"] == "pair"]
    for tr in result["parameters"]["transports"]:
        sub = {(r["client"], r["server"], r["case"]): r for r in prs if r["transport"] == tr}
        if not sub:
            continue
        pairs = []
        for (c, s, _) in sub:
            if (c, s) not in pairs:
                pairs.append((c, s))
        labels = ["%s→%s" % cs for cs in pairs]
        g = lambda case, label: sub.get(  # noqa: E731
            tuple(label.split("→")) + (case,))
        # Narrow tables: pairs as rows, cases as columns.
        def ptable(title, cs, key, digits=None):
            if not cs:
                return
            L.extend(["#### %s (%s)" % (title, tr), "",
                      "| client→server | " + " | ".join(cs) + " |",
                      "| --- |" + " ---: |" * len(cs)])
            for lab in labels:
                L.append("| %s | %s |" % (lab, " | ".join(cell(g(c, lab), key, digits)
                                                          for c in cs)))
            L.append("")
        ptable("Separate processes: first call (µs)",
               [c for c in cases if c == "first"], "p50_us")
        ptable("Separate processes: client process start to end (ms)",
               [c for c in cases if c == "first"], "process_wall_ms")
        ptable("Separate processes: latency p50 (µs)",
               [c for c in cases if c in LATENCY], "p50_us")
        ptable("Separate processes: latency p99 (µs)",
               [c for c in cases if c in LATENCY], "p99_us")
        ptable("Separate processes: throughput (MB/s)",
               [c for c in cases if c.startswith(("octets", "put", "get"))], "mb_per_s")
        ptable("Separate processes: calls/s", [c for c in cases if c in RATES], "rate")
        ptable("Separate processes: client CPU per call (µs)",
               [c for c in cases if c in ("ping", "octets:1048576", "oneway", "any")],
               "client_cpu_us_per_call")
        ptable("Separate processes: server CPU per call (µs)",
               [c for c in cases if c in ("ping", "octets:1048576", "oneway", "any")],
               "server_cpu_us_per_call")
        csw_cases = [c for c in cases if c in LATENCY or c in RATES or
                     c.startswith(("octets", "put", "get"))]
        if any(r.get("client_csw_per_call") is not None for r in sub.values()):
            ptable("Separate processes: client context switches per call",
                   csw_cases, "client_csw_per_call", 2)
        if any(r.get("server_csw_per_call") is not None for r in sub.values()):
            ptable("Separate processes: server context switches per call",
                   csw_cases, "server_csw_per_call", 2)
        L.extend(["#### Server memory (%s, MiB)" % tr, "", "| client→server | at start | peak |",
                  "| --- | ---: | ---: |"])
        for lab in labels:
            r = next((sub[k] for k in sub if "%s→%s" % k[:2] == lab), None)
            mib = lambda v: fmt(v / 2**20, 1) if v else "-"
            L.append("| %s | %s | %s |" % (
                lab, mib((r or {}).get("server_rss_start")),
                mib((r or {}).get("server_peak_rss"))))
        L.append("")
    counted = [r for r in med if r.get("counters_per_call")]
    if counted:
        L.extend(["#### Counters per call", "",
                  "`server.csw[NAME]`: the context switches of the server's threads of that "
                  "name (Linux). The others: rustorb's internal counters (--stats; "
                  "rustorb_core::stats). Counters that are 0 are left out.", ""])
        for r in counted:
            who = (r["impl"] + " in-process" if r["mode"] == "inproc" else
                   "%s→%s (%s)" % (r["client"], r["server"], r["transport"]))
            nonzero = [(k, v) for k, v in r["counters_per_call"].items() if v]
            if not nonzero:
                continue
            L.extend(["%s, `%s`:" % (who, r["case"]), "", "| counter | per call |",
                      "| --- | ---: |"])
            L.extend("| %s | %s |" % (k, fmt(v, 3)) for k, v in nonzero)
            L.append("")
    return "\n".join(L)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--impls")
    ap.add_argument("--modes", default="inproc,pairs")
    ap.add_argument("--pairs")
    ap.add_argument("--cases")
    ap.add_argument("--inproc-cases", help="the cases in one process (default: --cases)")
    ap.add_argument("--quick", action="store_true",
                    help="the short before/after set (see the description)")
    ap.add_argument("--stats", action="store_true",
                    help="record rustorb's internal counters (rustorb_core::stats)")
    ap.add_argument("--seconds", type=float, default=2.0)
    ap.add_argument("--warmup", type=float, default=0.3)
    ap.add_argument("--repeat", type=int, default=3)
    ap.add_argument("--transports", default="tcp")
    ap.add_argument("--timeout", type=float, default=120.0)
    ap.add_argument("--out")
    ap.add_argument("--no-build", action="store_true")
    args = ap.parse_args()
    if args.impls is None:
        args.impls = ",".join(QUICK_IMPLS if args.quick else IMPLS)
    if args.pairs is None:
        args.pairs = "same" if args.quick else "all"
    if args.cases is None:
        args.cases = ",".join(QUICK_CASES if args.quick else CASES)
    if args.inproc_cases is None:
        args.inproc_cases = ",".join(QUICK_INPROC_CASES) if args.quick else args.cases
    args.cases = [c for c in args.cases.split(",") if c]
    args.inproc_cases = [c for c in args.inproc_cases.split(",") if c]
    modes = args.modes.split(",")
    transports = args.transports.split(",")
    date = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    out = args.out or os.path.join(INTEROP, "out", "bench", date)
    logdir = os.path.join(out, "logs")
    os.makedirs(logdir, exist_ok=True)

    impls = Impls(args)
    available = [i for i in args.impls.split(",") if impls.build(i)]
    for i, why in impls.skipped.items():
        log("skipped %s: %s" % (i, why))
    base = {i: baseline(impls, i, args, logdir) for i in available}
    rows = []
    for rep in range(args.repeat):
        if "inproc" in modes:
            for impl in available:
                rows += inproc(impls, impl, args, logdir, rep)
        if "pairs" in modes:
            for tr in transports:
                for c, s in pairs_of(args, available):
                    rows += pair(impls, c, s, tr, args, logdir, rep, base[c])
        log("repetition %d done" % (rep + 1))
    per_call(rows)
    result = {
        "date": datetime.datetime.now().isoformat(timespec="seconds"),
        "machine": machine(),
        "parameters": {"seconds": args.seconds, "warmup": args.warmup, "repeat": args.repeat,
                       "cases": args.cases, "inproc_cases": args.inproc_cases, "modes": modes,
                       "transports": transports, "pairs": args.pairs, "orb_args": ORB_ARGS,
                       "quick": args.quick, "stats": args.stats},
        "impls": available,
        "skipped": impls.skipped,
        "client_baseline_cpu_s": {i: b["cpu"] for i, b in base.items()},
        "client_baseline_csw": {i: b["csw"] for i, b in base.items()},
        "runs": rows,
        "medians": medians(rows),
    }
    with open(os.path.join(out, "result.json"), "w") as f:
        json.dump(result, f, indent=1)
    with open(os.path.join(out, "result.md"), "w") as f:
        f.write(markdown(result))
    failed = [r for r in rows if not r.get("ok")]
    log("results: %s (%d runs, %d failed)" % (out, len(rows), len(failed)))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
