// Bidirectional GIOP (docs/bidir-giop.md) in the Java binding: the
// org.omg.BiDirPolicy module, ORB.create_policy, POAs with the
// BidirectionalPolicy, the ORBPolicyManager, and a callback over the
// connection the client opened.
//
// Two ORBs in this process, a server that accepts bidirectional GIOP and a
// client that offers it. The client's IORs name an address nobody listens
// on, so the callback works only over the client's own connection.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import InteropTest.Callback;
import InteropTest.CallbackHelper;
import InteropTest.CallbackPOA;
import InteropTest.Echo;
import InteropTest.EchoHelper;
import interop.EchoImpl;
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.omg.BiDirPolicy.BIDIRECTIONAL_POLICY_TYPE;
import org.omg.BiDirPolicy.BOTH;
import org.omg.BiDirPolicy.BidirectionalPolicy;
import org.omg.BiDirPolicy.BidirectionalPolicyValueHelper;
import org.omg.BiDirPolicy.NORMAL;
import org.omg.CORBA.Any;
import org.omg.CORBA.NO_PERMISSION;
import org.omg.CORBA.ORB;
import org.omg.CORBA.Policy;
import org.omg.CORBA.PolicyCurrent;
import org.omg.CORBA.PolicyError;
import org.omg.CORBA.PolicyManager;
import org.omg.CORBA.SetOverrideType;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;

class BiDirTest {
    static final int TYPE = BIDIRECTIONAL_POLICY_TYPE.value;
    static ORB server;
    static ORB client;
    static Thread thread;
    static Echo echo;
    static POA clientRoot;

    static final class CallbackImpl extends CallbackPOA {
        @Override
        public String _notify(String message) {
            return "ack:" + message;
        }
    }

    @BeforeAll
    static void init() throws Exception {
        server = ORB.init(new String[] {"-ORBendPoint", "giop:tcp:127.0.0.1:",
            "-ORBacceptBiDirectionalGIOP", "1"}, null);
        POA serverRoot = POAHelper.narrow(server.resolve_initial_references("RootPOA"));
        serverRoot.the_POAManager().activate();
        thread = new Thread(server::run, "server ORB");
        thread.start();
        client = ORB.init(new String[] {"-ORBendPoint", "giop:tcp:127.0.0.1:",
            "-ORBendPointPublish", "giop:tcp:127.0.0.1:1", "-ORBofferBiDirectionalGIOP", "1"},
                null);
        clientRoot = POAHelper.narrow(client.resolve_initial_references("RootPOA"));
        clientRoot.the_POAManager().activate();
        String ior = server.object_to_string(
                serverRoot.servant_to_reference(new EchoImpl(server)));
        echo = EchoHelper.narrow(client.string_to_object(ior));
    }

    @AfterAll
    static void destroy() throws Exception {
        client.destroy();
        server.shutdown(true);
        thread.join();
        server.destroy();
    }

    static Policy bidir(short value) throws PolicyError {
        Any a = client.create_any();
        BidirectionalPolicyValueHelper.insert(a, value);
        return client.create_policy(TYPE, a);
    }

    static Callback callback(String poaName, short value) throws Exception {
        POA poa = clientRoot.create_POA(poaName, clientRoot.the_POAManager(),
                new Policy[] {bidir(value)});
        return CallbackHelper.narrow(
                poa.id_to_reference(poa.activate_object(new CallbackImpl())));
    }

    @Test
    void createPolicy() throws Exception {
        Policy p = bidir(BOTH.value);
        assertEquals(37, p.policy_type());
        assertEquals(BOTH.value, ((BidirectionalPolicy) p).value());
        assertEquals(BOTH.value, ((BidirectionalPolicy) p.copy()).value());
        assertEquals(NORMAL.value, ((BidirectionalPolicy) bidir(NORMAL.value)).value());
        assertThrows(PolicyError.class, () -> bidir((short) 2));
        Any ulong = client.create_any();
        ulong.insert_ulong(1);
        assertThrows(PolicyError.class, () -> client.create_policy(TYPE, ulong));
        Any any = client.create_any();
        BidirectionalPolicyValueHelper.insert(any, BOTH.value);
        assertEquals(BOTH.value, BidirectionalPolicyValueHelper.extract(any));
    }

    @Test
    void callBackOverTheClientsConnection() throws Exception {
        Callback cb = callback("bidir", BOTH.value);
        assertEquals("ack:hi", echo.call_back(cb, "hi"));
        assertEquals("ack:again", echo.call_back(cb, "again"));
    }

    @Test
    void poaWithoutThePolicyRefuses() throws Exception {
        Callback cb = callback("normal", NORMAL.value);
        assertThrows(NO_PERMISSION.class, () -> echo.call_back(cb, "hi"));
    }

    @Test
    void policyManagerAndCurrent() throws Exception {
        PolicyManager manager =
                (PolicyManager) client.resolve_initial_references("ORBPolicyManager");
        try {
            manager.set_policy_overrides(new Policy[] {bidir(BOTH.value)},
                    SetOverrideType.ADD_OVERRIDE);
            Policy[] got = manager.get_policy_overrides(new int[] {TYPE});
            assertEquals(1, got.length);
            assertTrue(got[0] instanceof BidirectionalPolicy);
            assertEquals(BOTH.value, ((BidirectionalPolicy) got[0]).value());
        } finally {
            manager.set_policy_overrides(new Policy[0], SetOverrideType.SET_OVERRIDE);
        }
        assertEquals(0, manager.get_policy_overrides(new int[] {TYPE}).length);
        // Only the ORB can hold it.
        PolicyCurrent current = (PolicyCurrent) client.resolve_initial_references("PolicyCurrent");
        Policy both = bidir(BOTH.value);
        assertThrows(NO_PERMISSION.class, () -> current.set_policy_overrides(
                new Policy[] {both}, SetOverrideType.ADD_OVERRIDE));
    }
}
