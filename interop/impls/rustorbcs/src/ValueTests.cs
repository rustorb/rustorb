// The valuetype tests (idl/ValueTest.idl) on the IDL4 to C# mapping: the
// ValueEcho servant and the client cases, those of
// impls/java/interop/ValueTests.java (case names included).
//
// The tests use implementation classes of their own (so that a value read
// can be told from one of the OBV_ classes) and register a factory for each.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using Omg.Types;
using ValueTest;

namespace Interop
{
    public static class ValueTests
    {
        public sealed class PointImpl : OBV_ValueTest.Point
        {
        }

        public sealed class Point3Impl : OBV_ValueTest.Point3
        {
            public string Label
            {
                get => label;
                set => label = value;
            }
        }

        public sealed class NodeImpl : OBV_ValueTest.Node
        {
        }

        public sealed class ShapeImpl : OBV_ValueTest.Shape
        {
        }

        /// <summary>Tally supports Counter: its servant is a CounterPOATie of it.</summary>
        public sealed class TallyImpl : Tally
        {
            public TallyImpl()
            {
            }

            public TallyImpl(int n)
            {
                this.n = n;
            }

            public override int count() => n;
        }

        public sealed class AccountImpl : Account
        {
            public AccountImpl()
            {
            }

            public AccountImpl(int balance)
            {
                this.balance = balance;
            }

            public override int doubled() => balance * 2;

            public override void init(int balance) => this.balance = balance;
        }

        public static void RegisterFactories(Corba.ORB orb)
        {
            orb.register_value_factory(PointHelper.id(), new Corba.ValueFactory(() => new PointImpl()));
            orb.register_value_factory(Point3Helper.id(), new Corba.ValueFactory(() => new Point3Impl()));
            orb.register_value_factory(NodeHelper.id(), new Corba.ValueFactory(() => new NodeImpl()));
            orb.register_value_factory(ShapeHelper.id(), new Corba.ValueFactory(() => new ShapeImpl()));
            orb.register_value_factory(TallyHelper.id(), new Corba.ValueFactory(() => new TallyImpl()));
            orb.register_value_factory(AccountHelper.id(), new Corba.ValueFactory(() => new AccountImpl()));
        }

        public sealed class ValueEchoImpl : ValueEchoPOA
        {
            private readonly Corba.ORB orb;

            public ValueEchoImpl(Corba.ORB orb)
            {
                this.orb = orb;
            }

            public override ValueTest.Point echo_point(ValueTest.Point p) => p;
            public override Pair echo_pair(Pair p) => p;
            public override ValueTest.Node echo_node(ValueTest.Node n) => n;
            public override ValueTest.Shape echo_shape(ValueTest.Shape s) => s;
            public override ISequence<ValueTest.Point> echo_points(ISequence<ValueTest.Point> s) => s;
            public override bool same(ValueTest.Point a, ValueTest.Point b) => ReferenceEquals(a, b);

            public override int count_nodes(ValueTest.Node n)
            {
                var seen = new HashSet<object>(ReferenceEqualityComparer.Instance);
                for (ValueTest.Node m = n; m != null && seen.Add(m); m = m.next)
                {
                }
                return seen.Count;
            }

            public override LongBox echo_long_box(LongBox b) => b;
            public override StringBox echo_string_box(StringBox b) => b;
            public override PairBox echo_pair_box(PairBox b) => b;
            public override Corba.Any echo_any(Corba.Any a) => a;
            public override Tally echo_tally(Tally t) => t;
            public override int call_count(ICounter c) => c.count();
            public override int account_doubled(Account a) => a.doubled();

            public override Account make_account(int balance)
            {
                if (orb.lookup_value_factory(AccountHelper.id())?.create_for_unmarshal() is not Account a)
                {
                    throw new Corba.BAD_PARAM("no factory of Account", 0, Corba.CompletionStatus.COMPLETED_NO);
                }
                a.init(balance);
                return a;
            }

            public override void shutdown() => orb.shutdown(false);
        }

        public static ValueTest.Point MakePoint(int x, int y) => new PointImpl { x = x, y = y };

        public static ValueTest.Point MakePoint3() => new Point3Impl { x = 1, y = 2, z = 3, Label = "three" };

        private static ValueTest.Node MakeNode(int id, ValueTest.Node next) => new NodeImpl { id = id, next = next };

        /// <summary>Two nodes that refer to each other.</summary>
        private static ValueTest.Node Ring()
        {
            ValueTest.Node a = MakeNode(1, null);
            a.next = MakeNode(2, a);
            return a;
        }

        /// <summary>A list of n nodes, ids 0 to n - 1.</summary>
        private static ValueTest.Node Chain(int n)
        {
            ValueTest.Node head = null;
            for (int i = n - 1; i >= 0; --i)
            {
                head = MakeNode(i, head);
            }
            return head;
        }

        public static bool IsPoint(ValueTest.Point p, int x, int y) =>
            p != null && p is not ValueTest.Point3 && p.x == x && p.y == y;

        public static bool IsPoint3(ValueTest.Point p) =>
            p is Point3Impl q && q.x == 1 && q.y == 2 && q.z == 3 && q.Label == "three";

        private static void Check(bool cond, string what)
        {
            if (!cond)
            {
                throw new CheckFailed(what);
            }
        }

        public static void Run(Runner r, IValueEcho echo)
        {
            r.Run("echo_point[plain]", () => Check(IsPoint(echo.echo_point(MakePoint(1, -2)), 1, -2), "wrong point"));
            r.Run("echo_point[null]", () => Check(echo.echo_point(null) == null, "not null"));
            r.Run("echo_point[derived]", () => Check(IsPoint3(echo.echo_point(MakePoint3())), "wrong point3"));
            r.Run("echo_pair[shared]", () =>
            {
                ValueTest.Point p = MakePoint(5, 6);
                Pair got = echo.echo_pair(new Pair(p, p));
                Check(IsPoint(got.first, 5, 6), "wrong point");
                Check(ReferenceEquals(got.first, got.second), "not one instance");
            });
            r.Run("echo_pair[distinct]", () =>
            {
                Pair got = echo.echo_pair(new Pair(MakePoint(5, 6), MakePoint(5, 6)));
                Check(IsPoint(got.first, 5, 6) && IsPoint(got.second, 5, 6), "wrong points");
                Check(!ReferenceEquals(got.first, got.second), "one instance");
            });
            r.Run("echo_pair[null]", () =>
            {
                Pair got = echo.echo_pair(new Pair(null, MakePoint3()));
                Check(got.first == null, "not null");
                Check(IsPoint3(got.second), "wrong point3");
            });
            r.Run("same[shared]", () =>
            {
                ValueTest.Point p = MakePoint(0, 0);
                Check(echo.same(p, p), "not the same");
            });
            r.Run("same[distinct]", () => Check(!echo.same(MakePoint(0, 0), MakePoint(0, 0)), "the same"));
            r.Run("echo_node[list]", () =>
            {
                ValueTest.Node got = echo.echo_node(Chain(20));
                int i = 0;
                for (ValueTest.Node m = got; m != null; m = m.next, ++i)
                {
                    Check(m.id == i, "wrong id");
                }
                Check(i == 20, "wrong length");
            });
            r.Run("echo_node[ring]", () =>
            {
                ValueTest.Node got = echo.echo_node(Ring());
                Check(got.id == 1 && got.next.id == 2, "wrong ids");
                Check(ReferenceEquals(got.next.next, got), "the cycle is lost");
            });
            r.Run("count_nodes[list]", () => Check(echo.count_nodes(Chain(20)) == 20, "wrong count"));
            r.Run("count_nodes[ring]", () => Check(echo.count_nodes(Ring()) == 2, "wrong count"));
            r.Run("echo_shape", () =>
            {
                ValueTest.Point c = MakePoint(0, 0);
                var shape = new ShapeImpl
                {
                    name = "square",
                    points = new Sequence<ValueTest.Point> { MakePoint(1, 1), c },
                    center = c,
                    area = 1.5,
                };
                ValueTest.Shape got = echo.echo_shape(shape);
                Check(got.name == "square" && got.area == 1.5, "wrong name or area");
                Check(got.points.Count == 2, "wrong length");
                Check(IsPoint(got.points[0], 1, 1) && IsPoint(got.center, 0, 0), "wrong points");
                Check(ReferenceEquals(got.points[1], got.center), "not one instance");
            });
            r.Run("echo_points", () =>
            {
                ValueTest.Point p = MakePoint(7, 8);
                ValueTest.Point q = MakePoint3();
                ISequence<ValueTest.Point> got = echo.echo_points(new Sequence<ValueTest.Point> { p, q, p, null, q });
                Check(got.Count == 5, "wrong length");
                Check(IsPoint(got[0], 7, 8) && IsPoint3(got[1]), "wrong points");
                Check(ReferenceEquals(got[0], got[2]) && ReferenceEquals(got[1], got[4]), "sharing lost");
                Check(got[3] == null, "not null");
            });
            r.Run("echo_long_box[value]", () =>
            {
                LongBox got = echo.echo_long_box(new LongBox(7));
                Check(got != null && got.value == 7, "wrong box");
            });
            r.Run("echo_long_box[null]", () => Check(echo.echo_long_box(null) == null, "not null"));
            r.Run("echo_string_box[value]", () => Check(echo.echo_string_box(new StringBox("boxed"))?.value == "boxed", "wrong box"));
            r.Run("echo_string_box[null]", () => Check(echo.echo_string_box(null) == null, "not null"));
            r.Run("echo_pair_box", () =>
            {
                PairBox got = echo.echo_pair_box(new PairBox(new Pair(MakePoint(1, 2), null)));
                Check(got != null, "null");
                Check(IsPoint(got.value.first, 1, 2), "wrong point");
                Check(got.value.second == null, "not null");
            });
            r.Run("echo_any[ring]", () =>
            {
                var a = new Corba.Any();
                NodeHelper.insert(a, Ring());
                Corba.Any got = echo.echo_any(a);
                Check(got.type.equal(NodeHelper.type()), "wrong TypeCode");
                ValueTest.Node m = NodeHelper.extract(got);
                Check(m != null && m.id == 1 && m.next.id == 2 && ReferenceEquals(m.next.next, m), "wrong ring");
            });
            r.Run("echo_any[derived]", () =>
            {
                var a = new Corba.Any();
                PointHelper.insert(a, MakePoint3());
                Corba.Any got = echo.echo_any(a);
                Check(got.type.equal(PointHelper.type()), "wrong TypeCode");
                Check(IsPoint3(PointHelper.extract(got)), "wrong point3");
            });
            r.Run("echo_any[box]", () =>
            {
                var a = new Corba.Any();
                StringBoxHelper.insert(a, new StringBox("in an any"));
                Corba.Any got = echo.echo_any(a);
                Check(got.type.equal(StringBoxHelper.type()), "wrong TypeCode");
                Check(StringBoxHelper.extract(got)?.value == "in an any", "wrong box");
            });
            r.Run("echo_tally", () =>
            {
                Tally got = echo.echo_tally(new TallyImpl(3));
                Check(got is TallyImpl, "got " + got);
                Check(got.n == 3 && got.count() == 3, "wrong tally");
            });
            r.Run("call_count", () =>
            {
                ICounter c = new CounterPOATie(new TallyImpl(5))._this();
                Check(echo.call_count(c) == 5, "wrong count");
            });
            r.Run("account_doubled", () => Check(echo.account_doubled(new AccountImpl(21)) == 42, "wrong result"));
            r.Run("make_account", () =>
            {
                Account got = echo.make_account(7);
                Check(got is AccountImpl, "got " + got);
                Check(got.balance == 7 && got.doubled() == 14, "wrong account");
            });
        }
    }
}
