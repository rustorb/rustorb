"""omniORBpy implementation of the InteropTest::Echo server."""

import sys
import threading

from common import CORBA, CosNaming, orb_init, parse_args, root_context, to_name
from omniORB import PortableServer
import InteropTest
import InteropTest__POA


class CallbackError(Exception):
    pass


class EchoServant(InteropTest__POA.Echo):

    def __init__(self, orb):
        self._orb = orb
        self._lock = threading.Lock()
        self._posted = []
        self._counter = 0
        self._callback = None

    # --- basic types ---
    def echo_boolean(self, v): return v
    def echo_octet(self, v): return v
    def echo_char(self, v): return v
    def echo_wchar(self, v): return v
    def echo_short(self, v): return v
    def echo_ushort(self, v): return v
    def echo_long(self, v): return v
    def echo_ulong(self, v): return v
    def echo_longlong(self, v): return v
    def echo_ulonglong(self, v): return v
    def echo_float(self, v): return v
    def echo_double(self, v): return v
    def echo_string(self, v): return v
    def echo_wstring(self, v): return v

    # --- constructed types ---
    def echo_color(self, v): return v
    def echo_point(self, v): return v
    def echo_record(self, v): return v
    def echo_records(self, v): return v
    def echo_value(self, v): return v
    def echo_shade(self, v): return v
    def echo_long_array(self, v): return v
    def echo_matrix(self, v): return v
    def echo_long_seq(self, v): return v
    def echo_string_seq(self, v): return v
    def echo_octets(self, v): return v

    # --- parameter modes ---
    def swap(self, a, b):
        return (a + b, b, a)

    def divide(self, a, b):
        if b == 0:
            raise InteropTest.DivideByZero()
        q = abs(a) // abs(b)
        if (a < 0) != (b < 0):
            q = -q
        return (q, a - q * b)

    def make(self, x):
        p = InteropTest.Point(float(x), float(-x))
        r = InteropTest.Record(x, "r%d" % x, InteropTest.BLUE, p, [x, x + 1], True)
        return (p, r)

    # --- exceptions ---
    def fail(self, code, reason):
        raise InteropTest.Oops(code, reason)

    def fail_system(self):
        raise CORBA.BAD_PARAM(42, CORBA.COMPLETED_NO)

    # --- attributes ---
    def _get_counter(self):
        with self._lock:
            return self._counter

    def _set_counter(self, v):
        with self._lock:
            self._counter = v

    def _get_name(self):
        return "rustorb-interop-echo"

    # --- oneway ---
    def post(self, message):
        with self._lock:
            self._posted.append(message)

    def posted(self):
        with self._lock:
            result, self._posted = self._posted, []
        return result

    # --- object references ---
    def self_ref(self):
        return self._this()

    def echo_ref(self, other):
        if CORBA.is_nil(other):
            return self._this()
        return other

    def is_nil(self, other):
        return CORBA.is_nil(other)

    # --- callbacks ---
    def call_back(self, cb, message):
        return cb.notify(message)

    def register_callback(self, cb):
        with self._lock:
            self._callback = cb

    def trigger(self, message):
        with self._lock:
            cb = self._callback
        if cb is None:
            raise InteropTest.Oops(1, "no callback")
        return cb.notify(message)

    # --- lifecycle ---
    def shutdown(self):
        threading.Thread(target=self._orb.shutdown, args=(False,), daemon=True).start()


def bind(root, path, obj):
    """Binds obj at path, creating intermediate contexts as needed."""
    name = to_name(path)
    ctx = root
    for component in name[:-1]:
        try:
            ctx = ctx.bind_new_context([component])
        except CosNaming.NamingContext.AlreadyBound:
            ctx = ctx.resolve([component])._narrow(CosNaming.NamingContext)
    ctx.rebind(name[-1:], obj)


def main():
    args, orb_args = parse_args(__doc__)
    orb = orb_init(args, orb_args)

    root_poa = orb.resolve_initial_references("RootPOA")
    # A PERSISTENT / USER_ID POA gives the Echo object a fixed object key, so
    # that GIOP captures are reproducible between runs.
    policies = [
        root_poa.create_lifespan_policy(PortableServer.PERSISTENT),
        root_poa.create_id_assignment_policy(PortableServer.USER_ID),
    ]
    if args.suite == "value":
        # The valuetype suite: ValueTest::ValueEcho, which also stops the
        # server.
        import valuetest
        valuetest.register_factories(orb)
        poa = root_poa.create_POA("ValueTest", root_poa._get_the_POAManager(), policies)
        poa.activate_object_with_id(b"ValueEcho", valuetest.ValueEchoServant(orb))
        root_poa._get_the_POAManager().activate()
        bind(root_context(orb, args.timeout), args.name, poa.id_to_reference(b"ValueEcho"))
        print("server ready: %s" % args.name, flush=True)
        orb.run()
        orb.destroy()
        print("server stopped", flush=True)
        return 0
    if args.suite == "any":
        # The stage 2 suite: AnyTest::AnyEcho, plus an Echo object through
        # which the client stops the server.
        import anytest
        poa = root_poa.create_POA("AnyTest", root_poa._get_the_POAManager(), policies)
        poa.activate_object_with_id(b"AnyEcho", anytest.AnyEchoServant())
        poa.activate_object_with_id(b"Echo", EchoServant(orb))
        root_poa._get_the_POAManager().activate()
        obj = poa.id_to_reference(b"AnyEcho")
        root = root_context(orb, args.timeout)
        bind(root, args.name, obj)
        bind(root, args.name + "-Echo", poa.id_to_reference(b"Echo"))
        print("server ready: %s" % args.name, flush=True)
        orb.run()
        orb.destroy()
        print("server stopped", flush=True)
        return 0
    poa = root_poa.create_POA("InteropTest", root_poa._get_the_POAManager(), policies)
    servant = EchoServant(orb)
    poa.activate_object_with_id(b"Echo", servant)
    root_poa._get_the_POAManager().activate()

    obj = poa.id_to_reference(b"Echo")
    bind(root_context(orb, args.timeout), args.name, obj)
    print("server ready: %s" % args.name, flush=True)

    orb.run()
    orb.destroy()
    print("server stopped", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
