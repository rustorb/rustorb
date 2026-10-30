// The abstract interface tests (idl/AbstractTest.idl) on the standard IDL
// to Java mapping: the Person servant, the Card implementation, the
// AbstractEcho servant and the client cases, those of
// impls/rustorbpy/abstracttest.py.

package interop;

import AbstractTest.AbstractEcho;
import AbstractTest.AbstractEchoPOA;
import AbstractTest.Card;
import AbstractTest.CardHelper;
import AbstractTest.Named;
import AbstractTest.NamedHelper;
import AbstractTest.NamedPair;
import AbstractTest.Person;
import AbstractTest.PersonPOA;
import org.omg.CORBA.Any;
import org.omg.CORBA.ORB;

public final class AbstractTests {
    private AbstractTests() {}

    /** AbstractTest::Person: a Named object. */
    static final class PersonImpl extends PersonPOA {
        private final String name;
        private final int age;

        PersonImpl(String name, int age) {
            this.name = name;
            this.age = age;
        }

        @Override
        public String name() {
            return name;
        }

        @Override
        public int age() {
            return age;
        }
    }

    /** AbstractTest::Card: a Named value, whose name is its title. */
    static final class CardImpl extends Card {
        private static final long serialVersionUID = 1L;

        @Override
        public String name() {
            return title;
        }
    }

    public static void registerFactories(ORB orb) {
        ((org.omg.CORBA_2_3.ORB) orb).register_value_factory(CardHelper.id(),
                is -> is.read_value(new CardImpl()));
    }

    public static final class AbstractEchoImpl extends AbstractEchoPOA {
        private final ORB orb;

        public AbstractEchoImpl(ORB orb) {
            this.orb = orb;
        }

        @Override
        public Named echo_named(Named n) {
            return n;
        }

        @Override
        public String name_of(Named n) {
            return n.name();
        }

        @Override
        public boolean is_object(Named n) {
            return n instanceof org.omg.CORBA.Object;
        }

        @Override
        public NamedPair echo_pair(NamedPair p) {
            return p;
        }

        @Override
        public Named[] echo_seq(Named[] s) {
            return s;
        }

        @Override
        public Person make_person(String name) {
            return new PersonImpl(name, 42)._this(orb);
        }

        @Override
        public Any echo_any(Any a) {
            return a;
        }

        @Override
        public void shutdown() {
            orb.shutdown(false);
        }
    }

    static Card card(String title) {
        Card c = new CardImpl();
        c.title = title;
        return c;
    }

    private static void check(boolean cond, String what) {
        Cases.check(cond, what);
    }

    static void isCard(Named v, String title) {
        check(v instanceof CardImpl, "got " + v);
        check(title.equals(((Card) v).title) && title.equals(v.name()), "wrong card");
    }

    static void isAlice(Named v) {
        check(v instanceof org.omg.CORBA.Object, "got " + v);
        check("alice".equals(v.name()), "wrong name");
    }

    /** The client's RootPOA must be active: the Person it sends is
     * activated there. */
    public static void run(Cases.Runner r, ORB orb, AbstractEcho echo) {
        Person alice = new PersonImpl("alice", 30)._this(orb);
        r.run("echo_named[object]", () -> isAlice(echo.echo_named(alice)));
        r.run("echo_named[value]", () -> isCard(echo.echo_named(card("ace")), "ace"));
        r.run("echo_named[null]", () -> check(echo.echo_named(null) == null, "not null"));
        r.run("name_of[object]", () -> check("alice".equals(echo.name_of(alice)), "wrong name"));
        r.run("name_of[value]", () -> check("ace".equals(echo.name_of(card("ace"))), "wrong name"));
        r.run("is_object[object]", () -> check(echo.is_object(alice), "not an object"));
        r.run("is_object[value]", () -> check(!echo.is_object(card("ace")), "an object"));
        r.run("echo_pair[shared]", () -> {
            Card c = card("king");
            NamedPair got = echo.echo_pair(new NamedPair(c, c));
            isCard(got.first, "king");
            check(got.first == got.second, "not one instance");
        });
        r.run("echo_pair[mixed]", () -> {
            NamedPair got = echo.echo_pair(new NamedPair(alice, card("queen")));
            isAlice(got.first);
            isCard(got.second, "queen");
        });
        r.run("echo_seq", () -> {
            Card c = card("jack");
            Named[] got = echo.echo_seq(new Named[] {c, alice, null, c});
            check(got.length == 4, "wrong length");
            isCard(got[0], "jack");
            isAlice(got[1]);
            check(got[2] == null, "not null");
            check(got[0] == got[3], "not one instance");
        });
        r.run("make_person", () -> {
            Person p = echo.make_person("bob");
            check("bob".equals(p.name()) && p.age() == 42, "wrong person");
        });
        r.run("echo_any[value]", () -> {
            Any a = orb.create_any();
            NamedHelper.insert(a, card("joker"));
            Any got = echo.echo_any(a);
            check(got.type().equal(NamedHelper.type()), "wrong TypeCode " + got.type());
            isCard(NamedHelper.extract(got), "joker");
        });
        r.run("echo_any[object]", () -> {
            Any a = orb.create_any();
            NamedHelper.insert(a, alice);
            Any got = echo.echo_any(a);
            check(got.type().equal(NamedHelper.type()), "wrong TypeCode " + got.type());
            isAlice(NamedHelper.extract(got));
        });
    }
}
