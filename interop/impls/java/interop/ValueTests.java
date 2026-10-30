// The valuetype tests (idl/ValueTest.idl) on the standard IDL to Java
// mapping: the ValueEcho servant and the client cases. The cases are those
// of impls/omniorbpy/valuetest.py.
//
// The classes of the valuetypes come from both rustorb-idl -bjava (concrete,
// with default factories) and JacORB's IDL compiler (abstract, without), so
// the tests use implementation classes of their own and register a factory
// for each. Tally and Account (with operations) are abstract in both.

package interop;

import java.util.IdentityHashMap;
import java.util.Map;

import ValueTest.Account;
import ValueTest.AccountHelper;
import ValueTest.AccountValueFactory;
import ValueTest.Counter;
import ValueTest.CounterPOATie;
import ValueTest.LongBox;
import ValueTest.Node;
import ValueTest.NodeHelper;
import ValueTest.Pair;
import ValueTest.Point;
import ValueTest.Point3;
import ValueTest.Point3Helper;
import ValueTest.PointHelper;
import ValueTest.Shape;
import ValueTest.ShapeHelper;
import ValueTest.StringBoxHelper;
import ValueTest.Tally;
import ValueTest.TallyHelper;
import ValueTest.ValueEcho;
import ValueTest.ValueEchoPOA;
import org.omg.CORBA.Any;
import org.omg.CORBA.ORB;

public final class ValueTests {
    private ValueTests() {}

    static final class PointImpl extends Point {
        private static final long serialVersionUID = 1L;
    }

    static final class Point3Impl extends Point3 {
        private static final long serialVersionUID = 1L;

        String getLabel() {
            return label;
        }

        void setLabel(String l) {
            label = l;
        }
    }

    static final class NodeImpl extends Node {
        private static final long serialVersionUID = 1L;
    }

    static final class ShapeImpl extends Shape {
        private static final long serialVersionUID = 1L;
    }

    /** Tally supports Counter: its servant is a CounterPOATie of it. */
    static final class TallyImpl extends Tally {
        private static final long serialVersionUID = 1L;

        TallyImpl() {}

        TallyImpl(int n) {
            this.n = n;
        }

        @Override
        public int count() {
            return n;
        }
    }

    static final class AccountImpl extends Account {
        private static final long serialVersionUID = 1L;

        AccountImpl() {}

        AccountImpl(int balance) {
            this.balance = balance;
        }

        @Override
        public int doubled() {
            return balance * 2;
        }
    }

    static final class AccountFactory implements AccountValueFactory {
        @Override
        public java.io.Serializable read_value(org.omg.CORBA_2_3.portable.InputStream is) {
            return is.read_value(new AccountImpl());
        }

        @Override
        public Account init(int balance) {
            return new AccountImpl(balance);
        }
    }

    /** A factory that reads the state into a new instance of `make`. */
    private static org.omg.CORBA.portable.ValueFactory factory(
            java.util.function.Supplier<java.io.Serializable> make) {
        return is -> is.read_value(make.get());
    }

    public static void registerFactories(ORB orb) {
        org.omg.CORBA_2_3.ORB o = (org.omg.CORBA_2_3.ORB) orb;
        o.register_value_factory(PointHelper.id(), factory(PointImpl::new));
        o.register_value_factory(Point3Helper.id(), factory(Point3Impl::new));
        o.register_value_factory(NodeHelper.id(), factory(NodeImpl::new));
        o.register_value_factory(ShapeHelper.id(), factory(ShapeImpl::new));
        o.register_value_factory(TallyHelper.id(), factory(TallyImpl::new));
        o.register_value_factory(AccountHelper.id(), new AccountFactory());
    }

    public static final class ValueEchoImpl extends ValueEchoPOA {
        private final ORB orb;

        public ValueEchoImpl(ORB orb) {
            this.orb = orb;
        }

        @Override
        public Point echo_point(Point p) {
            return p;
        }

        @Override
        public Pair echo_pair(Pair p) {
            return p;
        }

        @Override
        public Node echo_node(Node n) {
            return n;
        }

        @Override
        public Shape echo_shape(Shape s) {
            return s;
        }

        @Override
        public Point[] echo_points(Point[] s) {
            return s;
        }

        @Override
        public boolean same(Point a, Point b) {
            return a == b;
        }

        @Override
        public int count_nodes(Node n) {
            Map<Node, Boolean> seen = new IdentityHashMap<>();
            for (Node m = n; m != null && seen.put(m, true) == null; m = m.next) {
                // counted
            }
            return seen.size();
        }

        @Override
        public LongBox echo_long_box(LongBox b) {
            return b;
        }

        @Override
        public String echo_string_box(String b) {
            return b;
        }

        @Override
        public Pair echo_pair_box(Pair b) {
            return b;
        }

        @Override
        public Any echo_any(Any a) {
            return a;
        }

        @Override
        public Tally echo_tally(Tally t) {
            return t;
        }

        @Override
        public int call_count(Counter c) {
            return c.count();
        }

        @Override
        public int account_doubled(Account a) {
            return a.doubled();
        }

        @Override
        public Account make_account(int balance) {
            org.omg.CORBA.portable.ValueFactory f =
                    ((org.omg.CORBA_2_3.ORB) orb).lookup_value_factory(AccountHelper.id());
            if (!(f instanceof AccountValueFactory)) {
                throw new org.omg.CORBA.BAD_PARAM("no factory of Account");
            }
            return ((AccountValueFactory) f).init(balance);
        }

        @Override
        public void shutdown() {
            orb.shutdown(false);
        }
    }

    static Point point(int x, int y) {
        Point p = new PointImpl();
        p.x = x;
        p.y = y;
        return p;
    }

    static Point point3() {
        Point3Impl p = new Point3Impl();
        p.x = 1;
        p.y = 2;
        p.z = 3;
        p.setLabel("three");
        return p;
    }

    static Node node(int id, Node next) {
        Node n = new NodeImpl();
        n.id = id;
        n.next = next;
        return n;
    }

    /** Two nodes that refer to each other. */
    static Node ring() {
        Node a = node(1, null);
        a.next = node(2, a);
        return a;
    }

    /** A list of n nodes, ids 0 to n - 1. */
    static Node chain(int n) {
        Node head = null;
        for (int i = n - 1; i >= 0; --i) {
            head = node(i, head);
        }
        return head;
    }

    static boolean isPoint(Point p, int x, int y) {
        return p != null && !(p instanceof Point3) && p.x == x && p.y == y;
    }

    static boolean isPoint3(Point p) {
        if (!(p instanceof Point3Impl)) {
            return false;
        }
        Point3Impl q = (Point3Impl) p;
        return q.x == 1 && q.y == 2 && q.z == 3 && "three".equals(q.getLabel());
    }

    private static void check(boolean cond, String what) {
        Cases.check(cond, what);
    }

    public static void run(Cases.Runner r, ORB orb, ValueEcho echo) {
        r.run("echo_point[plain]", () -> check(isPoint(echo.echo_point(point(1, -2)), 1, -2), "wrong point"));
        r.run("echo_point[null]", () -> check(echo.echo_point(null) == null, "not null"));
        r.run("echo_point[derived]", () -> check(isPoint3(echo.echo_point(point3())), "wrong point3"));
        r.run("echo_pair[shared]", () -> {
            Point p = point(5, 6);
            Pair got = echo.echo_pair(new Pair(p, p));
            check(isPoint(got.first, 5, 6), "wrong point");
            check(got.first == got.second, "not one instance");
        });
        r.run("echo_pair[distinct]", () -> {
            Pair got = echo.echo_pair(new Pair(point(5, 6), point(5, 6)));
            check(isPoint(got.first, 5, 6) && isPoint(got.second, 5, 6), "wrong points");
            check(got.first != got.second, "one instance");
        });
        r.run("echo_pair[null]", () -> {
            Pair got = echo.echo_pair(new Pair(null, point3()));
            check(got.first == null, "not null");
            check(isPoint3(got.second), "wrong point3");
        });
        r.run("same[shared]", () -> {
            Point p = point(0, 0);
            check(echo.same(p, p), "not the same");
        });
        r.run("same[distinct]", () -> check(!echo.same(point(0, 0), point(0, 0)), "the same"));
        r.run("echo_node[list]", () -> {
            Node got = echo.echo_node(chain(20));
            int i = 0;
            for (Node m = got; m != null; m = m.next, ++i) {
                check(m.id == i, "wrong id");
            }
            check(i == 20, "wrong length");
        });
        r.run("echo_node[ring]", () -> {
            Node got = echo.echo_node(ring());
            check(got.id == 1 && got.next.id == 2, "wrong ids");
            check(got.next.next == got, "the cycle is lost");
        });
        r.run("count_nodes[list]", () -> check(echo.count_nodes(chain(20)) == 20, "wrong count"));
        r.run("count_nodes[ring]", () -> check(echo.count_nodes(ring()) == 2, "wrong count"));
        r.run("echo_shape", () -> {
            Point c = point(0, 0);
            Shape shape = new ShapeImpl();
            shape.name = "square";
            shape.points = new Point[] {point(1, 1), c};
            shape.center = c;
            shape.area = 1.5;
            Shape got = echo.echo_shape(shape);
            check("square".equals(got.name) && got.area == 1.5, "wrong name or area");
            check(got.points.length == 2, "wrong length");
            check(isPoint(got.points[0], 1, 1) && isPoint(got.center, 0, 0), "wrong points");
            check(got.points[1] == got.center, "not one instance");
        });
        r.run("echo_points", () -> {
            Point p = point(7, 8);
            Point q = point3();
            Point[] got = echo.echo_points(new Point[] {p, q, p, null, q});
            check(got.length == 5, "wrong length");
            check(isPoint(got[0], 7, 8) && isPoint3(got[1]), "wrong points");
            check(got[0] == got[2] && got[1] == got[4], "sharing lost");
            check(got[3] == null, "not null");
        });
        r.run("echo_long_box[value]", () -> {
            LongBox got = echo.echo_long_box(new LongBox(7));
            check(got != null && got.value == 7, "wrong box");
        });
        r.run("echo_long_box[null]", () -> check(echo.echo_long_box(null) == null, "not null"));
        r.run("echo_string_box[value]", () -> check("boxed".equals(echo.echo_string_box("boxed")), "wrong box"));
        r.run("echo_string_box[null]", () -> check(echo.echo_string_box(null) == null, "not null"));
        r.run("echo_pair_box", () -> {
            Pair got = echo.echo_pair_box(new Pair(point(1, 2), null));
            check(got != null, "null");
            check(isPoint(got.first, 1, 2), "wrong point");
            check(got.second == null, "not null");
        });
        r.run("echo_any[ring]", () -> {
            Any a = orb.create_any();
            NodeHelper.insert(a, ring());
            Any got = echo.echo_any(a);
            check(got.type().equal(NodeHelper.type()), "wrong TypeCode");
            Node m = NodeHelper.extract(got);
            check(m != null && m.id == 1 && m.next.id == 2 && m.next.next == m, "wrong ring");
        });
        r.run("echo_any[derived]", () -> {
            Any a = orb.create_any();
            PointHelper.insert(a, point3());
            Any got = echo.echo_any(a);
            check(got.type().equal(PointHelper.type()), "wrong TypeCode");
            check(isPoint3(PointHelper.extract(got)), "wrong point3");
        });
        r.run("echo_any[box]", () -> {
            Any a = orb.create_any();
            StringBoxHelper.insert(a, "in an any");
            Any got = echo.echo_any(a);
            check(got.type().equal(StringBoxHelper.type()), "wrong TypeCode");
            check("in an any".equals(StringBoxHelper.extract(got)), "wrong box");
        });
        r.run("echo_tally", () -> {
            Tally got = echo.echo_tally(new TallyImpl(3));
            check(got instanceof TallyImpl, "got " + got);
            check(got.n == 3 && got.count() == 3, "wrong tally");
        });
        r.run("call_count", () -> {
            Counter c = new CounterPOATie(new TallyImpl(5))._this(orb);
            check(echo.call_count(c) == 5, "wrong count");
        });
        r.run("account_doubled", () -> check(echo.account_doubled(new AccountImpl(21)) == 42, "wrong result"));
        r.run("make_account", () -> {
            Account got = echo.make_account(7);
            check(got instanceof AccountImpl, "got " + got);
            check(got.balance == 7 && got.doubled() == 14, "wrong account");
        });
    }
}
