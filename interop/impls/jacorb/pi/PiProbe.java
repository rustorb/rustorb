// The Java side of run-pi-probe.sh: Portable Interceptors between JacORB
// and rustorb (docs/portable-interceptors.md), on the standard
// org.omg.PortableInterceptor API only. The same probe as
// impls/rustorb/src/pi_probe.rs, printing the same lines:
//
//   PiProbe server [ORB options]       prints the IOR of an Echo object
//                                      (DSI), then serves
//   PiProbe client IOR [ORB options]   calls echo (DII)
//
// The request and reply service contexts (id CONTEXT) and the tagged
// component (TAG) hold a string written with IOP::Codec (CDR encapsulation
// 1.2, encode_value).

package interop;

import java.util.ArrayList;
import java.util.List;
import java.util.Properties;

import org.omg.CORBA.Any;
import org.omg.CORBA.LocalObject;
import org.omg.CORBA.NVList;
import org.omg.CORBA.ORB;
import org.omg.CORBA.ServerRequest;
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
import org.omg.PortableServer.DynamicImplementation;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;

public final class PiProbe {
    private PiProbe() {}

    /** The service context id and the component tag ("PIPR"). */
    static final int CONTEXT = 0x50495052;
    static final int TAG = 0x50495052;
    static final String NAME = "jacorb";
    static final String ECHO_ID = "IDL:PiProbe/Echo:1.0";

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
            String context;
            try {
                context = decode(codec, ri.get_reply_service_context(CONTEXT).context_data);
            } catch (Exception e) {
                context = id(e);
            }
            String component;
            try {
                component = decode(codec, ri.get_effective_component(TAG).component_data);
            } catch (Exception e) {
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
            } catch (Exception e) {
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
                got = id(e);
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

    static final class Echo extends DynamicImplementation {
        final ORB orb;

        Echo(ORB orb) {
            this.orb = orb;
        }

        public String[] _all_interfaces(POA poa, byte[] id) {
            return new String[] {ECHO_ID};
        }

        public void invoke(ServerRequest request) {
            if (!request.operation().equals("echo")) {
                throw new org.omg.CORBA.BAD_OPERATION();
            }
            NVList args = orb.create_list(1);
            Any arg = orb.create_any();
            arg.type(orb.get_primitive_tc(TCKind.tk_string));
            args.add_value("s", arg, org.omg.CORBA.ARG_IN.value);
            request.arguments(args);
            Any result = orb.create_any();
            result.insert_string(arg.extract_string());
            request.set_result(result);
        }
    }

    public static void main(String[] args) throws Exception {
        Properties props = new Properties();
        props.setProperty("org.omg.PortableInterceptor.ORBInitializerClass." + Init.class.getName(), "");
        ORB orb = ORB.init(args, props);
        if (args.length >= 1 && args[0].equals("server")) {
            POA root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
            root.the_POAManager().activate();
            org.omg.CORBA.Object obj = root.servant_to_reference(new Echo(orb));
            System.out.println(orb.object_to_string(obj));
            System.out.flush();
            orb.run();
        } else if (args.length >= 2 && args[0].equals("client")) {
            org.omg.CORBA.Object obj = orb.string_to_object(args[1]);
            org.omg.CORBA.Request request = obj._request("echo");
            request.add_in_arg().insert_string("hello");
            request.set_return_type(orb.get_primitive_tc(TCKind.tk_string));
            String result;
            try {
                request.invoke();
                Exception e = request.env().exception();
                result = e == null ? request.return_value().extract_string() : id(e);
            } catch (Exception e) {
                result = id(e);
            }
            synchronized (probe.seen) {
                for (String line : probe.seen) {
                    System.out.println(line);
                }
            }
            System.out.println("result: " + result);
            orb.shutdown(true);
        } else {
            System.err.println("usage: PiProbe server|client IOR [ORB options]");
            System.exit(2);
        }
    }
}
