// The functions of the C ABI of librustorb_ffi (crates/rustorb-ffi) the
// binding uses, called with the FFM API (java.lang.foreign): one method for
// each, over the downcall handles of Ffi (generated from rustorb.h).
//
// Handles are longs (0 is NULL). A method that fails throws the CORBA
// exception of the C call; the classes of this package own the handles and
// release them. The callbacks of the C ABI are upcall stubs of the static
// methods at the end, which call Bridge.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import static java.lang.foreign.ValueLayout.JAVA_BYTE;
import static java.lang.foreign.ValueLayout.JAVA_CHAR;
import static java.lang.foreign.ValueLayout.JAVA_INT;
import static java.lang.foreign.ValueLayout.JAVA_LONG;
import static java.lang.foreign.ValueLayout.JAVA_SHORT;
import static org.rustorb.Scratch.getInt;
import static org.rustorb.Scratch.getLong;
import static org.rustorb.Scratch.putLong;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.MemorySegment;
import java.lang.invoke.MethodHandles;
import java.lang.ref.WeakReference;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;

final class Native {
    private Native() {}

    // rorb_status
    private static final int OK = 0;
    private static final int SYSTEM_EXCEPTION = 1;
    private static final int USER_EXCEPTION = 2;
    private static final int LOCAL_EXCEPTION = 3;
    private static final int CALLBACK_FAILED = 4;

    private static final String BAD_PARAM = "IDL:omg.org/CORBA/BAD_PARAM:1.0";
    private static final String UNKNOWN = "IDL:omg.org/CORBA/UNKNOWN:1.0";

    /** sizeof(rorb_exception) */
    private static final long EXCEPTION = 32;
    /** sizeof(rorb_bytes) */
    private static final long BYTES = 16;

    static {
        int abi;
        try {
            abi = (int) Ffi.rorb_abi_version.invokeExact();
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
        if (abi != 1) {
            throw new UnsatisfiedLinkError("librustorb_ffi has ABI version " + abi + ", not 1");
        }
        try {
            Ffi.rorb_set_value_type_resolver.invokeExact(Upcalls.RESOLVE, 0L);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    /** Forces the library to load (the static initializer). */
    static void ensureLoaded() {}

    // --- calls with an rorb_exception ---------------------------------------

    /** Throws the exception of a failed call and clears exc. A user
     * exception or a failed callback (whose error Java keeps) throws
     * nothing; the caller then returns what the results hold (0). */
    private static void check(int status, long exc) {
        if (status == OK) {
            return;
        }
        Throwable t = null;
        if (status == SYSTEM_EXCEPTION) {
            long id = getLong(exc);
            String repositoryId = id == 0 ? UNKNOWN : Library.utf8(id, getLong(exc + 8));
            t = Bridge.systemException(repositoryId, getInt(exc + 16), getInt(exc + 20));
        } else if (status == LOCAL_EXCEPTION) {
            t = Bridge.localException(getInt(exc + 24), getInt(exc + 28));
        }
        try {
            Ffi.rorb_exception_clear.invokeExact(exc);
        } catch (Throwable u) {
            throw Library.rethrow(u);
        }
        if (t != null) {
            throw Library.sneaky(t);
        }
    }

    private static RuntimeException badParam() {
        return Bridge.systemException(BAD_PARAM, 0, 1);
    }

    /** A C call that reports through exc. */
    @FunctionalInterface
    private interface Call0 {
        int call(Scratch s, long exc) throws Throwable;
    }

    /** A C call that fills a result at out (8 zeroed bytes) and reports
     * through exc. */
    @FunctionalInterface
    private interface Call1 {
        int call(Scratch s, long out, long exc) throws Throwable;
    }

    private static void run(Call0 f) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long exc = s.alloc(EXCEPTION);
            check(f.call(s, exc), exc);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    /** The pointer or 64-bit value the call fills. */
    private static long word(Call1 f) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long out = s.word();
            long exc = s.alloc(EXCEPTION);
            check(f.call(s, out, exc), exc);
            return getLong(out);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    /** The 32-bit value the call fills. */
    private static int int32(Call1 f) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long out = s.word();
            long exc = s.alloc(EXCEPTION);
            check(f.call(s, out, exc), exc);
            return getInt(out);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    /** The 16-bit value the call fills. */
    private static short int16(Call1 f) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long out = s.word();
            long exc = s.alloc(EXCEPTION);
            check(f.call(s, out, exc), exc);
            return Library.MEMORY.get(JAVA_SHORT, out);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    /** The bytes of the rorb_bytes the call fills (out is an rorb_bytes). */
    private static byte[] bytes(Call1 f) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long out = s.alloc(BYTES);
            long exc = s.alloc(EXCEPTION);
            int status = f.call(s, out, exc);
            check(status, exc);
            return takeBytes(out);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    /** The bytes of the rorb_bytes at p, freed. */
    private static byte[] takeBytes(long p) throws Throwable {
        byte[] b = Library.bytes(getLong(p), getLong(p + 8));
        Ffi.rorb_bytes_free.invokeExact(Library.MEMORY.asSlice(p, BYTES));
        return b;
    }

    private static String utf8(Call1 f) {
        return new String(bytes(f), StandardCharsets.UTF_8);
    }

    /** s as UTF-8 in scratch memory; its length is s.lastLength. */
    private static long str(Scratch s, String v) {
        if (v == null) {
            throw badParam();
        }
        return s.utf8(v);
    }

    /** The strings v: the address of the pointers in [0], of the lengths in
     * [1]. */
    private static long[] strs(Scratch s, String[] v, int n) {
        if (v == null) {
            throw badParam();
        }
        long ptrs = s.alloc(8L * n);
        long lens = s.alloc(8L * n);
        for (int i = 0; i < n; i++) {
            putLong(ptrs + 8L * i, str(s, v[i]));
            putLong(lens + 8L * i, s.lastLength);
        }
        return new long[] {ptrs, lens};
    }

    private static long id(Scratch s, byte[] id) {
        if (id == null) {
            throw badParam();
        }
        return s.bytes(id);
    }

    /** A string the C ABI returned at p, its length at lenp. */
    private static String cstring(long p, long lenp) {
        return p == 0 ? "" : Library.utf8(p, getLong(lenp));
    }

    // --- ORB -------------------------------------------------------------

    /** The ORB options are the -ORB... arguments; the others are ignored. */
    static long orbInit(String[] args) {
        return word((s, out, exc) -> {
            List<String> all = new ArrayList<>();
            all.add("java");
            if (args != null) {
                for (String a : args) {
                    if (a == null) {
                        continue;
                    }
                    if (a.indexOf('\0') >= 0) {
                        throw badParam();
                    }
                    all.add(a);
                }
            }
            int n = all.size();
            long argv = s.alloc(8L * (n + 1));
            for (int i = 0; i < n; i++) {
                putLong(argv + 8L * i, s.utf8(all.get(i)));
            }
            long argc = s.word();
            Library.MEMORY.set(JAVA_INT, argc, n);
            return (int) Ffi.rorb_orb_init.invokeExact(argc, argv, out, exc);
        });
    }

    static void orbRelease(long orb) {
        try {
            Ffi.rorb_orb_release.invokeExact(orb);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long orbStringToObject(long orb, String str) {
        return word((s, out, exc) -> {
            long p = str(s, str);
            return (int) Ffi.rorb_orb_string_to_object.invokeExact(orb, p, s.lastLength, out, exc);
        });
    }

    static String orbObjectToString(long orb, long obj) {
        return utf8((s, out, exc) -> (int) Ffi.rorb_orb_object_to_string.invokeExact(orb, obj, out, exc));
    }

    static long orbResolveInitialReferences(long orb, String name)
            throws org.omg.CORBA.UserException {
        return word((s, out, exc) -> {
            long p = str(s, name);
            return (int) Ffi.rorb_orb_resolve_initial_references.invokeExact(orb, p, s.lastLength,
                    out, exc);
        });
    }

    static String[] orbListInitialServices(long orb) {
        List<String> names = new ArrayList<>();
        long ctx = CONTEXTS.add(names);
        try {
            Ffi.rorb_orb_list_initial_services.invokeExact(orb, ctx, Upcalls.PUSH_NAME);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            CONTEXTS.remove(ctx);
        }
        return names.toArray(new String[0]);
    }

    static long orbRootPoa(long orb) {
        return word((s, out, exc) -> (int) Ffi.rorb_orb_root_poa.invokeExact(orb, out, exc));
    }

    static void orbRun(long orb) {
        try {
            Ffi.rorb_orb_run.invokeExact(orb);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static boolean orbWorkPending(long orb) {
        return int32((s, out, exc) -> (int) Ffi.rorb_orb_work_pending.invokeExact(orb, out, exc)) != 0;
    }

    static void orbPerformWork(long orb) {
        run((s, exc) -> (int) Ffi.rorb_orb_perform_work.invokeExact(orb, exc));
    }

    static void orbShutdown(long orb, boolean wait) {
        run((s, exc) -> (int) Ffi.rorb_orb_shutdown.invokeExact(orb, wait ? 1 : 0, exc));
    }

    static void orbDestroy(long orb) {
        run((s, exc) -> (int) Ffi.rorb_orb_destroy.invokeExact(orb, exc));
    }

    static long defaultRootPoa() {
        return word((s, out, exc) -> (int) Ffi.rorb_default_root_poa.invokeExact(out, exc));
    }

    /** Timeouts in microseconds, as the C ABI (rorb_orb_set_call_timeout) has them. */
    static void orbSetCallTimeout(long orb, long us) {
        try {
            Ffi.rorb_orb_set_call_timeout.invokeExact(orb, us);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long orbCallTimeout(long orb) {
        try {
            return (long) Ffi.rorb_orb_call_timeout.invokeExact(orb);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void orbSetConnectTimeout(long orb, long us) {
        try {
            Ffi.rorb_orb_set_connect_timeout.invokeExact(orb, us);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long orbConnectTimeout(long orb) {
        try {
            return (long) Ffi.rorb_orb_connect_timeout.invokeExact(orb);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void setThreadCallTimeout(long us) {
        try {
            Ffi.rorb_set_thread_call_timeout.invokeExact(us);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long threadCallTimeout() {
        try {
            return (long) Ffi.rorb_thread_call_timeout.invokeExact();
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    // --- policies and registered initial references ------------------------

    static long orbCreatePolicy(long orb, int type, long any) throws org.omg.CORBA.PolicyError {
        return word((s, out, exc) -> (int) Ffi.rorb_orb_create_policy.invokeExact(orb, type, any,
                out, exc));
    }

    static void orbRegisterInitialReference(long orb, String name, long obj)
            throws org.omg.CORBA.ORBPackage.InvalidName {
        run((s, exc) -> {
            long p = str(s, name);
            return (int) Ffi.rorb_orb_register_initial_reference.invokeExact(orb, p, s.lastLength,
                    obj, exc);
        });
    }

    static void orbRegisterLocalInitialReference(long orb, String name, Object obj)
            throws org.omg.CORBA.ORBPackage.InvalidName {
        long held = LOCALS.add(obj);
        boolean handed = false;
        try {
            run((s, exc) -> {
                long p = str(s, name);
                return (int) Ffi.rorb_orb_register_local_initial_reference.invokeExact(orb, p,
                        s.lastLength, held, Upcalls.RELEASE_LOCAL, exc);
            });
            handed = true;
        } finally {
            if (!handed) {
                // The C side calls release unless the name was refused
                // before taking the object; forgetting it twice is fine.
                LOCALS.remove(held);
            }
        }
    }

    static Object orbResolveLocalInitialReference(long orb, String name) {
        Object[] found = new Object[1];
        long ctx = CONTEXTS.add(found);
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long p = str(s, name);
            int unused = (int) Ffi.rorb_orb_resolve_local_initial_reference.invokeExact(orb, p,
                    s.lastLength, ctx, Upcalls.COPY_LOCAL);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
            CONTEXTS.remove(ctx);
        }
        return found[0];
    }

    /** Collects the policies a C call reports into a flattened long[]. */
    @FunctionalInterface
    private interface EachPolicy {
        void call(Scratch s, long ctx) throws Throwable;
    }

    private static long[] policyArray(EachPolicy f) {
        List<Long> list = new ArrayList<>();
        long ctx = CONTEXTS.add(list);
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            f.call(s, ctx);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
            CONTEXTS.remove(ctx);
        }
        long[] a = new long[list.size()];
        for (int i = 0; i < a.length; i++) {
            a[i] = list.get(i);
        }
        return a;
    }

    /** The number of policies of a list (the longer array is cut). */
    private static int count(int[] types, long[] values) {
        if (types == null || values == null) {
            throw badParam();
        }
        return Math.min(types.length, values.length);
    }

    private static int[] nonNull(int[] types) {
        if (types == null) {
            throw badParam();
        }
        return types;
    }

    static void orbSetPolicyOverrides(long orb, int[] types, long[] values, boolean add) {
        int n = count(types, values);
        run((s, exc) -> (int) Ffi.rorb_orb_set_policy_overrides.invokeExact(orb, s.ints(types, n),
                s.longs(values, n), (long) n, add ? 1 : 0, exc));
    }

    static long[] orbPolicyOverrides(long orb, int[] types) {
        int[] t = nonNull(types);
        return policyArray((s, ctx) -> {
            Ffi.rorb_orb_get_policy_overrides.invokeExact(orb,
                    s.ints(t, t.length), (long) t.length, ctx, Upcalls.PUSH_POLICY);
        });
    }

    static void setThreadPolicyOverrides(int[] types, long[] values, boolean add) {
        int n = count(types, values);
        run((s, exc) -> (int) Ffi.rorb_set_thread_policy_overrides.invokeExact(s.ints(types, n),
                s.longs(values, n), (long) n, add ? 1 : 0, exc));
    }

    static long[] threadPolicyOverrides(int[] types) {
        int[] t = nonNull(types);
        return policyArray((s, ctx) -> {
            Ffi.rorb_thread_policy_overrides.invokeExact(
                    s.ints(t, t.length), (long) t.length, ctx, Upcalls.PUSH_POLICY);
        });
    }

    static long objectSetPolicyOverrides(long obj, int[] types, long[] values, boolean add) {
        int n = count(types, values);
        return word((s, out, exc) -> (int) Ffi.rorb_object_set_policy_overrides.invokeExact(obj,
                s.ints(types, n), s.longs(values, n), (long) n, add ? 1 : 0, out, exc));
    }

    static long[] objectPolicyOverrides(long obj, int[] types) {
        int[] t = nonNull(types);
        return policyArray((s, ctx) -> {
            Ffi.rorb_object_get_policy_overrides.invokeExact(obj,
                    s.ints(t, t.length), (long) t.length, ctx, Upcalls.PUSH_POLICY);
        });
    }

    /** The value of the client policy in value[0]; false if there is none. */
    static boolean objectClientPolicy(long obj, int type, long[] value) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long v = s.word();
            int found = (int) Ffi.rorb_object_get_client_policy.invokeExact(obj, type, v);
            if (found != 0) {
                value[0] = getLong(v);
            }
            return found != 0;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static boolean objectValidateConnection(long obj) {
        return int32((s, out, exc) -> (int) Ffi.rorb_object_validate_connection.invokeExact(obj,
                out, exc)) != 0;
    }

    // --- object references -----------------------------------------------

    static long objectClone(long obj) {
        try {
            return (long) Ffi.rorb_object_clone.invokeExact(obj);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void objectRelease(long obj) {
        try {
            Ffi.rorb_object_release.invokeExact(obj);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long objectOrb(long obj) {
        try {
            return (long) Ffi.rorb_object_orb.invokeExact(obj);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static String objectTypeId(long obj) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long len = s.word();
            long p = (long) Ffi.rorb_object_type_id.invokeExact(obj, len);
            return cstring(p, len);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static boolean objectIsA(long obj, String id) {
        return int32((s, out, exc) -> {
            long p = str(s, id);
            return (int) Ffi.rorb_object_is_a.invokeExact(obj, p, s.lastLength, out, exc);
        }) != 0;
    }

    static boolean objectNonExistent(long obj) {
        return int32((s, out, exc) -> (int) Ffi.rorb_object_non_existent.invokeExact(obj, out,
                exc)) != 0;
    }

    static boolean objectIsEquivalent(long a, long b) {
        try {
            return (int) Ffi.rorb_object_is_equivalent.invokeExact(a, b) != 0;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static int objectHash(long obj, int maximum) {
        try {
            return (int) Ffi.rorb_object_hash.invokeExact(obj, maximum <= 0 ? -1 : maximum);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void objectSetCallTimeout(long obj, long us) {
        try {
            Ffi.rorb_object_set_call_timeout.invokeExact(obj, us);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long objectCallTimeout(long obj) {
        try {
            return (long) Ffi.rorb_object_call_timeout.invokeExact(obj);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    /** Operation names as UTF-8 for the C ABI, made once each. */
    private static final ConcurrentHashMap<String, MemorySegment> OPERATIONS =
            new ConcurrentHashMap<>();

    private static MemorySegment operation(String op) {
        if (op == null) {
            throw badParam();
        }
        MemorySegment m = OPERATIONS.get(op);
        if (m == null) {
            byte[] b = op.getBytes(StandardCharsets.UTF_8);
            m = Arena.global().allocate(b.length + 1L);
            MemorySegment.copy(b, 0, m, JAVA_BYTE, 0, b.length);
            m = m.asSlice(0, b.length);
            if (OPERATIONS.size() < 4096) {
                MemorySegment had = OPERATIONS.putIfAbsent(op, m);
                if (had != null) {
                    m = had;
                }
            }
        }
        return m;
    }

    /** sizeof(rorb_invocation) */
    private static final long INVOCATION = 64;

    /** An rorb_invocation of op with the callbacks given (0 for none). */
    private static long invocation(Scratch s, String op, boolean oneway, long timeoutUs, long ctx,
            long write, long read, long readException) {
        MemorySegment name = operation(op);
        long inv = s.alloc(INVOCATION);
        putLong(inv, name.address());
        putLong(inv + 8, name.byteSize());
        Library.MEMORY.set(JAVA_INT, inv + 16, oneway ? 1 : 0);
        putLong(inv + 24, timeoutUs);
        putLong(inv + 32, ctx);
        putLong(inv + 40, write);
        putLong(inv + 48, read);
        putLong(inv + 56, readException);
        return inv;
    }

    /** 0 (done), 2 (a user exception, taken by call) or 4 (call.error). */
    static int objectInvoke(long obj, String op, boolean oneway, long timeoutUs, Call call) {
        long ctx = CALLS.add(call);
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long inv = invocation(s, op, oneway, timeoutUs, ctx, Upcalls.CALL_WRITE,
                    Upcalls.CALL_READ, Upcalls.CALL_READ_EXCEPTION);
            long exc = s.alloc(EXCEPTION);
            int st = (int) Ffi.rorb_object_invoke.invokeExact(obj, inv, exc);
            if (st == USER_EXCEPTION || st == CALLBACK_FAILED) {
                return st;
            }
            check(st, exc);
            return st;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
            CALLS.remove(ctx);
        }
    }

    // --- output streams ----------------------------------------------------

    static void outBoolean(long out, boolean v) {
        try {
            Ffi.rorb_out_write_boolean.invokeExact(out, v ? 1 : 0);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outOctet(long out, byte v) {
        try {
            Ffi.rorb_out_write_octet.invokeExact(out, v);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outChar(long out, char v) {
        try {
            Ffi.rorb_out_write_char.invokeExact(out, (byte) v);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outWchar(long out, char v) {
        try {
            Ffi.rorb_out_write_wchar.invokeExact(out, (short) v);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outShort(long out, short v) {
        try {
            Ffi.rorb_out_write_short.invokeExact(out, v);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outUshort(long out, short v) {
        try {
            Ffi.rorb_out_write_ushort.invokeExact(out, v);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outLong(long out, int v) {
        try {
            Ffi.rorb_out_write_long.invokeExact(out, v);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outUlong(long out, int v) {
        try {
            Ffi.rorb_out_write_ulong.invokeExact(out, v);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outLonglong(long out, long v) {
        try {
            Ffi.rorb_out_write_longlong.invokeExact(out, v);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outUlonglong(long out, long v) {
        try {
            Ffi.rorb_out_write_ulonglong.invokeExact(out, v);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outFloat(long out, float v) {
        try {
            Ffi.rorb_out_write_float.invokeExact(out, v);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outDouble(long out, double v) {
        try {
            Ffi.rorb_out_write_double.invokeExact(out, v);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    /** A double, as a long double. */
    static void outLongdouble(long out, double v) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long hi = s.word();
            long lo = s.word();
            Ffi.rorb_longdouble_from_double.invokeExact(v, hi, lo);
            Ffi.rorb_out_write_longdouble.invokeExact(out, getLong(hi), getLong(lo));
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static void outString(long out, String v) {
        if (v == null) {
            throw badParam();
        }
        byte[] b = v.getBytes(StandardCharsets.UTF_8);
        try {
            Ffi.rorb_out_write_string.invokeExact(out, MemorySegment.ofArray(b), (long) b.length);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outWstring(long out, String v) {
        if (v == null) {
            throw badParam();
        }
        char[] c = v.toCharArray();
        try {
            Ffi.rorb_out_write_wstring.invokeExact(out, MemorySegment.ofArray(c), (long) c.length);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outObject(long out, long obj) {
        try {
            Ffi.rorb_out_write_object.invokeExact(out, obj);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outTypeCode(long out, long tc) {
        try {
            Ffi.rorb_out_write_typecode.invokeExact(out, tc);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void outAny(long out, long any) {
        try {
            Ffi.rorb_out_write_any.invokeExact(out, any);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    private static void outArray(long out, int kind, MemorySegment v, long count) {
        try {
            Ffi.rorb_out_write_array.invokeExact(out, kind, v, count);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    private static void bounds(int length, int off, int len) {
        if (off < 0 || len < 0 || off > length - len) {
            throw new ArrayIndexOutOfBoundsException("offset " + off + ", length " + len
                    + " of an array of " + length);
        }
    }

    static void outArrayByte(long out, int kind, byte[] v, int off, int len) {
        bounds(v.length, off, len);
        outArray(out, kind, MemorySegment.ofArray(v).asSlice(off, len), len);
    }

    static void outArrayShort(long out, int kind, short[] v, int off, int len) {
        bounds(v.length, off, len);
        outArray(out, kind, MemorySegment.ofArray(v).asSlice(2L * off, 2L * len), len);
    }

    static void outArrayInt(long out, int kind, int[] v, int off, int len) {
        bounds(v.length, off, len);
        outArray(out, kind, MemorySegment.ofArray(v).asSlice(4L * off, 4L * len), len);
    }

    static void outArrayLong(long out, int kind, long[] v, int off, int len) {
        bounds(v.length, off, len);
        outArray(out, kind, MemorySegment.ofArray(v).asSlice(8L * off, 8L * len), len);
    }

    static void outArrayFloat(long out, int kind, float[] v, int off, int len) {
        bounds(v.length, off, len);
        outArray(out, kind, MemorySegment.ofArray(v).asSlice(4L * off, 4L * len), len);
    }

    static void outArrayDouble(long out, int kind, double[] v, int off, int len) {
        bounds(v.length, off, len);
        outArray(out, kind, MemorySegment.ofArray(v).asSlice(8L * off, 8L * len), len);
    }

    static boolean outFailed(long out) {
        try {
            return (int) Ffi.rorb_out_failed.invokeExact(out) != 0;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    // --- input streams -----------------------------------------------------

    static boolean inBoolean(long in) {
        try {
            return (int) Ffi.rorb_in_read_boolean.invokeExact(in) != 0;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static byte inOctet(long in) {
        try {
            return (byte) Ffi.rorb_in_read_octet.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static char inChar(long in) {
        try {
            return (char) ((byte) Ffi.rorb_in_read_char.invokeExact(in) & 0xff);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static char inWchar(long in) {
        try {
            return (char) (short) Ffi.rorb_in_read_wchar.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static short inShort(long in) {
        try {
            return (short) Ffi.rorb_in_read_short.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static short inUshort(long in) {
        try {
            return (short) Ffi.rorb_in_read_ushort.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static int inLong(long in) {
        try {
            return (int) Ffi.rorb_in_read_long.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static int inUlong(long in) {
        try {
            return (int) Ffi.rorb_in_read_ulong.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long inLonglong(long in) {
        try {
            return (long) Ffi.rorb_in_read_longlong.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long inUlonglong(long in) {
        try {
            return (long) Ffi.rorb_in_read_ulonglong.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static float inFloat(long in) {
        try {
            return (float) Ffi.rorb_in_read_float.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static double inDouble(long in) {
        try {
            return (double) Ffi.rorb_in_read_double.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    /** A long double, as the nearest double. */
    static double inLongdouble(long in) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long hi = s.word();
            long lo = s.word();
            Ffi.rorb_in_read_longdouble.invokeExact(in, hi, lo);
            return (double) Ffi.rorb_longdouble_to_double.invokeExact(getLong(hi), getLong(lo));
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static String inString(long in) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long len = s.word();
            long p = (long) Ffi.rorb_in_read_string.invokeExact(in, len);
            return cstring(p, len);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static String inWstring(long in) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long len = s.word();
            long p = (long) Ffi.rorb_in_read_wstring.invokeExact(in, len);
            int n = Math.toIntExact(getLong(len));
            if (p == 0 || n == 0) {
                return "";
            }
            char[] c = new char[n];
            MemorySegment.copy(Library.MEMORY, JAVA_CHAR, p, c, 0, n);
            return new String(c);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static long inObject(long in) {
        try {
            return (long) Ffi.rorb_in_read_object.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long inTypeCode(long in) {
        try {
            return (long) Ffi.rorb_in_read_typecode.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long inAny(long in) {
        try {
            return (long) Ffi.rorb_in_read_any.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    private static void inArray(long in, int kind, MemorySegment v, long count) {
        try {
            Ffi.rorb_in_read_array.invokeExact(in, kind, v, count);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void inArrayByte(long in, int kind, byte[] v, int off, int len) {
        bounds(v.length, off, len);
        inArray(in, kind, MemorySegment.ofArray(v).asSlice(off, len), len);
    }

    static void inArrayShort(long in, int kind, short[] v, int off, int len) {
        bounds(v.length, off, len);
        inArray(in, kind, MemorySegment.ofArray(v).asSlice(2L * off, 2L * len), len);
    }

    static void inArrayInt(long in, int kind, int[] v, int off, int len) {
        bounds(v.length, off, len);
        inArray(in, kind, MemorySegment.ofArray(v).asSlice(4L * off, 4L * len), len);
    }

    static void inArrayLong(long in, int kind, long[] v, int off, int len) {
        bounds(v.length, off, len);
        inArray(in, kind, MemorySegment.ofArray(v).asSlice(8L * off, 8L * len), len);
    }

    static void inArrayFloat(long in, int kind, float[] v, int off, int len) {
        bounds(v.length, off, len);
        inArray(in, kind, MemorySegment.ofArray(v).asSlice(4L * off, 4L * len), len);
    }

    static void inArrayDouble(long in, int kind, double[] v, int off, int len) {
        bounds(v.length, off, len);
        inArray(in, kind, MemorySegment.ofArray(v).asSlice(8L * off, 8L * len), len);
    }

    static int inLength(long in, int minElementSize) {
        try {
            int n = (int) Ffi.rorb_in_read_length.invokeExact(in, (long) Math.max(minElementSize, 0));
            // Java arrays hold at most Integer.MAX_VALUE elements.
            if (n < 0) {
                Ffi.rorb_in_fail.invokeExact(in);
                return 0;
            }
            return n;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void inFail(long in) {
        try {
            Ffi.rorb_in_fail.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static boolean inFailed(long in) {
        try {
            return (int) Ffi.rorb_in_failed.invokeExact(in) != 0;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    /** Throws the error of the stream, if it has one. */
    static void inCheck(long in) {
        run((s, exc) -> (int) Ffi.rorb_in_error.invokeExact(in, exc));
    }

    static void outAnyValue(long out, long any) {
        try {
            Ffi.rorb_out_write_any_value.invokeExact(out, any);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    /** 0 after an error. */
    static long inAnyValue(long in, long tc) {
        try {
            return (long) Ffi.rorb_in_read_any_value.invokeExact(in, tc);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    // --- owned input streams (OwnedInputStream) -----------------------------

    /** The reply stream; 0 for a oneway call or when call.write failed. A
     * user exception is a stream with an exception id. */
    static long objectInvokeStream(long obj, String op, boolean oneway, long timeoutUs,
            Call call) {
        long ctx = CALLS.add(call);
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long inv = invocation(s, op, oneway, timeoutUs, ctx, Upcalls.CALL_WRITE, 0, 0);
            long reply = s.word();
            long exc = s.alloc(EXCEPTION);
            int st = (int) Ffi.rorb_object_invoke_stream.invokeExact(obj, inv, reply, exc);
            if (st == USER_EXCEPTION) {
                return getLong(reply);
            }
            if (st == CALLBACK_FAILED) {
                return 0;
            }
            check(st, exc);
            return getLong(reply);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
            CALLS.remove(ctx);
        }
    }

    /** The repository id of the user exception of a reply stream, or null. */
    static String inExceptionId(long in) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long len = s.word();
            long p = (long) Ffi.rorb_in_exception_id.invokeExact(in, len);
            return p == 0 ? null : Library.utf8(p, getLong(len));
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    /** A stream over what call.write writes; 0 when it failed. */
    static long inNew(long orb, Call call) {
        long ctx = CALLS.add(call);
        try {
            return word((s, out, exc) -> {
                int st = (int) Ffi.rorb_in_new.invokeExact(Upcalls.CALL_WRITE, ctx, orb, out, exc);
                return st == CALLBACK_FAILED ? OK : st;
            });
        } finally {
            CALLS.remove(ctx);
        }
    }

    static long anyInput(long any) {
        return word((s, out, exc) -> (int) Ffi.rorb_any_input.invokeExact(any, out, exc));
    }

    static void inRelease(long in) {
        try {
            Ffi.rorb_in_release.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    // --- valuetypes ----------------------------------------------------------

    // RORB_VALUE_NULL, RORB_VALUE_INDIRECTION, RORB_VALUE_NEW
    static final int VALUE_NULL = 0;
    static final int VALUE_INDIRECTION = 1;
    static final int VALUE_NEW = 2;

    static void outNullValue(long out) {
        try {
            Ffi.rorb_out_write_null_value.invokeExact(out);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    /** Whether the caller writes the state (else an indirection was written). */
    static boolean outBeginValue(long out, long key, String[] ids, boolean chunked) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            int n = ids.length;
            long[] l = strs(s, ids, n);
            return (int) Ffi.rorb_out_begin_value.invokeExact(out, key, (long) n, l[0], l[1],
                    chunked ? 1 : 0) != 0;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static void outEndValue(long out) {
        try {
            Ffi.rorb_out_end_value.invokeExact(out);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    /** Fills slotCount with the slot and the number of repository ids. */
    static int inValueHeader(long in, long[] slotCount) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long slot = s.word();
            long count = s.word();
            int kind = (int) Ffi.rorb_in_read_value_header.invokeExact(in, slot, count);
            slotCount[0] = getLong(slot);
            slotCount[1] = getLong(count);
            return kind;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static String inValueId(long in, int index) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long len = s.word();
            long p = (long) Ffi.rorb_in_value_id.invokeExact(in, (long) index, len);
            return cstring(p, len);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static void inEndValue(long in) {
        try {
            Ffi.rorb_in_end_value.invokeExact(in);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    // Element kinds of the array methods (RORB_PRIM_*).
    static final int PRIM_OCTET = 0;
    static final int PRIM_SHORT = 1;
    static final int PRIM_USHORT = 2;
    static final int PRIM_LONG = 3;
    static final int PRIM_ULONG = 4;
    static final int PRIM_LONGLONG = 5;
    static final int PRIM_ULONGLONG = 6;
    static final int PRIM_FLOAT = 7;
    static final int PRIM_DOUBLE = 8;

    // --- servants ------------------------------------------------------------

    /**
     * The peer of a Java servant, whose id is the self of the C ABI
     * (rorb_servant_get). It keeps a weak reference to the servant, and a
     * strong one while the C side holds references to it (between add_ref
     * and the matching remove_ref), so that a servant the POA holds is not
     * collected. The servant's delegate (ServantDelegateImpl) frees the peer
     * once the servant is unreachable, which can only happen when the C side
     * holds none.
     */
    private static final class Peer {
        final WeakReference<Object> weak;
        Object strong;
        int refs;

        Peer(Object servant) {
            weak = new WeakReference<>(servant);
        }

        synchronized Object strong() {
            return strong;
        }

        synchronized void addRef() {
            if (refs == 0) {
                strong = weak.get();
            }
            refs++;
        }

        synchronized void removeRef() {
            if (refs > 0) {
                refs--;
            }
            if (refs == 0) {
                strong = null;
            }
        }
    }

    static long servantPeerNew(Object servant) {
        if (servant == null) {
            throw badParam();
        }
        return PEERS.add(new Peer(servant));
    }

    static void servantPeerFree(long peer) {
        PEERS.remove(peer);
    }

    static long servantGet(long peer, String repositoryId) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long p = str(s, repositoryId);
            return (long) Ffi.rorb_servant_get.invokeExact(peer, p, s.lastLength,
                    Upcalls.SERVANT_VTABLE);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static void servantRelease(long servant) {
        try {
            Ffi.rorb_servant_release.invokeExact(servant);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long servantCurrentReference(long servant) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long obj = s.word();
            if ((int) Ffi.rorb_servant_current_reference.invokeExact(servant, obj) != 0) {
                return getLong(obj);
            }
            return 0;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static long replyOutput(long reply) {
        try {
            return (long) Ffi.rorb_server_reply_output.invokeExact(reply);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long replyUserException(long reply, String id) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long p = str(s, id);
            return (long) Ffi.rorb_server_reply_user_exception.invokeExact(reply, p, s.lastLength);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static void replySystemException(long reply, String id, int minor, int completed) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long p = str(s, id);
            Ffi.rorb_server_reply_system_exception.invokeExact(reply, p, s.lastLength, minor,
                    completed);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    /** The Java servant of a self the C side returned with a reference for
     * the caller (rorb_poa_id_to_servant...); gives that reference back. */
    private static Object servantObject(long self) {
        if (self == 0) {
            return null;
        }
        Peer peer = PEERS.get(self);
        if (peer == null) {
            return null;
        }
        Object servant = peer.strong();
        peer.removeRef();
        return servant;
    }

    /** The Java servant of a self the C side passes to a callback without a
     * reference added (etherealize, postinvoke). */
    private static Object borrowedServant(long self) {
        Peer peer = self == 0 ? null : PEERS.get(self);
        return peer == null ? null : peer.strong();
    }

    // --- POA -------------------------------------------------------------------

    static long poaClone(long poa) {
        try {
            return (long) Ffi.rorb_poa_clone.invokeExact(poa);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void poaRelease(long poa) {
        try {
            Ffi.rorb_poa_release.invokeExact(poa);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static String poaName(long poa) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long len = s.word();
            long p = (long) Ffi.rorb_poa_name.invokeExact(poa, len);
            return cstring(p, len);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static long poaParent(long poa) {
        try {
            return (long) Ffi.rorb_poa_parent.invokeExact(poa);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long[] poaChildren(long poa) {
        List<Long> children = new ArrayList<>();
        long ctx = CONTEXTS.add(children);
        try {
            Ffi.rorb_poa_children.invokeExact(poa, ctx, Upcalls.PUSH_POA);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            CONTEXTS.remove(ctx);
        }
        long[] a = new long[children.size()];
        for (int i = 0; i < a.length; i++) {
            a[i] = children.get(i);
        }
        return a;
    }

    static long poaTheManager(long poa) {
        try {
            return (long) Ffi.rorb_poa_the_manager.invokeExact(poa);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    /** The seven policy values, in the order of the policy ids. */
    static int[] poaPolicies(long poa) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long p = s.alloc(8 * 7);
            Ffi.rorb_poa_policies.invokeExact(poa, p);
            int[] values = new int[7];
            for (int i = 0; i < 7; i++) {
                values[i] = getInt(p + 8L * i + 4);
            }
            return values;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static long poaCreatePoa(long poa, String name, long manager, int[] types, int[] values)
            throws org.omg.CORBA.UserException {
        if (types == null || values == null) {
            throw badParam();
        }
        int n = types.length;
        if (values.length < n) {
            throw new ArrayIndexOutOfBoundsException(n);
        }
        return word((s, out, exc) -> {
            long p = str(s, name);
            long len = s.lastLength;
            long policies = s.alloc(8L * n);
            for (int i = 0; i < n; i++) {
                Library.MEMORY.set(JAVA_INT, policies + 8L * i, types[i]);
                Library.MEMORY.set(JAVA_INT, policies + 8L * i + 4, values[i]);
            }
            return (int) Ffi.rorb_poa_create_poa.invokeExact(poa, p, len, manager, policies,
                    (long) n, out, exc);
        });
    }

    static long poaFindPoa(long poa, String name, boolean activateIt)
            throws org.omg.CORBA.UserException {
        return word((s, out, exc) -> {
            long p = str(s, name);
            return (int) Ffi.rorb_poa_find_poa.invokeExact(poa, p, s.lastLength,
                    activateIt ? 1 : 0, out, exc);
        });
    }

    static void poaDestroy(long poa, boolean etherealize, boolean wait) {
        run((s, exc) -> (int) Ffi.rorb_poa_destroy.invokeExact(poa, etherealize ? 1 : 0,
                wait ? 1 : 0, exc));
    }

    static byte[] poaActivateObject(long poa, long servant) throws org.omg.CORBA.UserException {
        return bytes((s, out, exc) -> (int) Ffi.rorb_poa_activate_object.invokeExact(poa, servant,
                out, exc));
    }

    static void poaActivateObjectWithId(long poa, byte[] id, long servant)
            throws org.omg.CORBA.UserException {
        run((s, exc) -> (int) Ffi.rorb_poa_activate_object_with_id.invokeExact(poa, id(s, id),
                (long) id.length, servant, exc));
    }

    static void poaDeactivateObject(long poa, byte[] id) throws org.omg.CORBA.UserException {
        run((s, exc) -> (int) Ffi.rorb_poa_deactivate_object.invokeExact(poa, id(s, id),
                (long) id.length, exc));
    }

    static byte[] poaServantToId(long poa, long servant) throws org.omg.CORBA.UserException {
        return bytes((s, out, exc) -> (int) Ffi.rorb_poa_servant_to_id.invokeExact(poa, servant,
                out, exc));
    }

    static long poaServantToReference(long poa, long servant)
            throws org.omg.CORBA.UserException {
        return word((s, out, exc) -> (int) Ffi.rorb_poa_servant_to_reference.invokeExact(poa,
                servant, out, exc));
    }

    static Object poaIdToServant(long poa, byte[] id) throws org.omg.CORBA.UserException {
        return servantObject(word((s, out, exc) -> (int) Ffi.rorb_poa_id_to_servant.invokeExact(
                poa, id(s, id), (long) id.length, out, exc)));
    }

    static long poaIdToReference(long poa, byte[] id) throws org.omg.CORBA.UserException {
        return word((s, out, exc) -> (int) Ffi.rorb_poa_id_to_reference.invokeExact(poa,
                id(s, id), (long) id.length, out, exc));
    }

    static byte[] poaReferenceToId(long poa, long obj) throws org.omg.CORBA.UserException {
        return bytes((s, out, exc) -> (int) Ffi.rorb_poa_reference_to_id.invokeExact(poa, obj,
                out, exc));
    }

    static Object poaReferenceToServant(long poa, long obj)
            throws org.omg.CORBA.UserException {
        return servantObject(word((s, out, exc) ->
                (int) Ffi.rorb_poa_reference_to_servant.invokeExact(poa, obj, out, exc)));
    }

    static long poaCreateReference(long poa, String intf) throws org.omg.CORBA.UserException {
        return word((s, out, exc) -> {
            long p = str(s, intf);
            return (int) Ffi.rorb_poa_create_reference.invokeExact(poa, p, s.lastLength, out, exc);
        });
    }

    static long poaCreateReferenceWithId(long poa, byte[] id, String intf)
            throws org.omg.CORBA.UserException {
        return word((s, out, exc) -> {
            long i = id(s, id);
            long p = str(s, intf);
            return (int) Ffi.rorb_poa_create_reference_with_id.invokeExact(poa, i,
                    (long) id.length, p, s.lastLength, out, exc);
        });
    }

    static Object poaGetServant(long poa) throws org.omg.CORBA.UserException {
        return servantObject(word((s, out, exc) -> (int) Ffi.rorb_poa_get_servant.invokeExact(poa,
                out, exc)));
    }

    static void poaSetServant(long poa, long servant) throws org.omg.CORBA.UserException {
        run((s, exc) -> (int) Ffi.rorb_poa_set_servant.invokeExact(poa, servant, exc));
    }

    // --- servant managers and adapter activators -------------------------------

    /**
     * The self of a Java servant manager or adapter activator: the Java
     * object with the count of references the C side holds, forgotten when
     * that count drops to zero.
     */
    private static final class Held {
        final Object obj;
        final AtomicInteger refs = new AtomicInteger(1);

        Held(Object obj) {
            this.obj = obj;
        }
    }

    private static Object heldObject(long self) {
        Held h = HELD.get(self);
        return h == null ? null : h.obj;
    }

    private static void heldRemoveRef(long self) {
        Held h = HELD.get(self);
        if (h != null && h.refs.decrementAndGet() == 0) {
            HELD.remove(self);
        }
    }

    /** The Java object of a holder the C side returned with a reference
     * added for the caller; gives that reference back. */
    private static Object take(long self) {
        if (self == 0) {
            return null;
        }
        Object obj = heldObject(self);
        heldRemoveRef(self);
        return obj;
    }

    /** Hands a new holder of obj to set, which adds its own reference. */
    private static void handOver(Object obj, Call1 set) {
        if (obj == null) {
            throw badParam();
        }
        long held = HELD.add(new Held(obj));
        try {
            run((s, exc) -> set.call(s, held, exc));
        } finally {
            heldRemoveRef(held);
        }
    }

    /** set_servant_manager with a ServantLocator (locator) or a
     * ServantActivator; the callbacks go to Bridge. */
    static void poaSetServantManager(long poa, Object manager, boolean locator)
            throws org.omg.CORBA.UserException {
        handOver(manager, (s, held, exc) -> locator
                ? (int) Ffi.rorb_poa_set_servant_locator.invokeExact(poa, held,
                        Upcalls.LOCATOR_VTABLE, exc)
                : (int) Ffi.rorb_poa_set_servant_activator.invokeExact(poa, held,
                        Upcalls.ACTIVATOR_VTABLE, exc));
    }

    static Object poaGetServantManager(long poa) throws org.omg.CORBA.UserException {
        return take(word((s, out, exc) -> {
            long kind = s.word();
            return (int) Ffi.rorb_poa_get_servant_manager.invokeExact(poa, kind, out, exc);
        }));
    }

    /** null removes the adapter activator. */
    static void poaSetTheActivator(long poa, Object activator) {
        if (activator == null) {
            run((s, exc) -> (int) Ffi.rorb_poa_set_the_activator.invokeExact(poa, 0L, 0L, exc));
            return;
        }
        handOver(activator, (s, held, exc) -> (int) Ffi.rorb_poa_set_the_activator.invokeExact(
                poa, held, Upcalls.ADAPTER_ACTIVATOR_VTABLE, exc));
    }

    static Object poaTheActivator(long poa) {
        try {
            return take((long) Ffi.rorb_poa_the_activator.invokeExact(poa));
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    /** What incarnate or preinvoke raised (rorb_manager_error_*). */
    static void managerErrorForward(long error, long obj) {
        try {
            Ffi.rorb_manager_error_forward.invokeExact(error, obj);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void managerErrorSystem(long error, String id, int minor, int completed) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long p = str(s, id);
            Ffi.rorb_manager_error_system.invokeExact(error, p, s.lastLength, minor, completed);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    // --- PortableServer::Current ---------------------------------------------------

    static long currentGetPoa() throws org.omg.CORBA.UserException {
        return word((s, out, exc) -> (int) Ffi.rorb_current_get_poa.invokeExact(out, exc));
    }

    static byte[] currentGetObjectId() throws org.omg.CORBA.UserException {
        return bytes((s, out, exc) -> (int) Ffi.rorb_current_get_object_id.invokeExact(out, exc));
    }

    static long currentGetReference() throws org.omg.CORBA.UserException {
        return word((s, out, exc) -> (int) Ffi.rorb_current_get_reference.invokeExact(out, exc));
    }

    static Object currentGetServant() throws org.omg.CORBA.UserException {
        return servantObject(word((s, out, exc) -> (int) Ffi.rorb_current_get_servant.invokeExact(
                out, exc)));
    }

    // --- POAManager ---------------------------------------------------------------------

    static long pmClone(long manager) {
        try {
            return (long) Ffi.rorb_poa_manager_clone.invokeExact(manager);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void pmRelease(long manager) {
        try {
            Ffi.rorb_poa_manager_release.invokeExact(manager);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static int pmGetState(long manager) {
        try {
            return (int) Ffi.rorb_poa_manager_get_state.invokeExact(manager);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void pmActivate(long manager) throws org.omg.CORBA.UserException {
        run((s, exc) -> (int) Ffi.rorb_poa_manager_activate.invokeExact(manager, exc));
    }

    static void pmHoldRequests(long manager, boolean wait) throws org.omg.CORBA.UserException {
        run((s, exc) -> (int) Ffi.rorb_poa_manager_hold_requests.invokeExact(manager,
                wait ? 1 : 0, exc));
    }

    static void pmDiscardRequests(long manager, boolean wait)
            throws org.omg.CORBA.UserException {
        run((s, exc) -> (int) Ffi.rorb_poa_manager_discard_requests.invokeExact(manager,
                wait ? 1 : 0, exc));
    }

    static void pmDeactivate(long manager, boolean etherealize, boolean wait)
            throws org.omg.CORBA.UserException {
        run((s, exc) -> (int) Ffi.rorb_poa_manager_deactivate.invokeExact(manager,
                etherealize ? 1 : 0, wait ? 1 : 0, exc));
    }

    // --- TypeCodes ---------------------------------------------------------------

    static long tcClone(long tc) {
        try {
            return (long) Ffi.rorb_tc_clone.invokeExact(tc);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void tcRelease(long tc) {
        try {
            Ffi.rorb_tc_release.invokeExact(tc);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long tcSimple(int kind) {
        return word((s, out, exc) -> (int) Ffi.rorb_tc_simple.invokeExact(kind, out, exc));
    }

    static long tcString(boolean wide, int bound) {
        return word((s, out, exc) -> (int) Ffi.rorb_tc_string.invokeExact(wide ? 1 : 0, bound,
                out, exc));
    }

    static long tcFixed(short digits, short scale) {
        return word((s, out, exc) -> (int) Ffi.rorb_tc_fixed.invokeExact(digits, scale, out, exc));
    }

    static long tcSequence(long content, int bound) {
        return word((s, out, exc) -> (int) Ffi.rorb_tc_sequence.invokeExact(content, bound, out,
                exc));
    }

    static long tcArray(long content, int length) {
        return word((s, out, exc) -> (int) Ffi.rorb_tc_array.invokeExact(content, length, out,
                exc));
    }

    static long tcAlias(int kind, String id, String name, long content) {
        return word((s, out, exc) -> {
            long i = str(s, id);
            long il = s.lastLength;
            long n = str(s, name);
            return (int) Ffi.rorb_tc_alias.invokeExact(kind, i, il, n, s.lastLength, content, out,
                    exc);
        });
    }

    static long tcInterface(int kind, String id, String name) {
        return word((s, out, exc) -> {
            long i = str(s, id);
            long il = s.lastLength;
            long n = str(s, name);
            return (int) Ffi.rorb_tc_interface.invokeExact(kind, i, il, n, s.lastLength, out, exc);
        });
    }

    private static long[] nonNull(long[] v) {
        if (v == null) {
            throw badParam();
        }
        return v;
    }

    static long tcStruct(int kind, String id, String name, String[] names, long[] types) {
        int n = Math.min(names.length, nonNull(types).length);
        return word((s, out, exc) -> {
            long i = str(s, id);
            long il = s.lastLength;
            long nm = str(s, name);
            long nl = s.lastLength;
            long[] ns = strs(s, names, names.length);
            return (int) Ffi.rorb_tc_struct.invokeExact(kind, i, il, nm, nl, (long) n, ns[0], ns[1],
                    s.longs(types, n), out, exc);
        });
    }

    static long tcUnion(String id, String name, long discriminator, long[] labels,
            int defaultIndex, String[] names, long[] types) {
        int n = Math.min(Math.min(names.length, nonNull(types).length), nonNull(labels).length);
        return word((s, out, exc) -> {
            long i = str(s, id);
            long il = s.lastLength;
            long nm = str(s, name);
            long nl = s.lastLength;
            long[] ns = strs(s, names, names.length);
            return (int) Ffi.rorb_tc_union.invokeExact(i, il, nm, nl, discriminator, (long) n,
                    s.longs(labels, n), defaultIndex, ns[0], ns[1], s.longs(types, n), out, exc);
        });
    }

    static long tcEnum(String id, String name, String[] names) {
        return word((s, out, exc) -> {
            long i = str(s, id);
            long il = s.lastLength;
            long nm = str(s, name);
            long nl = s.lastLength;
            long[] ns = strs(s, names, names.length);
            return (int) Ffi.rorb_tc_enum.invokeExact(i, il, nm, nl, (long) names.length, ns[0],
                    ns[1], out, exc);
        });
    }

    static long tcRecursive(String id) {
        return word((s, out, exc) -> {
            long i = str(s, id);
            return (int) Ffi.rorb_tc_recursive.invokeExact(i, s.lastLength, out, exc);
        });
    }

    static long tcValue(String id, String name, short modifier, long base,
            String[] names, long[] types, short[] visibilities) {
        if (visibilities == null) {
            throw badParam();
        }
        int n = Math.min(Math.min(names.length, nonNull(types).length), visibilities.length);
        return word((s, out, exc) -> {
            long i = str(s, id);
            long il = s.lastLength;
            long nm = str(s, name);
            long nl = s.lastLength;
            long[] ns = strs(s, names, names.length);
            long vis = s.alloc(2L * n);
            MemorySegment.copy(visibilities, 0, Library.MEMORY, JAVA_SHORT, vis, n);
            return (int) Ffi.rorb_tc_value.invokeExact(29 /* RORB_TK_VALUE */, i, il, nm, nl,
                    modifier, base, (long) n, ns[0], ns[1], s.longs(types, n), vis, out, exc);
        });
    }

    static void registerValueType(long tc) {
        try {
            Ffi.rorb_register_value_type.invokeExact(tc);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static int tcKind(long tc) {
        try {
            return (int) Ffi.rorb_tc_kind.invokeExact(tc);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    /** A string query of a TypeCode: the C call fills a pointer at out and
     * the length after it. */
    private static String tcString(Call1 f) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long out = s.alloc(16);
            long exc = s.alloc(EXCEPTION);
            check(f.call(s, out, exc), exc);
            return cstring(getLong(out), out + 8);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    static String tcId(long tc) throws org.omg.CORBA.TypeCodePackage.BadKind {
        return tcString((s, out, exc) -> (int) Ffi.rorb_tc_id.invokeExact(tc, out, out + 8, exc));
    }

    static String tcName(long tc) throws org.omg.CORBA.TypeCodePackage.BadKind {
        return tcString((s, out, exc) -> (int) Ffi.rorb_tc_name.invokeExact(tc, out, out + 8,
                exc));
    }

    static int tcMemberCount(long tc) throws org.omg.CORBA.TypeCodePackage.BadKind {
        return int32((s, out, exc) -> (int) Ffi.rorb_tc_member_count.invokeExact(tc, out, exc));
    }

    static String tcMemberName(long tc, int index)
            throws org.omg.CORBA.TypeCodePackage.BadKind, org.omg.CORBA.TypeCodePackage.Bounds {
        return tcString((s, out, exc) -> (int) Ffi.rorb_tc_member_name.invokeExact(tc, index, out,
                out + 8, exc));
    }

    static long tcMemberType(long tc, int index)
            throws org.omg.CORBA.TypeCodePackage.BadKind, org.omg.CORBA.TypeCodePackage.Bounds {
        return word((s, out, exc) -> (int) Ffi.rorb_tc_member_type.invokeExact(tc, index, out,
                exc));
    }

    static long tcMemberLabel(long tc, int index)
            throws org.omg.CORBA.TypeCodePackage.BadKind, org.omg.CORBA.TypeCodePackage.Bounds {
        return word((s, out, exc) -> (int) Ffi.rorb_tc_member_label.invokeExact(tc, index, out,
                exc));
    }

    static long tcDiscriminatorType(long tc) throws org.omg.CORBA.TypeCodePackage.BadKind {
        return word((s, out, exc) -> (int) Ffi.rorb_tc_discriminator_type.invokeExact(tc, out,
                exc));
    }

    static int tcDefaultIndex(long tc) throws org.omg.CORBA.TypeCodePackage.BadKind {
        return int32((s, out, exc) -> (int) Ffi.rorb_tc_default_index.invokeExact(tc, out, exc));
    }

    static int tcLength(long tc) throws org.omg.CORBA.TypeCodePackage.BadKind {
        return int32((s, out, exc) -> (int) Ffi.rorb_tc_length.invokeExact(tc, out, exc));
    }

    static long tcContentType(long tc) throws org.omg.CORBA.TypeCodePackage.BadKind {
        return word((s, out, exc) -> (int) Ffi.rorb_tc_content_type.invokeExact(tc, out, exc));
    }

    static short tcFixedDigits(long tc) throws org.omg.CORBA.TypeCodePackage.BadKind {
        return int16((s, out, exc) -> (int) Ffi.rorb_tc_fixed_digits.invokeExact(tc, out, exc));
    }

    static short tcFixedScale(long tc) throws org.omg.CORBA.TypeCodePackage.BadKind {
        return int16((s, out, exc) -> (int) Ffi.rorb_tc_fixed_scale.invokeExact(tc, out, exc));
    }

    static short tcMemberVisibility(long tc, int index)
            throws org.omg.CORBA.TypeCodePackage.BadKind, org.omg.CORBA.TypeCodePackage.Bounds {
        return int16((s, out, exc) -> (int) Ffi.rorb_tc_member_visibility.invokeExact(tc, index,
                out, exc));
    }

    static short tcTypeModifier(long tc) throws org.omg.CORBA.TypeCodePackage.BadKind {
        return int16((s, out, exc) -> (int) Ffi.rorb_tc_type_modifier.invokeExact(tc, out, exc));
    }

    static long tcConcreteBaseType(long tc) throws org.omg.CORBA.TypeCodePackage.BadKind {
        return word((s, out, exc) -> (int) Ffi.rorb_tc_concrete_base_type.invokeExact(tc, out,
                exc));
    }

    static boolean tcEqual(long a, long b) {
        try {
            return (int) Ffi.rorb_tc_equal.invokeExact(a, b) != 0;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static boolean tcEquivalent(long a, long b) {
        try {
            return (int) Ffi.rorb_tc_equivalent.invokeExact(a, b) != 0;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long tcCompact(long tc) {
        try {
            return (long) Ffi.rorb_tc_compact.invokeExact(tc);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long tcUnaliased(long tc) {
        try {
            return (long) Ffi.rorb_tc_unaliased.invokeExact(tc);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    // --- anys --------------------------------------------------------------------

    static long anyNew() {
        try {
            return (long) Ffi.rorb_any_new.invokeExact();
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long anyClone(long any) {
        try {
            return (long) Ffi.rorb_any_clone.invokeExact(any);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void anyRelease(long any) {
        try {
            Ffi.rorb_any_release.invokeExact(any);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long anyType(long any) {
        try {
            return (long) Ffi.rorb_any_type.invokeExact(any);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void anySetType(long any, long tc) {
        run((s, exc) -> (int) Ffi.rorb_any_set_type.invokeExact(any, tc, exc));
    }

    static boolean anyEqual(long a, long b) {
        try {
            return (int) Ffi.rorb_any_equal.invokeExact(a, b) != 0;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    /** The value of the any as a union label in label[0]; false if it is not
     * of a discriminator type. */
    static boolean anyLabel(long any, long[] label) {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long v = s.word();
            if ((int) Ffi.rorb_any_label.invokeExact(any, v) == 0) {
                return false;
            }
            label[0] = getLong(v);
            return true;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    /** An any of type tc holding what call.write writes. */
    static long anyInsert(long tc, Call call) {
        long ctx = CALLS.add(call);
        try {
            return word((s, out, exc) -> (int) Ffi.rorb_any_insert.invokeExact(tc,
                    Upcalls.CALL_WRITE, ctx, out, exc));
        } finally {
            CALLS.remove(ctx);
        }
    }

    /** call.read reads the value of the any. */
    static void anyExtract(long any, Call call) {
        long ctx = CALLS.add(call);
        try {
            run((s, exc) -> (int) Ffi.rorb_any_extract.invokeExact(any, Upcalls.CALL_READ, ctx,
                    exc));
        } finally {
            CALLS.remove(ctx);
        }
    }

    // --- DynamicAny ----------------------------------------------------------------

    static long dynCreate(long any, boolean truncate) throws org.omg.CORBA.UserException {
        return word((s, out, exc) -> (int) Ffi.rorb_dynany_create.invokeExact(any,
                truncate ? 1 : 0, out, exc));
    }

    static long dynFromTypeCode(long tc) throws org.omg.CORBA.UserException {
        return word((s, out, exc) -> (int) Ffi.rorb_dynany_from_typecode.invokeExact(tc, out, exc));
    }

    static long dynClone(long d) {
        try {
            return (long) Ffi.rorb_dynany_clone.invokeExact(d);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static void dynRelease(long d) {
        try {
            Ffi.rorb_dynany_release.invokeExact(d);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static int dynKind(long d) {
        try {
            return (int) Ffi.rorb_dynany_kind.invokeExact(d);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static boolean dynSame(long a, long b) {
        try {
            return (int) Ffi.rorb_dynany_same.invokeExact(a, b) != 0;
        } catch (Throwable t) {
            throw Library.rethrow(t);
        }
    }

    static long dynType(long d) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_type.invokeExact(d, o, e));
    }

    static long dynTargetType(long d) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_target_type.invokeExact(d, o, e));
    }

    static long dynToAny(long d) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_to_any.invokeExact(d, o, e));
    }

    static long dynCopy(long d) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_copy.invokeExact(d, o, e));
    }

    static long dynGetDynAny(long d) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_get_dyn_any.invokeExact(d, o, e));
    }

    static long dynCurrentComponent(long d) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_current_component.invokeExact(d, o, e));
    }

    static long dynGetDiscriminator(long d) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_get_discriminator.invokeExact(d, o, e));
    }

    static long dynMember(long d) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_member.invokeExact(d, o, e));
    }

    static long dynGetBoxedValue(long d) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_get_boxed_value.invokeExact(d, o, e));
    }

    static long dynGetBoxedValueAsDynAny(long d) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_get_boxed_value_as_dyn_any.invokeExact(d,
                o, e));
    }

    static int dynComponentCount(long d) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_component_count.invokeExact(d, o, e));
    }

    static int dynGetAsUlong(long d) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_get_as_ulong.invokeExact(d, o, e));
    }

    static int dynCurrentMemberKind(long d) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_current_member_kind.invokeExact(d, o, e));
    }

    static int dynMemberCount(long d) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_member_count.invokeExact(d, o, e));
    }

    static int dynDiscriminatorKind(long d) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_discriminator_kind.invokeExact(d, o, e));
    }

    static int dynMemberKind(long d) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_member_kind.invokeExact(d, o, e));
    }

    static int dynGetLength(long d) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_get_length.invokeExact(d, o, e));
    }

    static int dynElementCount(long d) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_element_count.invokeExact(d, o, e));
    }

    static boolean dynNext(long d) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_next.invokeExact(d, o, e)) != 0;
    }

    static boolean dynHasNoActiveMember(long d) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_has_no_active_member.invokeExact(d, o,
                e)) != 0;
    }

    static boolean dynIsNull(long d) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_is_null.invokeExact(d, o, e)) != 0;
    }

    static String dynGetFixedValue(long d) throws org.omg.CORBA.UserException {
        return utf8((s, o, e) -> (int) Ffi.rorb_dynany_get_fixed_value.invokeExact(d, o, e));
    }

    static String dynGetAsString(long d) throws org.omg.CORBA.UserException {
        return utf8((s, o, e) -> (int) Ffi.rorb_dynany_get_as_string.invokeExact(d, o, e));
    }

    static String dynCurrentMemberName(long d) throws org.omg.CORBA.UserException {
        return utf8((s, o, e) -> (int) Ffi.rorb_dynany_current_member_name.invokeExact(d, o, e));
    }

    static String dynMemberName(long d) throws org.omg.CORBA.UserException {
        return utf8((s, o, e) -> (int) Ffi.rorb_dynany_member_name.invokeExact(d, o, e));
    }

    static void dynDestroy(long d) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_destroy.invokeExact(d, e));
    }

    static void dynRewind(long d) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_rewind.invokeExact(d, e));
    }

    static void dynNoImplement(long d) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_no_implement.invokeExact(d, e));
    }

    static void dynSetToDefaultMember(long d) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_set_to_default_member.invokeExact(d, e));
    }

    static void dynSetToNoActiveMember(long d) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_set_to_no_active_member.invokeExact(d, e));
    }

    static void dynSetToNull(long d) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_set_to_null.invokeExact(d, e));
    }

    static void dynSetToValue(long d) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_set_to_value.invokeExact(d, e));
    }

    static void dynAssign(long d, long h) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_assign.invokeExact(d, h, e));
    }

    static void dynFromAny(long d, long h) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_from_any.invokeExact(d, h, e));
    }

    static void dynInsertDynAny(long d, long h) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_insert_dyn_any.invokeExact(d, h, e));
    }

    static void dynSetDiscriminator(long d, long h) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_set_discriminator.invokeExact(d, h, e));
    }

    static void dynSetBoxedValue(long d, long h) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_set_boxed_value.invokeExact(d, h, e));
    }

    static void dynSetBoxedValueAsDynAny(long d, long h) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_set_boxed_value_as_dyn_any.invokeExact(d, h, e));
    }

    static void dynInsert(long d, int kind, long any) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_insert.invokeExact(d, kind, any, e));
    }

    static void dynInsertSeq(long d, int kind, long any) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_insert_seq.invokeExact(d, kind, any, e));
    }

    static long dynGet(long d, int kind) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_get.invokeExact(d, kind, o, e));
    }

    static long dynGetSeq(long d, int kind) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_get_seq.invokeExact(d, kind, o, e));
    }

    static void dynSetAsUlong(long d, int value) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_set_as_ulong.invokeExact(d, value, e));
    }

    static void dynSetLength(long d, int length) throws org.omg.CORBA.UserException {
        run((s, e) -> (int) Ffi.rorb_dynany_set_length.invokeExact(d, length, e));
    }

    static boolean dynEqual(long d, long other) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_equal.invokeExact(d, other, o, e)) != 0;
    }

    static boolean dynSeek(long d, int index) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> (int) Ffi.rorb_dynany_seek.invokeExact(d, index, o, e)) != 0;
    }

    static boolean dynSetFixedValue(long d, String value) throws org.omg.CORBA.UserException {
        return int32((s, o, e) -> {
            long p = str(s, value);
            return (int) Ffi.rorb_dynany_set_fixed_value.invokeExact(d, p, s.lastLength, o, e);
        }) != 0;
    }

    static void dynSetAsString(long d, String value) throws org.omg.CORBA.UserException {
        run((s, e) -> {
            long p = str(s, value);
            return (int) Ffi.rorb_dynany_set_as_string.invokeExact(d, p, s.lastLength, e);
        });
    }

    /** The member's DynAny; its name goes in name[0]. */
    static long dynMemberAt(long d, int index, String[] name) throws org.omg.CORBA.UserException {
        Scratch s = Scratch.get();
        long mark = s.mark();
        try {
            long n = s.alloc(BYTES);
            long out = s.word();
            long exc = s.alloc(EXCEPTION);
            check((int) Ffi.rorb_dynany_member_at.invokeExact(d, index, n, out, exc), exc);
            name[0] = new String(takeBytes(n), StandardCharsets.UTF_8);
            return getLong(out);
        } catch (Throwable t) {
            throw Library.rethrow(t);
        } finally {
            s.reset(mark);
        }
    }

    /** set_members (values are anys) or set_members_as_dyn_any (DynAnys). */
    static void dynSetMembers(long d, String[] names, long[] values, boolean asDynAny)
            throws org.omg.CORBA.UserException {
        if (names == null) {
            throw badParam();
        }
        int n = Math.min(nonNull(values).length, names.length);
        run((s, e) -> {
            long[] ns = strs(s, names, names.length);
            long v = s.longs(values, values.length);
            return asDynAny
                    ? (int) Ffi.rorb_dynany_set_members_as_dyn_any.invokeExact(d, (long) n, ns[0],
                            ns[1], v, e)
                    : (int) Ffi.rorb_dynany_set_members.invokeExact(d, (long) n, ns[0], ns[1], v,
                            e);
        });
    }

    static long dynElementAt(long d, int index) throws org.omg.CORBA.UserException {
        return word((s, o, e) -> (int) Ffi.rorb_dynany_element_at.invokeExact(d, index, o, e));
    }

    /** set_elements (anys) or set_elements_as_dyn_any (DynAnys). */
    static void dynSetElements(long d, long[] values, boolean asDynAny)
            throws org.omg.CORBA.UserException {
        int n = nonNull(values).length;
        run((s, e) -> {
            long v = s.longs(values, n);
            return asDynAny
                    ? (int) Ffi.rorb_dynany_set_elements_as_dyn_any.invokeExact(d, (long) n, v, e)
                    : (int) Ffi.rorb_dynany_set_elements.invokeExact(d, (long) n, v, e);
        });
    }

    // --- callbacks -------------------------------------------------------------

    /** Java objects named by the ids the C side holds as pointers. */
    private static final class Registry<T> {
        private final ConcurrentHashMap<Long, T> map = new ConcurrentHashMap<>();
        private final AtomicLong next = new AtomicLong(1);

        long add(T v) {
            long id = next.getAndIncrement();
            map.put(id, v);
            return id;
        }

        T get(long id) {
            return map.get(id);
        }

        T remove(long id) {
            return map.remove(id);
        }
    }

    /** The org.rustorb.Call of each call with callbacks. */
    private static final Registry<Call> CALLS = new Registry<>();
    /** What the each callbacks fill. */
    private static final Registry<Object> CONTEXTS = new Registry<>();
    private static final Registry<Peer> PEERS = new Registry<>();
    private static final Registry<Held> HELD = new Registry<>();
    /** The cookies of ServantLocator.preinvoke, until postinvoke. */
    private static final Registry<Object> COOKIES = new Registry<>();
    /** The objects of register_initial_reference with local objects. */
    private static final Registry<Object> LOCALS = new Registry<>();

    /**
     * The upcall stubs of the callbacks and the vtables of servants, servant
     * managers and adapter activators. Pointers are longs, as in Ffi. An
     * upcall must not throw (the JVM would exit), so each catches all.
     */
    private static final class Upcalls {
        private Upcalls() {}

        private static FunctionDescriptor fd(java.lang.foreign.MemoryLayout ret,
                java.lang.foreign.MemoryLayout... args) {
            return ret == null ? FunctionDescriptor.ofVoid(args) : FunctionDescriptor.of(ret, args);
        }

        private static long stub(String name, FunctionDescriptor fd) {
            return Library.upcall(MethodHandles.lookup(), name, fd);
        }

        static final long CALL_WRITE = stub("callWrite", fd(JAVA_INT, JAVA_LONG, JAVA_LONG));
        static final long CALL_READ = stub("callRead", fd(JAVA_INT, JAVA_LONG, JAVA_LONG));
        static final long CALL_READ_EXCEPTION = stub("callReadException",
                fd(JAVA_INT, JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG));
        static final long PUSH_NAME = stub("pushName", fd(null, JAVA_LONG, JAVA_LONG, JAVA_LONG));
        static final long PUSH_POA = stub("pushPoa", fd(null, JAVA_LONG, JAVA_LONG));
        static final long PUSH_POLICY = stub("pushPolicy", fd(null, JAVA_LONG, JAVA_INT, JAVA_LONG));
        static final long RELEASE_LOCAL = stub("releaseLocal", fd(null, JAVA_LONG));
        static final long COPY_LOCAL = stub("copyLocal", fd(null, JAVA_LONG, JAVA_LONG));
        static final long RESOLVE = stub("resolve", fd(JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG));

        private static long vtable(long... fns) {
            MemorySegment v = Arena.global().allocate(8L * fns.length, 8);
            for (int i = 0; i < fns.length; i++) {
                v.setAtIndex(JAVA_LONG, i, fns[i]);
            }
            return v.address();
        }

        private static final long SERVANT_ADD_REF = stub("servantAddRef", fd(null, JAVA_LONG));
        private static final long SERVANT_REMOVE_REF = stub("servantRemoveRef", fd(null, JAVA_LONG));
        private static final long HELD_ADD_REF = stub("heldAddRef", fd(null, JAVA_LONG));
        private static final long HELD_REMOVE_REF = stub("heldRemoveRefUpcall", fd(null, JAVA_LONG));

        /** rorb_servant_vtable */
        static final long SERVANT_VTABLE = vtable(
                stub("dispatch", fd(JAVA_INT, JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG)),
                stub("isA", fd(JAVA_INT, JAVA_LONG, JAVA_LONG, JAVA_LONG)),
                SERVANT_ADD_REF, SERVANT_REMOVE_REF);
        /** rorb_servant_activator_vtable */
        static final long ACTIVATOR_VTABLE = vtable(
                stub("incarnate", fd(JAVA_INT, JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG,
                        JAVA_LONG, JAVA_LONG)),
                stub("etherealize", fd(null, JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG,
                        JAVA_INT, JAVA_INT)),
                HELD_ADD_REF, HELD_REMOVE_REF);
        /** rorb_servant_locator_vtable */
        static final long LOCATOR_VTABLE = vtable(
                stub("preinvoke", fd(JAVA_INT, JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG,
                        JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG)),
                stub("postinvoke", fd(null, JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG,
                        JAVA_LONG, JAVA_LONG, JAVA_LONG)),
                HELD_ADD_REF, HELD_REMOVE_REF);
        /** rorb_adapter_activator_vtable */
        static final long ADAPTER_ACTIVATOR_VTABLE = vtable(
                stub("unknownAdapter", fd(JAVA_INT, JAVA_LONG, JAVA_LONG, JAVA_LONG, JAVA_LONG)),
                HELD_ADD_REF, HELD_REMOVE_REF);

        // invocations

        private static int callWrite(long ctx, long out) {
            try {
                Call call = CALLS.get(ctx);
                return call != null && Bridge.callWrite(call, out) ? OK : CALLBACK_FAILED;
            } catch (Throwable t) {
                return CALLBACK_FAILED;
            }
        }

        private static int callRead(long ctx, long in) {
            try {
                Call call = CALLS.get(ctx);
                return call != null && Bridge.callRead(call, in) ? OK : CALLBACK_FAILED;
            } catch (Throwable t) {
                return CALLBACK_FAILED;
            }
        }

        /** RORB_USER_EXCEPTION, RORB_SYSTEM_EXCEPTION (not declared) or
         * RORB_CALLBACK_FAILED. */
        private static int callReadException(long ctx, long id, long idLen, long in) {
            try {
                Call call = CALLS.get(ctx);
                if (call == null) {
                    return CALLBACK_FAILED;
                }
                return Bridge.callReadException(call, Library.utf8(id, idLen), in);
            } catch (Throwable t) {
                return CALLBACK_FAILED;
            }
        }

        // lists

        @SuppressWarnings("unchecked")
        private static void pushName(long ctx, long name, long len) {
            try {
                ((List<String>) CONTEXTS.get(ctx)).add(Library.utf8(name, len));
            } catch (Throwable t) {
                // dropped
            }
        }

        @SuppressWarnings("unchecked")
        private static void pushPoa(long ctx, long child) {
            try {
                ((List<Long>) CONTEXTS.get(ctx)).add(poaClone(child));
            } catch (Throwable t) {
                // dropped
            }
        }

        @SuppressWarnings("unchecked")
        private static void pushPolicy(long ctx, int type, long value) {
            try {
                List<Long> list = (List<Long>) CONTEXTS.get(ctx);
                list.add(type & 0xffffffffL);
                list.add(value);
            } catch (Throwable t) {
                // dropped
            }
        }

        private static void releaseLocal(long object) {
            LOCALS.remove(object);
        }

        private static void copyLocal(long ctx, long object) {
            try {
                ((Object[]) CONTEXTS.get(ctx))[0] = LOCALS.get(object);
            } catch (Throwable t) {
                // dropped
            }
        }

        /** The core asks for a valuetype it does not know (of a Helper not
         * loaded yet): a new TypeCode handle, or 0. */
        private static long resolve(long ctx, long id, long len) {
            try {
                return Bridge.resolveValueType(Library.utf8(id, len));
            } catch (Throwable t) {
                return 0;
            }
        }

        // servants

        private static void servantAddRef(long self) {
            Peer peer = PEERS.get(self);
            if (peer != null) {
                peer.addRef();
            }
        }

        private static void servantRemoveRef(long self) {
            Peer peer = PEERS.get(self);
            if (peer != null) {
                peer.removeRef();
            }
        }

        private static void systemReply(long reply, String id) {
            try {
                replySystemException(reply, id, 0, 2);
            } catch (Throwable t) {
                // nothing more to do
            }
        }

        private static int dispatch(long self, long op, long opLen, long in, long reply) {
            try {
                Peer peer = PEERS.get(self);
                Object servant = peer == null ? null : peer.strong();
                if (servant == null) {
                    systemReply(reply, "IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0");
                    return SYSTEM_EXCEPTION;
                }
                return Bridge.dispatch(servant, Library.utf8(op, opLen), in, reply);
            } catch (Throwable t) {
                systemReply(reply, UNKNOWN);
                return SYSTEM_EXCEPTION;
            }
        }

        private static int isA(long self, long id, long len) {
            try {
                Peer peer = PEERS.get(self);
                Object servant = peer == null ? null : peer.strong();
                return servant != null && Bridge.isA(servant, Library.utf8(id, len)) ? 1 : 0;
            } catch (Throwable t) {
                return 0;
            }
        }

        // servant managers and adapter activators

        private static void heldAddRef(long self) {
            Held h = HELD.get(self);
            if (h != null) {
                h.refs.incrementAndGet();
            }
        }

        private static void heldRemoveRefUpcall(long self) {
            heldRemoveRef(self);
        }

        private static int unknown(long error) {
            try {
                managerErrorSystem(error, UNKNOWN, 0, 2);
            } catch (Throwable t) {
                // nothing more to do
            }
            return SYSTEM_EXCEPTION;
        }

        private static int incarnate(long self, long id, long idLen, long adapter, long servant,
                long error) {
            try {
                long h = Bridge.incarnate(heldObject(self), Library.bytes(id, idLen), adapter, error);
                if (h == 0) {
                    return SYSTEM_EXCEPTION;
                }
                putLong(servant, h);
                return OK;
            } catch (Throwable t) {
                return unknown(error);
            }
        }

        private static void etherealize(long self, long id, long idLen, long adapter, long servant,
                int cleanupInProgress, int remainingActivations) {
            try {
                Bridge.etherealize(heldObject(self), Library.bytes(id, idLen), adapter,
                        borrowedServant(servant), cleanupInProgress != 0, remainingActivations != 0);
            } catch (Throwable t) {
                // Bridge reports it
            }
        }

        private static int preinvoke(long self, long id, long idLen, long adapter, long op,
                long opLen, long servant, long cookie, long error) {
            try {
                Object[] cookieOut = new Object[1];
                long h = Bridge.preinvoke(heldObject(self), Library.bytes(id, idLen), adapter,
                        Library.utf8(op, opLen), cookieOut, error);
                if (h == 0) {
                    return SYSTEM_EXCEPTION;
                }
                putLong(servant, h);
                putLong(cookie, cookieOut[0] == null ? 0 : COOKIES.add(cookieOut[0]));
                return OK;
            } catch (Throwable t) {
                return unknown(error);
            }
        }

        private static void postinvoke(long self, long id, long idLen, long adapter, long op,
                long opLen, long cookie, long servant) {
            try {
                Object c = cookie == 0 ? null : COOKIES.remove(cookie);
                Bridge.postinvoke(heldObject(self), Library.bytes(id, idLen), adapter,
                        Library.utf8(op, opLen), c, borrowedServant(servant));
            } catch (Throwable t) {
                // Bridge reports it
            }
        }

        private static int unknownAdapter(long self, long parent, long name, long len) {
            try {
                return Bridge.unknownAdapter(heldObject(self), parent, Library.utf8(name, len))
                        ? 1 : 0;
            } catch (Throwable t) {
                return 0;
            }
        }
    }
}
