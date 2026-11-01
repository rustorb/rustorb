"""Tests of the trace output (docs/tracing.md): ``-ORBtraceLevel`` writes
the core's events to standard error, or to :mod:`logging` once it has a
handler. Each case runs a small program in a process of its own, since
the trace level and the logging set-up belong to the process.

Without the native module the tests are skipped, unless
``RUSTORB_REQUIRE_NATIVE`` is set (as in CI).
"""

import os
import subprocess
import sys
import textwrap
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
PYTHON = os.path.join(ROOT, "python")
if PYTHON not in sys.path:
    sys.path.insert(0, PYTHON)

from rustorb import CORBA  # noqa: E402

MISSING = None
if CORBA._native is None:
    MISSING = "the native module is not built (python/build-native.sh)"
    if os.environ.get("RUSTORB_REQUIRE_NATIVE"):
        raise ImportError(MISSING)

# A server ORB and a client ORB in one process; the client asks the server
# about an object it does not have.
PROGRAM = textwrap.dedent(
    """
    import logging, socket, sys
    sys.path.insert(0, %(python)r)
    import rustorb
    from rustorb import CORBA
    mode, args = sys.argv[1], sys.argv[2:]
    if mode == "logging":
        logging.basicConfig(
            format="%%(name)s %%(levelname)s %%(message)s", stream=sys.stdout
        )
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    server = CORBA.ORB_init(
        ["-ORBendPoint", "giop:tcp:127.0.0.1:%%d" %% port] + args, "server"
    )
    server.resolve_initial_references("RootPOA")._get_the_POAManager().activate()
    if mode == "set":
        rustorb.set_trace_level(10)
        assert rustorb.trace_level() == 10
    client = CORBA.ORB_init([], "client")
    obj = client.string_to_object("corbaloc::127.0.0.1:%%d/missing" %% port)
    assert obj._non_existent()
    client.destroy()
    server.destroy()
    """
) % {"python": PYTHON}


def run(mode, *args):
    out = subprocess.run(
        [sys.executable, "-c", PROGRAM, mode] + list(args),
        capture_output=True,
        text=True,
        timeout=60,
    )
    if out.returncode != 0:
        raise AssertionError(out.stderr)
    return out.stdout, out.stderr


class TraceTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if MISSING:
            raise unittest.SkipTest(MISSING)

    def test_stderr_without_handlers(self):
        _, err = run("plain", "-ORBtraceLevel", "10")
        for line in (
            "rustorb: info: listening on 127.0.0.1:",
            "rustorb: info: connected to 127.0.0.1:",
            'rustorb: debug: request "_non_existent" (id 0) to 127.0.0.1:',
            "rustorb: debug: request id 0: replying CORBA::OBJECT_NOT_EXIST",
        ):
            self.assertIn(line, err)
        self.assertNotIn("rustorb: trace:", err)

    def test_logging(self):
        out, err = run("logging", "-ORBtraceLevel", "25")
        for line in (
            "rustorb.server INFO listening on 127.0.0.1:",
            "rustorb.connection INFO connected to 127.0.0.1:",
            'rustorb.object DEBUG request "_non_existent" (id 0) to',
            "rustorb.wire TRACE received Request (GIOP 1.0, ",
        ):
            self.assertIn(line, out)
        self.assertNotIn("rustorb:", err)

    def test_set_trace_level(self):
        _, err = run("set")
        self.assertIn('rustorb: debug: request "_non_existent"', err)
        # Set after the server started: its endpoint was not traced.
        self.assertNotIn("listening on", err)

    def test_default_is_quiet(self):
        out, err = run("plain")
        self.assertNotIn("rustorb", out + err)


if __name__ == "__main__":
    unittest.main()
