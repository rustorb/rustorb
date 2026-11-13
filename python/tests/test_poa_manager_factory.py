"""Tests of ``PortableServer.POAManagerFactory`` (CORBA 3):
``create_POAManager``, ``list``, ``find``, ``POAManager.get_id``, a manager
shared by several POAs, and the end of a manager with the last of its POAs.
Two ORBs in this process talk over TCP on the loopback interface.

Without the native module the tests are skipped, unless
``RUSTORB_REQUIRE_NATIVE`` is set (as in CI).
"""

import os
import sys
import threading
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
if os.path.join(ROOT, "python") not in sys.path:
    sys.path.insert(0, os.path.join(ROOT, "python"))

from rustorb import CORBA, PortableServer  # noqa: E402

MISSING = None
if CORBA._native is None:
    MISSING = "the native module is not built (python/build-native.sh)"
    if os.environ.get("RUSTORB_REQUIRE_NATIVE"):
        raise ImportError(MISSING)

ECHO_ID = "IDL:test/Echo:1.0"


class Echo(PortableServer.DynamicImplementation):
    """Answers every operation with nothing."""

    def _primary_interface(self, oid, poa):
        return ECHO_ID

    def invoke(self, request):
        request.arguments(CORBA.NVList())


@unittest.skipIf(MISSING, MISSING)
class POAManagerFactoryTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = CORBA.ORB_init(["-ORBendPoint", "giop:tcp:127.0.0.1:"], "pmf-server")
        cls.client = CORBA.ORB_init([], "pmf-client")
        cls.root = cls.server.resolve_initial_references("RootPOA")
        cls.root._get_the_POAManager().activate()
        cls.factory = cls.root.the_POAManagerFactory

    @classmethod
    def tearDownClass(cls):
        cls.client.destroy()
        cls.server.destroy()

    def ids(self):
        return [m.get_id() for m in self.factory.list()]

    def test_identities(self):
        root = self.root
        self.assertEqual(root.the_POAManager.get_id(), "RootPOAManager")
        self.assertIn("RootPOAManager", self.ids())
        a = root.create_POA("py-a", None, [])
        ida = a.the_POAManager.get_id()
        self.assertNotEqual(ida, "RootPOAManager")
        self.assertIn(ida, self.ids())
        self.assertEqual(self.factory.find(ida), a.the_POAManager)

        x = self.factory.create_POAManager("py-x", [])
        self.assertEqual(x.get_id(), "py-x")
        self.assertEqual(x.get_state(), PortableServer.POAManager.HOLDING)
        self.assertEqual(self.factory.find("py-x"), x)
        self.assertIsNone(self.factory.find("py-nope"))
        for id in ("py-x", ida, "RootPOAManager"):
            with self.assertRaises(PortableServer.POAManagerFactory.ManagerAlreadyExists):
                self.factory.create_POAManager(id, [])
        self.assertEqual(a.the_POAManagerFactory.find("py-x"), x)
        with self.assertRaises(CORBA.BAD_PARAM):
            self.factory.create_POAManager(None, [])
        a.destroy(False, True)
        self.assertNotIn(ida, self.ids())

    def test_policies(self):
        p = self.root.create_lifespan_policy(PortableServer.PERSISTENT)
        with self.assertRaises(CORBA.PolicyError) as cm:
            self.factory.create_POAManager("py-p", [p])
        self.assertEqual(cm.exception.reason, CORBA.UNSUPPORTED_POLICY)
        self.assertIsNone(self.factory.find("py-p"))

    def test_shared_manager(self):
        m = self.factory.create_POAManager("py-shared", [])
        p1 = self.root.create_POA("py-1", m, [])
        p2 = p1.create_POA("py-2", m, [])
        self.assertEqual(p1.the_POAManager, p2.the_POAManager)
        refs = [
            self.client.string_to_object(
                self.server.object_to_string(p.id_to_reference(p.activate_object(Echo())))
            )
            for p in (p1, p2)
        ]
        # Holding: the calls wait until the shared manager is activated.
        done = []
        threads = [
            threading.Thread(target=lambda r=r: done.append(r._non_existent())) for r in refs
        ]
        for t in threads:
            t.start()
        for t in threads:
            t.join(0.3)
        self.assertEqual(done, [])
        m.activate()
        for t in threads:
            t.join(5)
        self.assertEqual(done, [False, False])

        m.discard_requests(False)
        for r in refs:
            with self.assertRaises(CORBA.TRANSIENT):
                r._non_existent()

        # The manager ends with the last of its POAs.
        p1.destroy(False, True)
        self.assertIsNone(self.factory.find("py-shared"))
        with self.assertRaises(CORBA.OBJECT_NOT_EXIST):
            self.root.create_POA("py-3", m, [])
        self.assertIsNotNone(self.factory.create_POAManager("py-shared", []))


if __name__ == "__main__":
    unittest.main()
