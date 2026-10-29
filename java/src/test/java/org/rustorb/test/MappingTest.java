// The IDL to Java mapping of rustorb-idl -bjava (cpp/tests/idl/
// CxxMapping.idl, generated with -Wbpackage=mapping): constants, the
// generated types, and every type of the file through stubs and skeletons,
// between two ORBs in one process and colocated.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.Arrays;

import mapping.CxxMapping.Base;
import mapping.CxxMapping.BaseHelper;
import mapping.CxxMapping.BasePackage.BaseError;
import mapping.CxxMapping.BasePackage.BaseInfo;
import mapping.CxxMapping.ByBool;
import mapping.CxxMapping.ByChar;
import mapping.CxxMapping.ByEnum;
import mapping.CxxMapping.ByULongLong;
import mapping.CxxMapping.C_CHAR;
import mapping.CxxMapping.C_DOUBLE;
import mapping.CxxMapping.C_ENUM;
import mapping.CxxMapping.C_EXPR;
import mapping.CxxMapping.C_OCTET;
import mapping.CxxMapping.C_SHORT;
import mapping.CxxMapping.C_STRING;
import mapping.CxxMapping.C_TRUE;
import mapping.CxxMapping.C_ULONGLONG;
import mapping.CxxMapping.C_WCHAR;
import mapping.CxxMapping.C_WSTRING;
import mapping.CxxMapping.Empty;
import mapping.CxxMapping.Failure;
import mapping.CxxMapping.Fruit;
import mapping.CxxMapping.Holder;
import mapping.CxxMapping.Keywords;
import mapping.CxxMapping.Later;
import mapping.CxxMapping.LaterPOA;
import mapping.CxxMapping.Multi;
import mapping.CxxMapping.MultiHelper;
import mapping.CxxMapping.MultiPOA;
import mapping.CxxMapping.Node;
import mapping.CxxMapping.Other;
import mapping.CxxMapping.OtherHelper;
import mapping.CxxMapping.Outer;
import mapping.CxxMapping.OuterPackage.Inner;
import mapping.CxxMapping.OuterPackage.Mode;
import mapping.CxxMapping.Pair;
import mapping.CxxMapping.REOPENED;
import mapping.CxxMapping.Tree;
import mapping.CxxMapping.Types;
import mapping.CxxMapping.TypesHelper;
import mapping.CxxMapping.TypesPOA;
import mapping.CxxMapping.WithArray;
import mapping.GLOBAL_LIMIT;
import mapping.GlobalPoint;
import org.junit.jupiter.api.Test;
import org.omg.CORBA.BAD_OPERATION;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.IntHolder;
import org.omg.CORBA.NO_PERMISSION;
import org.omg.CORBA.ORB;

class MappingTest {
    static final class LaterImpl extends LaterPOA {
        private final int n;

        LaterImpl(int n) {
            this.n = n;
        }

        public Later next() {
            return n == 0 ? null : new LaterImpl(n - 1)._this();
        }
    }

    static final class TypesImpl extends TypesPOA {
        volatile String deleted;

        public Tree echo_tree(Tree t) { return t; }
        public Node echo_node(Node n) { return n; }
        public ByBool echo_by_bool(ByBool u) { return u; }
        public ByChar echo_by_char(ByChar u) { return u; }
        public ByEnum echo_by_enum(ByEnum u) { return u; }
        public ByULongLong echo_by_ulonglong(ByULongLong u) { return u; }
        public WithArray echo_with_array(WithArray u) { return u; }
        public Holder echo_holder(Holder h) { return h; }
        public Outer echo_outer(Outer o) { return o; }
        public Keywords echo_keywords(Keywords k) { return k; }
        public GlobalPoint echo_global(GlobalPoint p) { return p; }

        public String echo_short_string(String s) {
            // Longer than the bound: the reply cannot be marshalled.
            return s.equals("grow") ? "much too long" : s;
        }

        public void fail(int code) throws Failure, Empty {
            if (code == 0) {
                throw new Empty();
            }
            if (code < 0) {
                throw new NO_PERMISSION(42, CompletionStatus.COMPLETED_YES);
            }
            throw new Failure(code, "failed", Fruit.CHERRY);
        }

        public Later[] laters(int n) {
            Later[] seq = new Later[n];
            for (int i = 0; i < n; ++i) {
                seq[i] = new LaterImpl(i)._this();
            }
            return seq;
        }

        public void delete(String what) { deleted = what; }
    }

    static final class MultiImpl extends MultiPOA {
        private volatile String label = "start";

        public int id() { return 5; }
        public String label() { return label; }
        public void label(String v) { label = v; }

        public void base_op(boolean fail) throws BaseError {
            if (fail) {
                throw new BaseError("because");
            }
        }

        public int derived_op(int a, IntHolder b, IntHolder c) throws Failure {
            if (a < 0) {
                throw new Failure(a, "negative", Fruit.APPLE);
            }
            c.value = b.value * 2;
            b.value = b.value + a;
            return a + 1;
        }

        public void _notify(String msg) { label = msg; }
        public Base as_base() { return _this(); }
        public String other_op() { return "other"; }
        public BaseInfo multi_op(BaseInfo info) { return new BaseInfo(info.id + 1); }
    }

    @Test
    void constants() {
        assertEquals(10, GLOBAL_LIMIT.value);
        assertEquals(-3, C_SHORT.value);
        // unsigned long long in a Java long
        assertEquals("18000000000000000000", Long.toUnsignedString(C_ULONGLONG.value));
        assertEquals(0.25, C_DOUBLE.value);
        assertTrue(C_TRUE.value);
        assertEquals('q', C_CHAR.value);
        assertEquals('w', C_WCHAR.value);
        assertEquals("abcd", C_STRING.value);
        assertEquals("wide", C_WSTRING.value);
        assertEquals((byte) 255, C_OCTET.value);
        assertEquals((1 << 21) - 3, C_EXPR.value);
        assertSame(Fruit.BANANA, C_ENUM.value);
        assertEquals(7, Base.BASE_CONST);
        assertEquals(1, REOPENED.value);
        assertSame(Fruit.CHERRY, Fruit.from_int(Fruit._CHERRY));
        assertThrows(BAD_PARAM.class, () -> Fruit.from_int(3));
    }

    @Test
    void unions() {
        ByEnum e = new ByEnum();
        assertThrows(BAD_OPERATION.class, e::discriminator);
        e.cherry_name("bing");
        assertSame(Fruit.CHERRY, e.discriminator());
        assertEquals("bing", e.cherry_name());
        assertThrows(BAD_OPERATION.class, e::apple_count);
        e.other(Fruit.BANANA, true);
        assertSame(Fruit.BANANA, e.discriminator());
        assertThrows(BAD_OPERATION.class, () -> e.other(Fruit.APPLE, true));
        ByChar c = new ByChar();
        c.bc('c', "x");
        assertEquals('c', c.discriminator());
        assertThrows(BAD_OPERATION.class, () -> c.bc('a', "x"));
        ByULongLong u = new ByULongLong();
        u.__default();
        assertNotEquals(7L, u.discriminator());
        assertThrows(BAD_OPERATION.class, u::seven);
        assertThrows(BAD_OPERATION.class, () -> u.__default(7L));
    }

    @Test
    void calls() throws Exception {
        try (Orbs orbs = new Orbs()) {
            TypesImpl types = new TypesImpl();
            String typesIor = orbs.server.object_to_string(orbs.serverPoa.servant_to_reference(types));
            String multiIor = orbs.server.object_to_string(
                    orbs.serverPoa.servant_to_reference(new MultiImpl()));
            checkCalls(orbs.client, typesIor, multiIor, types);
            // The same calls on the server ORB: colocated.
            checkCalls(orbs.server, typesIor, multiIor, types);
        }
    }

    static Node leaf(int v) {
        Node n = new Node();
        n.leaf(v);
        return n;
    }

    static Node branch(Node... children) {
        Node n = new Node();
        n.branch(children);
        return n;
    }

    static void assertTree(Tree want, Tree got) {
        assertEquals(want.label, got.label);
        assertEquals(want.children.length, got.children.length);
        for (int i = 0; i < want.children.length; ++i) {
            assertTree(want.children[i], got.children[i]);
        }
    }

    static void assertNode(Node want, Node got) {
        assertEquals(want.discriminator(), got.discriminator());
        if (want.discriminator() == 1) {
            assertEquals(want.leaf(), got.leaf());
        } else {
            assertEquals(want.branch().length, got.branch().length);
            for (int i = 0; i < want.branch().length; ++i) {
                assertNode(want.branch()[i], got.branch()[i]);
            }
        }
    }

    static Holder holder(Types target) {
        ByEnum choice = new ByEnum();
        choice.apple_count(1);
        return new Holder(new int[] {1, -2, 3}, new Pair[] {new Pair(1, 2), new Pair(3, 4)},
                new short[][] {{7, 8, 9}, {10, 11, 12}}, "eight ch", new int[] {1, 2, 3, 4, 5},
                "w\u00e9", new boolean[] {true, false, true}, new char[] {'a', '\u00e9'},
                new String[][] {{"a", "b"}, {}, {"c"}}, target, choice);
    }

    static void checkCalls(ORB orb, String typesIor, String multiIor, TypesImpl servant)
            throws Exception {
        Types types = TypesHelper.narrow(orb.string_to_object(typesIor));
        assertNotNull(types);

        Tree t = new Tree("root", new Tree[] {
            new Tree("a", new Tree[0]), new Tree("b", new Tree[] {new Tree("c", new Tree[0])})});
        assertTree(t, types.echo_tree(t));

        Node n = branch(branch(leaf(3), leaf(3)), leaf(3));
        assertNode(n, types.echo_node(n));

        ByBool bb = new ByBool();
        bb.no("nope");
        ByBool bback = types.echo_by_bool(bb);
        assertFalse(bback.discriminator());
        assertEquals("nope", bback.no());
        bb.yes(9);
        assertEquals(9, types.echo_by_bool(bb).yes());

        ByChar bc = new ByChar();
        bc.a(1);
        assertEquals(1, types.echo_by_char(bc).a());
        bc.bc('b', "bc");
        ByChar bcback = types.echo_by_char(bc);
        assertEquals("bc", bcback.bc());
        assertEquals('b', bcback.discriminator());

        ByEnum be = new ByEnum();
        be.apple_count(4);
        assertEquals(4, types.echo_by_enum(be).apple_count());
        be.other(true);
        ByEnum beback = types.echo_by_enum(be);
        assertSame(Fruit.BANANA, beback.discriminator());
        assertTrue(beback.other());
        be.cherry_name("red");
        assertEquals("red", types.echo_by_enum(be).cherry_name());

        ByULongLong bu = new ByULongLong();
        bu.seven(7.5);
        assertEquals(7.5, types.echo_by_ulonglong(bu).seven());
        bu.__default();
        assertEquals(bu.discriminator(), types.echo_by_ulonglong(bu).discriminator());

        WithArray wa = new WithArray();
        wa.cells(new short[][] {{1, 2, 3}, {4, 5, 6}});
        assertTrue(Arrays.deepEquals(wa.cells(), types.echo_with_array(wa).cells()));
        wa.two(new Pair(1, 2));
        assertEquals(2, types.echo_with_array(wa).two().second);

        Holder h = holder(types);
        Holder back = types.echo_holder(h);
        assertArrayEquals(h.numbers, back.numbers);
        assertEquals(2, back.pairs.length);
        assertEquals(4, back.pairs[1].second);
        assertTrue(Arrays.deepEquals(h.cells, back.cells));
        assertEquals(h.tag, back.tag);
        assertArrayEquals(h.bounded, back.bounded);
        assertEquals(h.wide, back.wide);
        assertArrayEquals(h.flags, back.flags);
        assertArrayEquals(h.wchars, back.wchars);
        assertTrue(Arrays.deepEquals(h.nested, back.nested));
        assertTrue(back.target != null && back.target._is_equivalent(types));
        assertEquals(1, back.choice.apple_count());
        h.target = null;
        assertNull(types.echo_holder(h).target);

        // Bounds are checked when marshalling (BAD_PARAM, as in the other
        // bindings), on either side.
        Holder tooLong = holder(types);
        tooLong.tag = "nine chars";
        assertThrows(BAD_PARAM.class, () -> types.echo_holder(tooLong));
        Holder tooMany = holder(types);
        tooMany.bounded = new int[] {1, 2, 3, 4, 5, 6};
        assertThrows(BAD_PARAM.class, () -> types.echo_holder(tooMany));
        assertEquals("short", types.echo_short_string("short"));
        BAD_PARAM grow = assertThrows(BAD_PARAM.class, () -> types.echo_short_string("grow"));
        assertEquals(CompletionStatus._COMPLETED_YES, grow.completed.value());

        Outer o = types.echo_outer(new Outer(new Inner(3), Mode.OFF));
        assertEquals(3, o.nested.v);
        assertSame(Mode.OFF, o.switch_mode);
        assertEquals("ns", types.echo_keywords(new Keywords(1, "ns")).namespace);
        GlobalPoint gp = types.echo_global(new GlobalPoint(1, 2));
        assertEquals(1, gp.x);
        assertEquals(2, gp.y);

        Failure f = assertThrows(Failure.class, () -> types.fail(3));
        assertEquals(3, f.code);
        assertEquals("failed", f.message);
        assertSame(Fruit.CHERRY, f.kind);
        assertThrows(Empty.class, () -> types.fail(0));
        NO_PERMISSION np = assertThrows(NO_PERMISSION.class, () -> types.fail(-1));
        assertEquals(42, np.minor);
        assertEquals(CompletionStatus._COMPLETED_YES, np.completed.value());

        Later[] laters = types.laters(3);
        assertEquals(3, laters.length);
        assertNotNull(laters[2].next().next());
        assertNull(laters[2].next().next().next());

        types.delete("it");
        assertEquals("it", servant.deleted);

        // Inheritance: narrowing a reference to each base, attributes,
        // inout and out arguments, and oneway.
        org.omg.CORBA.Object obj = orb.string_to_object(multiIor);
        Multi multi = MultiHelper.narrow(obj);
        assertNotNull(multi);
        assertThrows(BAD_PARAM.class, () -> TypesHelper.narrow(obj));
        Base base = BaseHelper.narrow(obj);
        Other other = OtherHelper.narrow(obj);
        assertEquals(5, base.id());
        base.label("changed");
        assertEquals("changed", multi.label());
        assertEquals("other", other.other_op());
        assertThrows(BaseError.class, () -> base.base_op(true));
        base.base_op(false);
        IntHolder b = new IntHolder(10);
        IntHolder c = new IntHolder();
        assertEquals(2, multi.derived_op(1, b, c));
        assertEquals(11, b.value);
        assertEquals(20, c.value);
        assertThrows(Failure.class, () -> multi.derived_op(-1, b, c));
        assertEquals(2, multi.multi_op(new BaseInfo(1)).id);
        assertTrue(multi.as_base()._is_equivalent(multi));
        assertNotNull(MultiHelper.narrow(multi.as_base()));
        multi._notify("posted");
        for (int i = 0; i < 50 && !multi.label().equals("posted"); ++i) {
            Thread.sleep(20);
        }
        assertEquals("posted", multi.label());
        assertTrue(multi._is_a("IDL:rustorb.org/CxxMapping/Other:1.0"));
        assertFalse(multi._is_a("IDL:rustorb.org/CxxMapping/Types:1.0"));
    }
}
