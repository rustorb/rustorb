// Leak checks of the Java binding (issue #49): object references,
// servants, TypeCodes, anys and exceptions made and dropped many times.
// The Cleaner must release every native handle whose owner is gone
// (org.rustorb.Diagnostics.liveHandles), and deactivated servants must be
// collectable. RUSTORB_LEAK_ROUNDS multiplies the number of rounds.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import java.lang.ref.WeakReference;
import java.util.ArrayList;
import java.util.List;

import AnyTest.AnyEcho;
import AnyTest.AnyEchoHelper;
import AnyTest.AnyEchoPOA;
import AnyTest.NameValue;
import InteropTest.Color;
import InteropTest.Echo;
import InteropTest.EchoHelper;
import InteropTest.Oops;
import InteropTest.Point;
import InteropTest.Record;
import interop.EchoImpl;
import org.junit.jupiter.api.Test;
import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.ORB;
import org.omg.CORBA.StructMember;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.UnionMember;
import org.omg.PortableServer.POA;
import org.rustorb.Diagnostics;

class LeakTest {
    private static final int SCALE = rounds();

    private static int rounds() {
        try {
            return Math.max(1, Integer.parseInt(System.getenv("RUSTORB_LEAK_ROUNDS")));
        } catch (NumberFormatException e) {
            return 1;
        }
    }

    static final class AnyEchoImpl extends AnyEchoPOA {
        public Any echo(Any a) { return a; }
        public Any make(String name) { throw new BAD_PARAM(); }
        public boolean check(String name, Any a) { return false; }
        public void out_inout(String name, org.omg.CORBA.AnyHolder b, org.omg.CORBA.AnyHolder c) {
            throw new BAD_PARAM();
        }
        public TypeCode echo_tc(TypeCode tc) { return tc; }
        public TypeCode make_tc(String name) { throw new BAD_PARAM(); }
        public boolean check_tc(String name, TypeCode tc) { return false; }
        public NameValue[] echo_nvlist(NameValue[] l) { return l; }
        public String describe(Any a) { return ""; }
    }

    /** Collects garbage until the handles are back to at most {@code limit}. */
    private static long settle(long limit) throws InterruptedException {
        long live = Diagnostics.liveHandles();
        for (int i = 0; i < 100 && live > limit; i++) {
            System.gc();
            Thread.sleep(50);
            live = Diagnostics.liveHandles();
        }
        return live;
    }

    private static void references(ORB client, String ior, Record rec) {
        Echo echo = EchoHelper.narrow(client.string_to_object(ior));
        assertTrue(echo.echo_ref(echo)._is_equivalent(echo));
        assertEquals(2, echo.echo_records(new Record[] {rec, rec}).length);
        assertEquals(4096, echo.echo_octets(new byte[4096]).length);
        try {
            echo.fail(1, "reason");
            fail("no Oops");
        } catch (Oops e) {
            assertEquals(1, e.code);
        }
        try {
            echo.fail_system();
            fail("no BAD_PARAM");
        } catch (BAD_PARAM e) {
            assertEquals(42, e.minor);
        }
    }

    private static void typecodes(ORB orb, AnyEcho e, int i) {
        String id = "IDL:Leak/Node" + i + ":1.0";
        TypeCode seq = orb.create_sequence_tc(0, orb.create_recursive_tc(id));
        TypeCode node = orb.create_struct_tc(id, "Node", new StructMember[] {
            new StructMember("name", orb.get_primitive_tc(TCKind.tk_string), null),
            new StructMember("children", seq, null),
        });
        assertTrue(e.echo_tc(node).equal(node));
        Any l = orb.create_any();
        l.insert_long(1);
        TypeCode union = orb.create_union_tc("IDL:Leak/U" + i + ":1.0", "U",
                orb.get_primitive_tc(TCKind.tk_long), new UnionMember[] {
                    new UnionMember("l", l, orb.get_primitive_tc(TCKind.tk_long), null),
                });
        assertTrue(e.echo_tc(union).equal(union));
        Any a = orb.create_any();
        a.insert_TypeCode(node);
        Any outer = orb.create_any();
        outer.insert_any(a);
        assertTrue(e.echo(outer).extract_any().extract_TypeCode().equal(node));
        Any s = orb.create_any();
        s.insert_string("value " + i);
        assertEquals("value " + i, e.echo(s).extract_string());
    }

    @Test
    void handlesAreReleasedAndServantsCollected() throws Exception {
        try (Orbs orbs = new Orbs()) {
            POA poa = orbs.serverPoa;
            String ior = orbs.server.object_to_string(
                    poa.servant_to_reference(new EchoImpl(orbs.server)));
            AnyEcho anyEcho = AnyEchoHelper.narrow(orbs.client.string_to_object(
                    orbs.server.object_to_string(poa.servant_to_reference(new AnyEchoImpl()))));
            Record rec = new Record(1, "r", Color.BLUE, new Point(1, 2), new int[] {1, 2, 3}, true);

            // Warm up: the TypeCodes of the Helpers and other caches.
            for (int i = 0; i < 20; i++) {
                references(orbs.client, ior, rec);
                typecodes(orbs.client, anyEcho, i);
            }
            long base = settle(0);

            for (int i = 0; i < 300 * SCALE; i++) {
                references(orbs.client, ior, rec);
                typecodes(orbs.client, anyEcho, i);
            }
            List<WeakReference<EchoImpl>> servants = new ArrayList<>();
            for (int i = 0; i < 100 * SCALE; i++) {
                EchoImpl servant = new EchoImpl(orbs.server);
                servants.add(new WeakReference<>(servant));
                byte[] oid = poa.activate_object(servant);
                Echo echo = EchoHelper.narrow(orbs.client.string_to_object(
                        orbs.server.object_to_string(poa.id_to_reference(oid))));
                assertEquals("x", echo.echo_string("x"));
                poa.deactivate_object(oid);
            }

            // A few handles may still be held by objects the collector has
            // not reached yet; the count must come back to where it was.
            long live = settle(base);
            System.out.println("native handles: " + base + " after warm-up, " + live + " at the end");
            assertTrue(live <= base, "native handles grew from " + base + " to " + live);
            long kept = servants.stream().filter(r -> r.get() != null).count();
            for (int i = 0; i < 100 && kept > 0; i++) {
                System.gc();
                Thread.sleep(50);
                kept = servants.stream().filter(r -> r.get() != null).count();
            }
            assertEquals(0, kept, "deactivated servants still reachable");
        }
    }
}
