"""The valuetype tests (idl/ValueTest.idl): the ValueEcho servant and the
client cases.

The same file is in impls/omniorbpy/ and impls/rustorbpy/: it uses only
the standard IDL to Python mapping, and gets CORBA from common.py.
"""

from common import CORBA  # noqa: F401 (the ORB of the implementation)
import ValueTest
import ValueTest__POA

V = ValueTest


class TallyImpl(V.Tally):
    """ValueTest::Tally: also a servant of Counter."""

    def count(self):
        return self.n


class AccountImpl(V.Account):

    def doubled(self):
        return self.balance * 2


def register_factories(orb):
    """Registers the value factories of the valuetypes with operations."""
    orb.register_value_factory(V.Tally._NP_RepositoryId, TallyImpl)
    orb.register_value_factory(V.Account._NP_RepositoryId, AccountImpl)


class ValueEchoServant(ValueTest__POA.ValueEcho):

    def __init__(self, orb):
        self.orb = orb

    def echo_point(self, p):
        return p

    def echo_pair(self, p):
        return p

    def echo_node(self, n):
        return n

    def echo_shape(self, s):
        return s

    def echo_points(self, s):
        return s

    def same(self, a, b):
        return a is b

    def count_nodes(self, n):
        seen = set()
        while n is not None and id(n) not in seen:
            seen.add(id(n))
            n = n.next
        return len(seen)

    def echo_long_box(self, b):
        return b

    def echo_string_box(self, b):
        return b

    def echo_pair_box(self, b):
        return b

    def echo_any(self, a):
        return a

    def echo_tally(self, t):
        return t

    def call_count(self, c):
        return c.count()

    def account_doubled(self, a):
        return a.doubled()

    def make_account(self, balance):
        # The mapping has no class for the initializers: the factory is
        # the implementation class.
        return AccountImpl(balance)

    def shutdown(self):
        self.orb.shutdown(False)


def chain(n):
    """A list of n nodes, ids 0 to n - 1."""
    head = None
    for i in reversed(range(n)):
        head = V.Node(i, head)
    return head


def ring():
    """Two nodes that refer to each other."""
    a = V.Node(1, None)
    b = V.Node(2, a)
    a.next = b
    return a


def run_cases(r, echo, check):
    """Runs the valuetype cases with the Runner r against the ValueEcho
    echo."""

    def point(p, x, y):
        check(type(p) is V.Point, "got %r" % (p,))
        check((p.x, p.y) == (x, y), "got %r" % ((p.x, p.y),))

    def point3(p):
        check(type(p) is V.Point3, "got %r" % (p,))
        check((p.x, p.y, p.z, p.label) == (1, 2, 3, "three"),
              "got %r" % ((p.x, p.y, p.z, p.label),))

    r.case("echo_point[plain]", lambda: point(echo.echo_point(V.Point(1, -2)), 1, -2))
    r.case("echo_point[null]", lambda: check(echo.echo_point(None) is None))
    r.case("echo_point[derived]", lambda: point3(echo.echo_point(V.Point3(1, 2, 3, "three"))))

    def pair_shared():
        p = V.Point(5, 6)
        got = echo.echo_pair(V.Pair(p, p))
        point(got.first, 5, 6)
        check(got.first is got.second, "not one instance")

    def pair_distinct():
        got = echo.echo_pair(V.Pair(V.Point(5, 6), V.Point(5, 6)))
        point(got.first, 5, 6)
        point(got.second, 5, 6)
        check(got.first is not got.second, "one instance")

    def pair_null():
        got = echo.echo_pair(V.Pair(None, V.Point3(1, 2, 3, "three")))
        check(got.first is None)
        point3(got.second)

    r.case("echo_pair[shared]", pair_shared)
    r.case("echo_pair[distinct]", pair_distinct)
    r.case("echo_pair[null]", pair_null)

    def same_shared():
        p = V.Point(0, 0)
        check(echo.same(p, p) is True)

    r.case("same[shared]", same_shared)
    r.case("same[distinct]", lambda: check(echo.same(V.Point(0, 0), V.Point(0, 0)) is False))

    def node_list():
        got = echo.echo_node(chain(20))
        ids = []
        while got is not None:
            ids.append(got.id)
            got = got.next
        check(ids == list(range(20)), "got %r" % (ids,))

    def node_ring():
        got = echo.echo_node(ring())
        check((got.id, got.next.id) == (1, 2), "got %r" % ((got.id, got.next.id),))
        check(got.next.next is got, "the cycle is lost")

    r.case("echo_node[list]", node_list)
    r.case("echo_node[ring]", node_ring)
    r.case("count_nodes[list]", lambda: check(echo.count_nodes(chain(20)) == 20))
    r.case("count_nodes[ring]", lambda: check(echo.count_nodes(ring()) == 2))

    def shape():
        c = V.Point(0, 0)
        got = echo.echo_shape(V.Shape("square", [V.Point(1, 1), c], c, 1.5))
        check((got.name, got.area) == ("square", 1.5), "got %r" % ((got.name, got.area),))
        check(len(got.points) == 2, "got %r" % (got.points,))
        point(got.points[0], 1, 1)
        point(got.center, 0, 0)
        check(got.points[1] is got.center, "not one instance")

    r.case("echo_shape", shape)

    def points():
        p = V.Point(7, 8)
        q = V.Point3(1, 2, 3, "three")
        got = echo.echo_points([p, q, p, None, q])
        check(len(got) == 5, "got %r" % (got,))
        point(got[0], 7, 8)
        point3(got[1])
        check(got[0] is got[2] and got[1] is got[4], "sharing lost")
        check(got[3] is None)

    r.case("echo_points", points)
    r.case("echo_long_box[value]", lambda: check(echo.echo_long_box(7) == 7))
    r.case("echo_long_box[null]", lambda: check(echo.echo_long_box(None) is None))
    r.case("echo_string_box[value]", lambda: check(echo.echo_string_box("boxed") == "boxed"))
    r.case("echo_string_box[null]", lambda: check(echo.echo_string_box(None) is None))

    def pair_box():
        got = echo.echo_pair_box(V.Pair(V.Point(1, 2), None))
        point(got.first, 1, 2)
        check(got.second is None)

    r.case("echo_pair_box", pair_box)

    def any_ring():
        got = echo.echo_any(CORBA.Any(V._tc_Node, ring()))
        check(got.typecode().equal(V._tc_Node), "got %r" % (got.typecode(),))
        n = got.value()
        check((n.id, n.next.id) == (1, 2) and n.next.next is n, "got %r" % (n,))

    def any_derived():
        got = echo.echo_any(CORBA.Any(V._tc_Point, V.Point3(1, 2, 3, "three")))
        check(got.typecode().equal(V._tc_Point), "got %r" % (got.typecode(),))
        point3(got.value())

    def any_box():
        got = echo.echo_any(CORBA.Any(V._tc_StringBox, "in an any"))
        check(got.typecode().equal(V._tc_StringBox), "got %r" % (got.typecode(),))
        check(got.value() == "in an any", "got %r" % (got.value(),))

    r.case("echo_any[ring]", any_ring)
    r.case("echo_any[derived]", any_derived)
    r.case("echo_any[box]", any_box)

    def tally():
        got = echo.echo_tally(TallyImpl(3))
        check(isinstance(got, TallyImpl), "got %r" % (got,))
        check(got.n == 3 and got.count() == 3, "got %r" % (got,))

    def call_count():
        t = TallyImpl(5)
        check(echo.call_count(t._this()) == 5, "wrong count")

    def account():
        got = echo.make_account(7)
        check(isinstance(got, AccountImpl), "got %r" % (got,))
        check(got.balance == 7 and got.doubled() == 14, "got %r" % (got,))

    r.case("echo_tally", tally)
    r.case("call_count", call_count)
    r.case("account_doubled", lambda: check(echo.account_doubled(AccountImpl(21)) == 42))
    r.case("make_account", account)
