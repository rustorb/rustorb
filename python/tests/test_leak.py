"""Leak checks of the Python binding (issue #49): object references,
servants, arguments, results, exceptions, TypeCodes and Any values made
and dropped many times must not stay alive.

Python objects are checked through reference counts and weak references;
memory held by the native module through the resident size of the process
(Linux only). ``RUSTORB_LEAK_ROUNDS`` multiplies the number of rounds; the
scheduled workflow (.github/workflows/soak.yml) sets it. Run like
test_orb.py, with the native module built and idl_parser importable::

    PYTHONPATH=path/to/idl_parser python3 -m unittest discover -s python/tests -p test_leak.py
"""

import gc
import os
import sys
import tempfile
import unittest
import weakref

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
for p in (os.path.join(ROOT, "python"), os.path.join(ROOT, "crates", "rustorb-codegen", "python")):
    if p not in sys.path:
        sys.path.insert(0, p)

from rustorb import CORBA, PortableServer  # noqa: E402

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

SCALE = max(1, int(os.environ.get("RUSTORB_LEAK_ROUNDS", "1")))
OUT = tempfile.mkdtemp(prefix="rustorb-leak-test-")

server_orb = client_orb = None
T = T__POA = A = A__POA = None


def setUpModule():
    global server_orb, client_orb, T, T__POA, A, A__POA
    if MISSING:
        raise unittest.SkipTest(MISSING)
    idl_dir = os.path.join(ROOT, "interop", "idl")
    for idl in ("InteropTest.idl", "AnyTest.idl"):
        if cli.main(["-bpython", "-C", OUT, "-I" + idl_dir, os.path.join(idl_dir, idl)]) != 0:
            raise AssertionError("rustorb-idl failed for %s" % idl)
    sys.path.insert(0, OUT)
    import InteropTest
    import InteropTest__POA
    import AnyTest
    import AnyTest__POA

    T, T__POA, A, A__POA = InteropTest, InteropTest__POA, AnyTest, AnyTest__POA
    server_orb = CORBA.ORB_init(["-ORBendPoint", "giop:tcp:127.0.0.1:"], "leak-server")
    client_orb = CORBA.ORB_init(["-ORBendPoint", "giop:tcp:127.0.0.1:"], "leak-client")
    for orb in (server_orb, client_orb):
        orb.resolve_initial_references("RootPOA")._get_the_POAManager().activate()


def tearDownModule():
    for orb in (client_orb, server_orb):
        if orb is not None:
            orb.destroy()


def remote(obj, cls):
    """``obj`` of the server ORB as seen by the client ORB."""
    return client_orb.string_to_object(server_orb.object_to_string(obj))._narrow(cls)


def rss_kib():
    try:
        with open("/proc/self/status") as f:
            for line in f:
                if line.startswith("VmRSS:"):
                    return int(line.split()[1])
    except OSError:
        pass
    return None


def instances(cls):
    gc.collect()
    return sum(1 for o in gc.get_objects() if type(o) is cls)


def make_echo():
    class Echo(T__POA.Echo):
        def echo_string(self, v):
            return v

        def echo_octets(self, v):
            return v

        def echo_records(self, v):
            return v

        def echo_long_seq(self, v):
            return v

        def fail(self, code, reason):
            raise T.Oops(code, reason)

        def fail_system(self):
            raise CORBA.BAD_PARAM(42, CORBA.COMPLETED_NO)

        def echo_ref(self, other):
            return other

    return Echo()


def make_any_echo():
    class AnyEcho(A__POA.AnyEcho):
        def echo(self, a):
            return a

        def echo_tc(self, tc):
            return tc

    return AnyEcho()


class LeakTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root = server_orb.resolve_initial_references("RootPOA")
        cls.servant = make_echo()
        oid = cls.root.activate_object(cls.servant)
        cls.local = cls.root.id_to_reference(oid)
        cls.echo = remote(cls.local, T.Echo)
        aoid = cls.root.activate_object(make_any_echo())
        cls.any_echo = remote(cls.root.id_to_reference(aoid), A.AnyEcho)

    def assert_memory_settles(self, work, rounds):
        """Runs ``work`` for ``rounds`` rounds after a warm-up of the same
        length, and checks that the second half of the run did not add
        more than a little resident memory over the first."""
        for _ in range(rounds):
            work()
        gc.collect()
        before = rss_kib()
        for _ in range(rounds):
            work()
        gc.collect()
        after = rss_kib()
        if before is None:
            return
        allowed = max(before // 20, 4 * 1024)
        self.assertLessEqual(
            after, before + allowed, "resident memory grew from %d KiB to %d KiB" % (before, after)
        )

    def test_arguments_and_results_keep_no_references(self):
        s = "leak check " * 100
        b = bytes(range(256)) * 16
        seq = list(range(100))
        rec = T.Record(1, "r", T.BLUE, T.Point(1.0, 2.0), seq, True)
        recs = [rec, rec]
        held = (s, b, seq, rec, recs)
        counts = [sys.getrefcount(o) for o in held]
        for echo in (self.echo, self.local):
            for _ in range(200 * SCALE):
                self.assertEqual(echo.echo_string(s), s)
                self.assertEqual(echo.echo_octets(b), b)
                self.assertEqual(echo.echo_long_seq(seq), seq)
                self.assertEqual(echo.echo_records(recs), recs)
        gc.collect()
        self.assertEqual([sys.getrefcount(o) for o in held], counts)

    def test_exceptions_are_released(self):
        before = instances(T.Oops), instances(CORBA.BAD_PARAM)
        for echo in (self.echo, self.local):
            for i in range(200 * SCALE):
                with self.assertRaises(T.Oops):
                    echo.fail(i, "reason")
                with self.assertRaises(CORBA.BAD_PARAM):
                    echo.fail_system()
        self.assertEqual((instances(T.Oops), instances(CORBA.BAD_PARAM)), before)

    def test_servants_are_released_when_deactivated(self):
        refs = []
        for i in range(100 * SCALE):
            servant = make_echo()
            refs.append(weakref.ref(servant))
            if i % 2:
                oid = self.root.activate_object(servant)
                obj = self.root.id_to_reference(oid)
            else:
                obj = self.root.servant_to_reference(servant)
                oid = self.root.reference_to_id(obj)
            self.assertEqual(remote(obj, T.Echo).echo_string("x"), "x")
            self.root.deactivate_object(oid)
            del servant, obj
        gc.collect()
        self.assertEqual(sum(1 for r in refs if r() is not None), 0)

    def test_servants_are_released_when_their_poa_is_destroyed(self):
        refs = []
        for i in range(20 * SCALE):
            poa = self.root.create_POA("leak%d" % i, self.root._get_the_POAManager(), [])
            for _ in range(5):
                servant = make_echo()
                refs.append(weakref.ref(servant))
                obj = poa.id_to_reference(poa.activate_object(servant))
                self.assertEqual(remote(obj, T.Echo).echo_string("y"), "y")
                del servant, obj
            poa.destroy(True, True)
            del poa
        gc.collect()
        self.assertEqual(sum(1 for r in refs if r() is not None), 0)

    def test_object_references_are_released(self):
        ior = server_orb.object_to_string(self.local)
        before = instances(T._objref_Echo)

        def work():
            for _ in range(100):
                obj = client_orb.string_to_object(ior)
                echo = obj._narrow(T.Echo)
                self.assertIs(echo.echo_ref(echo)._is_equivalent(echo), True)
                self.assertIsNone(echo.echo_ref(None))

        self.assert_memory_settles(work, 10 * SCALE)
        self.assertEqual(instances(T._objref_Echo), before)

    def test_typecodes_and_anys_are_released(self):
        before = instances(CORBA.Any)

        def work():
            for i in range(20):
                orb = client_orb
                rec = orb.create_recursive_tc("IDL:Leak/Tree%d:1.0" % i)
                tree = orb.create_struct_tc(
                    "IDL:Leak/Tree%d:1.0" % i,
                    "Tree",
                    [
                        CORBA.StructMember("label", CORBA.TC_string, None),
                        CORBA.StructMember("kids", orb.create_sequence_tc(0, rec), None),
                    ],
                )
                Tree = tree._NP_class()
                value = Tree("top", [Tree("leaf", [])])
                got = self.any_echo.echo(CORBA.Any(tree, value))
                self.assertEqual(got.value().label, "top")
                self.assertTrue(self.any_echo.echo_tc(tree).equal(tree))
                node = A.Node("n", [A.Node("c", [])])
                self.assertEqual(self.any_echo.echo(CORBA.Any(A._tc_Node, node)).value(), node)
                self.assertTrue(self.any_echo.echo_tc(A._tc_Choice).equal(A._tc_Choice))

        self.assert_memory_settles(work, 10 * SCALE)
        self.assertEqual(instances(CORBA.Any), before)


if __name__ == "__main__":
    unittest.main()
