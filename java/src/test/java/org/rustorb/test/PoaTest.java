// Servant managers (ServantActivator, ServantLocator), adapter activators
// and default servants of the Java binding, called over GIOP from a second
// ORB in the same process, and colocated.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import mapping.CxxMapping.Base;
import mapping.CxxMapping.BasePackage.BaseInfo;
import mapping.CxxMapping.Multi;
import mapping.CxxMapping.MultiHelper;
import mapping.CxxMapping.MultiPOA;
import org.junit.jupiter.api.Test;
import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.IntHolder;
import org.omg.CORBA.LocalObject;
import org.omg.CORBA.OBJECT_NOT_EXIST;
import org.omg.CORBA.OBJ_ADAPTER;
import org.omg.CORBA.ORB;
import org.omg.CORBA.Policy;
import org.omg.CORBA.UNKNOWN;
import org.omg.PortableServer.AdapterActivator;
import org.omg.PortableServer.ForwardRequest;
import org.omg.PortableServer.IdAssignmentPolicyValue;
import org.omg.PortableServer.IdUniquenessPolicyValue;
import org.omg.PortableServer.LifespanPolicyValue;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAPackage.AdapterNonExistent;
import org.omg.PortableServer.POAPackage.WrongPolicy;
import org.omg.PortableServer.RequestProcessingPolicyValue;
import org.omg.PortableServer.Servant;
import org.omg.PortableServer.ServantActivator;
import org.omg.PortableServer.ServantLocator;
import org.omg.PortableServer.ServantLocatorPackage.CookieHolder;
import org.omg.PortableServer.ServantRetentionPolicyValue;

class PoaTest {
    static final String MULTI_ID = "IDL:rustorb.org/CxxMapping/Multi:1.0";

    /** What the servant managers saw, in order. */
    static final class Log {
        private final List<String> events = new ArrayList<>();

        synchronized void add(String s) {
            events.add(s);
        }

        synchronized List<String> take() {
            List<String> r = new ArrayList<>(events);
            events.clear();
            return r;
        }

        /** Waits for an event that arrives after the reply (etherealize). */
        boolean waitFor(String s) throws InterruptedException {
            for (int i = 0; i < 100; i++) {
                synchronized (this) {
                    if (events.remove(s)) {
                        return true;
                    }
                }
                Thread.sleep(20);
            }
            return false;
        }
    }

    final Log events = new Log();

    static final class MultiImpl extends MultiPOA {
        private volatile String label;

        MultiImpl(String label) {
            this.label = label;
        }

        public int id() { return 5; }
        public String label() { return label; }
        public void label(String v) { label = v; }
        public void base_op(boolean fail) {}
        public int derived_op(int a, IntHolder b, IntHolder c) { return a; }
        public void _notify(String msg) {}
        public Base as_base() { return _this(); }
        public String other_op() { return "other"; }
        public BaseInfo multi_op(BaseInfo info) { return info; }
    }

    static String str(byte[] oid) {
        return new String(oid, StandardCharsets.ISO_8859_1);
    }

    static byte[] oid(String s) {
        return s.getBytes(StandardCharsets.ISO_8859_1);
    }

    static String labelOf(Servant s) {
        return s instanceof MultiImpl ? ((MultiImpl) s).label() : "?";
    }

    /** A servant per object id, made when it is first called. */
    final class Activator extends LocalObject implements ServantActivator {
        private final org.omg.CORBA.Object forward;

        Activator(org.omg.CORBA.Object forward) {
            this.forward = forward;
        }

        public Servant incarnate(byte[] oid, POA adapter) throws ForwardRequest {
            String name = str(oid);
            events.add("incarnate " + name + " in " + adapter.the_name());
            switch (name) {
                case "bad":
                    throw new OBJECT_NOT_EXIST(7, CompletionStatus.COMPLETED_NO);
                case "fwd":
                    throw new ForwardRequest(forward);
                case "nil":
                    return null;
                case "throw":
                    throw new IllegalStateException("incarnate failed");
                default:
                    return new MultiImpl(name);
            }
        }

        public void etherealize(byte[] oid, POA adapter, Servant serv, boolean cleanup,
                boolean remaining) {
            events.add("etherealize " + str(oid) + " " + labelOf(serv) + (cleanup ? " cleanup" : "")
                    + (remaining ? " remaining" : ""));
        }
    }

    /** A servant for every request; the cookie counts the calls. */
    final class Locator extends LocalObject implements ServantLocator {
        private final org.omg.CORBA.Object forward;
        private int calls;

        Locator(org.omg.CORBA.Object forward) {
            this.forward = forward;
        }

        public Servant preinvoke(byte[] oid, POA adapter, String operation, CookieHolder cookie)
                throws ForwardRequest {
            String name = str(oid);
            events.add("preinvoke " + name + " " + operation);
            if (name.equals("bad")) {
                throw new OBJECT_NOT_EXIST(8, CompletionStatus.COMPLETED_NO);
            }
            if (name.equals("fwd")) {
                throw new ForwardRequest(forward);
            }
            synchronized (this) {
                cookie.value = ++calls;
            }
            return new MultiImpl(name);
        }

        public void postinvoke(byte[] oid, POA adapter, String operation, Object cookie,
                Servant servant) {
            events.add("postinvoke " + str(oid) + " " + operation + " " + labelOf(servant)
                    + " cookie " + cookie);
        }
    }

    /** Creates POAs named lazy* on demand, with a default servant. */
    final class LazyActivator extends LocalObject implements AdapterActivator {
        public boolean unknown_adapter(POA parent, String name) {
            events.add("unknown_adapter " + name);
            if (!name.startsWith("lazy")) {
                return false;
            }
            try {
                POA poa = parent.create_POA(name, parent.the_POAManager(), new Policy[] {
                    parent.create_lifespan_policy(LifespanPolicyValue.PERSISTENT),
                    parent.create_id_assignment_policy(IdAssignmentPolicyValue.USER_ID),
                    parent.create_servant_retention_policy(ServantRetentionPolicyValue.NON_RETAIN),
                    parent.create_request_processing_policy(
                            RequestProcessingPolicyValue.USE_DEFAULT_SERVANT),
                    parent.create_id_uniqueness_policy(IdUniquenessPolicyValue.MULTIPLE_ID),
                });
                poa.set_servant(new MultiImpl("default of " + name));
                return true;
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        }
    }

    static Policy[] policies(POA root, Policy... extra) {
        Policy[] p = Arrays.copyOf(new Policy[] {
            root.create_lifespan_policy(LifespanPolicyValue.PERSISTENT),
            root.create_id_assignment_policy(IdAssignmentPolicyValue.USER_ID),
        }, 2 + extra.length);
        System.arraycopy(extra, 0, p, 2, extra.length);
        return p;
    }

    /** A reference for client to id in poa; unchecked_narrow, as narrow
     * would call _is_a. */
    static Multi remote(ORB client, ORB server, POA poa, String id) throws Exception {
        org.omg.CORBA.Object ref = poa.create_reference_with_id(oid(id), MULTI_ID);
        return MultiHelper.unchecked_narrow(client.string_to_object(server.object_to_string(ref)));
    }

    void checkLocator(ORB client, ORB server, POA root, org.omg.CORBA.Object target)
            throws Exception {
        POA poa = root.create_POA("locator", root.the_POAManager(), policies(root,
                root.create_request_processing_policy(
                        RequestProcessingPolicyValue.USE_SERVANT_MANAGER),
                root.create_servant_retention_policy(ServantRetentionPolicyValue.NON_RETAIN)));
        assertThrows(OBJ_ADAPTER.class, () -> poa.set_servant_manager(new Activator(target)));
        Locator locator = new Locator(target);
        poa.set_servant_manager(locator);
        assertSame(locator, poa.get_servant_manager());

        Multi x = remote(client, server, poa, "x");
        assertEquals("x", x.label());
        assertEquals("other", x.other_op());
        assertEquals(Arrays.asList("preinvoke x _get_label", "postinvoke x _get_label x cookie 1",
                "preinvoke x other_op", "postinvoke x other_op x cookie 2"), events.take());
        assertEquals("target", remote(client, server, poa, "fwd").label());
        OBJECT_NOT_EXIST e = assertThrows(OBJECT_NOT_EXIST.class,
                () -> remote(client, server, poa, "bad").label());
        assertEquals(8, e.minor);
        // postinvoke is not called when preinvoke failed.
        assertEquals(Arrays.asList("preinvoke fwd _get_label", "preinvoke bad _get_label"),
                events.take());
    }

    @Test
    void servantManagersAndAdapterActivators() throws Exception {
        try (Orbs orbs = new Orbs()) {
            ORB server = orbs.server;
            ORB client = orbs.client;
            POA root = orbs.serverPoa;
            org.omg.CORBA.Object target = root.servant_to_reference(new MultiImpl("target"));

            // ServantActivator.
            POA poa = root.create_POA("activator", root.the_POAManager(), policies(root,
                    root.create_request_processing_policy(
                            RequestProcessingPolicyValue.USE_SERVANT_MANAGER)));
            assertNull(poa.get_servant_manager());
            Activator activator = new Activator(target);
            // A locator does not fit a RETAIN POA.
            assertThrows(OBJ_ADAPTER.class, () -> poa.set_servant_manager(new Locator(target)));
            poa.set_servant_manager(activator);
            assertSame(activator, poa.get_servant_manager());
            assertThrows(BAD_INV_ORDER.class, () -> poa.set_servant_manager(activator));
            assertThrows(WrongPolicy.class, () -> root.get_servant_manager());
            assertThrows(WrongPolicy.class, () -> root.set_servant_manager(activator));

            Multi a = remote(client, server, poa, "a");
            assertEquals("a", a.label());
            a.label("A");
            assertEquals("A", a.label());
            assertEquals(Arrays.asList("incarnate a in activator"), events.take());
            assertEquals("A", labelOf(poa.id_to_servant(oid("a"))));

            assertEquals("target", remote(client, server, poa, "fwd").label());
            OBJECT_NOT_EXIST e = assertThrows(OBJECT_NOT_EXIST.class,
                    () -> remote(client, server, poa, "bad").label());
            assertEquals(7, e.minor);
            assertThrows(OBJ_ADAPTER.class, () -> remote(client, server, poa, "nil").label());
            assertThrows(UNKNOWN.class, () -> remote(client, server, poa, "throw").label());
            events.take();

            poa.deactivate_object(oid("a"));
            assertTrue(events.waitFor("etherealize a A"));
            assertEquals("a", a.label());
            assertEquals(Arrays.asList("incarnate a in activator"), events.take());
            poa.destroy(true, true);
            assertTrue(events.waitFor("etherealize a a cleanup"));

            // ServantLocator.
            checkLocator(client, server, root, target);

            // AdapterActivator.
            assertNull(root.the_activator());
            LazyActivator lazy = new LazyActivator();
            root.the_activator(lazy);
            assertSame(lazy, root.the_activator());
            POA lazy1 = root.find_POA("lazy1", true);
            assertEquals("lazy1", lazy1.the_name());
            assertThrows(AdapterNonExistent.class, () -> root.find_POA("nope", true));
            assertThrows(AdapterNonExistent.class, () -> root.find_POA("lazy9", false));
            assertEquals(Arrays.asList("unknown_adapter lazy1", "unknown_adapter nope"),
                    events.take());

            // A request for a POA that does not exist (any more) makes it
            // again.
            POA made = root.find_POA("lazy2", true);
            Multi obj = remote(client, server, made, "o");
            made.destroy(false, true);
            events.take();
            assertEquals("default of lazy2", obj.label());
            assertEquals(Arrays.asList("unknown_adapter lazy2"), events.take());
            assertNotNull(lazy1.get_servant());

            root.the_activator(null);
            assertNull(root.the_activator());
            assertThrows(AdapterNonExistent.class, () -> root.find_POA("lazy3", true));
            assertTrue(events.take().isEmpty());

            // The same through colocated calls.
            checkLocator(server, server, lazy1, target);
        }
    }
}
