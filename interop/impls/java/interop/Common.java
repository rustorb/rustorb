// Helpers shared by the Java InteropTest servers and clients (rustorbjava and
// jacorb): the same options and naming service handling as the other
// implementations, on the standard IDL to Java mapping.

package interop;

import java.util.ArrayList;
import java.util.List;
import java.util.Properties;

import org.omg.CORBA.COMM_FAILURE;
import org.omg.CORBA.OBJECT_NOT_EXIST;
import org.omg.CORBA.ORB;
import org.omg.CORBA.Policy;
import org.omg.CORBA.TRANSIENT;
import org.omg.CosNaming.NameComponent;
import org.omg.CosNaming.NamingContext;
import org.omg.CosNaming.NamingContextHelper;
import org.omg.CosNaming.NamingContextPackage.AlreadyBound;
import org.omg.CosNaming.NamingContextPackage.NotFound;
import org.omg.PortableServer.IdAssignmentPolicyValue;
import org.omg.PortableServer.LifespanPolicyValue;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;
import org.omg.PortableServer.POAManager;

public final class Common {
    private Common() {}

    public static final class Options {
        public String naming;
        public String name;
        public String host = "";
        public int port;
        public double timeout = 30.0;
        public boolean shutdownServer;
        public String suite = "basic";
        public final List<String> orbArgs = new ArrayList<>();
    }

    /** Parses the options shared with the other implementations; unknown
     * arguments are passed to ORB.init. */
    public static Options parseArgs(String prog, String[] argv) {
        Options opt = new Options();
        for (int i = 0; i < argv.length; ++i) {
            String a = argv[i];
            boolean hasValue = i + 1 < argv.length;
            switch (a) {
                case "--naming":
                    opt.naming = value(argv, ++i, a);
                    break;
                case "--name":
                    opt.name = value(argv, ++i, a);
                    break;
                case "--host":
                    opt.host = value(argv, ++i, a);
                    break;
                case "--port":
                    opt.port = Integer.parseInt(value(argv, ++i, a));
                    break;
                case "--timeout":
                    opt.timeout = Double.parseDouble(value(argv, ++i, a));
                    break;
                case "--shutdown-server":
                    opt.shutdownServer = true;
                    break;
                case "--suite":
                    opt.suite = value(argv, ++i, a);
                    if (!opt.suite.equals("basic") && !opt.suite.equals("any")
                            && !opt.suite.equals("value") && !opt.suite.equals("custom")) {
                        System.err.println("unknown suite " + opt.suite);
                        System.exit(2);
                    }
                    break;
                default:
                    opt.orbArgs.add(a);
                    break;
            }
        }
        if (opt.naming == null || opt.name == null) {
            System.err.println("usage: " + prog
                    + " --naming CORBALOC --name PATH [--host H] [--port N] [--timeout S]"
                    + " [--shutdown-server] [--suite basic|any|value|custom] [ORB options]");
            System.exit(2);
        }
        return opt;
    }

    private static String value(String[] argv, int i, String option) {
        if (i >= argv.length) {
            System.err.println("missing value for " + option);
            System.exit(2);
        }
        return argv[i];
    }

    /** The ORB, with the naming service, the endpoint and the GIOP limit
     * in the form of the ORB in use (the system property interop.orb:
     * rustorb or jacorb). */
    public static ORB orbInit(Options opt, String implName) {
        List<String> args = new ArrayList<>(opt.orbArgs);
        Properties props = new Properties();
        // -ORBInitRef is standard (CORBA 3.3 part 1, 8.5.3.2).
        args.add("-ORBInitRef");
        args.add("NameService=" + opt.naming);
        String giop = System.getenv("ORBmaxGIOPVersion");
        boolean jacorb = "jacorb".equals(System.getProperty("interop.orb"));
        if (jacorb) {
            props.setProperty("org.omg.CORBA.ORBClass", "org.jacorb.orb.ORB");
            props.setProperty("org.omg.CORBA.ORBSingletonClass", "org.jacorb.orb.ORBSingleton");
            // PERSISTENT POAs need an implementation name.
            props.setProperty("jacorb.implname", implName);
            if (opt.port != 0) {
                props.setProperty("OAIAddr", opt.host);
                props.setProperty("OAPort", Integer.toString(opt.port));
            }
            if (giop != null && !giop.isEmpty()) {
                props.setProperty("jacorb.giop_minor_version", giop.substring(giop.indexOf('.') + 1));
            }
        } else {
            if (opt.port != 0) {
                args.add("-ORBendPoint");
                args.add("giop:tcp:" + opt.host + ":" + opt.port);
            }
            if (giop != null && !giop.isEmpty()) {
                args.add("-ORBmaxGIOPVersion");
                args.add(giop);
            }
        }
        return ORB.init(args.toArray(new String[0]), props);
    }

    public static NameComponent[] toName(String path) {
        List<NameComponent> name = new ArrayList<>();
        for (String part : path.split("/")) {
            if (!part.isEmpty()) {
                name.add(new NameComponent(part, ""));
            }
        }
        return name.toArray(new NameComponent[0]);
    }

    public interface Attempt<T> {
        T run() throws Exception;
    }

    /** Calls fn until it stops throwing "not there yet" exceptions or the
     * timeout passes. */
    public static <T> T retry(String what, Attempt<T> fn, double timeout) throws Exception {
        long deadline = System.nanoTime() + (long) (timeout * 1e9);
        for (;;) {
            try {
                return fn.run();
            } catch (TRANSIENT | COMM_FAILURE | OBJECT_NOT_EXIST | NotFound e) {
                // not there yet
            }
            if (System.nanoTime() >= deadline) {
                System.err.println("timed out waiting for " + what);
                System.exit(1);
            }
            Thread.sleep(200);
        }
    }

    public static NamingContext rootContext(ORB orb, double timeout) throws Exception {
        return retry("the naming service", () -> {
            NamingContext ctx = NamingContextHelper.narrow(orb.resolve_initial_references("NameService"));
            if (ctx == null) {
                throw new TRANSIENT();
            }
            return ctx;
        }, timeout);
    }

    /** Binds obj at path, creating intermediate contexts as needed. */
    public static void bindName(NamingContext root, String path, org.omg.CORBA.Object obj)
            throws Exception {
        NameComponent[] name = toName(path);
        NamingContext ctx = root;
        for (int i = 0; i + 1 < name.length; ++i) {
            NameComponent[] component = {name[i]};
            try {
                ctx = ctx.bind_new_context(component);
            } catch (AlreadyBound e) {
                ctx = NamingContextHelper.narrow(ctx.resolve(component));
            }
        }
        ctx.rebind(new NameComponent[] {name[name.length - 1]}, obj);
    }

    /** A PERSISTENT / USER_ID child POA, so that object keys are fixed and
     * GIOP captures are reproducible between runs. */
    public static POA persistentPoa(ORB orb, String name) throws Exception {
        POA root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        POAManager mgr = root.the_POAManager();
        Policy[] policies = {
            root.create_lifespan_policy(LifespanPolicyValue.PERSISTENT),
            root.create_id_assignment_policy(IdAssignmentPolicyValue.USER_ID),
        };
        POA poa = root.create_POA(name, mgr, policies);
        mgr.activate();
        return poa;
    }
}
