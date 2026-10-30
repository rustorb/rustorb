// Valuetypes: the value interop cases (interop/impls/java ValueTests) in one
// process, over IIOP on the loopback interface, then the generated default
// factories, truncation, unknown types and ORB.create_value_tc; then the
// abstract interface cases (AbstractTests) and narrowing.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.Collections;

import AbstractTest.AbstractEcho;
import AbstractTest.AbstractEchoHelper;
import AbstractTest.Named;
import AbstractTest.NamedHelper;
import AbstractTest.Person;
import AbstractTest.PersonHelper;
import ValueTest.LongBox;
import ValueTest.LongBoxHelper;
import ValueTest.Node;
import ValueTest.NodeHelper;
import ValueTest.Pair;
import ValueTest.Point;
import ValueTest.Point3;
import ValueTest.Point3Helper;
import ValueTest.PointHelper;
import ValueTest.ShapeHelper;
import ValueTest.ValueEcho;
import ValueTest.ValueEchoHelper;
import interop.AbstractTests;
import interop.Cases;
import interop.ValueTests;
import org.junit.jupiter.api.Test;
import org.omg.CORBA.MARSHAL;
import org.omg.CORBA.ORB;
import org.omg.CORBA.PRIVATE_MEMBER;
import org.omg.CORBA.PUBLIC_MEMBER;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.VM_NONE;
import org.omg.CORBA.VM_TRUNCATABLE;
import org.omg.CORBA.ValueMember;
import org.omg.CORBA.portable.OutputStream;

class ValuetypeTest {
    /** A Point with a derived type that no factory knows: truncatable to
     * Point, with state of its own. */
    static final class Point4 extends Point {
        private static final long serialVersionUID = 1L;
        private static final String[] IDS = {"IDL:Test/Point4:1.0", PointHelper.id()};

        @Override
        public String[] _truncatable_ids() {
            return IDS;
        }

        @Override
        public void _write(OutputStream out) {
            super._write(out);
            out.write_string("dropped");
        }
    }

    /** A value of a type without a factory nor a truncatable base. */
    static final class Lone extends Point {
        private static final long serialVersionUID = 1L;
        private static final String[] IDS = {"IDL:Test/Lone:1.0"};

        @Override
        public String[] _truncatable_ids() {
            return IDS;
        }
    }

    private static Point point(int x, int y) {
        Point p = new Point();
        p.x = x;
        p.y = y;
        return p;
    }

    /** The ValueEcho object of a new server ORB, for the client ORB. */
    private static ValueEcho echo(Orbs orbs) throws Exception {
        String ior = orbs.server.object_to_string(
                orbs.serverPoa.servant_to_reference(new ValueTests.ValueEchoImpl(orbs.server)));
        return ValueEchoHelper.narrow(orbs.client.string_to_object(ior));
    }

    @Test
    void cases() throws Exception {
        try (Orbs orbs = new Orbs()) {
            ValueTests.registerFactories(orbs.server);
            Cases.Runner r = new Cases.Runner();
            ValueTests.run(r, orbs.client, echo(orbs));
            r.summary();
            assertEquals(Collections.emptyList(), r.failed());
            // The factories are those of the process.
            org.omg.CORBA_2_3.ORB o = (org.omg.CORBA_2_3.ORB) orbs.client;
            for (String id : new String[] {PointHelper.id(), Point3Helper.id(), NodeHelper.id(),
                    ShapeHelper.id()}) {
                o.unregister_value_factory(id);
            }
        }
    }

    @Test
    void defaultFactories() throws Exception {
        try (Orbs orbs = new Orbs()) {
            // Without registered factories, the generated default factories
            // make the generated classes.
            org.omg.CORBA_2_3.ORB o = (org.omg.CORBA_2_3.ORB) orbs.client;
            assertNotNull(o.lookup_value_factory(PointHelper.id()));
            assertNull(o.lookup_value_factory("IDL:nothing:1.0"));
            ValueEcho echo = echo(orbs);
            Node a = new Node();
            a.id = 1;
            a.next = new Node();
            a.next.id = 2;
            a.next.next = a;
            Node got = echo.echo_node(a);
            assertEquals(Node.class, got.getClass());
            assertEquals(2, got.next.id);
            assertSame(got, got.next.next);
            Point q = echo.echo_point(point(3, 4));
            assertEquals(Point.class, q.getClass());
            assertEquals(4, q.y);
            assertEquals(7, echo.echo_long_box(new LongBox(7)).value);
            assertNull(echo.echo_long_box(null));
        }
    }

    @Test
    void truncation() throws Exception {
        try (Orbs orbs = new Orbs()) {
            // The server knows no Point4: it reads its truncatable base.
            Point4 p = new Point4();
            p.x = 1;
            p.y = 2;
            Pair got = echo(orbs).echo_pair(new Pair(p, p));
            assertEquals(Point.class, got.first.getClass());
            assertEquals(1, got.first.x);
            assertEquals(2, got.first.y);
            assertSame(got.first, got.second);
        }
    }

    @Test
    void noFactory() throws Exception {
        try (Orbs orbs = new Orbs()) {
            ValueEcho echo = echo(orbs);
            assertThrows(MARSHAL.class, () -> echo.echo_point(new Lone()));
        }
    }

    @Test
    void createValueTc() throws Exception {
        try (Orbs orbs = new Orbs()) {
            ORB orb = orbs.client;
            TypeCode tlong = orb.get_primitive_tc(TCKind.tk_long);
            ValueMember[] members = {
                new ValueMember("x", "", "", "", tlong, null, PUBLIC_MEMBER.value),
                new ValueMember("y", "", "", "", tlong, null, PUBLIC_MEMBER.value),
            };
            TypeCode tc = ((org.omg.CORBA_2_3.ORB) orb).create_value_tc(
                    PointHelper.id(), "Point", VM_NONE.value, null, members);
            assertTrue(tc.equal(PointHelper.type()));
            TypeCode t3 = Point3Helper.type();
            assertEquals(VM_TRUNCATABLE.value, t3.type_modifier());
            assertTrue(t3.concrete_base_type().equal(PointHelper.type()));
            assertEquals(PRIVATE_MEMBER.value, t3.member_visibility(1));
            assertEquals(TCKind.tk_value_box, LongBoxHelper.type().kind());
            assertFalse(point(1, 2) instanceof Point3);
        }
    }

    @Test
    void abstractInterfaces() throws Exception {
        try (Orbs orbs = new Orbs()) {
            AbstractTests.registerFactories(orbs.server);
            String ior = orbs.server.object_to_string(orbs.serverPoa.servant_to_reference(
                    new AbstractTests.AbstractEchoImpl(orbs.server)));
            AbstractEcho echo = AbstractEchoHelper.narrow(orbs.client.string_to_object(ior));
            Cases.Runner r = new Cases.Runner();
            AbstractTests.run(r, orbs.client, echo);
            r.summary();
            assertEquals(Collections.emptyList(), r.failed());
            // A reference received as Named is a stub of Named, which
            // narrows on to Person; a plain reference narrows to Named.
            Person bob = echo.make_person("bob");
            Named n = echo.echo_named(bob);
            assertTrue(n instanceof org.omg.CORBA.Object);
            assertEquals(42, PersonHelper.narrow((org.omg.CORBA.Object) n).age());
            org.omg.CORBA.Object obj = orbs.client.string_to_object(orbs.client.object_to_string(bob));
            assertEquals("bob", NamedHelper.narrow(obj).name());
            assertThrows(org.omg.CORBA.BAD_PARAM.class, () -> NamedHelper.narrow(echo));
            assertThrows(org.omg.CORBA.BAD_PARAM.class, () -> NamedHelper.narrow("not a Named"));
            assertEquals(TCKind.tk_abstract_interface, NamedHelper.type().kind());
        }
    }
}
