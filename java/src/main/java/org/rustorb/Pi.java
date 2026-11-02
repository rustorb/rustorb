// Portable Interceptors and IOP::Codec of the Java binding: the info
// objects the points get, PICurrent, the Codec, and the calls of the
// points (docs/portable-interceptors.md).
//
// An info object is only usable during its point: kept past it, it raises
// BAD_INV_ORDER (minor 14), and an ORBInitInfo kept past ORB.init raises
// OBJECT_NOT_EXIST (minor 10). Uses from other threads during the point
// wait for each other.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;
import java.util.ArrayList;
import java.util.List;
import java.util.Properties;

import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.NO_IMPLEMENT;
import org.omg.CORBA.NO_RESOURCES;
import org.omg.CORBA.OBJECT_NOT_EXIST;
import org.omg.CORBA.ParameterMode;
import org.omg.CORBA.Policy;
import org.omg.CORBA.SystemException;
import org.omg.CORBA.TypeCode;
import org.omg.Dynamic.Parameter;
import org.omg.IOP.Codec;
import org.omg.IOP.CodecFactory;
import org.omg.IOP.CodecFactoryPackage.UnknownEncoding;
import org.omg.IOP.CodecPackage.FormatMismatch;
import org.omg.IOP.CodecPackage.InvalidTypeForEncoding;
import org.omg.IOP.CodecPackage.TypeMismatch;
import org.omg.IOP.Encoding;
import org.omg.IOP.ServiceContext;
import org.omg.IOP.TaggedComponent;
import org.omg.IOP.TaggedProfile;
import org.omg.PortableInterceptor.ClientRequestInterceptor;
import org.omg.PortableInterceptor.ForwardRequest;
import org.omg.PortableInterceptor.IORInterceptor;
import org.omg.PortableInterceptor.Interceptor;
import org.omg.PortableInterceptor.InvalidSlot;
import org.omg.PortableInterceptor.ORBInitInfoPackage.DuplicateName;
import org.omg.PortableInterceptor.ORBInitInfoPackage.InvalidName;
import org.omg.PortableInterceptor.ORBInitializer;
import org.omg.PortableInterceptor.PolicyFactory;
import org.omg.PortableInterceptor.ServerRequestInterceptor;

final class Pi {
    private Pi() {}

    // The points, as the upcalls of Native name them.
    static final int SEND_REQUEST = 0;
    static final int RECEIVE_REPLY = 1;
    static final int RECEIVE_EXCEPTION = 2;
    static final int RECEIVE_OTHER = 3;
    static final int RECEIVE_REQUEST_SERVICE_CONTEXTS = 10;
    static final int RECEIVE_REQUEST = 11;
    static final int SEND_REPLY = 12;
    static final int SEND_EXCEPTION = 13;
    static final int SEND_OTHER = 14;
    static final int ESTABLISH_COMPONENTS = 20;
    static final int PRE_INIT = 30;
    static final int POST_INIT = 31;

    private static final int OK = 0;
    private static final int SYSTEM_EXCEPTION = 1;
    private static final String UNKNOWN = "IDL:omg.org/CORBA/UNKNOWN:1.0";
    private static final int OMG = 0x4f4d0000;

    /** The property prefix that names an ORBInitializer class. */
    static final String INITIALIZER_PREFIX = "org.omg.PortableInterceptor.ORBInitializerClass.";

    /** The ORB the interceptors of one ORB.init belong to, once it exists
     * (for the references the infos give). */
    static final class OrbBox {
        volatile OrbImpl orb;
    }

    /** The self of an interceptor or an initializer. */
    static final class Entry {
        final Object target;
        final OrbBox box;

        Entry(Object target, OrbBox box) {
            this.target = target;
            this.box = box;
        }
    }

    /**
     * The initializers ORB.init names: the classes of the properties
     * org.omg.PortableInterceptor.ORBInitializerClass.&lt;class&gt; of props,
     * then of the system properties. A class that cannot be loaded or made
     * is skipped, as the specification says.
     */
    static List<ORBInitializer> initializers(Properties props) {
        List<String> names = new ArrayList<>();
        for (Properties p : new Properties[] {props, systemProperties()}) {
            if (p == null) {
                continue;
            }
            for (String key : p.stringPropertyNames()) {
                if (key.startsWith(INITIALIZER_PREFIX)) {
                    String name = key.substring(INITIALIZER_PREFIX.length());
                    if (!name.isEmpty() && !names.contains(name)) {
                        names.add(name);
                    }
                }
            }
        }
        List<ORBInitializer> inits = new ArrayList<>();
        ClassLoader loader = Thread.currentThread().getContextClassLoader();
        for (String name : names) {
            try {
                Class<?> c = Class.forName(name, true,
                        loader != null ? loader : Pi.class.getClassLoader());
                inits.add((ORBInitializer) c.getDeclaredConstructor().newInstance());
            } catch (ReflectiveOperationException | ClassCastException | LinkageError e) {
                // skipped
            }
        }
        return inits;
    }

    private static Properties systemProperties() {
        try {
            return System.getProperties();
        } catch (SecurityException e) {
            return null;
        }
    }

    // --- the points ----------------------------------------------------------------

    /** Runs a point with the info handle h: RORB_OK, or a status after
     * rorb_pi_error_*. */
    static int point(int which, Object self, long h, long error) {
        if (!(self instanceof Entry)) {
            Native.piErrorSystem(error, UNKNOWN, 0, CompletionStatus._COMPLETED_MAYBE);
            return SYSTEM_EXCEPTION;
        }
        Entry e = (Entry) self;
        Info info = null;
        try {
            switch (which) {
                case SEND_REQUEST:
                case RECEIVE_REPLY:
                case RECEIVE_EXCEPTION:
                case RECEIVE_OTHER: {
                    ClientInfo ri = new ClientInfo(h, e.box);
                    info = ri;
                    ClientRequestInterceptor ci = (ClientRequestInterceptor) e.target;
                    if (which == SEND_REQUEST) {
                        ci.send_request(ri);
                    } else if (which == RECEIVE_REPLY) {
                        ci.receive_reply(ri);
                    } else if (which == RECEIVE_EXCEPTION) {
                        ci.receive_exception(ri);
                    } else {
                        ci.receive_other(ri);
                    }
                    break;
                }
                case ESTABLISH_COMPONENTS: {
                    IorInfo ii = new IorInfo(h);
                    info = ii;
                    ((IORInterceptor) e.target).establish_components(ii);
                    break;
                }
                case PRE_INIT:
                case POST_INIT: {
                    InitInfo ii = new InitInfo(h, e.box);
                    info = ii;
                    if (which == PRE_INIT) {
                        ((ORBInitializer) e.target).pre_init(ii);
                    } else {
                        ((ORBInitializer) e.target).post_init(ii);
                    }
                    break;
                }
                default: {
                    ServerInfo ri = new ServerInfo(h, e.box);
                    info = ri;
                    ServerRequestInterceptor si = (ServerRequestInterceptor) e.target;
                    if (which == RECEIVE_REQUEST_SERVICE_CONTEXTS) {
                        si.receive_request_service_contexts(ri);
                    } else if (which == RECEIVE_REQUEST) {
                        si.receive_request(ri);
                    } else if (which == SEND_REPLY) {
                        si.send_reply(ri);
                    } else if (which == SEND_EXCEPTION) {
                        si.send_exception(ri);
                    } else {
                        si.send_other(ri);
                    }
                    break;
                }
            }
            return OK;
        } catch (ForwardRequest f) {
            try {
                if (f.forward == null) {
                    throw new BAD_PARAM("ForwardRequest without a reference", 0,
                            CompletionStatus.COMPLETED_NO);
                }
                ObjectDelegateImpl d = ObjectDelegateImpl.of(f.forward);
                try {
                    Native.piErrorForward(error, d.handle());
                } finally {
                    Reference.reachabilityFence(d);
                }
            } catch (SystemException s) {
                system(error, s);
            }
            return SYSTEM_EXCEPTION;
        } catch (SystemException s) {
            system(error, s);
            return SYSTEM_EXCEPTION;
        } catch (Throwable t) {
            // Only system exceptions and ForwardRequest may leave a point.
            Native.piErrorSystem(error, UNKNOWN, 0, CompletionStatus._COMPLETED_MAYBE);
            return SYSTEM_EXCEPTION;
        } finally {
            if (info != null) {
                info.close();
            }
        }
    }

    private static void system(long error, SystemException s) {
        Native.piErrorSystem(error, SystemExceptions.id(s), s.minor,
                s.completed == null ? CompletionStatus._COMPLETED_MAYBE : s.completed.value());
    }

    /** Interceptor.destroy, when the ORB is destroyed. */
    static void destroy(Object self) {
        if (self instanceof Entry && ((Entry) self).target instanceof Interceptor) {
            try {
                ((Interceptor) ((Entry) self).target).destroy();
            } catch (RuntimeException e) {
                System.err.println("rustorb: exception in Interceptor.destroy: " + e);
            }
        }
    }

    // --- the info objects -------------------------------------------------------------

    /** The handle a point lends, until close. */
    abstract static class Info extends org.omg.CORBA.LocalObject {
        private static final long serialVersionUID = 1L;
        private long handle;

        Info(long handle) {
            this.handle = handle;
        }

        /** The handle, while the point runs (call with the lock held). */
        final long h() {
            if (handle == 0) {
                throw gone();
            }
            return handle;
        }

        RuntimeException gone() {
            return new BAD_INV_ORDER("the interception point is over", OMG | 14,
                    CompletionStatus.COMPLETED_NO);
        }

        final synchronized void close() {
            handle = 0;
        }
    }

    private static NO_RESOURCES notAvailable() {
        return new NO_RESOURCES("not available", OMG | 1, CompletionStatus.COMPLETED_NO);
    }

    private static Any any(long h) {
        return AnyImpl.of(h);
    }

    private static long anyHandle(Any a) {
        if (a == null) {
            throw new BAD_PARAM("null any", 0, CompletionStatus.COMPLETED_NO);
        }
        return AnyImpl.cast(a).handle();
    }

    private static Parameter[] parameters(long[] v) {
        Parameter[] p = new Parameter[v.length / 2];
        for (int i = 0; i < p.length; i++) {
            p[i] = new Parameter(any(v[2 * i]), ParameterMode.from_int((int) v[2 * i + 1]));
        }
        return p;
    }

    private static TypeCode[] typeCodes(long[] v) {
        TypeCode[] t = new TypeCode[v.length];
        for (int i = 0; i < v.length; i++) {
            t[i] = TypeCodeImpl.of(v[i]);
        }
        return t;
    }

    private static byte[] data(byte[] v) {
        if (v == null) {
            throw new BAD_PARAM("null data", 0, CompletionStatus.COMPLETED_NO);
        }
        return v;
    }

    /** What ClientRequestInfo and ServerRequestInfo share. */
    abstract static class RequestInfoBase extends Info {
        private static final long serialVersionUID = 1L;
        private final boolean server;
        final transient OrbBox box;

        RequestInfoBase(long handle, boolean server, OrbBox box) {
            super(handle);
            this.server = server;
            this.box = box;
        }

        org.omg.CORBA.Object object(long h) {
            return ObjectStub.of(h, box.orb);
        }

        public synchronized int request_id() {
            return Native.riRequestId(server, h());
        }

        public synchronized String operation() {
            return Native.riOperation(server, h());
        }

        public synchronized Parameter[] arguments() {
            return parameters(Native.riArguments(server, h()));
        }

        public synchronized TypeCode[] exceptions() {
            return typeCodes(Native.riExceptions(server, h()));
        }

        public synchronized String[] contexts() {
            h();
            throw notAvailable();
        }

        public synchronized String[] operation_context() {
            h();
            throw notAvailable();
        }

        public synchronized Any result() {
            return any(Native.riResult(server, h()));
        }

        public synchronized boolean response_expected() {
            return Native.riResponseExpected(server, h());
        }

        public synchronized short sync_scope() {
            return Native.riSyncScope(server, h());
        }

        public synchronized short reply_status() {
            return Native.riReplyStatus(server, h());
        }

        public synchronized org.omg.CORBA.Object forward_reference() {
            return object(Native.riForwardReference(server, h()));
        }

        public synchronized Any get_slot(int id) throws InvalidSlot {
            return any(Native.riGetSlot(server, h(), id));
        }

        public synchronized ServiceContext get_request_service_context(int id) {
            return new ServiceContext(id, Native.riGetRequestServiceContext(server, h(), id));
        }

        public synchronized ServiceContext get_reply_service_context(int id) {
            return new ServiceContext(id, Native.riGetReplyServiceContext(server, h(), id));
        }
    }

    static final class ClientInfo extends RequestInfoBase
            implements org.omg.PortableInterceptor.ClientRequestInfo {
        private static final long serialVersionUID = 1L;
        private static final String[] IDS = {
            "IDL:omg.org/PortableInterceptor/ClientRequestInfo:1.0",
            "IDL:omg.org/PortableInterceptor/RequestInfo:1.0"
        };

        ClientInfo(long handle, OrbBox box) {
            super(handle, false, box);
        }

        public String[] _ids() {
            return IDS.clone();
        }

        @Override
        public synchronized org.omg.CORBA.Object target() {
            return object(Native.criTarget(h()));
        }

        @Override
        public synchronized org.omg.CORBA.Object effective_target() {
            return object(Native.criEffectiveTarget(h()));
        }

        @Override
        public synchronized TaggedProfile effective_profile() {
            int[] tag = new int[1];
            byte[] data = Native.criEffectiveProfile(h(), tag);
            return new TaggedProfile(tag[0], data);
        }

        @Override
        public synchronized Any received_exception() {
            return any(Native.criReceivedException(h()));
        }

        @Override
        public synchronized String received_exception_id() {
            return Native.criReceivedExceptionId(h());
        }

        @Override
        public synchronized TaggedComponent get_effective_component(int id) {
            return new TaggedComponent(id, Native.criGetEffectiveComponent(h(), id));
        }

        @Override
        public synchronized TaggedComponent[] get_effective_components(int id) {
            List<Object> v = Native.criGetEffectiveComponents(h(), id);
            TaggedComponent[] c = new TaggedComponent[v.size() / 2];
            for (int i = 0; i < c.length; i++) {
                c[i] = new TaggedComponent((Integer) v.get(2 * i), (byte[]) v.get(2 * i + 1));
            }
            return c;
        }

        /** The policy of the type in effect, or null; INV_POLICY (minor 2)
         * for a type that is not a client side policy. */
        @Override
        public synchronized Policy get_request_policy(int type) {
            long[] found = Native.criGetRequestPolicy(h(), type);
            return found[0] == 0 ? null : Policies.make(type, found[1]);
        }

        @Override
        public synchronized void add_request_service_context(ServiceContext service_context,
                boolean replace) {
            if (service_context == null) {
                throw new BAD_PARAM("null service context", 0, CompletionStatus.COMPLETED_NO);
            }
            Native.criAddRequestServiceContext(h(), service_context.context_id,
                    data(service_context.context_data), replace);
        }
    }

    static final class ServerInfo extends RequestInfoBase
            implements org.omg.PortableInterceptor.ServerRequestInfo {
        private static final long serialVersionUID = 1L;
        private static final String[] IDS = {
            "IDL:omg.org/PortableInterceptor/ServerRequestInfo:1.0",
            "IDL:omg.org/PortableInterceptor/RequestInfo:1.0"
        };

        ServerInfo(long handle, OrbBox box) {
            super(handle, true, box);
        }

        public String[] _ids() {
            return IDS.clone();
        }

        @Override
        public synchronized Any sending_exception() {
            return any(Native.sriSendingException(h()));
        }

        @Override
        public synchronized String server_id() {
            return Native.sriServerId(h());
        }

        @Override
        public synchronized String orb_id() {
            return Native.sriOrbId(h());
        }

        @Override
        public synchronized String[] adapter_name() {
            return Native.sriAdapterName(h());
        }

        @Override
        public synchronized byte[] object_id() {
            return Native.sriObjectId(h());
        }

        @Override
        public synchronized byte[] adapter_id() {
            return Native.sriAdapterId(h());
        }

        @Override
        public synchronized String target_most_derived_interface() {
            return Native.sriTargetMostDerivedInterface(h());
        }

        @Override
        public synchronized Policy get_server_policy(int type) {
            return Policies.make(type, Native.sriGetServerPolicy(h(), type));
        }

        @Override
        public synchronized void set_slot(int id, Any data) throws InvalidSlot {
            Native.sriSetSlot(h(), id, anyHandle(data));
            Reference.reachabilityFence(data);
        }

        @Override
        public synchronized boolean target_is_a(String id) {
            if (id == null) {
                throw new BAD_PARAM("null id", 0, CompletionStatus.COMPLETED_NO);
            }
            return Native.sriTargetIsA(h(), id);
        }

        @Override
        public synchronized void add_reply_service_context(ServiceContext service_context,
                boolean replace) {
            if (service_context == null) {
                throw new BAD_PARAM("null service context", 0, CompletionStatus.COMPLETED_NO);
            }
            Native.sriAddReplyServiceContext(h(), service_context.context_id,
                    data(service_context.context_data), replace);
        }
    }

    static final class IorInfo extends Info implements org.omg.PortableInterceptor.IORInfo {
        private static final long serialVersionUID = 1L;
        private static final String[] IDS = {"IDL:omg.org/PortableInterceptor/IORInfo:1.0"};

        IorInfo(long handle) {
            super(handle);
        }

        public String[] _ids() {
            return IDS.clone();
        }

        @Override
        public synchronized Policy get_effective_policy(int type) {
            return Policies.make(type, Native.iorInfoGetEffectivePolicy(h(), type));
        }

        @Override
        public synchronized void add_ior_component(TaggedComponent a_component) {
            if (a_component == null) {
                throw new BAD_PARAM("null component", 0, CompletionStatus.COMPLETED_NO);
            }
            Native.iorInfoAddIorComponent(h(), a_component.tag, data(a_component.component_data));
        }

        /** BAD_PARAM (minor 29) but for TAG_INTERNET_IOP. */
        @Override
        public synchronized void add_ior_component_to_profile(TaggedComponent a_component,
                int profile_id) {
            if (a_component == null) {
                throw new BAD_PARAM("null component", 0, CompletionStatus.COMPLETED_NO);
            }
            Native.iorInfoAddIorComponentToProfile(h(), a_component.tag,
                    data(a_component.component_data), profile_id);
        }
    }

    /** The initial references the ORB answers itself (not the core). */
    private static boolean ownReference(String id) {
        return OrbImpl.isLocalService(id);
    }

    static final class InitInfo extends Info implements org.omg.PortableInterceptor.ORBInitInfo {
        private static final long serialVersionUID = 1L;
        private static final String[] IDS = {"IDL:omg.org/PortableInterceptor/ORBInitInfo:1.0"};
        private final transient OrbBox box;

        InitInfo(long handle, OrbBox box) {
            super(handle);
            this.box = box;
        }

        @Override
        RuntimeException gone() {
            return new OBJECT_NOT_EXIST("the ORB is initialised", OMG | 10,
                    CompletionStatus.COMPLETED_NO);
        }

        public String[] _ids() {
            return IDS.clone();
        }

        /** The ORB being created, on a new handle (calls may not work yet). */
        private OrbImpl orb() {
            return OrbImpl.wrap(Native.initInfoOrb(h()));
        }

        @Override
        public synchronized String[] arguments() {
            return Native.initInfoArguments(h());
        }

        @Override
        public synchronized String orb_id() {
            return Native.initInfoOrbId(h());
        }

        @Override
        public synchronized CodecFactory codec_factory() {
            return new CodecFactoryImpl(Native.initInfoOrb(h()));
        }

        @Override
        public synchronized void register_initial_reference(String id, org.omg.CORBA.Object obj)
                throws InvalidName {
            if (id == null) {
                throw new BAD_PARAM("null id", 0, CompletionStatus.COMPLETED_NO);
            }
            try {
                if (obj instanceof org.omg.CORBA.LocalObject) {
                    orb().register_initial_reference(id, obj);
                    return;
                }
                ObjectDelegateImpl d = obj == null ? null : ObjectDelegateImpl.of(obj);
                try {
                    Native.initInfoRegisterInitialReference(h(), id, d == null ? 0 : d.handle());
                } finally {
                    Reference.reachabilityFence(d);
                }
            } catch (org.omg.CORBA.ORBPackage.InvalidName e) {
                throw new InvalidName();
            }
        }

        /** Only in post_init (BAD_INV_ORDER in pre_init). */
        @Override
        public synchronized org.omg.CORBA.Object resolve_initial_references(String id)
                throws InvalidName {
            if (id == null) {
                throw new BAD_PARAM("null id", 0, CompletionStatus.COMPLETED_NO);
            }
            long h = h();
            try {
                OrbImpl orb = null;
                if (!ownReference(id)) {
                    orb = orb();
                    if (Native.orbResolveLocalInitialReference(orb.handle(), id) == null) {
                        return ObjectStub.of(Native.initInfoResolveInitialReferences(h, id),
                                box.orb);
                    }
                }
                // The ORB's own: the core raises BAD_INV_ORDER in pre_init
                // for every name, and InvalidName for "" in post_init.
                try {
                    Native.initInfoResolveInitialReferences(h, "");
                } catch (org.omg.CORBA.ORBPackage.InvalidName e) {
                    // post_init
                }
                return (orb != null ? orb : orb()).resolve_initial_references(id);
            } catch (org.omg.CORBA.ORBPackage.InvalidName e) {
                throw new InvalidName();
            }
        }

        private void add(int kind, Interceptor interceptor) throws DuplicateName {
            if (interceptor == null) {
                throw new BAD_PARAM("null interceptor", 0, CompletionStatus.COMPLETED_NO);
            }
            String name = interceptor.name();
            try {
                Native.initInfoAddInterceptor(h(), kind, name, new Entry(interceptor, box));
            } catch (DuplicateName e) {
                throw new DuplicateName(name);
            }
        }

        @Override
        public synchronized void add_client_request_interceptor(
                ClientRequestInterceptor interceptor) throws DuplicateName {
            add(0, interceptor);
        }

        @Override
        public synchronized void add_server_request_interceptor(
                ServerRequestInterceptor interceptor) throws DuplicateName {
            add(1, interceptor);
        }

        @Override
        public synchronized void add_ior_interceptor(IORInterceptor interceptor)
                throws DuplicateName {
            add(2, interceptor);
        }

        @Override
        public synchronized int allocate_slot_id() {
            return Native.initInfoAllocateSlotId(h());
        }

        /** Not supported: NO_IMPLEMENT. */
        @Override
        public synchronized void register_policy_factory(int type, PolicyFactory policy_factory) {
            h();
            throw new NO_IMPLEMENT("policy factories are not supported", 0,
                    CompletionStatus.COMPLETED_NO);
        }
    }

    // --- PICurrent and the Codec ------------------------------------------------------

    /** The initial reference "PICurrent": the slots as the calling thread
     * sees them. */
    static final class CurrentImpl extends org.omg.PortableInterceptor._CurrentLocalBase {
        private static final long serialVersionUID = 1L;
        private final transient Owned handle;

        CurrentImpl(long handle) {
            this.handle = Owned.register(this, handle, Native::piCurrentRelease);
        }

        @Override
        public Any get_slot(int id) throws InvalidSlot {
            try {
                return any(Native.piCurrentGetSlot(handle.get(), id));
            } finally {
                Reference.reachabilityFence(this);
            }
        }

        @Override
        public void set_slot(int id, Any data) throws InvalidSlot {
            try {
                Native.piCurrentSetSlot(handle.get(), id, anyHandle(data));
            } finally {
                Reference.reachabilityFence(this);
                Reference.reachabilityFence(data);
            }
        }
    }

    /** The initial reference "CodecFactory". */
    static final class CodecFactoryImpl extends org.omg.IOP._CodecFactoryLocalBase {
        private static final long serialVersionUID = 1L;
        private final transient Owned orb;

        /** Takes an rorb_orb handle. */
        CodecFactoryImpl(long orb) {
            this.orb = Owned.register(this, orb, Native::orbRelease);
        }

        @Override
        public Codec create_codec(Encoding enc) throws UnknownEncoding {
            if (enc == null) {
                throw new BAD_PARAM("null encoding", 0, CompletionStatus.COMPLETED_NO);
            }
            try {
                return new CodecImpl(Native.orbCreateCodec(orb.get(), enc.format,
                        enc.major_version, enc.minor_version));
            } finally {
                Reference.reachabilityFence(this);
            }
        }
    }

    static final class CodecImpl extends org.omg.IOP._CodecLocalBase {
        private static final long serialVersionUID = 1L;
        private final transient Owned handle;

        CodecImpl(long handle) {
            this.handle = Owned.register(this, handle, Native::codecRelease);
        }

        private byte[] encode(Any data, boolean valueOnly) throws InvalidTypeForEncoding {
            try {
                return Native.codecEncode(handle.get(), anyHandle(data), valueOnly);
            } finally {
                Reference.reachabilityFence(this);
                Reference.reachabilityFence(data);
            }
        }

        private Any decode(byte[] data, TypeCode tc) throws FormatMismatch, TypeMismatch {
            TypeCodeImpl t = tc == null ? null : TypeCodeImpl.cast(tc);
            try {
                return any(Native.codecDecode(handle.get(), data(data), t == null ? 0 : t.handle()));
            } finally {
                Reference.reachabilityFence(this);
                Reference.reachabilityFence(t);
            }
        }

        @Override
        public byte[] encode(Any data) throws InvalidTypeForEncoding {
            return encode(data, false);
        }

        @Override
        public Any decode(byte[] data) throws FormatMismatch {
            try {
                return decode(data, null);
            } catch (TypeMismatch e) {
                throw new FormatMismatch();
            }
        }

        @Override
        public byte[] encode_value(Any data) throws InvalidTypeForEncoding {
            return encode(data, true);
        }

        @Override
        public Any decode_value(byte[] data, TypeCode tc) throws FormatMismatch, TypeMismatch {
            if (tc == null) {
                throw new BAD_PARAM("null TypeCode", 0, CompletionStatus.COMPLETED_NO);
            }
            return decode(data, tc);
        }
    }
}
