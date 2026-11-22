// The abstract interface tests (idl/AbstractTest.idl) on the IDL4 to C#
// mapping: the Person servant, the Card implementation, the AbstractEcho
// servant and the client cases, those of
// impls/java/interop/AbstractTests.java.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using AbstractTest;
using Omg.Types;

namespace Interop
{
    public static class AbstractTests
    {
        /// <summary>AbstractTest::Person: a Named object.</summary>
        public sealed class PersonImpl : PersonPOA
        {
            private readonly string personName;
            private readonly int personAge;

            public PersonImpl(string name, int age)
            {
                personName = name;
                personAge = age;
            }

            public override string name() => personName;
            public override int age() => personAge;
        }

        /// <summary>AbstractTest::Card: a Named value, whose name is its title.</summary>
        public sealed class CardImpl : Card
        {
            public override string name() => title;
        }

        public static void RegisterFactories(Corba.ORB orb)
        {
            orb.register_value_factory(CardHelper.id(), new Corba.ValueFactory(() => new CardImpl()));
        }

        public sealed class AbstractEchoImpl : AbstractEchoPOA
        {
            private readonly Corba.ORB orb;

            public AbstractEchoImpl(Corba.ORB orb)
            {
                this.orb = orb;
            }

            public override INamed echo_named(INamed n) => n;
            public override string name_of(INamed n) => n.name();
            public override bool is_object(INamed n) => n is Corba.IObject;
            public override NamedPair echo_pair(NamedPair p) => p;
            public override ISequence<INamed> echo_seq(ISequence<INamed> s) => s;
            public override IPerson make_person(string name) => new PersonImpl(name, 42)._this();
            public override Corba.Any echo_any(Corba.Any a) => a;
            public override void shutdown() => orb.shutdown(false);
        }

        private static Card MakeCard(string title) => new CardImpl { title = title };

        private static void Check(bool cond, string what)
        {
            if (!cond)
            {
                throw new CheckFailed(what);
            }
        }

        private static void IsCard(INamed v, string title)
        {
            Check(v is CardImpl, "got " + v);
            Check(((Card)v).title == title && v.name() == title, "wrong card");
        }

        private static void IsAlice(INamed v)
        {
            Check(v is Corba.IObject, "got " + v);
            Check(v.name() == "alice", "wrong name");
        }

        /// <summary>The client's RootPOA must be active: the Person it sends is
        /// activated there.</summary>
        public static void Run(Runner r, IAbstractEcho echo)
        {
            IPerson alice = new PersonImpl("alice", 30)._this();
            r.Run("echo_named[object]", () => IsAlice(echo.echo_named(alice)));
            r.Run("echo_named[value]", () => IsCard(echo.echo_named(MakeCard("ace")), "ace"));
            r.Run("echo_named[null]", () => Check(echo.echo_named(null) == null, "not null"));
            r.Run("name_of[object]", () => Check(echo.name_of(alice) == "alice", "wrong name"));
            r.Run("name_of[value]", () => Check(echo.name_of(MakeCard("ace")) == "ace", "wrong name"));
            r.Run("is_object[object]", () => Check(echo.is_object(alice), "not an object"));
            r.Run("is_object[value]", () => Check(!echo.is_object(MakeCard("ace")), "an object"));
            r.Run("echo_pair[shared]", () =>
            {
                Card c = MakeCard("king");
                NamedPair got = echo.echo_pair(new NamedPair(c, c));
                IsCard(got.first, "king");
                Check(ReferenceEquals(got.first, got.second), "not one instance");
            });
            r.Run("echo_pair[mixed]", () =>
            {
                NamedPair got = echo.echo_pair(new NamedPair(alice, MakeCard("queen")));
                IsAlice(got.first);
                IsCard(got.second, "queen");
            });
            r.Run("echo_seq", () =>
            {
                Card c = MakeCard("jack");
                ISequence<INamed> got = echo.echo_seq(new Sequence<INamed> { c, alice, null, c });
                Check(got.Count == 4, "wrong length");
                IsCard(got[0], "jack");
                IsAlice(got[1]);
                Check(got[2] == null, "not null");
                Check(ReferenceEquals(got[0], got[3]), "not one instance");
            });
            r.Run("make_person", () =>
            {
                IPerson p = echo.make_person("bob");
                Check(p.name() == "bob" && p.age() == 42, "wrong person");
            });
            r.Run("echo_any[value]", () =>
            {
                var a = new Corba.Any();
                NamedHelper.insert(a, MakeCard("joker"));
                Corba.Any got = echo.echo_any(a);
                Check(got.type.equal(NamedHelper.type()), "wrong TypeCode " + got.type);
                IsCard(NamedHelper.extract(got), "joker");
            });
            r.Run("echo_any[object]", () =>
            {
                var a = new Corba.Any();
                NamedHelper.insert(a, alice);
                Corba.Any got = echo.echo_any(a);
                Check(got.type.equal(NamedHelper.type()), "wrong TypeCode " + got.type);
                IsAlice(NamedHelper.extract(got));
            });
        }
    }
}
