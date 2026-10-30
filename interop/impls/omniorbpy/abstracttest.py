"""The abstract interface tests (idl/AbstractTest.idl): the Person servant,
the Card implementation, the AbstractEcho servant and the client cases.

The same file is in impls/omniorbpy/ and impls/rustorbpy/: it uses only
the standard IDL to Python mapping, and gets CORBA from common.py.
"""

from common import CORBA
import AbstractTest
import AbstractTest__POA

A = AbstractTest


class PersonServant(AbstractTest__POA.Person):
    """AbstractTest::Person: a Named object."""

    def __init__(self, name, age):
        self._name = name
        self._age = age

    def name(self):
        return self._name

    def age(self):
        return self._age


class CardImpl(A.Card):
    """AbstractTest::Card: a Named value, whose name is its title."""

    def name(self):
        return self.title


def register(orb):
    """Registers the value factory of Card with orb."""
    orb.register_value_factory(A.Card._NP_RepositoryId, CardImpl)


class AbstractEchoServant(AbstractTest__POA.AbstractEcho):

    def __init__(self, orb):
        self.orb = orb

    def echo_named(self, n):
        return n

    def name_of(self, n):
        return n.name()

    def is_object(self, n):
        return isinstance(n, CORBA.Object)

    def echo_pair(self, p):
        return p

    def echo_seq(self, s):
        return s

    def make_person(self, name):
        return PersonServant(name, 42)._this()

    def echo_any(self, a):
        return a

    def shutdown(self):
        self.orb.shutdown(False)


def run_cases(r, echo, check):
    """Runs the abstract interface cases with the Runner r against the
    AbstractEcho echo. The client's RootPOA must be active: the Person it
    sends is activated there."""
    alice = PersonServant("alice", 30)._this()

    def is_card(v, title):
        check(isinstance(v, CardImpl), "got %r" % (v,))
        check(v.title == title and v.name() == title, "got %r" % (v.title,))

    def is_alice(v):
        check(isinstance(v, CORBA.Object), "got %r" % (v,))
        check(v.name() == "alice", "wrong name")

    r.case("echo_named[object]", lambda: is_alice(echo.echo_named(alice)))
    r.case("echo_named[value]", lambda: is_card(echo.echo_named(CardImpl("ace")), "ace"))
    r.case("echo_named[null]", lambda: check(echo.echo_named(None) is None, "not null"))
    r.case("name_of[object]", lambda: check(echo.name_of(alice) == "alice", "wrong name"))
    r.case("name_of[value]", lambda: check(echo.name_of(CardImpl("ace")) == "ace", "wrong name"))
    r.case("is_object[object]", lambda: check(echo.is_object(alice), "not an object"))
    r.case("is_object[value]", lambda: check(not echo.is_object(CardImpl("ace")), "an object"))

    def pair_shared():
        c = CardImpl("king")
        got = echo.echo_pair(A.NamedPair(c, c))
        is_card(got.first, "king")
        check(got.first is got.second, "not one instance")

    def pair_mixed():
        got = echo.echo_pair(A.NamedPair(alice, CardImpl("queen")))
        is_alice(got.first)
        is_card(got.second, "queen")

    r.case("echo_pair[shared]", pair_shared)
    r.case("echo_pair[mixed]", pair_mixed)

    def seq():
        c = CardImpl("jack")
        got = echo.echo_seq([c, alice, None, c])
        check(len(got) == 4, "wrong length")
        is_card(got[0], "jack")
        is_alice(got[1])
        check(got[2] is None, "not null")
        check(got[0] is got[3], "not one instance")

    r.case("echo_seq", seq)

    def person():
        p = echo.make_person("bob")
        check(p.name() == "bob" and p.age() == 42, "wrong person")

    r.case("make_person", person)

    def any_value():
        got = echo.echo_any(CORBA.Any(A._tc_Named, CardImpl("joker")))
        check(got.typecode().equal(A._tc_Named), "got %r" % (got.typecode(),))
        is_card(got.value(), "joker")

    def any_object():
        got = echo.echo_any(CORBA.Any(A._tc_Named, alice))
        check(got.typecode().equal(A._tc_Named), "got %r" % (got.typecode(),))
        is_alice(got.value())

    r.case("echo_any[value]", any_value)
    r.case("echo_any[object]", any_object)
