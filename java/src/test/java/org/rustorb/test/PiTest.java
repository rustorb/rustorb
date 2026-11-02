// Portable Interceptors and IOP::Codec in the Java binding (#100): ORB
// initializers named by ORB.init properties, client, server and IOR
// interceptors on a server ORB and a client ORB in one process, service
// contexts both ways, slots from the client thread to the servant,
// ForwardRequest, exceptions raised by interceptors, info objects kept past
// their point, and the Codec.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Properties;

import InteropTest.Echo;
import InteropTest.EchoHelper;
import InteropTest.Oops;
import interop.EchoImpl;
import org.junit.jupiter.api.Test;
import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.LocalObject;
import org.omg.CORBA.NO_PERMISSION;
import org.omg.CORBA.NO_RESOURCES;
import org.omg.CORBA.OBJECT_NOT_EXIST;
import org.omg.CORBA.ORB;
import org.omg.CORBA.TCKind;
import org.omg.IOP.Codec;
import org.omg.IOP.CodecFactory;
import org.omg.IOP.CodecFactoryHelper;
import org.omg.IOP.CodecFactoryPackage.UnknownEncoding;
import org.omg.IOP.ENCODING_CDR_ENCAPS;
import org.omg.IOP.Encoding;
import org.omg.IOP.ServiceContext;
import org.omg.IOP.TaggedComponent;
import org.omg.PortableInterceptor.ClientRequestInfo;
import org.omg.PortableInterceptor.ClientRequestInterceptor;
import org.omg.PortableInterceptor.Current;
import org.omg.PortableInterceptor.CurrentHelper;
import org.omg.PortableInterceptor.ForwardRequest;
import org.omg.PortableInterceptor.IORInfo;
import org.omg.PortableInterceptor.IORInterceptor;
import org.omg.PortableInterceptor.InvalidSlot;
import org.omg.PortableInterceptor.LOCATION_FORWARD;
import org.omg.PortableInterceptor.ORBInitInfo;
import org.omg.PortableInterceptor.ORBInitInfoPackage.DuplicateName;
import org.omg.PortableInterceptor.ORBInitInfoPackage.InvalidName;
import org.omg.PortableInterceptor.ORBInitializer;
import org.omg.PortableInterceptor.SUCCESSFUL;
import org.omg.PortableInterceptor.ServerRequestInfo;
import org.omg.PortableInterceptor.ServerRequestInterceptor;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;

public class PiTest {
    static final int REQUEST_CONTEXT = 0x52520010;
    static final int REPLY_CONTEXT = 0x52520011;
    static final int COMPONENT = 0x52520001;
    static final String ECHO_ID = "IDL:rustorb.org/InteropTest/Echo:1.0";
    static final int OMG = 0x4f4d0000;

    /** What the interceptors saw. */
    static final class Seen {
        int slot;
        final List<String> preInitIds = new ArrayList<>();
        final List<String> postInitIds = new ArrayList<>();
        String[] arguments;
        int sendRequest;
        int receiveOther;
        int receiveException;
        String replyContext;
        String receivedExceptionId;
        String sendingExceptionId;
        boolean componentOk;
        boolean argumentsUnavailable;
        boolean serverOrbIdOk;
        boolean targetIsA;
        boolean adapterNameOk;
        boolean duplicateName;
        boolean preInitResolveBadOrder;
        boolean postInitResolveOk;
        boolean postInitInvalidName;
        boolean piCurrentOk;
        boolean poaCurrentOk;
        boolean codecFactoryOk;
        ClientRequestInfo keptRi;
        ORBInitInfo keptInfo;
        org.omg.CORBA.Object forwardFrom;
        org.omg.CORBA.Object forwardTo;
        org.omg.CORBA.Object serverForward;
        byte[] serverForwardOid;
    }

    static final Seen SEEN = new Seen();

    static byte[] octets(String s) {
        return s.getBytes(StandardCharsets.UTF_8);
    }

    static String text(byte[] b) {
        return new String(b, StandardCharsets.UTF_8);
    }

    static class Named extends LocalObject {
        private final String name;

        Named(String name) {
            this.name = name;
        }

        public String name() {
            return name;
        }

        public void destroy() {}
    }

    static final class Client extends Named implements ClientRequestInterceptor {
        Client() {
            super("ctx");
        }

        @Override
        public void send_request(ClientRequestInfo ri) throws ForwardRequest {
            try {
                sendRequest(ri);
            } catch (InvalidSlot e) {
                throw new AssertionError(e);
            }
        }

        private void sendRequest(ClientRequestInfo ri) throws ForwardRequest, InvalidSlot {
            synchronized (SEEN) {
                SEEN.sendRequest++;
                String op = ri.operation();
                if (op.equals("echo_string")) {
                    Any a = ri.get_slot(SEEN.slot);
                    if (a.type().kind() == TCKind.tk_string) {
                        ri.add_request_service_context(
                                new ServiceContext(REQUEST_CONTEXT, octets(a.extract_string())),
                                false);
                    }
                    SEEN.componentOk = Arrays.equals(new byte[] {1, 2, 3},
                            ri.get_effective_component(COMPONENT).component_data);
                    try {
                        ri.arguments();
                    } catch (NO_RESOURCES e) {
                        SEEN.argumentsUnavailable = true;
                    }
                    SEEN.keptRi = ri;
                }
                if (op.equals("echo_short") && SEEN.forwardTo != null
                        && ri.effective_target()._is_equivalent(SEEN.forwardFrom)) {
                    throw new ForwardRequest(SEEN.forwardTo);
                }
            }
        }

        @Override
        public void send_poll(ClientRequestInfo ri) {}

        @Override
        public void receive_reply(ClientRequestInfo ri) {
            synchronized (SEEN) {
                if (ri.operation().equals("echo_string") && ri.reply_status() == SUCCESSFUL.value) {
                    SEEN.replyContext = text(ri.get_reply_service_context(REPLY_CONTEXT).context_data);
                }
            }
        }

        @Override
        public void receive_exception(ClientRequestInfo ri) {
            synchronized (SEEN) {
                SEEN.receiveException++;
                SEEN.receivedExceptionId = ri.received_exception_id();
            }
        }

        @Override
        public void receive_other(ClientRequestInfo ri) {
            synchronized (SEEN) {
                if (ri.reply_status() == LOCATION_FORWARD.value) {
                    SEEN.receiveOther++;
                }
            }
        }
    }

    static final class Server extends Named implements ServerRequestInterceptor {
        private final ORB orbForAnys = ORB.init();

        Server() {
            super("ctx");
        }

        @Override
        public void receive_request_service_contexts(ServerRequestInfo ri) {
            if (!ri.operation().equals("echo_string")) {
                return;
            }
            Any a = orbForAnys.create_any();
            a.insert_string(text(ri.get_request_service_context(REQUEST_CONTEXT).context_data));
            try {
                ri.set_slot(SEEN.slot, a);
            } catch (InvalidSlot e) {
                throw new AssertionError(e);
            }
        }

        @Override
        public void receive_request(ServerRequestInfo ri) throws ForwardRequest {
            String op = ri.operation();
            if (op.equals("echo_ulong")) {
                throw new NO_PERMISSION(7, org.omg.CORBA.CompletionStatus.COMPLETED_NO);
            }
            if (op.equals("echo_octet") && Arrays.equals(ri.object_id(), SEEN.serverForwardOid)) {
                throw new ForwardRequest(SEEN.serverForward);
            }
            if (op.equals("echo_string")) {
                synchronized (SEEN) {
                    SEEN.serverOrbIdOk = ri.orb_id().equals("server");
                    SEEN.targetIsA = ri.target_is_a(ECHO_ID) && !ri.target_is_a("IDL:Other:1.0")
                            && ri.target_most_derived_interface().equals(ECHO_ID);
                    SEEN.adapterNameOk = Arrays.equals(new String[] {"RootPOA"}, ri.adapter_name());
                }
            }
        }

        @Override
        public void send_reply(ServerRequestInfo ri) {
            ri.add_reply_service_context(
                    new ServiceContext(REPLY_CONTEXT, octets("reply:" + ri.operation())), true);
        }

        @Override
        public void send_exception(ServerRequestInfo ri) {
            synchronized (SEEN) {
                SEEN.sendingExceptionId = ri.sending_exception().type().kind() == TCKind.tk_except
                        ? sendingId(ri) : null;
            }
        }

        private static String sendingId(ServerRequestInfo ri) {
            try {
                return ri.sending_exception().type().id();
            } catch (org.omg.CORBA.TypeCodePackage.BadKind e) {
                return null;
            }
        }

        @Override
        public void send_other(ServerRequestInfo ri) {}
    }

    static final class Ior extends Named implements IORInterceptor {
        Ior() {
            super("component");
        }

        @Override
        public void establish_components(IORInfo info) {
            info.add_ior_component(new TaggedComponent(COMPONENT, new byte[] {1, 2, 3}));
        }
    }

    static final class Marker extends LocalObject {}

    /** Named by the ORB.init property of each ORB. */
    public static final class Init extends LocalObject implements ORBInitializer {
        public Init() {}

        @Override
        public void pre_init(ORBInitInfo info) {
            synchronized (SEEN) {
                SEEN.preInitIds.add(info.orb_id());
                SEEN.arguments = info.arguments();
                SEEN.slot = info.allocate_slot_id();
                try {
                    info.add_client_request_interceptor(new Client());
                    info.add_server_request_interceptor(new Server());
                    info.add_ior_interceptor(new Ior());
                    try {
                        info.add_client_request_interceptor(new Client());
                    } catch (DuplicateName e) {
                        SEEN.duplicateName = "ctx".equals(e.name);
                    }
                    info.register_initial_reference("PiTestMarker", new Marker());
                } catch (DuplicateName | InvalidName e) {
                    throw new AssertionError(e);
                }
                try {
                    info.resolve_initial_references("PiTestMarker");
                } catch (BAD_INV_ORDER e) {
                    SEEN.preInitResolveBadOrder = true;
                } catch (InvalidName e) {
                    throw new AssertionError(e);
                }
            }
        }

        @Override
        public void post_init(ORBInitInfo info) {
            synchronized (SEEN) {
                SEEN.postInitIds.add(info.orb_id());
                try {
                    SEEN.postInitResolveOk =
                            info.resolve_initial_references("PiTestMarker") instanceof Marker;
                    // One of the ORB's own (not the RootPOA: a POA made in
                    // post_init would not have the components of the IOR
                    // interceptors, which only work once the ORB is
                    // initialised).
                    SEEN.poaCurrentOk = info.resolve_initial_references("POACurrent")
                            instanceof org.omg.PortableServer.Current;
                    SEEN.piCurrentOk =
                            info.resolve_initial_references("PICurrent") instanceof Current;
                } catch (InvalidName e) {
                    throw new AssertionError(e);
                }
                try {
                    info.resolve_initial_references("NoSuchName");
                } catch (InvalidName e) {
                    SEEN.postInitInvalidName = true;
                }
                try {
                    Codec codec = info.codec_factory().create_codec(
                            new Encoding(ENCODING_CDR_ENCAPS.value, (byte) 1, (byte) 2));
                    Any a = ORB.init().create_any();
                    a.insert_long(5);
                    SEEN.codecFactoryOk = codec.decode(codec.encode(a)).extract_long() == 5;
                } catch (org.omg.CORBA.UserException e) {
                    throw new AssertionError(e);
                }
                SEEN.keptInfo = info;
            }
        }
    }

    /** echo_string returns the slot the server interceptor set from the
     * request's service context. */
    static final class SlotEcho extends EchoImpl {
        private final Current current;

        SlotEcho(ORB orb) throws Exception {
            super(orb);
            current = CurrentHelper.narrow(orb.resolve_initial_references("PICurrent"));
        }

        @Override
        public String echo_string(String v) {
            try {
                Any a = current.get_slot(SEEN.slot);
                return a.type().kind() == TCKind.tk_string ? a.extract_string() : "no slot:" + v;
            } catch (InvalidSlot e) {
                throw new AssertionError(e);
            }
        }
    }

    static ORB init(String[] args, String id) {
        Properties props = new Properties();
        props.setProperty("org.omg.PortableInterceptor.ORBInitializerClass." + Init.class.getName(),
                "");
        props.setProperty("org.omg.CORBA.ORBId", id);
        return ORB.init(args, props);
    }

    @Test
    void interceptors() throws Exception {
        ORB server = init(new String[] {"-ORBendPoint", "giop:tcp:127.0.0.1:"}, "server");
        assertNotNull(SEEN.keptInfo);
        OBJECT_NOT_EXIST gone = assertThrows(OBJECT_NOT_EXIST.class, () -> SEEN.keptInfo.orb_id());
        assertEquals(OMG | 10, gone.minor);
        POA root = POAHelper.narrow(server.resolve_initial_references("RootPOA"));
        root.the_POAManager().activate();
        byte[] oid = root.activate_object(new SlotEcho(server));
        String ior = server.object_to_string(root.id_to_reference(oid));
        byte[] oid2 = root.activate_object(new SlotEcho(server));
        String ior2 = server.object_to_string(root.id_to_reference(oid2));
        SEEN.serverForward = root.id_to_reference(oid);
        SEEN.serverForwardOid = oid2;
        Thread thread = new Thread(server::run, "server ORB");
        thread.start();
        ORB client = init(new String[0], "client");
        try {
            assertEquals(Arrays.asList("server", "client"), SEEN.preInitIds);
            assertEquals(SEEN.preInitIds, SEEN.postInitIds);
            assertTrue(SEEN.duplicateName);
            assertTrue(SEEN.preInitResolveBadOrder);
            assertTrue(SEEN.postInitResolveOk);
            assertTrue(SEEN.postInitInvalidName);
            assertTrue(SEEN.piCurrentOk);
            assertTrue(SEEN.poaCurrentOk);
            assertTrue(SEEN.codecFactoryOk);
            assertNotNull(SEEN.arguments);

            Echo echo = EchoHelper.narrow(client.string_to_object(ior));

            // A slot of the client thread goes to the servant in a service
            // context, and a reply context comes back.
            Current current = CurrentHelper.narrow(client.resolve_initial_references("PICurrent"));
            Any a = client.create_any();
            a.insert_string("from the client");
            current.set_slot(SEEN.slot, a);
            assertEquals("from the client", echo.echo_string("x"));
            assertEquals("reply:echo_string", SEEN.replyContext);
            assertTrue(SEEN.componentOk);
            assertTrue(SEEN.argumentsUnavailable);
            assertTrue(SEEN.serverOrbIdOk);
            assertTrue(SEEN.targetIsA);
            assertTrue(SEEN.adapterNameOk);
            assertThrows(InvalidSlot.class, () -> current.get_slot(SEEN.slot + 100));

            // The info kept past its point.
            BAD_INV_ORDER late = assertThrows(BAD_INV_ORDER.class, () -> SEEN.keptRi.operation());
            assertEquals(OMG | 14, late.minor);
            SEEN.keptRi = null;

            // An exception from receive_request reaches the client.
            int before = SEEN.receiveException;
            NO_PERMISSION denied = assertThrows(NO_PERMISSION.class, () -> echo.echo_ulong(1));
            assertEquals(7, denied.minor);
            assertEquals(before + 1, SEEN.receiveException);
            assertEquals("IDL:omg.org/CORBA/NO_PERMISSION:1.0", SEEN.receivedExceptionId);
            assertEquals("IDL:omg.org/CORBA/NO_PERMISSION:1.0", SEEN.sendingExceptionId);

            // A user exception of the servant.
            assertThrows(Oops.class, () -> echo.fail(1, "no"));
            assertEquals("IDL:rustorb.org/InteropTest/Oops:1.0", SEEN.receivedExceptionId);

            // ForwardRequest from send_request: the call goes to the forward
            // reference. The interceptor that raised it gets no ending point.
            Echo other = EchoHelper.narrow(client.string_to_object(ior2));
            SEEN.forwardFrom = client.string_to_object(ior2);
            SEEN.forwardTo = client.string_to_object(ior);
            int sends = SEEN.sendRequest;
            int others = SEEN.receiveOther;
            assertEquals(3, other.echo_short((short) 3));
            assertEquals(sends + 2, SEEN.sendRequest);
            assertEquals(others, SEEN.receiveOther);
            SEEN.forwardTo = null;

            // ForwardRequest from receive_request: the client sees
            // LOCATION_FORWARD in receive_other and follows it.
            Echo third = EchoHelper.narrow(client.string_to_object(ior2));
            assertEquals(4, third.echo_octet((byte) 4));
            assertEquals(others + 1, SEEN.receiveOther);

            // The Codec.
            CodecFactory factory =
                    CodecFactoryHelper.narrow(client.resolve_initial_references("CodecFactory"));
            Codec codec = factory.create_codec(
                    new Encoding(ENCODING_CDR_ENCAPS.value, (byte) 1, (byte) 2));
            Any hello = client.create_any();
            hello.insert_string("hello");
            assertEquals("hello", codec.decode(codec.encode(hello)).extract_string());
            byte[] value = codec.encode_value(hello);
            assertEquals("hello",
                    codec.decode_value(value, client.get_primitive_tc(TCKind.tk_string))
                            .extract_string());
            assertThrows(UnknownEncoding.class,
                    () -> factory.create_codec(new Encoding((short) 1, (byte) 1, (byte) 2)));
            assertArrayEquals(codec.encode(hello), codec.encode(hello));
        } finally {
            server.shutdown(true);
            thread.join();
            server.destroy();
            client.destroy();
        }
    }
}
