#!/usr/bin/env python3
"""Runs the examples against each other: starts rustorb-names, then each
server in turn, calls it with every client, and stops it with the last one.

    examples/check.py --names target/release/rustorb-names \\
        --impl "rust=examples/rust/target/debug/server|examples/rust/target/debug/client" \\
        --impl "python=python examples/python/server.py|python examples/python/client.py"

Each --impl is NAME=SERVER|CLIENT, two commands (split like a shell would)
to which the ORB options are appended. Fails if a server does not come up
or a client does not print the reply the example expects.
"""

import argparse
import os
import shlex
import socket
import subprocess
import sys
import time


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def wait_for(proc, text, timeout):
    deadline = time.monotonic() + timeout
    lines = []
    while time.monotonic() < deadline:
        line = proc.stdout.readline()
        if not line:
            break
        lines.append(line)
        if text in line:
            return lines
    raise RuntimeError("no %r within %ss: %s" % (text, timeout, "".join(lines)))


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--names", required=True, help="the rustorb-names binary")
    ap.add_argument("--impl", dest="impls", action="append", required=True, metavar="NAME=SERVER|CLIENT")
    ap.add_argument("--timeout", type=float, default=60.0)
    args = ap.parse_args()

    impls = []
    for spec in args.impls:
        name, _, cmds = spec.partition("=")
        server, sep, client = cmds.partition("|")
        if not sep:
            ap.error("--impl needs NAME=SERVER|CLIENT: %s" % spec)
        impls.append((name, shlex.split(server), shlex.split(client)))

    port = free_port()
    orb_args = ["-ORBInitRef", "NameService=corbaloc::127.0.0.1:%d/NameService" % port]
    env = dict(os.environ, PYTHONUNBUFFERED="1")
    names = subprocess.Popen(
        [args.names, "-memory", "-start", str(port)],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    )
    failures = []
    try:
        wait_for(names, "listening", args.timeout)
        for sname, server_cmd, _ in impls:
            server = subprocess.Popen(
                server_cmd + orb_args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env=env,
            )
            try:
                wait_for(server, "server: ready", args.timeout)
                for i, (cname, _, client_cmd) in enumerate(impls):
                    last = i == len(impls) - 1
                    cmd = client_cmd + orb_args + (["--shutdown"] if last else [])
                    r = subprocess.run(
                        cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                        timeout=args.timeout, env=env,
                    )
                    lang = {"rust": "Rust", "python": "Python", "cpp": "C++", "java": "Java"}.get(cname, cname)
                    ok = r.returncode == 0 and ("client: hello from %s" % lang) in r.stdout
                    print("%-6s server, %-6s client: %s" % (sname, cname, "ok" if ok else "FAILED"))
                    if not ok:
                        failures.append((sname, cname, r.stdout))
                server.wait(timeout=args.timeout)
                if server.returncode != 0:
                    failures.append((sname, "(exit)", server.stdout.read()))
            except Exception as e:  # noqa: BLE001 - report and go on with the next server
                failures.append((sname, "-", str(e)))
            finally:
                if server.poll() is None:
                    server.kill()
                    server.wait()
    finally:
        names.kill()
        names.wait()
    for sname, cname, out in failures:
        print("--- %s server, %s client:\n%s" % (sname, cname, out), file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
