// Messaging::SyncScopePolicy in the Java binding (#153): ORB.create_policy,
// the overrides of references, of the ORB and of the thread, and the answer
// of the server to a oneway request (docs/sync-scope.md).

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_POLICY_TYPE;
import org.omg.CORBA.BAD_POLICY_VALUE;
import org.omg.CORBA.OBJECT_NOT_EXIST;
import org.omg.CORBA.ORB;
import org.omg.CORBA.Policy;
import org.omg.CORBA.PolicyCurrent;
import org.omg.CORBA.PolicyError;
import org.omg.CORBA.PolicyManager;
import org.omg.CORBA.SetOverrideType;
import org.omg.CORBA.portable.ObjectImpl;
import org.omg.Messaging.SYNC_NONE;
import org.omg.Messaging.SYNC_SCOPE_POLICY_TYPE;
import org.omg.Messaging.SYNC_WITH_SERVER;
import org.omg.Messaging.SYNC_WITH_TARGET;
import org.omg.Messaging.SYNC_WITH_TRANSPORT;
import org.omg.Messaging.SyncScopeHelper;
import org.omg.Messaging.SyncScopePolicy;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;

class SyncScopeTest {
    static final int SYNC = SYNC_SCOPE_POLICY_TYPE.value;
    static ORB orb;
    static POA root;

    @BeforeAll
    static void init() throws Exception {
        orb = ORB.init(new String[] {"-ORBendPoint", "giop:tcp:127.0.0.1:"}, null);
        root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        root.the_POAManager().activate();
    }

    @AfterAll
    static void destroy() {
        orb.destroy();
    }

    static Any scope(short v) {
        Any a = orb.create_any();
        SyncScopeHelper.insert(a, v);
        return a;
    }

    static Policy sync(short v) throws PolicyError {
        return orb.create_policy(SYNC, scope(v));
    }

    static short synchronization(Policy p) {
        return ((SyncScopePolicy) p).synchronization();
    }

    static short reason(Any val) {
        return assertThrows(PolicyError.class, () -> orb.create_policy(SYNC, val)).reason;
    }

    /** A oneway call of "op" through the portable stub API. */
    static void oneway(org.omg.CORBA.Object obj) {
        ObjectImpl stub = (ObjectImpl) obj;
        try {
            stub._invoke(stub._request("op", false));
        } catch (org.omg.CORBA.portable.ApplicationException
                | org.omg.CORBA.portable.RemarshalException e) {
            throw new AssertionError(e);
        }
    }

    @Test
    void createPolicy() throws Exception {
        for (short v = 0; v < 4; v++) {
            Policy p = sync(v);
            assertEquals(SYNC, p.policy_type());
            assertEquals(v, synchronization(p));
            assertEquals(v, synchronization(p.copy()));
        }
        Any plain = orb.create_any();
        plain.insert_short(SYNC_WITH_SERVER.value);
        assertEquals(SYNC_WITH_SERVER.value, synchronization(orb.create_policy(SYNC, plain)));
        assertEquals(BAD_POLICY_VALUE.value, reason(scope((short) 4)));
        assertEquals(BAD_POLICY_VALUE.value, reason(scope((short) -1)));
        Any ulong = orb.create_any();
        ulong.insert_ulong(1);
        assertEquals(BAD_POLICY_TYPE.value, reason(ulong));
    }

    @Test
    void overridesAndTheServersAnswer() throws Exception {
        org.omg.CORBA.Object missing = root.create_reference("IDL:test/Nothing:1.0");
        assertNull(missing._get_client_policy(SYNC));
        org.omg.CORBA.Object server = missing._set_policy_override(
                new Policy[] {sync(SYNC_WITH_SERVER.value)}, SetOverrideType.SET_OVERRIDE);
        assertEquals(SYNC_WITH_SERVER.value, synchronization(server._get_client_policy(SYNC)));
        Policy[] list = server._get_policy_overrides(new int[] {SYNC});
        assertEquals(1, list.length);
        assertEquals(SYNC_WITH_SERVER.value, synchronization(list[0]));

        // The server's answer to SYNC_WITH_SERVER and SYNC_WITH_TARGET can be
        // an exception; without one nothing comes back.
        assertThrows(OBJECT_NOT_EXIST.class, () -> oneway(server));
        assertThrows(OBJECT_NOT_EXIST.class, () -> oneway(missing._set_policy_override(
                new Policy[] {sync(SYNC_WITH_TARGET.value)}, SetOverrideType.SET_OVERRIDE)));
        oneway(missing._set_policy_override(new Policy[] {sync(SYNC_WITH_TRANSPORT.value)},
                SetOverrideType.SET_OVERRIDE));
        oneway(missing);

        PolicyManager manager = (PolicyManager) orb.resolve_initial_references("ORBPolicyManager");
        PolicyCurrent current = (PolicyCurrent) orb.resolve_initial_references("PolicyCurrent");
        try {
            manager.set_policy_overrides(new Policy[] {sync(SYNC_WITH_TARGET.value)},
                    SetOverrideType.ADD_OVERRIDE);
            assertThrows(OBJECT_NOT_EXIST.class, () -> oneway(missing));
            current.set_policy_overrides(new Policy[] {sync(SYNC_NONE.value)},
                    SetOverrideType.ADD_OVERRIDE);
            assertEquals(SYNC_NONE.value, synchronization(missing._get_client_policy(SYNC)));
            oneway(missing);
        } finally {
            current.set_policy_overrides(new Policy[0], SetOverrideType.SET_OVERRIDE);
            manager.set_policy_overrides(new Policy[0], SetOverrideType.SET_OVERRIDE);
        }
        assertNull(missing._get_client_policy(SYNC));
    }
}
