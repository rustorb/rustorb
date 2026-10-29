"""Tests of the client call timeouts: ``rustorb.timeouts`` and the
omniORBpy functions of the compatibility package
(``omniORB.setClientCallTimeout`` and friends), against a server of the
test's own that reads requests and never answers them.

Without the native module the tests are skipped, unless
``RUSTORB_REQUIRE_NATIVE`` is set (as in CI).
"""

import math
import os
import socket
import struct
import sys
import threading
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
for p in (os.path.join(ROOT, "python"), os.path.join(ROOT, "python", "omniorb-compat")):
    if p not in sys.path:
        sys.path.insert(0, p)

from rustorb import CORBA  # noqa: E402

MISSING = None
if CORBA._native is None:
    MISSING = "the native module is not built (python/build-native.sh)"
    if os.environ.get("RUSTORB_REQUIRE_NATIVE"):
        raise ImportError(MISSING)


class Sink:
    """Accepts connections and counts the CancelRequest messages read."""

    def __init__(self):
        self.sock = socket.socket()
        self.sock.bind(("127.0.0.1", 0))
        self.sock.listen(16)
        self.url = "corbaloc::127.0.0.1:%d/Sink" % self.sock.getsockname()[1]
        self.cancels = 0
        threading.Thread(target=self._accept, daemon=True).start()

    def _accept(self):
        while True:
            try:
                conn, _ = self.sock.accept()
            except OSError:
                return
            threading.Thread(target=self._read, args=(conn,), daemon=True).start()

    def _read(self, conn):
        buf = b""
        while True:
            data = conn.recv(65536)
            if not data:
                return
            buf += data
            while len(buf) >= 12:
                size = struct.unpack("<I" if buf[6] & 1 else ">I", buf[8:12])[0]
                if len(buf) < 12 + size:
                    break
                if buf[7] == 2:
                    self.cancels += 1
                buf = buf[12 + size:]


class TimeoutTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if MISSING:
            raise unittest.SkipTest(MISSING)
        CORBA.ORB_init(["prog"], "timeouts")
        # The ORB the functions without an ORB use.
        cls.orb = CORBA._default_orb()
        cls.sink = Sink()

    def tearDown(self):
        from rustorb import timeouts

        timeouts.set_call_timeout(None, self.orb)
        timeouts.set_connect_timeout(None, self.orb)
        timeouts.set_thread_call_timeout(None)

    def ref(self):
        return self.orb.string_to_object(self.sink.url)

    def time_out(self, obj):
        start = time.monotonic()
        with self.assertRaises(CORBA.TIMEOUT) as cm:
            obj._non_existent()
        self.assertEqual(cm.exception.completed, CORBA.COMPLETED_MAYBE)
        return (time.monotonic() - start) * 1000

    def assertAbout(self, ms, expected):
        self.assertTrue(expected <= ms < expected + 400, "%d ms, expected about %d" % (ms, expected))

    def test_rustorb_timeouts(self):
        from rustorb import timeouts

        cancels = self.sink.cancels
        obj = self.ref()
        timeouts.set_call_timeout(0.2)
        self.assertEqual(timeouts.call_timeout(), 0.2)
        self.assertAbout(self.time_out(obj), 200)
        timeouts.set_call_timeout(30, self.orb)

        # The thread's overrides the ORB's, on that thread only.
        timeouts.set_thread_call_timeout(0.3)
        self.assertEqual(timeouts.thread_call_timeout(), 0.3)
        self.assertAbout(self.time_out(obj), 300)
        other = []

        def run():
            timeouts.set_thread_call_timeout(0.1)
            other.append(self.time_out(obj))

        t = threading.Thread(target=run)
        t.start()
        t.join()
        self.assertAbout(other[0], 100)

        # The reference's overrides the thread's, and a narrowed reference
        # shares it; None unsets it.
        timeouts.set_object_call_timeout(obj, 0.1)
        self.assertEqual(timeouts.object_call_timeout(obj), 0.1)
        self.assertAbout(self.time_out(type(obj)(obj._obj)), 100)
        self.assertIsNone(timeouts.object_call_timeout(self.ref()))
        timeouts.set_object_call_timeout(obj, None)
        self.assertAbout(self.time_out(obj), 300)

        # inf is no limit, overriding the ORB's.
        timeouts.set_thread_call_timeout(None)
        timeouts.set_call_timeout(0.2)
        timeouts.set_object_call_timeout(obj, math.inf)
        self.assertEqual(timeouts.object_call_timeout(obj), math.inf)
        timeouts.set_object_call_timeout(obj, 0.1)
        self.assertAbout(self.time_out(obj), 100)

        timeouts.set_connect_timeout(1.5)
        self.assertEqual(timeouts.connect_timeout(), 1.5)
        timeouts.set_connect_timeout(None)
        self.assertIsNone(timeouts.connect_timeout())
        with self.assertRaises(ValueError):
            timeouts.set_call_timeout(-1)
        with self.assertRaises(CORBA.BAD_PARAM):
            timeouts.set_object_call_timeout(None, 1)

        time.sleep(0.2)
        self.assertEqual(self.sink.cancels - cancels, 6)

    def test_omniorb_functions(self):
        import omniORB

        obj = self.ref()
        omniORB.setClientCallTimeout(200)
        self.assertEqual(self.orb._native.call_timeout(), 0.2)
        self.assertAbout(self.time_out(obj), 200)
        omniORB.setClientCallTimeout(1000)

        omniORB.setClientThreadCallTimeout(300)
        self.assertAbout(self.time_out(obj), 300)
        omniORB.setClientCallTimeout(obj, 100)
        self.assertAbout(self.time_out(obj), 100)
        # 0 unsets the timeouts of the reference and the thread.
        omniORB.setClientCallTimeout(obj, 0)
        self.assertAbout(self.time_out(obj), 300)
        omniORB.setClientThreadCallTimeout(0)
        self.assertAbout(self.time_out(obj), 1000)
        # and means no limit for the ORB.
        omniORB.setClientCallTimeout(0)
        self.assertIsNone(self.orb._native.call_timeout())

        omniORB.setClientConnectTimeout(500)
        self.assertEqual(self.orb._native.connect_timeout(), 0.5)
        omniORB.setClientConnectTimeout(0)
        self.assertIsNone(self.orb._native.connect_timeout())
        self.assertEqual(omniORB.TIMEOUT_CallTimedOutOnClient, 0)
        with self.assertRaises(TypeError):
            omniORB.setClientCallTimeout()


if __name__ == "__main__":
    unittest.main()
