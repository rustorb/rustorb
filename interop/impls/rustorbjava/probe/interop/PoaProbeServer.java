// rustorb Java server for the POA and thread model probes (PoaProbe.idl).
//
//   poa-probe-server --host HOST --port N --ior-file FILE [ORB options]
//
// The same objects and POAs as impls/omniorbpy/poa_probe_server.py, on the
// IDL to Java mapping; see run-poa-probe.sh.

package interop;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import PoaProbe.CallInfo;
import PoaProbe.ControlPOA;
import PoaProbe.Failed;
import PoaProbe.Target;
import PoaProbe.TargetPOA;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.LocalObject;
import org.omg.CORBA.OBJECT_NOT_EXIST;
import org.omg.CORBA.ORB;
import org.omg.CORBA.Policy;
import org.omg.CORBA.StringHolder;
import org.omg.CORBA.SystemException;
import org.omg.PortableServer.AdapterActivator;
import org.omg.PortableServer.Current;
import org.omg.PortableServer.CurrentHelper;
import org.omg.PortableServer.ForwardRequest;
import org.omg.PortableServer.IdAssignmentPolicyValue;
import org.omg.PortableServer.IdUniquenessPolicyValue;
import org.omg.PortableServer.LifespanPolicyValue;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;
import org.omg.PortableServer.POAManager;
import org.omg.PortableServer.POAManagerPackage.AdapterInactive;
import org.omg.PortableServer.POAPackage.AdapterNonExistent;
import org.omg.PortableServer.POAPackage.ObjectAlreadyActive;
import org.omg.PortableServer.POAPackage.ObjectNotActive;
import org.omg.PortableServer.POAPackage.ServantAlreadyActive;
import org.omg.PortableServer.RequestProcessingPolicyValue;
import org.omg.PortableServer.Servant;
import org.omg.PortableServer.ServantActivator;
import org.omg.PortableServer.ServantLocator;
import org.omg.PortableServer.ServantLocatorPackage.CookieHolder;
import org.omg.PortableServer.ServantRetentionPolicyValue;
import org.omg.PortableServer.ThreadPolicyValue;

public final class PoaProbeServer {
    static final long START = System.nanoTime();
    static final String TARGET_ID = "IDL:rustorb.org/PoaProbe/Target:1.0";
    static Thread mainThread;

    static final Map<Thread, String> labels = new HashMap<>();
    static final List<String> events = new ArrayList<>();

    static double now() {
        return (System.nanoTime() - START) / 1e9;
    }

    static synchronized String threadLabel() {
        Thread t = Thread.currentThread();
        if (t == mainThread) {
            return "main";
        }
        return labels.computeIfAbsent(t, k -> "T" + (labels.size() + 1));
    }

    static void event(String text) {
        synchronized (events) {
            events.add(text);
        }
    }

    static String str(byte[] oid) {
        return new String(oid, StandardCharsets.ISO_8859_1);
    }

    static byte[] oid(String s) {
        return s.getBytes(StandardCharsets.ISO_8859_1);
    }

    static void sleep(int ms) {
        if (ms <= 0) {
            return;
        }
        try {
            Thread.sleep(ms);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    static String poaPath(POA poa) {
        List<String> names = new ArrayList<>();
        while (poa != null) {
            POA parent = poa.the_parent();
            if (parent == null) {
                break;
            }
            names.add(0, poa.the_name());
            poa = parent;
        }
        return names.isEmpty() ? "RootPOA" : String.join("/", names);
    }

    static final class TargetImpl extends TargetPOA {
        private final Current current;

        TargetImpl(Current current) {
            this.current = current;
        }

        public CallInfo work(int ms) {
            double start = now();
            String thread = threadLabel();
            String id;
            String poa;
            try {
                id = str(current.get_object_id());
                poa = poaPath(current.get_POA());
            } catch (org.omg.PortableServer.CurrentPackage.NoContext e) {
                throw new org.omg.CORBA.INTERNAL("no context", 0, CompletionStatus.COMPLETED_NO);
            }
            sleep(ms);
            return new CallInfo(thread, id, poa, start, now());
        }

        public void post(int seq, int ms) {
            sleep(ms);
            event("post " + seq);
        }

        public CallInfo relay(Target other, int ms, StringHolder outerThread) {
            outerThread.value = threadLabel();
            return other.work(ms);
        }
    }

    static final class Activator extends LocalObject implements ServantActivator {
        private final Servant servant;
        private final org.omg.CORBA.Object forward;

        Activator(Servant servant, org.omg.CORBA.Object forward) {
            this.servant = servant;
            this.forward = forward;
        }

        public Servant incarnate(byte[] oid, POA adapter) throws ForwardRequest {
            String name = str(oid);
            event("incarnate " + name);
            if (name.equals("bad")) {
                throw new OBJECT_NOT_EXIST(0, CompletionStatus.COMPLETED_NO);
            }
            if (name.equals("fwd")) {
                throw new ForwardRequest(forward);
            }
            return servant;
        }

        public void etherealize(byte[] oid, POA adapter, Servant serv, boolean cleanupInProgress,
                boolean remainingActivations) {
            event("etherealize " + str(oid));
        }
    }

    static final class Locator extends LocalObject implements ServantLocator {
        private final Servant servant;
        private final org.omg.CORBA.Object forward;

        Locator(Servant servant, org.omg.CORBA.Object forward) {
            this.servant = servant;
            this.forward = forward;
        }

        public Servant preinvoke(byte[] oid, POA adapter, String operation, CookieHolder cookie)
                throws ForwardRequest {
            String name = str(oid);
            event("preinvoke " + name + " " + operation);
            if (name.equals("bad")) {
                throw new OBJECT_NOT_EXIST(0, CompletionStatus.COMPLETED_NO);
            }
            if (name.equals("fwd")) {
                throw new ForwardRequest(forward);
            }
            return servant;
        }

        public void postinvoke(byte[] oid, POA adapter, String operation, Object cookie,
                Servant servant) {
            event("postinvoke " + str(oid) + " " + operation);
        }
    }

    /** Creates POAs named lazy* on demand, with a default servant. */
    static final class LazyActivator extends LocalObject implements AdapterActivator {
        private final Servant servant;

        LazyActivator(Servant servant) {
            this.servant = servant;
        }

        public boolean unknown_adapter(POA parent, String name) {
            event("unknown_adapter " + name);
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
                poa.set_servant(servant);
                return true;
            } catch (Exception e) {
                System.err.println("unknown_adapter: " + e);
                return false;
            }
        }
    }

    static final class ControlImpl extends ControlPOA {
        private final ORB orb;
        private final POA root;
        private final POAManager managed;
        private final Current current;
        private final Map<String, org.omg.CORBA.Object> systemRefs;

        ControlImpl(ORB orb, POA root, POAManager managed, Current current,
                Map<String, org.omg.CORBA.Object> systemRefs) {
            this.orb = orb;
            this.root = root;
            this.managed = managed;
            this.current = current;
            this.systemRefs = systemRefs;
        }

        private POA find(String path) throws Failed {
            POA poa = root;
            for (String name : path.split("/")) {
                if (name.isEmpty() || name.equals("RootPOA")) {
                    continue;
                }
                try {
                    poa = poa.find_POA(name, true);
                } catch (AdapterNonExistent e) {
                    throw new Failed("no POA " + path);
                }
            }
            return poa;
        }

        public String reference(String poa, String id) throws Failed {
            org.omg.CORBA.Object ref = systemRefs.get(poa);
            if (ref == null) {
                ref = find(poa).create_reference_with_id(oid(id), TARGET_ID);
            }
            return orb.object_to_string(ref);
        }

        public void set_state(String state) throws Failed {
            try {
                switch (state) {
                    case "active":
                        managed.activate();
                        break;
                    case "holding":
                        managed.hold_requests(false);
                        break;
                    case "discarding":
                        managed.discard_requests(false);
                        break;
                    case "inactive":
                        managed.deactivate(false, false);
                        break;
                    default:
                        throw new Failed("unknown state " + state);
                }
            } catch (AdapterInactive e) {
                throw new Failed("AdapterInactive");
            }
        }

        public void deactivate_object(String poa, String id) throws Failed {
            try {
                find(poa).deactivate_object(oid(id));
            } catch (ObjectNotActive e) {
                throw new Failed("ObjectNotActive");
            } catch (org.omg.PortableServer.POAPackage.WrongPolicy e) {
                throw new Failed("WrongPolicy");
            }
        }

        public void activate_object(String poa, String id) throws Failed {
            try {
                find(poa).activate_object_with_id(oid(id), new TargetImpl(current));
            } catch (ObjectAlreadyActive e) {
                throw new Failed("ObjectAlreadyActive");
            } catch (ServantAlreadyActive e) {
                throw new Failed("ServantAlreadyActive");
            } catch (org.omg.PortableServer.POAPackage.WrongPolicy e) {
                throw new Failed("WrongPolicy");
            }
        }

        public void destroy_poa(String poa) throws Failed {
            find(poa).destroy(true, false);
        }

        public String[] events() {
            synchronized (events) {
                String[] r = events.toArray(new String[0]);
                events.clear();
                return r;
            }
        }

        public String shutdown_inside(boolean waitForCompletion) {
            try {
                orb.shutdown(waitForCompletion);
            } catch (SystemException e) {
                String id = "IDL:omg.org/CORBA/" + e.getClass().getSimpleName() + ":1.0";
                return String.format("%s minor 0x%08x", id, e.minor);
            }
            return "";
        }

        public void shutdown_later(int delayMs) {
            Thread t = new Thread(() -> {
                sleep(delayMs);
                orb.shutdown(false);
            });
            t.setDaemon(true);
            t.start();
        }
    }

    public static void main(String[] argv) throws Exception {
        mainThread = Thread.currentThread();
        String host = "";
        String iorFile = null;
        int port = 0;
        List<String> orbArgs = new ArrayList<>();
        for (int i = 0; i < argv.length; i++) {
            String a = argv[i];
            if (a.equals("--host") && i + 1 < argv.length) {
                host = argv[++i];
            } else if (a.equals("--port") && i + 1 < argv.length) {
                port = Integer.parseInt(argv[++i]);
            } else if (a.equals("--ior-file") && i + 1 < argv.length) {
                iorFile = argv[++i];
            } else {
                orbArgs.add(a);
            }
        }
        if (iorFile == null) {
            System.err.println("usage: poa-probe-server --ior-file FILE [--host H] [--port N]");
            System.exit(2);
        }
        if (port != 0) {
            orbArgs.add("-ORBendPoint");
            orbArgs.add("giop:tcp:" + host + ":" + port);
        }

        ORB orb = ORB.init(orbArgs.toArray(new String[0]), null);
        POA root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        Current current = CurrentHelper.narrow(orb.resolve_initial_references("POACurrent"));
        POAManager rootManager = root.the_POAManager();
        rootManager.activate();

        Map<String, org.omg.CORBA.Object> systemRefs = new HashMap<>();
        TargetImpl servant = new TargetImpl(current);
        // RootPOA: TRANSIENT, SYSTEM_ID, IMPLICIT_ACTIVATION.
        systemRefs.put("RootPOA", root.servant_to_reference(servant));

        POA persistentUser = root.create_POA("persistent_user", rootManager, policies(root));
        persistentUser.activate_object_with_id(oid("obj"), servant);
        persistentUser.activate_object_with_id(oid("victim"), new TargetImpl(current));
        POA nested = persistentUser.create_POA("nested", rootManager, policies(root));
        nested.activate_object_with_id(oid("obj"), servant);

        POA persistentSystem = root.create_POA("persistent_system", rootManager, new Policy[] {
            root.create_lifespan_policy(LifespanPolicyValue.PERSISTENT),
            root.create_id_assignment_policy(IdAssignmentPolicyValue.SYSTEM_ID),
        });
        byte[] id = persistentSystem.activate_object(new TargetImpl(current));
        systemRefs.put("persistent_system", persistentSystem.id_to_reference(id));

        POA transientUser = root.create_POA("transient_user", rootManager, new Policy[] {
            root.create_lifespan_policy(LifespanPolicyValue.TRANSIENT),
            root.create_id_assignment_policy(IdAssignmentPolicyValue.USER_ID),
        });
        transientUser.activate_object_with_id(oid("obj"), servant);

        POA transientSystem = root.create_POA("transient_system", rootManager, new Policy[] {
            root.create_lifespan_policy(LifespanPolicyValue.TRANSIENT),
            root.create_id_assignment_policy(IdAssignmentPolicyValue.SYSTEM_ID),
        });
        id = transientSystem.activate_object(new TargetImpl(current));
        systemRefs.put("transient_system", transientSystem.id_to_reference(id));

        for (String name : new String[] {"single", "single2"}) {
            POA poa = root.create_POA(name, rootManager, policies(root,
                    root.create_thread_policy(ThreadPolicyValue.SINGLE_THREAD_MODEL)));
            poa.activate_object_with_id(oid("a"), servant);
            poa.activate_object_with_id(oid("b"), new TargetImpl(current));
        }

        POA mainPoa = root.create_POA("main", rootManager, policies(root,
                root.create_thread_policy(ThreadPolicyValue.MAIN_THREAD_MODEL)));
        mainPoa.activate_object_with_id(oid("a"), servant);
        mainPoa.activate_object_with_id(oid("b"), new TargetImpl(current));

        POA managed = root.create_POA("managed", null, policies(root));
        POAManager managedManager = managed.the_POAManager();
        managed.activate_object_with_id(oid("obj"), servant);

        POA doomed = root.create_POA("doomed", rootManager, policies(root));
        doomed.activate_object_with_id(oid("obj"), servant);

        org.omg.CORBA.Object forward = persistentUser.id_to_reference(oid("obj"));
        POA activator = root.create_POA("activator", rootManager, policies(root,
                root.create_request_processing_policy(
                        RequestProcessingPolicyValue.USE_SERVANT_MANAGER)));
        activator.set_servant_manager(new Activator(servant, forward));
        POA locator = root.create_POA("locator", rootManager, policies(root,
                root.create_request_processing_policy(
                        RequestProcessingPolicyValue.USE_SERVANT_MANAGER),
                root.create_servant_retention_policy(ServantRetentionPolicyValue.NON_RETAIN)));
        locator.set_servant_manager(new Locator(servant, forward));

        POA dflt = root.create_POA("default", rootManager, policies(root,
                root.create_request_processing_policy(
                        RequestProcessingPolicyValue.USE_DEFAULT_SERVANT),
                root.create_servant_retention_policy(ServantRetentionPolicyValue.NON_RETAIN),
                root.create_id_uniqueness_policy(IdUniquenessPolicyValue.MULTIPLE_ID)));
        dflt.set_servant(servant);

        root.the_activator(new LazyActivator(servant));

        // The Control object has its own POAManager, so that it keeps
        // working whatever the probe does to the others.
        POA controlPoa = root.create_POA("control", null, policies(root));
        controlPoa.activate_object_with_id(oid("Control"),
                new ControlImpl(orb, root, managedManager, current, systemRefs));
        controlPoa.the_POAManager().activate();
        managedManager.activate();

        String ior = orb.object_to_string(controlPoa.id_to_reference(oid("Control")));
        Path tmp = Paths.get(iorFile + ".tmp");
        Files.write(tmp, (ior + "\n").getBytes(StandardCharsets.US_ASCII));
        Files.move(tmp, Paths.get(iorFile), StandardCopyOption.REPLACE_EXISTING,
                StandardCopyOption.ATOMIC_MOVE);
        System.out.println("server ready");
        System.out.flush();

        orb.run();
        System.out.println("server stopped");
        orb.destroy();
    }

    static Policy[] policies(POA root, Policy... extra) {
        Policy[] p = new Policy[2 + extra.length];
        p[0] = root.create_lifespan_policy(LifespanPolicyValue.PERSISTENT);
        p[1] = root.create_id_assignment_policy(IdAssignmentPolicyValue.USER_ID);
        System.arraycopy(extra, 0, p, 2, extra.length);
        return p;
    }

    private PoaProbeServer() {}
}
