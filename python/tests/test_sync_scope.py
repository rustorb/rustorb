"""Tests of ``Messaging.SyncScopePolicy``: ``ORB.create_policy``, the
overrides of references, of the ORB and of the thread, and when a oneway
call returns (rustorb's docs/sync-scope.md). Two ORBs in this process
talk over TCP on the loopback interface.

Without the native module the tests are skipped, unless
``RUSTORB_REQUIRE_NATIVE`` is set (as in CI).
"""

import os
import sys
import threading
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
if os.path.join(ROOT, "python") not in sys.path:
    sys.path.insert(0, os.path.join(ROOT, "python"))

from rustorb import CORBA, Messaging, PortableServer  # noqa: E402

MISSING = None
if CORBA._native is None:
    MISSING = "the native module is not built (python/build-native.sh)"
    if os.environ.get("RUSTORB_REQUIRE_NATIVE"):
        raise ImportError(MISSING)

SYNC = Messaging.SYNC_SCOPE_POLICY_TYPE
SLOW_ID = "IDL:test/Slow:1.0"
DELAY = 0.3


class Slow(PortableServer.DynamicImplementation):
    """Every operation takes ``DELAY`` seconds."""

    def __init__(self):
        self.calls = []

    def _primary_interface(self, oid, poa):
        return SLOW_ID

    def invoke(self, request):
        request.arguments(CORBA.NVList())
        time.sleep(DELAY)
        self.calls.append(request.operation())


def sync(v):
    return Messaging.SyncScopePolicy(v)


@unittest.skipIf(MISSING, MISSING)
class SyncScopeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = CORBA.ORB_init(["-ORBendPoint", "giop:tcp:127.0.0.1:"], "sync-server")
        cls.client = CORBA.ORB_init([], "sync-client")
        poa = cls.server.resolve_initial_references("RootPOA")
        poa._get_the_POAManager().activate()
        cls.servant = Slow()
        cls.slow = cls.remote(poa.id_to_reference(poa.activate_object(cls.servant)))
        cls.missing = cls.remote(poa.create_reference(SLOW_ID))

    @classmethod
    def tearDownClass(cls):
        cls.client.destroy()
        cls.server.destroy()

    @classmethod
    def remote(cls, obj):
        return cls.client.string_to_object(cls.server.object_to_string(obj))

    def test_create_policy(self):
        orb = self.client
        for v in range(4):
            p = orb.create_policy(SYNC, CORBA.Any(Messaging._tc_SyncScope, v))
            self.assertIsInstance(p, Messaging.SyncScopePolicy)
            self.assertEqual(p.synchronization, v)
            self.assertEqual(p.policy_type, 24)
        p = orb.create_policy(SYNC, CORBA.Any(CORBA.TC_short, Messaging.SYNC_WITH_SERVER))
        self.assertEqual(p.synchronization, 2)
        for any in (CORBA.Any(CORBA.TC_short, 4), CORBA.Any(CORBA.TC_short, -1)):
            with self.assertRaises(CORBA.PolicyError) as cm:
                orb.create_policy(SYNC, any)
            self.assertEqual(cm.exception.reason, CORBA.BAD_POLICY_VALUE)
        with self.assertRaises(CORBA.PolicyError) as cm:
            orb.create_policy(SYNC, CORBA.Any(CORBA.TC_long, 1))
        self.assertEqual(cm.exception.reason, CORBA.BAD_POLICY_TYPE)

    def test_overrides(self):
        obj = self.slow
        self.assertIsNone(obj._get_client_policy(SYNC))
        with_sync = obj._set_policy_overrides([sync(Messaging.SYNC_NONE)], CORBA.SET_OVERRIDE)
        [p] = with_sync._get_policy_overrides([SYNC])
        self.assertEqual(p.synchronization, Messaging.SYNC_NONE)
        self.assertEqual(with_sync._get_client_policy(SYNC).synchronization, 0)
        self.assertIsNone(obj._get_client_policy(SYNC))

        current = self.client.resolve_initial_references("PolicyCurrent")
        manager = self.client.resolve_initial_references("ORBPolicyManager")
        try:
            manager.set_policy_overrides([sync(Messaging.SYNC_WITH_TARGET)], CORBA.ADD_OVERRIDE)
            self.assertEqual(obj._get_client_policy(SYNC).synchronization, 3)
            current.set_policy_overrides([sync(Messaging.SYNC_WITH_SERVER)], CORBA.ADD_OVERRIDE)
            self.assertEqual(obj._get_client_policy(SYNC).synchronization, 2)
            self.assertEqual(with_sync._get_client_policy(SYNC).synchronization, 0)
            seen = []
            t = threading.Thread(target=lambda: seen.append(obj._get_client_policy(SYNC).synchronization))
            t.start()
            t.join()
            self.assertEqual(seen, [3])
        finally:
            current.set_policy_overrides([], CORBA.SET_OVERRIDE)
            manager.set_policy_overrides([], CORBA.SET_OVERRIDE)
        self.assertIsNone(obj._get_client_policy(SYNC))

    def oneway(self, obj, v, op="slow"):
        target = obj._set_policy_overrides([sync(v)], CORBA.SET_OVERRIDE)
        started = time.monotonic()
        target._request(op).send_oneway()
        return time.monotonic() - started

    def wait_calls(self, n):
        deadline = time.monotonic() + 10
        while len(self.servant.calls) < n and time.monotonic() < deadline:
            time.sleep(0.01)
        self.assertEqual(len(self.servant.calls), n)

    def test_when_a_oneway_call_returns(self):
        del self.servant.calls[:]
        n = 0
        for v, waits in [
            (Messaging.SYNC_NONE, False),
            (Messaging.SYNC_WITH_TRANSPORT, False),
            (Messaging.SYNC_WITH_SERVER, False),
            (Messaging.SYNC_WITH_TARGET, True),
        ]:
            took = self.oneway(self.slow, v)
            self.assertEqual(took >= DELAY, waits, "sync scope %d took %.3f s" % (v, took))
            n += 1
            self.wait_calls(n)

    def test_the_server_answer_can_be_an_exception(self):
        for v in (Messaging.SYNC_WITH_SERVER, Messaging.SYNC_WITH_TARGET):
            with self.assertRaises(CORBA.OBJECT_NOT_EXIST):
                self.oneway(self.missing, v)
        for v in (Messaging.SYNC_NONE, Messaging.SYNC_WITH_TRANSPORT):
            self.oneway(self.missing, v)


if __name__ == "__main__":
    unittest.main()
