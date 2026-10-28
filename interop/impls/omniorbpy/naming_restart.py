"""Checks that a naming service keeps its bindings, and the references it
handed out, across a restart.

Run after naming_check.py, which leaves test/b.kind and test/loose bound:

    naming_restart.py --naming URL --save FILE   # before the restart
    naming_restart.py --naming URL --load FILE   # after it

--save writes the IOR of the context test to FILE; --load uses that IOR
again. Both print the same lines, so the output of the two runs can be
compared with one expected file.
"""

import argparse
import sys

from common import CORBA, CosNaming, orb_init, root_context

NC = CosNaming.NamingContext


def fmt_bindings(bl):
    items = sorted(("/".join(c.id + ("." + c.kind if c.kind else "") for c in b.binding_name),
                    str(b.binding_type)) for b in bl)
    return "[" + ", ".join("%s:%s" % i for i in items) + "]"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--naming", required=True)
    parser.add_argument("--timeout", type=float, default=30.0)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--save", metavar="FILE")
    mode.add_argument("--load", metavar="FILE")
    args, orb_args = parser.parse_known_args()
    args.port = 0
    args.host = None
    orb = orb_init(args, orb_args)
    root = root_context(orb, args.timeout)
    label = "before restart" if args.save else "after restart"

    if args.save:
        test = root.resolve([CosNaming.NameComponent("test", "")])
        with open(args.save, "w") as f:
            f.write(orb.object_to_string(test))
    else:
        with open(args.load) as f:
            test = orb.string_to_object(f.read().strip())
    test = test._narrow(NC)

    def step(what, fn):
        try:
            result = fn()
        except (CORBA.UserException, CORBA.SystemException) as e:
            result = "raises %s" % type(e).__name__
        print("%-40s -> %s" % ("%s: %s" % (label, what), result))

    step("list test (saved IOR)", lambda: fmt_bindings(test.list(100)[0]))
    # The object bound depends on the client that ran naming_check.
    step("resolve test/b.kind", lambda: "nil" if root.resolve(
        [CosNaming.NameComponent("test", ""), CosNaming.NameComponent("b", "kind")])
        is None else "object")
    step("resolve test/loose (context)", lambda: "NamingContext" if root.resolve(
        [CosNaming.NameComponent("test", ""), CosNaming.NameComponent("loose", "")])
        ._narrow(NC) is not None else "not a context")
    orb.destroy()
    return 0


if __name__ == "__main__":
    sys.exit(main())
