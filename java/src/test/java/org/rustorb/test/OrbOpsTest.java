// The ORB operations of #39 in the Java binding: ORB.create_policy,
// register_initial_reference, get_service_information, the policy
// overrides of references, of the ORB (ORBPolicyManager) and of the thread
// (PolicyCurrent), and Object._validate_connection.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.BAD_POLICY;
import org.omg.CORBA.BAD_POLICY_TYPE;
import org.omg.CORBA.NO_IMPLEMENT;
import org.omg.CORBA.NO_PERMISSION;
import org.omg.CORBA.OBJECT_NOT_EXIST;
import org.omg.CORBA.ORB;
import org.omg.CORBA.ORBPackage.InvalidName;
import org.omg.CORBA.Policy;
import org.omg.CORBA.PolicyCurrent;
import org.omg.CORBA.PolicyError;
import org.omg.CORBA.PolicyListHolder;
import org.omg.CORBA.PolicyManager;
import org.omg.CORBA.ServiceInformationHolder;
import org.omg.CORBA.SetOverrideType;
import org.omg.CORBA.TRANSIENT;
import org.omg.Messaging.RELATIVE_RT_TIMEOUT_POLICY_TYPE;
import org.omg.Messaging.RelativeRoundtripTimeoutPolicy;
import org.omg.PortableServer.ID_ASSIGNMENT_POLICY_ID;
import org.omg.PortableServer.IdAssignmentPolicyValue;
import org.omg.PortableServer.IdAssignmentPolicyValueHelper;
import org.omg.PortableServer.LIFESPAN_POLICY_ID;
import org.omg.PortableServer.LifespanPolicy;
import org.omg.PortableServer.LifespanPolicyValue;
import org.omg.PortableServer.LifespanPolicyValueHelper;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;
import org.omg.PortableServer.POAPackage.WrongPolicy;
import org.omg.PortableServer.Servant;
import org.omg.PortableServer.ThreadPolicyValue;
import org.omg.PortableServer.ThreadPolicyValueHelper;

class OrbOpsTest {
    static final int RT = RELATIVE_RT_TIMEOUT_POLICY_TYPE.value;
    static ORB orb;
    static POA root;

    static final class Nothing extends Servant {
        @Override
        public String[] _all_interfaces(POA poa, byte[] objectId) {
            return new String[] {"IDL:test/Nothing:1.0"};
        }
    }

    @BeforeAll
    static void init() throws Exception {
        orb = ORB.init(new String[] {"-ORBInitRef", "Foo=corbaloc::127.0.0.1:1/Foo",
            "-ORBendPoint", "giop:tcp:127.0.0.1:"}, null);
        root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        root.the_POAManager().activate();
    }

    @AfterAll
    static void destroy() {
        orb.destroy();
    }

    static Any lifespan(LifespanPolicyValue v) {
        Any a = orb.create_any();
        LifespanPolicyValueHelper.insert(a, v);
        return a;
    }

    static Any timeT(long t) {
        Any a = orb.create_any();
        a.insert_ulonglong(t);
        return a;
    }

    static short reason(int type, Any val) {
        return assertThrows(PolicyError.class, () -> orb.create_policy(type, val)).reason;
    }

    static long expiry(Policy p) {
        return ((RelativeRoundtripTimeoutPolicy) p).relative_expiry();
    }

    static Policy rt(long t) throws PolicyError {
        return orb.create_policy(RT, timeT(t));
    }

    @Test
    void createPolicy() throws Exception {
        Policy p = orb.create_policy(LIFESPAN_POLICY_ID.value, lifespan(LifespanPolicyValue.PERSISTENT));
        assertSame(LifespanPolicyValue.PERSISTENT, ((LifespanPolicy) p).value());
        assertEquals(17, p.policy_type());
        Policy t = rt(2000000);
        assertEquals(2000000, expiry(t));
        assertEquals(2000000, expiry(t.copy()));

        Any ulong = orb.create_any();
        ulong.insert_ulong(1);
        assertEquals(BAD_POLICY_TYPE.value, reason(LIFESPAN_POLICY_ID.value, ulong));
        Any thread = orb.create_any();
        ThreadPolicyValueHelper.insert(thread, ThreadPolicyValue.ORB_CTRL_MODEL);
        assertEquals(BAD_POLICY_TYPE.value, reason(LIFESPAN_POLICY_ID.value, thread));
        assertEquals(BAD_POLICY_TYPE.value, reason(RT, ulong));
        assertEquals(BAD_POLICY.value, reason(9999, ulong));

        // A policy from create_policy works in create_POA.
        Any userId = orb.create_any();
        IdAssignmentPolicyValueHelper.insert(userId, IdAssignmentPolicyValue.USER_ID);
        assertSame(IdAssignmentPolicyValue.USER_ID, IdAssignmentPolicyValueHelper.extract(userId));
        POA child = root.create_POA("CreatePolicy", null,
                new Policy[] {orb.create_policy(ID_ASSIGNMENT_POLICY_ID.value, userId)});
        assertThrows(WrongPolicy.class, () -> child.activate_object(new Nothing()));
    }

    @Test
    void initialReferences() throws Exception {
        org.omg.CORBA.Object bar = orb.string_to_object("corbaloc::127.0.0.1:2/Bar");
        for (String name : new String[] {"", "Foo", "RootPOA", "PolicyCurrent"}) {
            assertThrows(InvalidName.class, () -> orb.register_initial_reference(name, bar));
        }
        BAD_PARAM nil = assertThrows(BAD_PARAM.class,
                () -> orb.register_initial_reference("Nil", null));
        assertEquals(0x4f4d001b, nil.minor);
        orb.register_initial_reference("Bar", bar);
        assertThrows(InvalidName.class, () -> orb.register_initial_reference("Bar", bar));
        assertTrue(orb.resolve_initial_references("Bar")._is_equivalent(bar));
        // A local object.
        orb.register_initial_reference("Root", root);
        assertSame(root, orb.resolve_initial_references("Root"));
        List<String> names = Arrays.asList(orb.list_initial_services());
        for (String name : new String[] {"Foo", "Bar", "Root", "RootPOA", "POACurrent",
            "ORBPolicyManager", "PolicyCurrent"}) {
            assertTrue(names.contains(name), name);
        }
        ServiceInformationHolder info = new ServiceInformationHolder();
        assertFalse(orb.get_service_information((short) 1, info));
        assertEquals(0, info.value.service_options.length);
    }

    @Test
    void objectOverrides() throws Exception {
        org.omg.CORBA.Object obj = orb.string_to_object("corbaloc::127.0.0.1:1/x");
        assertNull(obj._get_client_policy(RT));
        assertNull(obj._get_policy(LIFESPAN_POLICY_ID.value));
        org.omg.CORBA.Object with = obj._set_policy_override(new Policy[] {rt(30)},
                SetOverrideType.SET_OVERRIDE);
        assertTrue(with._is_equivalent(obj));
        assertEquals(0, obj._get_policy_overrides(new int[0]).length);
        Policy[] list = with._get_policy_overrides(new int[0]);
        assertEquals(1, list.length);
        assertEquals(30, expiry(list[0]));
        assertEquals(0, with._get_policy_overrides(new int[] {LIFESPAN_POLICY_ID.value}).length);
        assertEquals(30, expiry(with._get_policy(RT)));
        assertEquals(30, expiry(with._set_policy_override(new Policy[0],
                SetOverrideType.ADD_OVERRIDE)._get_client_policy(RT)));
        assertNull(with._set_policy_override(new Policy[0], SetOverrideType.SET_OVERRIDE)
                ._get_client_policy(RT));

        BAD_PARAM twice = assertThrows(BAD_PARAM.class, () -> obj._set_policy_override(
                new Policy[] {rt(1), rt(2)}, SetOverrideType.SET_OVERRIDE));
        assertEquals(0x4f4d001e, twice.minor);
        Policy persistent = orb.create_policy(LIFESPAN_POLICY_ID.value, lifespan(LifespanPolicyValue.PERSISTENT));
        assertThrows(NO_PERMISSION.class, () -> obj._set_policy_override(
                new Policy[] {persistent}, SetOverrideType.ADD_OVERRIDE));
    }

    @Test
    void orbAndThreadOverrides() throws Exception {
        org.omg.CORBA.Object obj = orb.string_to_object("corbaloc::127.0.0.1:1/x");
        PolicyManager manager = (PolicyManager) orb.resolve_initial_references("ORBPolicyManager");
        PolicyCurrent current = (PolicyCurrent) orb.resolve_initial_references("PolicyCurrent");
        try {
            manager.set_policy_overrides(new Policy[] {rt(50)}, SetOverrideType.SET_OVERRIDE);
            assertEquals(50, expiry(obj._get_client_policy(RT)));
            current.set_policy_overrides(new Policy[] {rt(40)}, SetOverrideType.ADD_OVERRIDE);
            assertEquals(40, expiry(obj._get_client_policy(RT)));
            org.omg.CORBA.Object with = obj._set_policy_override(new Policy[] {rt(30)},
                    SetOverrideType.SET_OVERRIDE);
            assertEquals(30, expiry(with._get_client_policy(RT)));
            List<Long> seen = new ArrayList<>();
            Thread t = new Thread(() -> {
                seen.add((long) current.get_policy_overrides(new int[0]).length);
                seen.add(expiry(obj._get_client_policy(RT)));
            });
            t.start();
            t.join();
            assertEquals(Arrays.asList(0L, 50L), seen);
            Policy[] orbs = manager.get_policy_overrides(new int[] {RT});
            assertEquals(1, orbs.length);
            assertEquals(50, expiry(orbs[0]));
        } finally {
            current.set_policy_overrides(new Policy[0], SetOverrideType.SET_OVERRIDE);
            manager.set_policy_overrides(new Policy[0], SetOverrideType.SET_OVERRIDE);
        }
        assertArrayEquals(new Policy[0], current.get_policy_overrides(new int[0]));
        assertArrayEquals(new Policy[0], manager.get_policy_overrides(new int[0]));
        assertNull(obj._get_client_policy(RT));
    }

    @Test
    void validateConnection() throws Exception {
        org.omg.CORBA.Object obj = root.servant_to_reference(new Nothing());
        PolicyListHolder inconsistent = new PolicyListHolder();
        assertTrue(obj._validate_connection(inconsistent));
        assertEquals(0, inconsistent.value.length);
        org.omg.CORBA.Object missing = root.create_reference("IDL:test/Nothing:1.0");
        assertThrows(OBJECT_NOT_EXIST.class, () -> missing._validate_connection(inconsistent));
        org.omg.CORBA.Object dead = orb.string_to_object("corbaloc::127.0.0.1:1/x");
        assertThrows(TRANSIENT.class, () -> dead._validate_connection(inconsistent));
        assertThrows(NO_IMPLEMENT.class, () -> root._validate_connection(inconsistent));
    }
}
