"""Tests of the DII (``CORBA.Request``) and the DSI
(``PortableServer.DynamicImplementation``) of the rustorb package.

Two ORBs in this process, a server and a client, talk over TCP on the
loopback interface. Run like test_orb.py.
"""

import os
import sys
import tempfile
import threading
import time
import unittest

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

OUT = tempfile.mkdtemp(prefix="rustorb-dii-test-")

server_orb = client_orb = None
T = T__POA = None
ECHO_ID = "IDL:rustorb.org/InteropTest/Echo:1.0"


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
    server_orb = CORBA.ORB_init(["-ORBendPoint", "giop:tcp:127.0.0.1:"], "dii-server")
    client_orb = CORBA.ORB_init([], "dii-client")
    server_orb.resolve_initial_references("RootPOA")._get_the_POAManager().activate()


def tearDownModule():
    for orb in (client_orb, server_orb):
        if orb is not None:
            orb.destroy()


def remote(obj):
    """``obj`` of the server ORB as the client ORB sees it."""
    return client_orb.string_to_object(server_orb.object_to_string(obj))


def activate(servant):
    poa = server_orb.resolve_initial_references("RootPOA")
    return poa.id_to_reference(poa.activate_object(servant))


def tc_long():
    return CORBA.TC_long


class StaticEcho(object):
    """The part of InteropTest::Echo the DII tests call, as a static
    servant."""

    @staticmethod
    def make():
        class Echo(T__POA.Echo):
            def __init__(self):
                self.posted_ = []

            def echo_long(self, v):
                return v

            def echo_string(self, v):
                return v

            def echo_record(self, v):
                return v

            def swap(self, a, b):
                return a + b, b, a

            def divide(self, a, b):
                if b == 0:
                    raise T.DivideByZero()
                return a // b, a % b

            def fail(self, code, reason):
                raise T.Oops(code, reason)

            def fail_system(self):
                raise CORBA.NO_PERMISSION(5, CORBA.COMPLETED_NO)

            def post(self, message):
                self.posted_.append(message)

            def slow(self):
                pass

        return Echo()


class DynamicEcho(PortableServer.DynamicImplementation):
    """The same operations of InteropTest::Echo, by the DSI."""

    def __init__(self):
        self.posted_ = []

    def _primary_interface(self, oid, poa):
        return ECHO_ID

    def invoke(self, request):
        op = request.operation()
        orb = server_orb
        if op == "echo_long":
            args = orb.create_list(1)
            args.add_value("v", CORBA.Any(CORBA.TC_long, None), CORBA.ARG_IN)
            request.arguments(args)
            request.set_result(args.item(0).argument)
        elif op == "echo_string":
            args = request.arguments(
                CORBA.NVList([CORBA.NamedValue("v", CORBA.Any(CORBA.TC_string, None), CORBA.ARG_IN)])
            )
            request.set_result(CORBA.Any(CORBA.TC_string, args.item(0).argument.value()))
        elif op == "echo_record":
            args = request.arguments(
                CORBA.NVList([CORBA.NamedValue("v", CORBA.Any(T._tc_Record, None), CORBA.ARG_IN)])
            )
            request.set_result(args.item(0).argument)
        elif op == "swap":
            args = CORBA.NVList()
            args.add_value("a", CORBA.Any(CORBA.TC_long, None), CORBA.ARG_INOUT)
            args.add_value("b", CORBA.Any(CORBA.TC_long, None), CORBA.ARG_INOUT)
            request.arguments(args)
            a, b = args.item(0).argument.value(), args.item(1).argument.value()
            args.item(0).argument = CORBA.Any(CORBA.TC_long, b)
            args.item(1).argument = CORBA.Any(CORBA.TC_long, a)
            request.set_result(CORBA.Any(CORBA.TC_long, a + b))
        elif op == "divide":
            args = CORBA.NVList()
            args.add_value("a", CORBA.Any(CORBA.TC_long, None), CORBA.ARG_IN)
            args.add_value("b", CORBA.Any(CORBA.TC_long, None), CORBA.ARG_IN)
            args.add_value("remainder", CORBA.Any(CORBA.TC_long, None), CORBA.ARG_OUT)
            request.arguments(args)
            a, b = args.item(0).argument.value(), args.item(1).argument.value()
            if b == 0:
                request.set_exception(CORBA.Any(T._tc_DivideByZero, T.DivideByZero()))
                return
            args.item(2).argument = CORBA.Any(CORBA.TC_long, a % b)
            request.set_result(CORBA.Any(CORBA.TC_long, a // b))
        elif op == "fail":
            args = CORBA.NVList()
            args.add_value("code", CORBA.Any(CORBA.TC_long, None), CORBA.ARG_IN)
            args.add_value("reason", CORBA.Any(CORBA.TC_string, None), CORBA.ARG_IN)
            request.arguments(args)
            exc = T.Oops(args.item(0).argument.value(), args.item(1).argument.value())
            request.set_exception(CORBA.Any(T._tc_Oops, exc))
        elif op == "fail_system":
            request.arguments(CORBA.NVList())
            raise CORBA.NO_PERMISSION(5, CORBA.COMPLETED_NO)
        elif op == "post":
            args = request.arguments(
                CORBA.NVList([CORBA.NamedValue("m", CORBA.Any(CORBA.TC_string, None), CORBA.ARG_IN)])
            )
            self.posted_.append(args.item(0).argument.value())
        else:
            raise CORBA.BAD_OPERATION(0, CORBA.COMPLETED_NO)


def long_any(v):
    return CORBA.Any(CORBA.TC_long, v)


class DiiCases(object):
    """DII calls of the servant ``self.servant`` (static or dynamic)."""

    servant = None

    def setUp(self):
        self.servant = self.make_servant()
        self.obj = remote(activate(self.servant))

    def test_return_value(self):
        req = self.obj._request("echo_long")
        req.add_in_arg(long_any(-42))
        req.set_return_type(CORBA.TC_long)
        req.invoke()
        self.assertEqual(req.return_value().value(), -42)
        self.assertEqual(req.result().argument.value(), -42)
        req = self.obj._request("echo_string")
        req.add_in_arg(CORBA.Any(CORBA.TC_string, "café あ"))
        req.set_return_type(CORBA.TC_string)
        req.invoke()
        self.assertEqual(req.return_value().value(), "café あ")

    def test_struct(self):
        rec = T.Record(7, "seven", T.GREEN, T.Point(1.5, -2.0), [1, 2, 3], True)
        req = self.obj._request("echo_record")
        req.add_in_arg(CORBA.Any(T._tc_Record, rec))
        req.set_return_type(T._tc_Record)
        req.invoke()
        self.assertEqual(req.return_value().value(), rec)

    def test_inout_and_out(self):
        req = self.obj._request("swap")
        req.add_inout_arg(long_any(3), "a")
        req.add_inout_arg(long_any(4), "b")
        req.set_return_type(CORBA.TC_long)
        req.invoke()
        self.assertEqual(req.return_value().value(), 7)
        self.assertEqual([nv.argument.value() for nv in req.arguments()], [4, 3])
        self.assertEqual([nv.name for nv in req.arguments()], ["a", "b"])

        args = client_orb.create_list(3)
        args.add_value("a", long_any(17), CORBA.ARG_IN)
        args.add_value("b", long_any(5), CORBA.ARG_IN)
        args.add_item("remainder", CORBA.ARG_OUT).argument = CORBA.Any(CORBA.TC_long, None)
        result = client_orb.create_named_value("", CORBA.Any(CORBA.TC_long, None), 0)
        req = self.obj._create_request(None, "divide", args, result, [T._tc_DivideByZero])
        req.invoke()
        self.assertEqual(result.argument.value(), 3)
        self.assertEqual(args.item(2).argument.value(), 2)

    def test_user_exceptions(self):
        req = self.obj._request("fail")
        req.add_in_arg(long_any(9))
        req.add_in_arg(CORBA.Any(CORBA.TC_string, "nine"))
        # Not listed: UNKNOWN.
        with self.assertRaises(CORBA.UNKNOWN):
            req.invoke()
        req.add_exception(T._tc_Oops)
        with self.assertRaises(T.Oops) as cm:
            req.invoke()
        self.assertEqual((cm.exception.code, cm.exception.reason), (9, "nine"))

        req = self.obj._request("divide")
        req.add_in_arg(long_any(1))
        req.add_in_arg(long_any(0))
        req.add_out_arg(CORBA.TC_long)
        req.set_return_type(CORBA.TC_long)
        req.add_exception(T._tc_DivideByZero)
        with self.assertRaises(T.DivideByZero):
            req.invoke()

    def test_system_exceptions(self):
        with self.assertRaises(CORBA.NO_PERMISSION) as cm:
            self.obj._request("fail_system").invoke()
        self.assertEqual(cm.exception.minor, 5)
        with self.assertRaises(CORBA.BAD_OPERATION):
            self.obj._request("no_such_operation").invoke()
        # Too few arguments for the operation.
        with self.assertRaises(CORBA.MARSHAL):
            self.obj._request("echo_long").invoke()
        with self.assertRaises(CORBA.NO_IMPLEMENT):
            self.obj._create_request(object(), "echo_long", None, None)

    def test_oneway(self):
        req = self.obj._request("post")
        req.add_in_arg(CORBA.Any(CORBA.TC_string, "hello"))
        req.send_oneway()
        for _ in range(200):
            if self.servant.posted_:
                break
            time.sleep(0.01)
        self.assertEqual(self.servant.posted_, ["hello"])

    def test_deferred(self):
        reqs = []
        for i in range(5):
            req = self.obj._request("echo_long")
            req.add_in_arg(long_any(i))
            req.set_return_type(CORBA.TC_long)
            reqs.append(req)
        reqs[0].send_deferred()
        reqs[0].get_response()
        self.assertEqual(reqs[0].return_value().value(), 0)
        with self.assertRaises(CORBA.BAD_INV_ORDER):
            reqs[0].get_response()

        client_orb.send_multiple_requests_deferred(reqs[1:])
        got = []
        for _ in reqs[1:]:
            r = client_orb.get_next_response()
            r.get_response()
            got.append(r.return_value().value())
        self.assertEqual(sorted(got), [1, 2, 3, 4])
        self.assertFalse(client_orb.poll_next_response())
        with self.assertRaises(CORBA.BAD_INV_ORDER):
            client_orb.get_next_response()

        # A deferred call that fails raises at get_response.
        req = self.obj._request("fail_system")
        req.send_deferred()
        deadline = time.time() + 10
        while not req.poll_response() and time.time() < deadline:
            time.sleep(0.01)
        with self.assertRaises(CORBA.NO_PERMISSION):
            req.get_response()

    def test_static_stub_still_works(self):
        echo = self.obj._narrow(T.Echo)
        self.assertEqual(echo.echo_long(5), 5)
        self.assertEqual(echo.swap(1, 2), (3, 2, 1))
        with self.assertRaises(T.Oops):
            echo.fail(1, "x")


class DiiToStatic(DiiCases, unittest.TestCase):
    def make_servant(self):
        return StaticEcho.make()


class DiiToDynamic(DiiCases, unittest.TestCase):
    def make_servant(self):
        return DynamicEcho()

    def test_dynamic_servant(self):
        self.assertTrue(self.obj._is_a(ECHO_ID))
        self.assertFalse(self.obj._is_a("IDL:rustorb.org/InteropTest/Callback:1.0"))
        self.assertEqual(self.obj._orb_object().type_id(), ECHO_ID)

    def test_server_request_rules(self):
        errors = []

        class Rules(PortableServer.DynamicImplementation):
            def _primary_interface(self, oid, poa):
                return ECHO_ID

            def invoke(self, request):
                try:
                    request.set_result(long_any(1))
                except CORBA.BAD_INV_ORDER:
                    errors.append("result before arguments")
                request.arguments(CORBA.NVList())
                try:
                    request.arguments(CORBA.NVList())
                except CORBA.BAD_INV_ORDER:
                    errors.append("arguments twice")
                try:
                    request.set_exception(long_any(1))
                except CORBA.BAD_PARAM:
                    errors.append("not an exception")
                request.set_result(long_any(5))

        obj = remote(activate(Rules()))
        req = obj._request("anything")
        req.set_return_type(CORBA.TC_long)
        req.invoke()
        self.assertEqual(req.return_value().value(), 5)
        self.assertEqual(errors, ["result before arguments", "arguments twice", "not an exception"])

    def test_concurrent_requests(self):
        results = []

        def call(i):
            req = self.obj._request("echo_long")
            req.add_in_arg(long_any(i))
            req.set_return_type(CORBA.TC_long)
            req.invoke()
            results.append(req.return_value().value())

        threads = [threading.Thread(target=call, args=(i,)) for i in range(20)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        self.assertEqual(sorted(results), list(range(20)))


if __name__ == "__main__":
    unittest.main()
