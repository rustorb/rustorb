// The ORB of rustorb (an rorb_orb handle).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;
import java.util.ArrayList;
import java.util.List;
import java.util.Properties;

import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.ORBPackage.InvalidName;

public final class OrbImpl extends org.omg.CORBA_2_3.ORB {
    private final Owned handle;
    private volatile boolean destroyed;

    /** The ORBs initialised and not destroyed, oldest first. */
    private static final List<OrbImpl> LIVE = new ArrayList<>();

    private OrbImpl(long handle) {
        this.handle = Owned.register(this, handle, Native::orbRelease);
    }

    /**
     * ORB.init(args, props): the -ORB options of args, then the properties
     * that name one (ORBInitRef, or org.omg.CORBA.ORBInitRef), and
     * org.omg.CORBA.ORBInitialHost / ORBInitialPort as the initial
     * reference NameService.
     */
    public static OrbImpl create(String[] args, Properties props) {
        List<String> argv = new ArrayList<>();
        if (args != null) {
            for (String a : args) {
                if (a != null) {
                    argv.add(a);
                }
            }
        }
        if (props != null) {
            String host = null;
            String port = null;
            for (String key : props.stringPropertyNames()) {
                String value = props.getProperty(key);
                String name = key.startsWith("org.omg.CORBA.") ? key.substring(14) : key;
                if (name.equals("ORBInitialHost")) {
                    host = value;
                } else if (name.equals("ORBInitialPort")) {
                    port = value;
                } else if (name.startsWith("ORB") && !name.equals("ORBClass")
                        && !name.equals("ORBSingletonClass")) {
                    argv.add("-" + name);
                    argv.add(value);
                }
            }
            if (host != null || port != null) {
                argv.add("-ORBInitRef");
                argv.add("NameService=corbaloc::" + (host != null ? host : "localhost") + ":"
                        + (port != null ? port : "2809") + "/NameService");
            }
        }
        OrbImpl orb = new OrbImpl(Native.orbInit(argv.toArray(new String[0])));
        synchronized (LIVE) {
            LIVE.add(orb);
        }
        return orb;
    }

    /** The first ORB initialised and not destroyed, or null. */
    static OrbImpl defaultOrb() {
        synchronized (LIVE) {
            return LIVE.isEmpty() ? null : LIVE.get(0);
        }
    }

    long handle() {
        if (destroyed) {
            throw new org.omg.CORBA.OBJECT_NOT_EXIST("the ORB is destroyed", 0,
                    CompletionStatus.COMPLETED_NO);
        }
        return handle.get();
    }

    @Override
    public String[] list_initial_services() {
        try {
            String[] names = Native.orbListInitialServices(handle());
            List<String> all = new ArrayList<>(java.util.Arrays.asList(names));
            for (String local : LOCAL_SERVICES) {
                if (!all.contains(local)) {
                    all.add(local);
                }
            }
            return all.toArray(new String[0]);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /** The names resolve_initial_references answers without the core. */
    private static final String[] LOCAL_SERVICES = {
        "RootPOA", "POACurrent", "ORBPolicyManager", "PolicyCurrent", "DynAnyFactory"
    };

    @Override
    public void register_initial_reference(String id, org.omg.CORBA.Object obj)
            throws InvalidName {
        for (String local : LOCAL_SERVICES) {
            if (local.equals(id)) {
                throw new InvalidName();
            }
        }
        try {
            if (obj instanceof org.omg.CORBA.LocalObject) {
                Native.orbRegisterLocalInitialReference(handle(), id, obj);
                return;
            }
            ObjectDelegateImpl d = obj == null ? null : ObjectDelegateImpl.of(obj);
            try {
                Native.orbRegisterInitialReference(handle(), id, d == null ? 0 : d.handle());
            } finally {
                Reference.reachabilityFence(d);
            }
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public org.omg.CORBA.Policy create_policy(int type, org.omg.CORBA.Any val)
            throws org.omg.CORBA.PolicyError {
        if (!(val instanceof AnyImpl)) {
            throw new org.omg.CORBA.BAD_PARAM("not an any of rustorb", 0,
                    CompletionStatus.COMPLETED_NO);
        }
        AnyImpl any = (AnyImpl) val;
        try {
            return Policies.make(type, Native.orbCreatePolicy(handle(), type, any.handle()));
        } finally {
            Reference.reachabilityFence(this);
            Reference.reachabilityFence(any);
        }
    }

    @Override
    public org.omg.CORBA.Object resolve_initial_references(String object_name)
            throws InvalidName {
        if ("RootPOA".equals(object_name)) {
            try {
                return PoaImpl.of(Native.orbRootPoa(handle()));
            } finally {
                Reference.reachabilityFence(this);
            }
        }
        if ("POACurrent".equals(object_name)) {
            return PoaCurrentImpl.INSTANCE;
        }
        if ("ORBPolicyManager".equals(object_name)) {
            return new PolicyManagerImpl.OfOrb(this);
        }
        if ("PolicyCurrent".equals(object_name)) {
            return PolicyManagerImpl.Current.INSTANCE;
        }
        if ("DynAnyFactory".equals(object_name)) {
            return DynAnyFactoryImpl.INSTANCE;
        }
        try {
            Object local = Native.orbResolveLocalInitialReference(handle(), object_name);
            if (local != null) {
                return (org.omg.CORBA.Object) local;
            }
        } finally {
            Reference.reachabilityFence(this);
        }
        try {
            return ObjectStub.of(Native.orbResolveInitialReferences(handle(), object_name), this);
        } catch (InvalidName e) {
            throw e;
        } catch (org.omg.CORBA.UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public String object_to_string(org.omg.CORBA.Object obj) {
        ObjectDelegateImpl d = obj == null ? null : ObjectDelegateImpl.of(obj);
        try {
            return Native.orbObjectToString(handle(), d == null ? 0 : d.handle());
        } finally {
            Reference.reachabilityFence(this);
            Reference.reachabilityFence(d);
        }
    }

    @Override
    public org.omg.CORBA.Object string_to_object(String str) {
        try {
            return ObjectStub.of(Native.orbStringToObject(handle(), str), this);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /** Runs the ORB until shutdown, with the requests of MAIN_THREAD_MODEL
     * POAs. */
    @Override
    public void run() {
        try {
            Native.orbRun(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public void shutdown(boolean wait_for_completion) {
        try {
            Native.orbShutdown(handle(), wait_for_completion);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public boolean work_pending() {
        try {
            return Native.orbWorkPending(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public void perform_work() {
        try {
            Native.orbPerformWork(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public void destroy() {
        if (destroyed) {
            return;
        }
        try {
            Native.orbDestroy(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
        destroyed = true;
        synchronized (LIVE) {
            LIVE.remove(this);
        }
    }

}
