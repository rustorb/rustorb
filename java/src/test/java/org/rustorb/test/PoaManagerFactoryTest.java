// PortableServer::POAManagerFactory in the Java binding (#154):
// create_POAManager, list, find, POAManager.get_id, sharing a manager among
// POAs, and the end of a manager with the last of its POAs.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.List;

import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.omg.CORBA.OBJECT_NOT_EXIST;
import org.omg.CORBA.ORB;
import org.omg.CORBA.Policy;
import org.omg.CORBA.PolicyError;
import org.omg.CORBA.UNSUPPORTED_POLICY;
import org.omg.PortableServer.LifespanPolicyValue;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;
import org.omg.PortableServer.POAManager;
import org.omg.PortableServer.POAManagerFactory;
import org.omg.PortableServer.POAManagerFactoryPackage.ManagerAlreadyExists;
import org.omg.PortableServer.POAManagerPackage.State;

class PoaManagerFactoryTest {
    static ORB orb;
    static POA root;
    static POAManagerFactory factory;

    @BeforeAll
    static void init() throws Exception {
        orb = ORB.init(new String[] {"-ORBendPoint", "giop:tcp:127.0.0.1:"}, null);
        root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        factory = root.the_POAManagerFactory();
    }

    @AfterAll
    static void destroy() {
        orb.destroy();
    }

    static List<String> ids() {
        List<String> ids = new ArrayList<>();
        for (POAManager m : factory.list()) {
            ids.add(m.get_id());
        }
        return ids;
    }

    @Test
    void identities() throws Exception {
        assertEquals("RootPOAManager", root.the_POAManager().get_id());
        assertTrue(ids().contains("RootPOAManager"));

        POA a = root.create_POA("idA", null, new Policy[0]);
        String ida = a.the_POAManager().get_id();
        assertTrue(ids().contains(ida));
        assertTrue(factory.find(ida)._is_equivalent(a.the_POAManager()));

        POAManager x = factory.create_POAManager("j-x", new Policy[0]);
        assertEquals("j-x", x.get_id());
        assertEquals(State.HOLDING, x.get_state());
        assertTrue(factory.find("j-x")._is_equivalent(x));
        assertFalse(factory.find("j-x")._is_equivalent(root.the_POAManager()));
        assertNull(factory.find("j-nope"));
        assertThrows(ManagerAlreadyExists.class,
                () -> factory.create_POAManager("j-x", new Policy[0]));
        assertThrows(ManagerAlreadyExists.class,
                () -> factory.create_POAManager(ida, new Policy[0]));
        assertNotNull(a.the_POAManagerFactory().find("RootPOAManager"));
        a.destroy(false, true);
    }

    @Test
    void policies() {
        Policy p = root.create_lifespan_policy(LifespanPolicyValue.PERSISTENT);
        PolicyError e = assertThrows(PolicyError.class,
                () -> factory.create_POAManager("j-p", new Policy[] {p}));
        assertEquals(UNSUPPORTED_POLICY.value, e.reason);
        assertNull(factory.find("j-p"));
    }

    @Test
    void sharing() throws Exception {
        POAManager m = factory.create_POAManager("j-shared", new Policy[0]);
        POA p1 = root.create_POA("j-p1", m, new Policy[0]);
        POA p2 = p1.create_POA("j-p2", m, new Policy[0]);
        assertTrue(p1.the_POAManager()._is_equivalent(p2.the_POAManager()));
        m.activate();
        assertEquals(State.ACTIVE, p1.the_POAManager().get_state());
        assertEquals(State.ACTIVE, p2.the_POAManager().get_state());
        p2.the_POAManager().discard_requests(false);
        assertEquals(State.DISCARDING, p1.the_POAManager().get_state());

        // The manager ends with the last of its POAs.
        p1.destroy(false, true);
        assertNull(factory.find("j-shared"));
        assertThrows(OBJECT_NOT_EXIST.class, () -> root.create_POA("j-p3", m, new Policy[0]));
        POAManager again = factory.create_POAManager("j-shared", new Policy[0]);
        assertFalse(again._is_equivalent(m));
    }

    @Test
    void children() throws Exception {
        POA parent = root.create_POA("j-parent", null, new Policy[0]);
        parent.create_POA("j-c1", null, new Policy[0]);
        parent.create_POA("j-c2", null, new Policy[0]);
        assertEquals(2, parent.the_children().length);
        parent.destroy(false, true);
    }
}
