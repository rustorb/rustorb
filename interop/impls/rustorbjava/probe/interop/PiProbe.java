// The rustorb Java side of run-pi-probe.sh: the probe of
// impls/rustorb/src/pi_probe.rs on the standard
// org.omg.PortableInterceptor API, printing the same lines. Like
// impls/jacorb/pi/PiProbe.java, but on the stubs and skeletons of
// idl/PiProbe.idl instead of the DII and the DSI.
//
//   PiProbe server [ORB options]       prints the IOR of an Echo object,
//                                      then serves
//   PiProbe client IOR [ORB options]   calls echo

package interop;

import java.util.ArrayList;
import java.util.List;
import java.util.Properties;

import PiProbe.EchoHelper;
import PiProbe.EchoPOA;
import org.omg.CORBA.Any;
import org.omg.CORBA.LocalObject;
import org.omg.CORBA.ORB;
import org.omg.CORBA.TCKind;
import org.omg.IOP.Codec;
import org.omg.IOP.ENCODING_CDR_ENCAPS;
import org.omg.IOP.Encoding;
import org.omg.IOP.ServiceContext;
import org.omg.IOP.TaggedComponent;
import org.omg.PortableInterceptor.ClientRequestInfo;
import org.omg.PortableInterceptor.ClientRequestInterceptor;
import org.omg.PortableInterceptor.ForwardRequest;
import org.omg.PortableInterceptor.IORInfo;
import org.omg.PortableInterceptor.IORInterceptor;
import org.omg.PortableInterceptor.ORBInitInfo;
import org.omg.PortableInterceptor.ORBInitializer;
import org.omg.PortableInterceptor.ServerRequestInfo;
import org.omg.PortableInterceptor.ServerRequestInterceptor;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;

public final class PiProbe {
    static final int CONTEXT = 0x50495052;
    static final int TAG = 0x50495052;
    static final String NAME = "rustorbjava";

    static Probe probe;

    static byte[] encode(Codec codec, String s) {
        Any any = ORB.init().create_any();
        any.insert_string(s);
        try {
            return codec.encode_value(any);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    static String decode(Codec codec, byte[] data) {
        try {
            return codec.decode_value(data, ORB.init().get_primitive_tc(TCKind.tk_string))
                    .extract_string();
        } catch (Exception e) {
            return "<" + e + ">";
        }
    }

    static String id(Exception e) {
        return e instanceof org.omg.CORBA.SystemException
                ? "IDL:omg.org/CORBA/" + e.getClass().getSimpleName() + ":1.0"
                : e.toString();
    }

    static final class Probe extends LocalObject
            implements ClientRequestInterceptor, ServerRequestInterceptor, IORInterceptor {
        final Codec codec;
        final int slot;
        final List<String> seen = new ArrayList<>();

        Probe(Codec codec, int slot) {
            this.codec = codec;
            this.slot = slot;
        }

        public String name() {
            return "pi-probe";
        }

        public void destroy() {}

        public void send_request(ClientRequestInfo ri) throws ForwardRequest {
            ri.add_request_service_context(
                    new ServiceContext(CONTEXT, encode(codec, "from " + NAME)), false);
        }

        public void send_poll(ClientRequestInfo ri) {}

        public void receive_reply(ClientRequestInfo ri) {
            if (!ri.operation().equals("echo")) {
                return; // narrow's _is_a
            }
            String context;
            String component;
            try {
                context = decode(codec, ri.get_reply_service_context(CONTEXT).context_data);
            } catch (org.omg.CORBA.SystemException e) {
                context = id(e);
            }
            try {
                component = decode(codec, ri.get_effective_component(TAG).component_data);
            } catch (org.omg.CORBA.SystemException e) {
                component = id(e);
            }
            synchronized (seen) {
                seen.add("reply context: " + context);
                seen.add("component: " + component);
            }
        }

        public void receive_exception(ClientRequestInfo ri) throws ForwardRequest {}

        public void receive_other(ClientRequestInfo ri) throws ForwardRequest {}

        public void receive_request_service_contexts(ServerRequestInfo ri) throws ForwardRequest {
            String got;
            try {
                got = decode(codec, ri.get_request_service_context(CONTEXT).context_data);
            } catch (org.omg.CORBA.SystemException e) {
                got = id(e);
            }
            Any any = ORB.init().create_any();
            any.insert_string(got);
            try {
                ri.set_slot(slot, any);
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        }

        public void receive_request(ServerRequestInfo ri) throws ForwardRequest {}

        public void send_reply(ServerRequestInfo ri) {
            String got;
            try {
                got = ri.get_slot(slot).extract_string();
            } catch (Exception e) {
                got = "<" + e + ">";
            }
            ri.add_reply_service_context(
                    new ServiceContext(CONTEXT, encode(codec, NAME + " got: " + got)), false);
        }

        public void send_exception(ServerRequestInfo ri) throws ForwardRequest {}

        public void send_other(ServerRequestInfo ri) throws ForwardRequest {}

        public void establish_components(IORInfo info) {
            info.add_ior_component(new TaggedComponent(TAG, encode(codec, NAME + " component")));
        }
    }

    public static final class Init extends LocalObject implements ORBInitializer {
        public void pre_init(ORBInitInfo info) {
            try {
                Codec codec = info.codec_factory().create_codec(
                        new Encoding(ENCODING_CDR_ENCAPS.value, (byte) 1, (byte) 2));
                probe = new Probe(codec, info.allocate_slot_id());
                info.add_client_request_interceptor(probe);
                info.add_server_request_interceptor(probe);
                info.add_ior_interceptor(probe);
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        }

        public void post_init(ORBInitInfo info) {}
    }

    static final class Echo extends EchoPOA {
        public String echo(String s) {
            return s;
        }
    }

    public static void main(String[] args) throws Exception {
        Properties props = new Properties();
        props.setProperty("org.omg.PortableInterceptor.ORBInitializerClass." + Init.class.getName(),
                "");
        ORB orb = ORB.init(args, props);
        if (args.length >= 1 && args[0].equals("server")) {
            POA root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
            root.the_POAManager().activate();
            org.omg.CORBA.Object obj = root.servant_to_reference(new Echo());
            System.out.println(orb.object_to_string(obj));
            System.out.flush();
            orb.run();
        } else if (args.length >= 2 && args[0].equals("client")) {
            String result;
            try {
                result = EchoHelper.narrow(orb.string_to_object(args[1])).echo("hello");
            } catch (Exception e) {
                result = id(e);
            }
            synchronized (probe.seen) {
                for (String line : probe.seen) {
                    System.out.println(line);
                }
            }
            System.out.println("result: " + result);
            orb.destroy();
        } else {
            System.err.println("usage: PiProbe server|client IOR [ORB options]");
            System.exit(2);
        }
    }
}
