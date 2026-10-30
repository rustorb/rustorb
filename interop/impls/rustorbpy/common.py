"""Helpers shared by the rustorb Python server and client.

run-matrix.sh builds the rustorb Python package (with its native module)
into lib/ and generates the InteropTest and CosNaming stubs with
rustorb-idl into stubs/ (build.sh).
"""

import argparse
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [os.path.join(HERE, "lib"), os.path.join(HERE, "stubs")]

from rustorb import CORBA  # noqa: E402
import CosNaming  # noqa: E402


def parse_args(description, extra=None):
    parser = argparse.ArgumentParser(description=description)
    parser.add_argument("--naming", required=True,
                        help="corbaloc of the root naming context")
    parser.add_argument("--name", required=True,
                        help="slash separated name of the Echo object, e.g. interop/Echo")
    parser.add_argument("--host", default=None,
                        help="host name to publish in IORs (default: omniORB picks one)")
    parser.add_argument("--port", type=int, default=0,
                        help="fixed GIOP endpoint port (0: any)")
    parser.add_argument("--timeout", type=float, default=30.0,
                        help="seconds to wait for the naming service / server")
    parser.add_argument("--suite", choices=("basic", "any", "value", "custom"), default="basic",
                        help="the InteropTest cases (basic) or the AnyTest ones (any)")
    if extra:
        extra(parser)
    args, orb_args = parser.parse_known_args()
    return args, orb_args


def orb_init(args, orb_args):
    argv = [sys.argv[0]] + orb_args
    argv += ["-ORBInitRef", "NameService=" + args.naming]
    # omniORB reads ORB options from ORB<option> environment variables;
    # the interop compose file sets this one for clients.
    max_giop = os.environ.get("ORBmaxGIOPVersion")
    if max_giop:
        argv += ["-ORBmaxGIOPVersion", max_giop]
    if args.port:
        endpoint = "giop:tcp:%s:%d" % (args.host or "", args.port)
        argv += ["-ORBendPoint", endpoint]
    return CORBA.ORB_init(argv, CORBA.ORB_ID)


def to_name(path):
    return [CosNaming.NameComponent(c, "") for c in path.split("/") if c]


def retry(what, fn, timeout):
    """Calls fn until it succeeds or timeout seconds have passed."""
    deadline = time.monotonic() + timeout
    while True:
        try:
            return fn()
        except (CORBA.TRANSIENT, CORBA.COMM_FAILURE, CORBA.OBJECT_NOT_EXIST,
                CosNaming.NamingContext.NotFound):
            if time.monotonic() >= deadline:
                print("timed out waiting for %s" % what, file=sys.stderr)
                raise
            time.sleep(0.2)


def root_context(orb, timeout):
    def resolve():
        obj = orb.resolve_initial_references("NameService")
        ctx = obj._narrow(CosNaming.NamingContext)
        if ctx is None:
            raise CORBA.TRANSIENT()
        return ctx
    return retry("the naming service", resolve, timeout)
