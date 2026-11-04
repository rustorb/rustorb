"""Tests of callback AMI from Python (rustorb's docs/ami.md): the
``sendc_`` operations of code generated with ``rustorb-idl -bpython
-Wbami``, the ``AMI_<I>Handler`` servants and ``Messaging.ExceptionHolder``.

A server ORB, a client ORB and an ORB for handlers talk over TCP on the
loopback interface. Run like test_orb.py, with a rustorb-idl that has
``-Wbami`` (0.3 or later, or the checkout in RUSTORB_IDL_DIR).
"""

import os
import queue
import sys
import tempfile
import threading
import time
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
# rustorb-idl: installed (tools/rustorb-idl.txt), or the checkout in RUSTORB_IDL_DIR.
_idl_dir = os.environ.get("RUSTORB_IDL_DIR")
RUSTORB_IDL_PATH = [os.path.join(_idl_dir, "python")] if _idl_dir else []
for p in [os.path.join(ROOT, "python")] + RUSTORB_IDL_PATH:
    if p not in sys.path:
        sys.path.insert(0, p)

from rustorb import CORBA, Messaging, timeouts  # noqa: E402

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

OUT = tempfile.mkdtemp(prefix="rustorb-ami-test-")
WAIT = 10

server_orb = client_orb = handler_orb = None
A = A__POA = None


def setUpModule():
    global server_orb, client_orb, handler_orb, A, A__POA
    if MISSING:
        raise unittest.SkipTest(MISSING)
    idl = os.path.join(HERE, "idl", "AmiTest.idl")
    if cli.main(["-bpython", "-Wbami", "-C", OUT, idl]) != 0:
        if os.environ.get("RUSTORB_REQUIRE_NATIVE"):
            raise AssertionError("rustorb-idl -Wbami failed")
        raise unittest.SkipTest("this rustorb-idl has no -Wbami")
    sys.path.insert(0, OUT)
    import AmiPy
    import AmiPy__POA

    A, A__POA = AmiPy, AmiPy__POA
    server_orb = CORBA.ORB_init(["-ORBendPoint", "giop:tcp:127.0.0.1:"], "ami-server")
    client_orb = CORBA.ORB_init(["-ORBendPoint", "giop:tcp:127.0.0.1:"], "ami-client")
    handler_orb = CORBA.ORB_init(["-ORBendPoint", "giop:tcp:127.0.0.1:"], "ami-handlers")
    for orb in (server_orb, client_orb, handler_orb):
        orb.resolve_initial_references("RootPOA")._get_the_POAManager().activate()


def tearDownModule():
    for orb in (client_orb, handler_orb, server_orb):
        if orb is not None:
            orb.destroy()


def make_calc():
    class Calc(A__POA.Calc):
        def __init__(self):
            self._label = "start"
            self.fired = queue.Queue()

        def add(self, a, b):
            if a < 0:
                raise A.Oops(a, "negative")
            s = a + b
            return s % 100, s // 100

        def swap(self, ps, tag):
            return list(reversed(ps)), [A.Pair(len(ps), tag)]

        def fail(self, minor):
            raise CORBA.NO_PERMISSION(minor, CORBA.COMPLETED_YES)

        def sleep(self, ms):
            time.sleep(ms / 1000.0)

        def _get_label(self):
            return self._label

        def _set_label(self, value):
            self._label = value

        def _get_count(self):
            return 7

        def fire(self, n):
            self.fired.put(n)

    return Calc


class Handler(object):
    """An ``AMI_CalcHandler`` servant that puts each call into a queue:
    ``(operation, arguments, thread name)``."""

    def __new__(cls):
        class Impl(A__POA.AMI_CalcHandler):
            def __init__(self):
                self.calls = queue.Queue()

            def _record(self, name, *args):
                self.calls.put((name, args, threading.current_thread().name))

            def add(self, ami_return_val, carry):
                self._record("add", ami_return_val, carry)

            def add_excep(self, excep_holder):
                self._record("add_excep", excep_holder)

            def swap(self, ami_return_val, ps):
                self._record("swap", ami_return_val, ps)

            def swap_excep(self, excep_holder):
                self._record("swap_excep", excep_holder)

            def fail(self):
                self._record("fail")

            def fail_excep(self, excep_holder):
                self._record("fail_excep", excep_holder)

            def sleep(self):
                self._record("sleep")

            def sleep_excep(self, excep_holder):
                self._record("sleep_excep", excep_holder)

            def get_label(self, ami_return_val):
                self._record("get_label", ami_return_val)

            def get_label_excep(self, excep_holder):
                self._record("get_label_excep", excep_holder)

            def set_label(self):
                self._record("set_label")

            def set_label_excep(self, excep_holder):
                self._record("set_label_excep", excep_holder)

            def get_count(self, ami_return_val):
                self._record("get_count", ami_return_val)

            def get_count_excep(self, excep_holder):
                self._record("get_count_excep", excep_holder)

            def next(self):
                return self.calls.get(timeout=WAIT)

        return Impl()


def activate(orb, servant):
    poa = orb.resolve_initial_references("RootPOA")
    return poa.id_to_reference(poa.activate_object(servant))



class AmiTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.servant = make_calc()()
        obj = activate(server_orb, cls.servant)
        cls.calc = client_orb.string_to_object(server_orb.object_to_string(obj))

    def local_handler(self):
        """A handler served by the client ORB, and its reference."""
        h = Handler()
        return h, activate(client_orb, h)

    def remote_handler(self):
        """A handler served by another ORB, as the client sees it."""
        h = Handler()
        ref = activate(handler_orb, h)
        return h, client_orb.string_to_object(handler_orb.object_to_string(ref))

    def test_results(self):
        h, ref = self.local_handler()
        self.calc.sendc_add(ref, 70, 45)
        name, args, thread = h.next()
        self.assertEqual((name, args), ("add", (15, 1)))
        # Not the thread that sent.
        self.assertNotEqual(thread, threading.current_thread().name)

    def test_inout_and_out(self):
        h, ref = self.local_handler()
        ps = [A.Pair(1, "one"), A.Pair(2, "two")]
        self.calc.sendc_swap(ref, ps, "tag")
        # The arguments are copied: changing them later changes nothing.
        ps.append(A.Pair(3, "three"))
        name, (ret, out), _ = h.next()
        self.assertEqual(name, "swap")
        self.assertEqual([(p.a, p.b) for p in ret], [(2, "two"), (1, "one")])
        self.assertEqual([(p.a, p.b) for p in out], [(2, "tag")])

    def test_attributes(self):
        h, ref = self.local_handler()
        self.calc.sendc_set_label(ref, "new")
        self.assertEqual(h.next()[:2], ("set_label", ()))
        self.calc.sendc_get_label(ref)
        self.assertEqual(h.next()[:2], ("get_label", ("new",)))
        self.calc.sendc_get_count(ref)
        self.assertEqual(h.next()[:2], ("get_count", (7,)))

    def check_user_exception(self, holder):
        self.assertIsInstance(holder, Messaging.ExceptionHolder)
        self.assertFalse(holder.is_system_exception)
        self.assertTrue(len(holder.marshaled_exception) > 0)
        with self.assertRaises(A.Oops) as cm:
            holder.raise_exception()
        self.assertEqual((cm.exception.code, cm.exception.why), (-1, "negative"))
        with self.assertRaises(A.Oops):
            holder.raise_exception_with_list([A._tc_Oops])
        with self.assertRaises(CORBA.UNKNOWN):
            holder.raise_exception_with_list([])

    def check_system_exception(self, holder):
        self.assertTrue(holder.is_system_exception)
        with self.assertRaises(CORBA.NO_PERMISSION) as cm:
            holder.raise_exception()
        self.assertEqual(cm.exception.minor, 5)
        self.assertEqual(cm.exception.completed, CORBA.COMPLETED_YES)
        with self.assertRaises(CORBA.NO_PERMISSION):
            holder.raise_exception_with_list([])

    def test_user_exception(self):
        h, ref = self.local_handler()
        self.calc.sendc_add(ref, -1, 0)
        name, (holder,), _ = h.next()
        self.assertEqual(name, "add_excep")
        self.check_user_exception(holder)

    def test_system_exception(self):
        h, ref = self.local_handler()
        self.calc.sendc_fail(ref, 5)
        name, (holder,), _ = h.next()
        self.assertEqual(name, "fail_excep")
        self.check_system_exception(holder)

    def test_handler_of_another_orb(self):
        # The holder is sent as a valuetype: the other side has only its
        # state, marshaled_exception.
        h, ref = self.remote_handler()
        self.calc.sendc_add(ref, 1, 2)
        self.assertEqual(h.next()[:2], ("add", (3, 0)))
        self.calc.sendc_add(ref, -1, 0)
        name, (holder,), _ = h.next()
        self.assertEqual(name, "add_excep")
        self.assertIsNone(holder._NP_exception)
        self.check_user_exception(holder)
        self.calc.sendc_fail(ref, 5)
        name, (holder,), _ = h.next()
        self.assertIsNone(holder._NP_exception)
        self.check_system_exception(holder)

    def test_timeout(self):
        h, ref = self.local_handler()
        timeouts.set_object_call_timeout(self.calc, 0.1)
        try:
            self.calc.sendc_sleep(ref, 2000)
        finally:
            timeouts.set_object_call_timeout(self.calc, None)
        name, (holder,), _ = h.next()
        self.assertEqual(name, "sleep_excep")
        with self.assertRaises(CORBA.TIMEOUT) as cm:
            holder.raise_exception()
        self.assertEqual(cm.exception.completed, CORBA.COMPLETED_MAYBE)

    def test_nil_handler(self):
        # The request is sent; the reply is dropped.
        self.calc.sendc_set_label(None, "from nil")
        deadline = time.time() + WAIT
        while self.calc._get_label() != "from nil":
            self.assertLess(time.time(), deadline)
            time.sleep(0.01)

    def test_many(self):
        h, ref = self.remote_handler()
        n = 100
        for i in range(n):
            self.calc.sendc_add(ref, i, 1000)
        got = sorted(h.next()[1] for _ in range(n))
        self.assertEqual(got, sorted(((i + 1000) % 100, (i + 1000) // 100) for i in range(n)))

    def test_handler_raises(self):
        class Bad(A__POA.AMI_CalcHandler):
            def __init__(self):
                self.calls = queue.Queue()

            def get_count(self, ami_return_val):
                self.calls.put(ami_return_val)
                raise RuntimeError("dropped")

        bad = Bad()
        ref = activate(client_orb, bad)
        self.calc.sendc_get_count(ref)
        self.calc.sendc_get_count(ref)
        self.assertEqual([bad.calls.get(timeout=WAIT) for _ in range(2)], [7, 7])

    def test_oneway_and_nil_target(self):
        self.assertFalse(hasattr(self.calc, "sendc_fire"))
        h, ref = self.local_handler()
        nil = A._objref_Calc(None)
        with self.assertRaises(CORBA.SystemException):
            nil.sendc_add(ref, 1, 2)

    def test_shut_down_client(self):
        # A client destroyed while waiting: the handler, called through a
        # reference of an ORB still running, gets the failure, once. (A
        # handler reference of the destroyed ORB could not be called.)
        orb = CORBA.ORB_init([], "ami-short-client")
        calc = orb.string_to_object(client_orb.object_to_string(self.calc))
        h, ref = self.remote_handler()
        calc.sendc_sleep(ref, 1000)
        time.sleep(0.1)
        orb.destroy()
        name, (holder,), _ = h.next()
        self.assertEqual(name, "sleep_excep")
        with self.assertRaises((CORBA.COMM_FAILURE, CORBA.TRANSIENT)):
            holder.raise_exception()
        with self.assertRaises(queue.Empty):
            h.calls.get(timeout=0.2)
        # Sending once shut down fails at once.
        with self.assertRaises(CORBA.BAD_INV_ORDER):
            calc.sendc_add(ref, 1, 2)


if __name__ == "__main__":
    unittest.main()
