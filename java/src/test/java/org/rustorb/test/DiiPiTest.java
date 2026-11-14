// Portable Interceptors on DII and DSI requests in the Java binding (#156):
// the client interceptor sees the arguments, result and exception list of
// a DII request, the server interceptor those that a DSI servant read and
// set (docs/portable-interceptors.md).

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Properties;

import InteropTest.OopsHelper;
import interop.DiiAdapters;
import interop.EchoImpl;
import org.junit.jupiter.api.Test;
import org.omg.CORBA.LocalObject;
import org.omg.CORBA.MARSHAL;
import org.omg.CORBA.ORB;
import org.omg.CORBA.Request;
import org.omg.CORBA.TCKind;
import org.omg.Dynamic.Parameter;
import org.omg.PortableInterceptor.ClientRequestInfo;
import org.omg.PortableInterceptor.ClientRequestInterceptor;
import org.omg.PortableInterceptor.ORBInitInfo;
import org.omg.PortableInterceptor.ORBInitializer;
import org.omg.PortableInterceptor.ServerRequestInfo;
import org.omg.PortableInterceptor.ServerRequestInterceptor;
import org.omg.PortableInterceptor.ORBInitInfoPackage.DuplicateName;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;

public class DiiPiTest {
    /** What the interceptors saw, one line for each point. */
    static final List<String> SEEN = Collections.synchronizedList(new ArrayList<>());

    static String describe(Parameter[] params, boolean reply) {
        StringBuilder b = new StringBuilder();
        for (Parameter p : params) {
            b.append(p.mode.value()).append(':');
            // An out argument has only its type before the reply.
            b.append(p.argument.type().kind() != TCKind.tk_long
                    || (p.mode.value() == org.omg.CORBA.ParameterMode._PARAM_OUT && !reply)
                    ? "type " + p.argument.type().kind().value()
                    : Integer.toString(p.argument.extract_long()));
            b.append(' ');
        }
        return b.toString().trim();
    }

    static class Named extends LocalObject {
        public String name() {
            return "dii";
        }

        public void destroy() {}
    }

    static final class Client extends Named implements ClientRequestInterceptor {
        @Override
        public void send_request(ClientRequestInfo ri) {
            SEEN.add("send_request " + ri.operation() + " " + describe(ri.arguments(), false)
                    + " exceptions " + ri.exceptions().length);
        }

        @Override
        public void send_poll(ClientRequestInfo ri) {}

        @Override
        public void receive_reply(ClientRequestInfo ri) {
            SEEN.add("receive_reply " + ri.operation() + " " + describe(ri.arguments(), true)
                    + " result " + ri.result().extract_long());
        }

        @Override
        public void receive_exception(ClientRequestInfo ri) {
            SEEN.add("receive_exception " + ri.operation() + " " + ri.received_exception_id());
        }

        @Override
        public void receive_other(ClientRequestInfo ri) {}
    }

    static final class Server extends Named implements ServerRequestInterceptor {
        @Override
        public void receive_request_service_contexts(ServerRequestInfo ri) {}

        @Override
        public void receive_request(ServerRequestInfo ri) {}

        @Override
        public void send_reply(ServerRequestInfo ri) {
            SEEN.add("send_reply " + ri.operation() + " " + describe(ri.arguments(), true)
                    + " result " + ri.result().extract_long());
        }

        @Override
        public void send_exception(ServerRequestInfo ri) {
            SEEN.add("send_exception " + ri.operation() + " "
                    + ri.sending_exception().type().kind().value());
        }

        @Override
        public void send_other(ServerRequestInfo ri) {}
    }

    public static final class Init extends LocalObject implements ORBInitializer {
        public Init() {}

        @Override
        public void pre_init(ORBInitInfo info) {
            try {
                info.add_client_request_interceptor(new Client());
                info.add_server_request_interceptor(new Server());
            } catch (DuplicateName e) {
                throw new AssertionError(e);
            }
        }

        @Override
        public void post_init(ORBInitInfo info) {}
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
        ORB server = init(new String[] {"-ORBendPoint", "giop:tcp:127.0.0.1:"}, "dii-server");
        POA root = POAHelper.narrow(server.resolve_initial_references("RootPOA"));
        root.the_POAManager().activate();
        Thread t = new Thread(server::run, "server ORB");
        t.start();
        ORB client = init(new String[0], "dii-client");
        try {
            String ior = server.object_to_string(root.servant_to_reference(
                    DiiAdapters.dsi(server, new EchoImpl(server))));
            org.omg.CORBA.Object obj = client.string_to_object(ior);

            Request req = obj._request("divide");
            req.add_in_arg().insert_long(7);
            req.add_in_arg().insert_long(2);
            req.add_out_arg().type(client.get_primitive_tc(TCKind.tk_long));
            req.set_return_type(client.get_primitive_tc(TCKind.tk_long));
            req.exceptions().add(InteropTest.DivideByZeroHelper.type());
            req.invoke();
            assertEquals(3, req.return_value().extract_long());
            assertEquals(List.of("send_request divide 0:7 0:2 1:type " + TCKind._tk_long + " exceptions 1",
                    "send_reply divide 0:7 0:2 1:1 result 3",
                    "receive_reply divide 0:7 0:2 1:1 result 3"), SEEN);

            SEEN.clear();
            Request fail = obj._request("fail");
            fail.add_in_arg().insert_long(1);
            fail.add_in_arg().insert_long(2);
            fail.exceptions().add(OopsHelper.type());
            // The server reads a string: the argument does not fit (MARSHAL).
            fail.invoke();
            assertTrue(fail.env().exception() instanceof MARSHAL);
            SEEN.clear();
            fail.arguments().item(1).value().insert_string("x");
            fail.invoke();
            assertTrue(fail.env().exception() instanceof org.omg.CORBA.UnknownUserException);
            assertEquals(List.of("send_request fail 0:1 0:type " + TCKind._tk_string
                    + " exceptions 1", "send_exception fail " + TCKind._tk_except,
                    "receive_exception fail " + OopsHelper.id()), SEEN);
        } finally {
            server.shutdown(true);
            t.join();
            server.destroy();
            client.destroy();
        }
    }
}
