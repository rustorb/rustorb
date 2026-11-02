"""Tests of bidirectional GIOP (docs/bidir-giop.md) in the rustorb
package: the ``BiDirPolicy`` module, ``create_policy``, POAs with the
``BidirectionalPolicy`` and a callback over the client's own connection.

Two ORBs in this process, a server that accepts bidirectional GIOP and a
client that offers it. The client's IORs name an address nobody listens
on, so the callback works only over the connection the client opened.
Run like test_orb.py.
"""

import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
for p in (os.path.join(ROOT, "python"), os.path.join(ROOT, "crates", "rustorb-codegen", "python")):
    if p not in sys.path:
        sys.path.insert(0, p)

from rustorb import BiDirPolicy, CORBA, PortableServer  # noqa: E402

try:
    from rustorb_idl import cli
    import idl_parser  # noqa: F401
except ImportError:
    cli = None

MISSING = None
if CORBA._native is None:
    MISSING = "the native module is not built (python/build-native.sh)"
elif cli is None:
    MISSING = "idl_parser is not importable"
if MISSING and os.environ.get("RUSTORB_REQUIRE_NATIVE"):
    raise ImportError(MISSING)

OUT = tempfile.mkdtemp(prefix="rustorb-bidir-test-")

server_orb = client_orb = None
T = T__POA = None


def setUpModule():
    global server_orb, client_orb, T, T__POA
    if MISSING:
        raise unittest.SkipTest(MISSING)
    idl_dir = os.path.join(ROOT, "interop", "idl")
    if cli.main(["-bpython", "-C", OUT, "-I" + idl_dir, os.path.join(idl_dir, "InteropTest.idl")]) != 0:
        raise AssertionError("rustorb-idl failed")
    sys.path.insert(0, OUT)
    import InteropTest
    import InteropTest__POA

    T, T__POA = InteropTest, InteropTest__POA
    server_orb = CORBA.ORB_init(
        ["-ORBendPoint", "giop:tcp:127.0.0.1:", "-ORBacceptBiDirectionalGIOP", "1"],
        "bidir-server",
    )
    client_orb = CORBA.ORB_init(
        ["-ORBendPoint", "giop:tcp:127.0.0.1:", "-ORBendPointPublish", "giop:tcp:127.0.0.1:1",
         "-ORBofferBiDirectionalGIOP", "1"],
        "bidir-client",
    )
    for orb in (server_orb, client_orb):
        orb.resolve_initial_references("RootPOA")._get_the_POAManager().activate()


def tearDownModule():
    for orb in (client_orb, server_orb):
        if orb is not None:
            orb.destroy()


def bidir_policy(orb, value):
    return orb.create_policy(
        BiDirPolicy.BIDIRECTIONAL_POLICY_TYPE,
        CORBA.Any(BiDirPolicy._tc_BidirectionalPolicyValue, value),
    )


class BiDirTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        class Echo(T__POA.Echo):
            def call_back(self, cb, message):
                return cb.notify(message)

        class Callback(T__POA.Callback):
            def notify(self, message):
                return "ack:" + message

        poa = server_orb.resolve_initial_references("RootPOA")
        echo = poa.id_to_reference(poa.activate_object(Echo()))
        cls.echo = client_orb.string_to_object(server_orb.object_to_string(echo))._narrow(T.Echo)
        root = client_orb.resolve_initial_references("RootPOA")
        cls.Callback = Callback
        both = root.create_POA(
            "bidir", root._get_the_POAManager(), [bidir_policy(client_orb, BiDirPolicy.BOTH)]
        )
        cls.cb = both.id_to_reference(both.activate_object(Callback()))._narrow(T.Callback)
        normal = root.create_POA(
            "normal", root._get_the_POAManager(), [bidir_policy(client_orb, BiDirPolicy.NORMAL)]
        )
        cls.cb_normal = normal.id_to_reference(normal.activate_object(Callback()))._narrow(
            T.Callback
        )

    def test_create_policy(self):
        p = bidir_policy(client_orb, BiDirPolicy.BOTH)
        self.assertIsInstance(p, BiDirPolicy.BidirectionalPolicy)
        self.assertEqual(p.policy_type, 37)
        self.assertEqual(p.value, BiDirPolicy.BOTH)
        with self.assertRaises(CORBA.PolicyError):
            bidir_policy(client_orb, 2)

    def test_call_back_over_the_clients_connection(self):
        self.assertEqual(self.echo.call_back(self.cb, "hi"), "ack:hi")
        self.assertEqual(self.echo.call_back(self.cb, "again"), "ack:again")

    def test_poa_without_the_policy_refuses(self):
        with self.assertRaises(CORBA.NO_PERMISSION):
            self.echo.call_back(self.cb_normal, "hi")

    def test_orb_policy_manager(self):
        pm = client_orb.resolve_initial_references("ORBPolicyManager")
        pm.set_policy_overrides([bidir_policy(client_orb, BiDirPolicy.BOTH)], CORBA.ADD_OVERRIDE)
        got = pm.get_policy_overrides([BiDirPolicy.BIDIRECTIONAL_POLICY_TYPE])
        self.assertEqual(len(got), 1)
        self.assertIsInstance(got[0], BiDirPolicy.BidirectionalPolicy)
        self.assertEqual(got[0].value, BiDirPolicy.BOTH)
        pm.set_policy_overrides([], CORBA.SET_OVERRIDE)
        self.assertEqual(pm.get_policy_overrides([BiDirPolicy.BIDIRECTIONAL_POLICY_TYPE]), [])

    def test_omniorb_compat_import(self):
        sys.path.insert(0, os.path.join(ROOT, "python", "omniorb-compat"))
        try:
            from omniORB import BiDirPolicy as compat
        finally:
            sys.path.pop(0)
        self.assertIs(compat, BiDirPolicy)


if __name__ == "__main__":
    unittest.main()
