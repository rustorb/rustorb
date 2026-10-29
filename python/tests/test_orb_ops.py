"""Tests of the ORB operations of #39: ``ORB.create_policy``,
``register_initial_reference``, ``get_service_information``, the policy
overrides of references, of the ORB (``ORBPolicyManager``) and of the
thread (``PolicyCurrent``), and ``Object._validate_connection``.

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

from rustorb import CORBA, Messaging, PortableServer, PortableServer__POA, TimeBase  # noqa: E402

MISSING = None
if CORBA._native is None:
    MISSING = "the native module is not built (python/build-native.sh)"
    if os.environ.get("RUSTORB_REQUIRE_NATIVE"):
        raise ImportError(MISSING)

RT = Messaging.RELATIVE_RT_TIMEOUT_POLICY_TYPE


class Nothing(PortableServer.Servant):
    _NP_RepositoryId = "IDL:test/Nothing:1.0"


def rt(t):
    return Messaging.RelativeRoundtripTimeoutPolicy(t)


@unittest.skipIf(MISSING, MISSING)
class OrbOpsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.orb = CORBA.ORB_init(
            ["test", "-ORBInitRef", "Foo=corbaloc::127.0.0.1:1/Foo",
             "-ORBendPoint", "giop:tcp:127.0.0.1:"],
            "test_orb_ops",
        )

    @classmethod
    def tearDownClass(cls):
        cls.orb.destroy()

    def policy_error(self, type, any):
        with self.assertRaises(CORBA.PolicyError) as cm:
            self.orb.create_policy(type, any)
        return cm.exception.reason

    def test_create_policy(self):
        orb = self.orb
        p = orb.create_policy(
            PortableServer.LIFESPAN_POLICY_ID,
            CORBA.Any(PortableServer._tc_LifespanPolicyValue, PortableServer.PERSISTENT),
        )
        self.assertIsInstance(p, PortableServer.LifespanPolicy)
        self.assertIs(p.value, PortableServer.PERSISTENT)
        self.assertEqual(p.policy_type, 17)

        t = orb.create_policy(RT, CORBA.Any(TimeBase._tc_TimeT, 2000000))
        self.assertIsInstance(t, Messaging.RelativeRoundtripTimeoutPolicy)
        self.assertEqual(t.relative_expiry, 2000000)
        self.assertEqual(orb.create_policy(RT, CORBA.Any(CORBA.TC_ulonglong, 5)).relative_expiry, 5)

        lifespan = PortableServer.LIFESPAN_POLICY_ID
        self.assertEqual(self.policy_error(lifespan, CORBA.Any(CORBA.TC_ulong, 1)),
                         CORBA.BAD_POLICY_TYPE)
        self.assertEqual(
            self.policy_error(lifespan, CORBA.Any(PortableServer._tc_ThreadPolicyValue,
                                                  PortableServer.ORB_CTRL_MODEL)),
            CORBA.BAD_POLICY_TYPE)
        self.assertEqual(self.policy_error(RT, CORBA.Any(CORBA.TC_long, 1)), CORBA.BAD_POLICY_TYPE)
        self.assertEqual(self.policy_error(9999, CORBA.Any(CORBA.TC_long, 1)), CORBA.BAD_POLICY)

        # A policy from create_policy works in create_POA.
        root = orb.resolve_initial_references("RootPOA")
        user_id = orb.create_policy(
            PortableServer.ID_ASSIGNMENT_POLICY_ID,
            CORBA.Any(PortableServer._tc_IdAssignmentPolicyValue, PortableServer.USER_ID),
        )
        child = root.create_POA("CreatePolicy", None, [user_id])
        with self.assertRaises(PortableServer.POA.WrongPolicy):
            child.activate_object(Nothing())
        # A timeout is no POA policy.
        with self.assertRaises(PortableServer.POA.InvalidPolicy):
            root.create_POA("Timeout", None, [t])

    def test_initial_references(self):
        orb = self.orb
        self.assertEqual(orb.id(), "test_orb_ops")
        bar = orb.string_to_object("corbaloc::127.0.0.1:2/Bar")
        for name in ("", "Foo", "RootPOA", "PolicyCurrent"):
            with self.assertRaises(CORBA.ORB.InvalidName):
                orb.register_initial_reference(name, bar)
        with self.assertRaises(CORBA.BAD_PARAM) as cm:
            orb.register_initial_reference("Nil", None)
        self.assertEqual(cm.exception.minor, 0x4F4D001B)
        orb.register_initial_reference("Bar", bar)
        with self.assertRaises(CORBA.ORB.InvalidName):
            orb.register_initial_reference("Bar", bar)
        self.assertTrue(orb.resolve_initial_references("Bar")._is_equivalent(bar))
        # A local object.
        root = orb.resolve_initial_references("RootPOA")
        orb.register_initial_reference("Root", root)
        self.assertIs(orb.resolve_initial_references("Root"), root)
        with self.assertRaises(CORBA.ORB.InvalidName):
            orb.register_initial_reference("Root", bar)
        names = orb.list_initial_services()
        for name in ("Foo", "Bar", "Root", "RootPOA", "POACurrent", "ORBPolicyManager",
                     "PolicyCurrent"):
            self.assertIn(name, names)
        self.assertEqual(orb.get_service_information(1), (False, CORBA.ServiceInformation([], [])))

    def test_object_overrides(self):
        obj = self.orb.string_to_object("corbaloc::127.0.0.1:1/x")
        self.assertIsNone(obj._get_client_policy(RT))
        self.assertIsNone(obj._get_policy(PortableServer.LIFESPAN_POLICY_ID))
        with_rt = obj._set_policy_overrides([rt(30)], CORBA.SET_OVERRIDE)
        self.assertIs(type(with_rt), type(obj))
        self.assertTrue(with_rt._is_equivalent(obj))
        self.assertEqual(obj._get_policy_overrides([]), [])
        [p] = with_rt._get_policy_overrides([])
        self.assertEqual(p.relative_expiry, 30)
        self.assertEqual(with_rt._get_policy_overrides([PortableServer.LIFESPAN_POLICY_ID]), [])
        self.assertEqual(with_rt._get_policy(RT).relative_expiry, 30)
        self.assertAlmostEqual(with_rt._obj.call_timeout(), 3e-6)
        self.assertEqual(
            with_rt._set_policy_overrides([], CORBA.ADD_OVERRIDE)._get_client_policy(RT)
            .relative_expiry, 30)
        self.assertIsNone(with_rt._set_policy_overrides([], CORBA.SET_OVERRIDE)
                          ._get_client_policy(RT))

        with self.assertRaises(CORBA.BAD_PARAM) as cm:
            obj._set_policy_overrides([rt(1), rt(2)], CORBA.SET_OVERRIDE)
        self.assertEqual(cm.exception.minor, 0x4F4D001E)
        with self.assertRaises(CORBA.NO_PERMISSION):
            obj._set_policy_overrides([PortableServer.LifespanPolicy(PortableServer.PERSISTENT)],
                                      CORBA.ADD_OVERRIDE)

    def test_orb_and_thread_overrides(self):
        orb = self.orb
        obj = orb.string_to_object("corbaloc::127.0.0.1:1/x")
        manager = orb.resolve_initial_references("ORBPolicyManager")
        current = orb.resolve_initial_references("PolicyCurrent")
        self.assertIsInstance(manager, CORBA.PolicyManager)
        self.assertIsInstance(current, CORBA.PolicyCurrent)
        try:
            manager.set_policy_overrides([rt(50)], CORBA.SET_OVERRIDE)
            self.assertEqual(obj._get_client_policy(RT).relative_expiry, 50)
            current.set_policy_overrides([rt(40)], CORBA.ADD_OVERRIDE)
            self.assertEqual(obj._get_client_policy(RT).relative_expiry, 40)
            with_rt = obj._set_policy_overrides([rt(30)], CORBA.SET_OVERRIDE)
            self.assertEqual(with_rt._get_client_policy(RT).relative_expiry, 30)
            seen = []
            t = threading.Thread(target=lambda: seen.append(
                (current.get_policy_overrides([]), obj._get_client_policy(RT).relative_expiry)))
            t.start()
            t.join()
            self.assertEqual(seen, [([], 50)])
            [p] = manager.get_policy_overrides([RT])
            self.assertEqual(p.relative_expiry, 50)
        finally:
            current.set_policy_overrides([], CORBA.SET_OVERRIDE)
            manager.set_policy_overrides([], CORBA.SET_OVERRIDE)
        self.assertEqual(current.get_policy_overrides([]), [])
        self.assertEqual(manager.get_policy_overrides([]), [])
        self.assertIsNone(obj._get_client_policy(RT))

    def test_validate_connection(self):
        orb = self.orb
        root = orb.resolve_initial_references("RootPOA")
        root._get_the_POAManager().activate()
        obj = root.servant_to_reference(Nothing())
        self.assertEqual(obj._validate_connection(), (True, []))
        missing = root.create_reference("IDL:test/Nothing:1.0")
        with self.assertRaises(CORBA.OBJECT_NOT_EXIST):
            missing._validate_connection()
        with self.assertRaises(CORBA.TRANSIENT):
            orb.string_to_object("corbaloc::127.0.0.1:1/x")._validate_connection()
        with self.assertRaises(CORBA.NO_IMPLEMENT):
            root._validate_connection()


if __name__ == "__main__":
    unittest.main()
