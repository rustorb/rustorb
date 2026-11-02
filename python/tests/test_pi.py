"""Tests of Portable Interceptors in the rustorb package
(``rustorb.PortableInterceptor``, ``rustorb.IOP``): an ``ORBInitializer``
registered before ``ORB_init``, client and server request interceptors,
an IOR interceptor, ``PICurrent`` and ``Codec``.

Two ORBs in this process, a server and a client, talk over TCP on the
loopback interface. The initializer is registered for the whole process,
so it installs its interceptors only in the ORBs whose id starts with
``pi-``. Run like test_orb.py.
"""

import os
import sys
import tempfile
import threading
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
for p in (os.path.join(ROOT, "python"), os.path.join(ROOT, "crates", "rustorb-codegen", "python")):
    if p not in sys.path:
        sys.path.insert(0, p)

from rustorb import CORBA, IOP, PortableInterceptor, PortableServer  # noqa: E402

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

OUT = tempfile.mkdtemp(prefix="rustorb-pi-test-")

#: The request context, the reply context and the tagged component.
CTX = 0x52540001
REPLY_CTX = 0x52540002
TAG = 0x52540003
ECHO_ID = "IDL:rustorb.org/InteropTest/Echo:1.0"

server_orb = client_orb = None
T = T__POA = None
#: The initializer of each ORB, by ORB id.
inits = {}


def string_any(s):
    return CORBA.Any(CORBA.TC_string, s)


class Recorder(PortableInterceptor.ClientRequestInterceptor,
               PortableInterceptor.ServerRequestInterceptor):
    """Logs each point; carries slot ``slot`` in the request context
    ``CTX`` and answers with ``REPLY_CTX``. ``behavior`` maps an operation
    to what to do: ("forward", obj) in send_request, ("fail", exc) in
    send_request / receive_request_service_contexts, ("keep",) keeps the
    info object."""

    def __init__(self, name, slot):
        self.name = name
        self.slot = slot
        self.log = []
        self.behavior = {}
        self.kept = []
        self.lock = threading.Lock()

    def add(self, line):
        with self.lock:
            self.log.append(line)

    def take(self):
        with self.lock:
            log, self.log = self.log, []
            return log

    def line(self, point, ri):
        line = "%s %s %s" % (self.name, point, ri._get_operation())
        try:
            line += " status=%d" % ri._get_reply_status()
        except CORBA.BAD_INV_ORDER:
            pass
        return line

    # -- client --

    def send_request(self, ri):
        self.add(self.line("send_request", ri))
        b = self.behavior.get(ri._get_operation())
        if b and b[0] == "keep":
            self.kept.append(ri)
        if b and b[0] == "fail":
            raise b[1]
        if b and b[0] == "forward":
            del self.behavior[ri._get_operation()]
            raise PortableInterceptor.ForwardRequest(b[1])
        value = ri.get_slot(self.slot).value()
        ri.add_request_service_context(
            IOP.ServiceContext(CTX, (value or "none").encode()), False
        )

    def receive_reply(self, ri):
        line = self.line("receive_reply", ri)
        try:
            line += " reply_ctx=%s" % ri.get_reply_service_context(REPLY_CTX).context_data.decode()
        except CORBA.BAD_PARAM:
            pass
        self.add(line)

    def receive_exception(self, ri):
        self.add(self.line("receive_exception", ri) + " ex=" + ri._get_received_exception_id())

    def receive_other(self, ri):
        self.add(self.line("receive_other", ri))

    # -- server --

    def receive_request_service_contexts(self, ri):
        self.add(self.line("rrsc", ri))
        b = self.behavior.get(ri._get_operation())
        if b and b[0] == "fail":
            raise b[1]
        try:
            data = ri.get_request_service_context(CTX).context_data.decode()
        except CORBA.BAD_PARAM:
            data = "no context"
        ri.set_slot(self.slot, string_any("from " + data))

    def receive_request(self, ri):
        self.add(self.line("receive_request", ri))

    def send_reply(self, ri):
        self.add(self.line("send_reply", ri))
        ri.add_reply_service_context(IOP.ServiceContext(REPLY_CTX, b"server"), False)

    def send_exception(self, ri):
        self.add(self.line("send_exception", ri) + " ex=" + ri._get_sending_exception_id())

    def send_other(self, ri):
        self.add(self.line("send_other", ri))


class Tagger(PortableInterceptor.IORInterceptor):
    def __init__(self):
        self.adapters = []

    def _get_name(self):
        return "tagger"

    def establish_components(self, info):
        self.adapters.append(info._get_adapter_name())
        # The POA's own policies are there.
        policy = info.get_effective_policy(PortableServer.LIFESPAN_POLICY_ID)
        assert policy.policy_type == PortableServer.LIFESPAN_POLICY_ID
        info.add_ior_component(IOP.TaggedComponent(TAG, "/".join(info._get_adapter_name()).encode()))


class State(object):
    """What the initializer set up in one ORB."""

    def __init__(self):
        self.points = []


class Init(PortableInterceptor.ORBInitializer):
    def pre_init(self, info):
        orb_id = info._get_orb_id()
        if not orb_id.startswith("pi-"):
            return
        state = inits[orb_id] = State()
        state.points.append("pre_init")
        state.arguments = info._get_arguments()
        # resolve_initial_references is for post_init.
        try:
            info.resolve_initial_references("PICurrent")
            state.points.append("resolved in pre_init")
        except CORBA.BAD_INV_ORDER:
            pass
        state.slot = info.allocate_slot_id()
        state.recorder = Recorder(orb_id, state.slot)
        info.add_client_request_interceptor(state.recorder)
        info.add_server_request_interceptor(state.recorder)
        try:
            info.add_client_request_interceptor(state.recorder)
        except PortableInterceptor.ORBInitInfo.DuplicateName as e:
            state.points.append("duplicate " + e.name)
        state.tagger = Tagger()
        info.add_ior_interceptor(state.tagger)
        state.codec = info._get_codec_factory().create_codec(
            IOP.Encoding(IOP.ENCODING_CDR_ENCAPS, 1, 2)
        )

    def post_init(self, info):
        orb_id = info._get_orb_id()
        if not orb_id.startswith("pi-"):
            return
        state = inits[orb_id]
        state.points.append("post_init")
        state.current = info.resolve_initial_references("PICurrent")


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
    PortableInterceptor.register_orb_initializer(Init())
    server_orb = CORBA.ORB_init(
        ["prog", "-ORBendPoint", "giop:tcp:127.0.0.1:", "-ORBServerId", "srv"], "pi-server"
    )
    client_orb = CORBA.ORB_init(["prog"], "pi-client")
    server_orb.resolve_initial_references("RootPOA")._get_the_POAManager().activate()


def tearDownModule():
    for orb in (client_orb, server_orb):
        if orb is not None:
            orb.destroy()


def remote(obj):
    return client_orb.string_to_object(server_orb.object_to_string(obj))


def activate(servant):
    poa = server_orb.resolve_initial_references("RootPOA")
    return poa.id_to_reference(poa.activate_object(servant))


def make_echo():
    class Echo(T__POA.Echo):
        def echo_string(self, v):
            # What receive_request_service_contexts put in the slot, as
            # the servant's thread sees it.
            current = server_orb.resolve_initial_references("PICurrent")
            slot = inits["pi-server"].slot
            return "%s, slot: %s" % (v, current.get_slot(slot).value())

        def fail(self, code, reason):
            raise T.Oops(code, reason)

        def fail_system(self):
            raise CORBA.NO_PERMISSION(5, CORBA.COMPLETED_NO)

        def post(self, message):
            pass

    return Echo()


class DynamicEcho(PortableServer.DynamicImplementation):
    def _primary_interface(self, oid, poa):
        return ECHO_ID

    def invoke(self, request):
        args = server_orb.create_list(1)
        args.add_value("v", CORBA.Any(CORBA.TC_long, None), CORBA.ARG_IN)
        request.arguments(args)
        v = args.item(0).argument.value()
        request.set_result(CORBA.Any(CORBA.TC_long, v + 1))


class PiTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = inits["pi-server"]
        cls.client = inits["pi-client"]
        cls.local = activate(make_echo())
        cls.echo = remote(cls.local)._narrow(T.Echo)

    def setUp(self):
        self.server.recorder.take()
        self.client.recorder.take()
        self.client.recorder.behavior.clear()
        self.server.recorder.behavior.clear()

    def test_initializer(self):
        self.assertEqual(
            self.client.points, ["pre_init", "duplicate pi-client", "post_init"]
        )
        self.assertEqual(self.client.arguments, ["prog"])
        self.assertIsInstance(self.client.current, PortableInterceptor.Current)

    def test_round_trip(self):
        self.client.current.set_slot(self.client.slot, string_any("hello"))
        try:
            self.assertEqual(self.echo.echo_string("x"), "x, slot: from hello")
        finally:
            self.client.current.set_slot(self.client.slot, CORBA.Any(CORBA.TC_null, None))
        self.assertEqual(
            self.client.recorder.take(),
            [
                "pi-client send_request echo_string",
                "pi-client receive_reply echo_string status=0 reply_ctx=server",
            ],
        )
        self.assertEqual(
            self.server.recorder.take(),
            [
                "pi-server rrsc echo_string",
                "pi-server receive_request echo_string",
                "pi-server send_reply echo_string status=0",
            ],
        )

    def test_exceptions(self):
        with self.assertRaises(T.Oops):
            self.echo.fail(1, "no")
        self.assertEqual(
            self.client.recorder.take()[-1],
            "pi-client receive_exception fail status=2 ex=IDL:rustorb.org/InteropTest/Oops:1.0",
        )
        self.assertEqual(
            self.server.recorder.take()[-1],
            "pi-server send_exception fail status=2 ex=IDL:rustorb.org/InteropTest/Oops:1.0",
        )
        with self.assertRaises(CORBA.NO_PERMISSION):
            self.echo.fail_system()
        self.assertEqual(
            self.client.recorder.take()[-1],
            "pi-client receive_exception fail_system status=1 ex=IDL:omg.org/CORBA/NO_PERMISSION:1.0",
        )

    def test_failing_points(self):
        self.client.recorder.behavior["echo_string"] = ("fail", CORBA.NO_RESOURCES(3, CORBA.COMPLETED_NO))
        with self.assertRaises(CORBA.NO_RESOURCES) as cm:
            self.echo.echo_string("x")
        self.assertEqual(cm.exception.minor, 3)
        self.assertEqual(self.server.recorder.take(), [])
        self.client.recorder.behavior.clear()
        # A server interceptor's exception goes back to the client.
        self.server.recorder.behavior["echo_string"] = ("fail", CORBA.NO_PERMISSION(4, CORBA.COMPLETED_NO))
        with self.assertRaises(CORBA.NO_PERMISSION) as cm:
            self.echo.echo_string("x")
        self.assertEqual(cm.exception.minor, 4)
        # Anything not a CORBA exception is UNKNOWN.
        self.server.recorder.behavior["echo_string"] = ("fail", ValueError("oops"))
        with self.assertRaises(CORBA.UNKNOWN):
            self.echo.echo_string("x")

    def test_forward_request(self):
        other = remote(activate(DynamicEcho()))
        self.client.recorder.behavior["echo_long"] = ("forward", other)
        # The Echo object answers v, the dynamic one v + 1. Like
        # LOCATION_FORWARD, the forward stays with the reference, so this
        # one is the test's own.
        echo = remote(self.local)._narrow(T.Echo)
        self.assertEqual(echo.echo_long(41), 42)
        self.assertEqual(echo.echo_long(41), 42)
        log = self.client.recorder.take()
        self.assertEqual(log[0], "pi-client send_request echo_long")
        self.assertEqual(log[1], "pi-client send_request echo_long")

    def test_ior_components(self):
        component = IOP.TaggedComponent(TAG, b"RootPOA")
        self.assertIn(["RootPOA"], self.server.tagger.adapters)
        log = []

        class Reader(PortableInterceptor.ClientRequestInterceptor):
            pass

        # The client sees the component in send_request.
        class Probe(Recorder):
            def send_request(self, ri):
                log.append(ri.get_effective_component(TAG))
                log.append(ri._get_effective_profile().tag)
                try:
                    ri.get_effective_component(TAG + 1)
                except CORBA.BAD_PARAM as e:
                    log.append(e.minor)

        recorder = self.client.recorder
        recorder.__class__ = Probe
        try:
            self.echo.echo_string("x")
        finally:
            recorder.__class__ = Recorder
        self.assertEqual(log, [component, IOP.TAG_INTERNET_IOP, 0x4F4D0000 | 28])
        # A child POA's references carry its own path.
        root = server_orb.resolve_initial_references("RootPOA")
        root.create_POA("pi-child", None, [])
        self.assertIn(["RootPOA", "pi-child"], self.server.tagger.adapters)

    def test_info_is_valid_in_its_point_only(self):
        self.client.recorder.behavior["echo_string"] = ("keep",)
        self.echo.echo_string("x")
        ri = self.client.recorder.kept.pop()
        with self.assertRaises(CORBA.BAD_INV_ORDER):
            ri._get_operation()

    def test_dii_arguments(self):
        seen = []

        class Args(Recorder):
            def receive_reply(self, ri):
                seen.append([(p.argument.value(), p.mode) for p in ri._get_arguments()])
                seen.append(ri._get_result().value())

            def send_reply(self, ri):
                seen.append([(p.argument.value(), p.mode) for p in ri._get_arguments()])
                seen.append(ri._get_result().value())
                seen.append(ri._get_target_most_derived_interface())
                seen.append(ri._get_adapter_name())
                seen.append((ri._get_orb_id(), ri._get_server_id()))
                seen.append(ri.get_server_policy(PortableServer.LIFESPAN_POLICY_ID).policy_type)

        obj = remote(activate(DynamicEcho()))
        for r in (self.client.recorder, self.server.recorder):
            r.__class__ = Args
        try:
            req = obj._request("echo_long")
            req.add_in_arg(CORBA.Any(CORBA.TC_long, 9))
            req.set_return_type(CORBA.TC_long)
            req.invoke()
        finally:
            for r in (self.client.recorder, self.server.recorder):
                r.__class__ = Recorder
        self.assertEqual(req.return_value().value(), 10)
        self.assertEqual(
            seen,
            [
                [(9, PortableInterceptor.PARAM_IN)],
                10,
                ECHO_ID,
                ["RootPOA"],
                ("pi-server", "srv"),
                PortableServer.LIFESPAN_POLICY_ID,
                [(9, PortableInterceptor.PARAM_IN)],
                10,
            ],
        )
        # A static stub has no arguments to show.
        failed = []

        class Static(Recorder):
            def receive_reply(self, ri):
                try:
                    ri._get_arguments()
                except CORBA.NO_RESOURCES:
                    failed.append(True)

        self.client.recorder.__class__ = Static
        try:
            self.echo.echo_string("x")
        finally:
            self.client.recorder.__class__ = Recorder
        self.assertEqual(failed, [True])

    def test_pi_current(self):
        current = client_orb.resolve_initial_references("PICurrent")
        slot = self.client.slot
        current.set_slot(slot, string_any("mine"))
        self.assertEqual(current.get_slot(slot).value(), "mine")
        # Each thread has its own slots.
        other = []
        t = threading.Thread(target=lambda: other.append(current.get_slot(slot).value()))
        t.start()
        t.join()
        self.assertEqual(other, [None])
        with self.assertRaises(PortableInterceptor.InvalidSlot):
            current.get_slot(slot + 100)
        current.set_slot(slot, CORBA.Any(CORBA.TC_null, None))
        self.assertIn("PICurrent", client_orb.list_initial_services())


class CodecTest(unittest.TestCase):
    def setUp(self):
        self.factory = client_orb.resolve_initial_references("CodecFactory")

    def test_round_trip(self):
        codec = self.factory.create_codec(IOP.Encoding(IOP.ENCODING_CDR_ENCAPS, 1, 2))
        any = string_any("hello")
        self.assertEqual(codec.decode(codec.encode(any)).value(), "hello")
        data = codec.encode_value(any)
        # Byte order flag, padding, length 6 and "hello\0".
        self.assertEqual(len(data), 4 + 4 + 6)
        self.assertEqual(codec.decode_value(data, CORBA.TC_string).value(), "hello")
        point = CORBA.Any(T._tc_Point, T.Point(1, 2)) if T else None
        if point is not None:
            self.assertEqual(codec.decode(codec.encode(point)).value(), T.Point(1, 2))

    def test_errors(self):
        with self.assertRaises(IOP.CodecFactory.UnknownEncoding):
            self.factory.create_codec(IOP.Encoding(IOP.ENCODING_CDR_ENCAPS, 1, 3))
        with self.assertRaises(IOP.CodecFactory.UnknownEncoding):
            self.factory.create_codec(IOP.Encoding(5, 1, 2))
        codec = self.factory.create_codec(IOP.Encoding(IOP.ENCODING_CDR_ENCAPS, 1, 0))
        with self.assertRaises(IOP.Codec.InvalidTypeForEncoding):
            codec.encode(CORBA.Any(CORBA.TC_wstring, "ω"))
        with self.assertRaises(IOP.Codec.FormatMismatch):
            codec.decode(b"")
        with self.assertRaises(IOP.Codec.TypeMismatch):
            codec.decode_value(b"\x01\x00\x00\x00\xff\xff\xff\x7f", CORBA.TC_string)


if __name__ == "__main__":
    unittest.main()
