"""Runs a fixed sequence of CosNaming operations and prints the results.

The output is plain text with one line per operation, so that the output
for omniNames can be kept as the expected result and compared with the
output for another naming service (such as the rustorb Naming Service).

    naming_check.py --naming corbaloc::naming:2809/NameService
"""

import argparse
import sys

from common import CORBA, CosNaming, orb_init, root_context
import CosNaming__POA  # noqa: F401 - makes sure the POA stubs load too

NC = CosNaming.NamingContext


def fmt_name(name):
    return "/".join(c.id + ("." + c.kind if c.kind else "") for c in name) or "<empty>"


def fmt_exc(e):
    if isinstance(e, NC.NotFound):
        return "NotFound(%s, rest=%s)" % (e.why, fmt_name(e.rest_of_name))
    if isinstance(e, NC.CannotProceed):
        return "CannotProceed(rest=%s)" % fmt_name(e.rest_of_name)
    if isinstance(e, CORBA.SystemException):
        return "%s(completed=%s)" % (type(e).__name__, e.completed)
    return type(e).__name__


class Checker:

    def __init__(self, orb, root):
        self.orb = orb
        self.root = root
        self.ext = root._narrow(CosNaming.NamingContextExt)

    def step(self, label, fn, show=None):
        try:
            result = fn()
        except CORBA.UserException as e:
            print("%-40s -> raises %s" % (label, fmt_exc(e)))
            return None
        except CORBA.SystemException as e:
            print("%-40s -> raises %s" % (label, fmt_exc(e)))
            return None
        print("%-40s -> %s" % (label, show(result) if show else "ok"))
        return result


def fmt_components(name):
    return "[" + ", ".join("(%r, %r)" % (c.id, c.kind) for c in name) + "]"


def n(path):
    return [CosNaming.NameComponent(*(c.split(".", 1) + [""])[:2]) for c in path.split("/") if c]


def fmt_bindings(bl):
    items = sorted((fmt_name(b.binding_name), str(b.binding_type)) for b in bl)
    return "[" + ", ".join("%s:%s" % i for i in items) + "]"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--naming", required=True)
    parser.add_argument("--timeout", type=float, default=30.0)
    args, orb_args = parser.parse_known_args()
    args.port = 0
    args.host = None
    orb = orb_init(args, orb_args)
    root = root_context(orb, args.timeout)
    c = Checker(orb, root)
    obj = root  # any object reference will do

    t = c.step("bind_new_context test", lambda: root.bind_new_context(n("test")))
    if t is None:
        return 1
    c.step("bind test/a", lambda: root.bind(n("test/a"), obj))
    c.step("bind test/a (again)", lambda: root.bind(n("test/a"), obj))
    c.step("rebind test/a", lambda: root.rebind(n("test/a"), obj))
    c.step("resolve test/a", lambda: root.resolve(n("test/a")),
           lambda o: "equivalent" if o._is_equivalent(obj) else "different object")
    c.step("bind test/b.kind", lambda: t.bind(n("b.kind"), obj))
    c.step("resolve test/b (no kind)", lambda: root.resolve(n("test/b")))
    c.step("resolve test/b.kind", lambda: root.resolve(n("test/b.kind")),
           lambda o: "equivalent" if o._is_equivalent(obj) else "different object")
    c.step("resolve test/missing", lambda: root.resolve(n("test/missing")))
    c.step("resolve test/missing/deeper", lambda: root.resolve(n("test/missing/deeper")))
    c.step("resolve test/a/x (not a context)", lambda: root.resolve(n("test/a/x")))
    c.step("resolve <empty>", lambda: root.resolve([]))
    c.step("bind <empty>", lambda: root.bind([], obj))

    sub = c.step("bind_new_context test/sub", lambda: root.bind_new_context(n("test/sub")))
    c.step("bind_new_context test/sub (again)", lambda: root.bind_new_context(n("test/sub")))
    c.step("bind test/sub/c", lambda: root.bind(n("test/sub/c"), obj))
    c.step("resolve test/sub (is a context)", lambda: root.resolve(n("test/sub")),
           lambda o: "NamingContext" if o._narrow(NC) is not None else "not a context")
    loose = c.step("new_context", root.new_context)
    c.step("bind_context test/loose", lambda: root.bind_context(n("test/loose"), loose))
    c.step("rebind_context test/loose", lambda: root.rebind_context(n("test/loose"), loose))

    c.step("list test (how_many=10)", lambda: t.list(10),
           lambda r: fmt_bindings(r[0]) + (" + iterator" if r[1] is not None else ""))

    def list_with_iterator():
        bl, it = t.list(1)
        names = list(bl)
        steps = ["first=%d" % len(bl)]
        while it is not None:
            more, chunk = it.next_n(2)
            steps.append("next_n(2)=%s/%d" % (more, len(chunk)))
            names += list(chunk)
            if not more:
                it.destroy()
                break
        return "%s %s" % (fmt_bindings(names), " ".join(steps))
    c.step("list test (how_many=1) + iterator", list_with_iterator, str)

    c.step("destroy test/sub (not empty)", lambda: sub.destroy())
    c.step("unbind test/sub/c", lambda: root.unbind(n("test/sub/c")))
    c.step("destroy test/sub", lambda: sub.destroy())
    c.step("resolve test/sub (dangling)", lambda: root.resolve(n("test/sub")),
           lambda o: "still bound")
    c.step("unbind test/sub", lambda: root.unbind(n("test/sub")))
    c.step("unbind test/a", lambda: root.unbind(n("test/a")))
    c.step("unbind test/a (again)", lambda: root.unbind(n("test/a")))
    c.step("resolve test/a", lambda: root.resolve(n("test/a")))

    if c.ext is not None:
        c.step("to_string [a.b, c/d, .e]", lambda: c.ext.to_string(
            [CosNaming.NameComponent("a", "b"), CosNaming.NameComponent("c/d", ""),
             CosNaming.NameComponent("", "e")]), str)
        c.step("to_name a.b/c\\/d/.e", lambda: c.ext.to_name("a.b/c\\/d/.e"), fmt_components)
        c.step("to_url host:2809 a/b", lambda: c.ext.to_url("host:2809", "a/b"), str)
        c.step("to_url :host:2809 a/b c", lambda: c.ext.to_url(":host:2809", "a/b c"), str)
        c.step("resolve_str test/b.kind", lambda: c.ext.resolve_str("test/b.kind"),
               lambda o: "equivalent" if o._is_equivalent(obj) else "different object")
        c.step("to_name <invalid>", lambda: c.ext.to_name("a//b"), fmt_components)
    else:
        print("root context is not a NamingContextExt")

    c.step("list test (final)", lambda: t.list(100), lambda r: fmt_bindings(r[0]))
    orb.destroy()
    return 0


if __name__ == "__main__":
    sys.exit(main())
