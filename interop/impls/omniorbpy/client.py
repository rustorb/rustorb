"""omniORBpy implementation of the InteropTest client.

Resolves the Echo object from the naming service, runs every test case
against it and exits non-zero if any case fails.
"""

import sys
import time
import traceback

from common import CORBA, orb_init, parse_args, retry, root_context, to_name
import omniORB
from omniORB import PortableServer
import InteropTest
import InteropTest__POA


class CallbackServant(InteropTest__POA.Callback):

    def __init__(self):
        self.received = []

    def notify(self, message):
        self.received.append(message)
        return "ack:" + message


def same_struct(a, b):
    """Compares IDL values field by field (generated classes lack __eq__)."""
    if isinstance(a, (list, tuple)) and isinstance(b, (list, tuple)):
        return len(a) == len(b) and all(same_struct(x, y) for x, y in zip(a, b))
    if isinstance(a, omniORB.Union) and isinstance(b, omniORB.Union):
        return type(a) is type(b) and a._d == b._d and same_struct(a._v, b._v)
    if isinstance(a, omniORB.StructBase) and type(a) is type(b):
        return same_struct(sorted(vars(a).items()), sorted(vars(b).items()))
    return a == b


class Runner:

    def __init__(self):
        self.passed = 0
        self.failed = []

    def case(self, name, fn):
        try:
            fn()
        except Exception as e:  # noqa: BLE001 - every failure is reported
            self.failed.append(name)
            print("FAIL %s: %r" % (name, e))
            traceback.print_exc(file=sys.stdout)
        else:
            self.passed += 1
            print("ok   %s" % name)


def check(cond, msg="check failed"):
    if not cond:
        raise AssertionError(msg)


def echo_case(echo, op, value, cmp=None):
    def run():
        got = getattr(echo, op)(value)
        ok = cmp(got, value) if cmp else same_struct(got, value)
        check(ok, "sent %r, got %r" % (value, got))
    return run


def expect_raises(exc_type, fn, verify=None):
    try:
        fn()
    except exc_type as e:
        if verify:
            verify(e)
        return
    raise AssertionError("%s was not raised" % exc_type.__name__)


def float_eq(a, b):
    import struct
    return struct.pack(">f", a) == struct.pack(">f", b)


def run_cases(r, echo, poa, orb):
    T = InteropTest

    # --- basic types ---
    for op, values in [
        ("echo_boolean", [True, False]),
        ("echo_octet", [0, 0x7F, 0xFF]),
        ("echo_char", ["a", "\x00", "~"]),
        ("echo_wchar", ["a", "あ", "é"]),
        ("echo_short", [0, -32768, 32767]),
        ("echo_ushort", [0, 65535]),
        ("echo_long", [0, -2147483648, 2147483647, T.MAGIC]),
        ("echo_ulong", [0, 4294967295]),
        ("echo_longlong", [0, -(2 ** 63), 2 ** 63 - 1]),
        ("echo_ulonglong", [0, 2 ** 64 - 1]),
        ("echo_double", [0.0, -1.5, 1e300, 2.0 ** -1074]),
        ("echo_string", ["", "hello", "x" * 10000, "café"]),
        ("echo_wstring", ["", "hello", "こんにちは", "\U0001F980"]),
    ]:
        for i, v in enumerate(values):
            r.case("%s[%d]" % (op, i), echo_case(echo, op, v))
    for i, v in enumerate([0.0, -1.5, 3.4028234663852886e38, 1.1754943508222875e-38]):
        r.case("echo_float[%d]" % i, echo_case(echo, "echo_float", v, float_eq))

    # --- constructed types ---
    p = T.Point(1.25, -2.5)
    rec = T.Record(7, "seven", T.GREEN, p, [1, -2, 3], True)
    rec2 = T.Record(-1, "", T.RED, T.Point(0.0, 0.0), [], False)
    r.case("echo_color", lambda: [echo_case(echo, "echo_color", c)() for c in (T.RED, T.GREEN, T.BLUE)])
    r.case("echo_point", echo_case(echo, "echo_point", p))
    r.case("echo_record", echo_case(echo, "echo_record", rec))
    r.case("echo_records", echo_case(echo, "echo_records", [rec, rec2, rec]))
    r.case("echo_records[empty]", echo_case(echo, "echo_records", []))
    for i, v in enumerate([
        T.Value(l=-5), T.Value(s="union"), T.Value(p=p),
        T.Value(4, [1, 2]), T.Value(5, []), T.Value(99, True), T.Value(0, False),
    ]):
        r.case("echo_value[%d]" % i, echo_case(echo, "echo_value", v))
    for i, v in enumerate([T.Shade(level=200), T.Shade(label="leaf"), T.Shade(T.BLUE, None)]):
        r.case("echo_shade[%d]" % i, echo_case(echo, "echo_shade", v))
    r.case("echo_long_array", echo_case(echo, "echo_long_array", [1, -1, 2147483647, 0]))
    r.case("echo_matrix", echo_case(echo, "echo_matrix", [[1.0, 2.0, 3.0], [-4.0, 0.5, 1e-9]]))
    r.case("echo_long_seq", echo_case(echo, "echo_long_seq", list(range(-500, 500))))
    r.case("echo_long_seq[empty]", echo_case(echo, "echo_long_seq", []))
    r.case("echo_string_seq", echo_case(echo, "echo_string_seq", ["a", "", "ccc"]))
    r.case("echo_octets", echo_case(echo, "echo_octets", bytes(range(256))))

    # --- parameter modes ---
    def swap():
        check(echo.swap(3, -10) == (-7, -10, 3))
    r.case("swap", swap)

    def divide():
        check(echo.divide(17, 5) == (3, 2))
        check(echo.divide(-17, 5) == (-3, -2))
        expect_raises(T.DivideByZero, lambda: echo.divide(1, 0))
    r.case("divide", divide)

    def make():
        pt, rc = echo.make(12)
        check(same_struct(pt, T.Point(12.0, -12.0)))
        check(same_struct(rc, T.Record(12, "r12", T.BLUE, T.Point(12.0, -12.0), [12, 13], True)))
    r.case("make", make)

    # --- exceptions ---
    def oops(e):
        check(e.code == 7 and e.reason == "boom", repr(e))
    r.case("fail", lambda: expect_raises(T.Oops, lambda: echo.fail(7, "boom"), oops))

    def bad_param(e):
        check(e.minor == 42 and e.completed == CORBA.COMPLETED_NO, repr(e))
    r.case("fail_system", lambda: expect_raises(CORBA.BAD_PARAM, echo.fail_system, bad_param))

    # --- attributes ---
    def counter():
        echo._set_counter(41)
        check(echo._get_counter() == 41)
        echo._set_counter(-1)
        check(echo._get_counter() == -1)
    r.case("attribute counter", counter)
    r.case("attribute name", lambda: check(echo._get_name() == "rustorb-interop-echo"))

    # --- object references ---
    def self_ref():
        ref = echo.self_ref()
        check(ref._is_equivalent(echo))
        check(ref.echo_long(5) == 5)
    r.case("self_ref", self_ref)

    def echo_ref():
        check(echo.echo_ref(echo)._is_equivalent(echo))
        check(echo.echo_ref(None)._is_equivalent(echo))
    r.case("echo_ref", echo_ref)
    r.case("is_nil", lambda: check(echo.is_nil(None) and not echo.is_nil(echo)))

    # --- callbacks ---
    cb_servant = CallbackServant()
    poa.activate_object_with_id(b"Callback", cb_servant)
    cb = poa.id_to_reference(b"Callback")._narrow(T.Callback)

    r.case("call_back", lambda: check(echo.call_back(cb, "hi") == "ack:hi"))

    def trigger():
        echo.register_callback(cb)
        check(echo.trigger("later") == "ack:later")
        check(cb_servant.received[-1] == "later")
    r.case("register_callback/trigger", trigger)

    # --- oneway ---
    # Last, because the number of posted() polls depends on timing and would
    # otherwise change the messages after it (request ids, and the stale bytes
    # omniORB leaves in alignment padding), which breaks the golden comparison.
    def oneway():
        echo.posted()
        sent = ["one", "two", "three"]
        for m in sent:
            echo.post(m)
        got = []
        deadline = time.monotonic() + 5
        while len(got) < len(sent) and time.monotonic() < deadline:
            got += echo.posted()
            time.sleep(0.05)
        # Oneway requests may be dispatched concurrently, so order is not checked.
        check(sorted(got) == sorted(sent), "got %r" % got)
    r.case("oneway post", oneway)


def main():
    def extra(parser):
        parser.add_argument("--shutdown-server", action="store_true",
                            help="call Echo::shutdown after the tests")
    args, orb_args = parse_args(__doc__, extra)
    orb = orb_init(args, orb_args)

    root_poa = orb.resolve_initial_references("RootPOA")
    policies = [
        root_poa.create_lifespan_policy(PortableServer.PERSISTENT),
        root_poa.create_id_assignment_policy(PortableServer.USER_ID),
    ]
    poa = root_poa.create_POA("InteropTestClient", root_poa._get_the_POAManager(), policies)
    root_poa._get_the_POAManager().activate()

    root = root_context(orb, args.timeout)
    name = to_name(args.name)
    echo = retry("the Echo server", lambda: root.resolve(name), args.timeout)
    echo = echo._narrow(InteropTest.Echo)
    if echo is None:
        print("%s is not an InteropTest::Echo" % args.name)
        return 1
    retry("the Echo server", echo._non_existent, args.timeout)

    r = Runner()
    run_cases(r, echo, poa, orb)

    if args.shutdown_server:
        echo.shutdown()

    print("\n%d passed, %d failed" % (r.passed, len(r.failed)))
    if r.failed:
        print("failed: " + ", ".join(r.failed))
    orb.destroy()
    return 1 if r.failed else 0


if __name__ == "__main__":
    sys.exit(main())
