// The functions of the C ABI (crates/rustorb-ffi, librustorb_ffi) that the
// prototype calls, bound with the FFM API (java.lang.foreign, Java 22).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb.ffm;

import static java.lang.foreign.ValueLayout.ADDRESS;
import static java.lang.foreign.ValueLayout.JAVA_INT;
import static java.lang.foreign.ValueLayout.JAVA_LONG;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemoryLayout;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.StructLayout;
import java.lang.foreign.SymbolLookup;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;

final class Ffi {
    private Ffi() {}

    static final int OK = 0;
    static final int SYSTEM_EXCEPTION = 1;
    static final int USER_EXCEPTION = 2;
    static final int CALLBACK_FAILED = 4;
    static final int PRIM_OCTET = 0;

    static final Linker LINKER = Linker.nativeLinker();
    private static final SymbolLookup LIB = SymbolLookup.libraryLookup(libraryPath(), Arena.global());

    /** rorb_exception */
    static final StructLayout EXCEPTION = MemoryLayout.structLayout(
            ADDRESS.withName("repository_id"), JAVA_LONG.withName("repository_id_len"),
            JAVA_INT.withName("minor"), JAVA_INT.withName("completed"),
            JAVA_INT.withName("local"), JAVA_INT.withName("index"));
    /** rorb_bytes */
    static final StructLayout BYTES = MemoryLayout.structLayout(
            ADDRESS.withName("data"), JAVA_LONG.withName("len"));
    /** rorb_invocation */
    static final StructLayout INVOCATION = MemoryLayout.structLayout(
            ADDRESS.withName("operation"), JAVA_LONG.withName("operation_len"),
            JAVA_INT.withName("oneway"), MemoryLayout.paddingLayout(4),
            JAVA_LONG.withName("timeout_us"), ADDRESS.withName("context"),
            ADDRESS.withName("write_args"), ADDRESS.withName("read_reply"),
            ADDRESS.withName("read_user_exception"));
    /** rorb_servant_vtable */
    static final StructLayout SERVANT_VTABLE = MemoryLayout.structLayout(
            ADDRESS.withName("dispatch"), ADDRESS.withName("is_a"),
            ADDRESS.withName("add_ref"), ADDRESS.withName("remove_ref"));

    private static Path libraryPath() {
        String path = System.getProperty("rustorb.ffi.library", System.getenv("RUSTORB_FFI_LIBRARY"));
        if (path == null) {
            throw new UnsatisfiedLinkError("set rustorb.ffi.library to the path of librustorb_ffi");
        }
        return Path.of(path);
    }

    private static MethodHandle fn(String name, FunctionDescriptor fd, Linker.Option... options) {
        MemorySegment symbol = LIB.find(name).orElseThrow(() -> new UnsatisfiedLinkError(name));
        return LINKER.downcallHandle(symbol, fd, options);
    }

    private static FunctionDescriptor ret(MemoryLayout r, MemoryLayout... args) {
        return FunctionDescriptor.of(r, args);
    }

    private static FunctionDescriptor none(MemoryLayout... args) {
        return FunctionDescriptor.ofVoid(args);
    }

    // Short calls that do not call back may skip the thread state
    // transition (critical); the array ones also read Java arrays in place.
    private static final Linker.Option CRITICAL = Linker.Option.critical(false);
    private static final Linker.Option CRITICAL_HEAP = Linker.Option.critical(true);

    static final MethodHandle abiVersion = fn("rorb_abi_version", ret(JAVA_INT));
    static final MethodHandle exceptionClear = fn("rorb_exception_clear", none(ADDRESS));
    static final MethodHandle bytesFree = fn("rorb_bytes_free", none(BYTES));

    static final MethodHandle orbInit = fn("rorb_orb_init", ret(JAVA_INT, ADDRESS, ADDRESS, ADDRESS, ADDRESS));
    static final MethodHandle orbRelease = fn("rorb_orb_release", none(ADDRESS));
    static final MethodHandle orbStringToObject = fn("rorb_orb_string_to_object",
            ret(JAVA_INT, ADDRESS, ADDRESS, JAVA_LONG, ADDRESS, ADDRESS));
    static final MethodHandle orbObjectToString = fn("rorb_orb_object_to_string",
            ret(JAVA_INT, ADDRESS, ADDRESS, ADDRESS, ADDRESS));
    static final MethodHandle orbRootPoa = fn("rorb_orb_root_poa", ret(JAVA_INT, ADDRESS, ADDRESS, ADDRESS));
    static final MethodHandle orbRun = fn("rorb_orb_run", none(ADDRESS));
    static final MethodHandle orbShutdown = fn("rorb_orb_shutdown", ret(JAVA_INT, ADDRESS, JAVA_INT, ADDRESS));
    static final MethodHandle orbDestroy = fn("rorb_orb_destroy", ret(JAVA_INT, ADDRESS, ADDRESS));

    static final MethodHandle objectRelease = fn("rorb_object_release", none(ADDRESS));
    static final MethodHandle objectInvoke = fn("rorb_object_invoke", ret(JAVA_INT, ADDRESS, ADDRESS, ADDRESS));

    static final MethodHandle poaRelease = fn("rorb_poa_release", none(ADDRESS));
    static final MethodHandle poaTheManager = fn("rorb_poa_the_manager", ret(ADDRESS, ADDRESS));
    static final MethodHandle poaManagerRelease = fn("rorb_poa_manager_release", none(ADDRESS));
    static final MethodHandle poaManagerActivate = fn("rorb_poa_manager_activate", ret(JAVA_INT, ADDRESS, ADDRESS));
    static final MethodHandle poaServantToReference = fn("rorb_poa_servant_to_reference",
            ret(JAVA_INT, ADDRESS, ADDRESS, ADDRESS, ADDRESS));

    static final MethodHandle servantGet = fn("rorb_servant_get", ret(ADDRESS, ADDRESS, ADDRESS, JAVA_LONG, ADDRESS));
    static final MethodHandle servantRelease = fn("rorb_servant_release", none(ADDRESS));
    static final MethodHandle serverReplyOutput = fn("rorb_server_reply_output", ret(ADDRESS, ADDRESS), CRITICAL);
    static final MethodHandle serverReplySystemException = fn("rorb_server_reply_system_exception",
            none(ADDRESS, ADDRESS, JAVA_LONG, JAVA_INT, JAVA_INT));

    static final MethodHandle outLong = fn("rorb_out_write_long", none(ADDRESS, JAVA_INT), CRITICAL);
    static final MethodHandle outUlong = fn("rorb_out_write_ulong", none(ADDRESS, JAVA_INT), CRITICAL);
    static final MethodHandle outArray = fn("rorb_out_write_array",
            none(ADDRESS, JAVA_INT, ADDRESS, JAVA_LONG), CRITICAL_HEAP);
    static final MethodHandle inLong = fn("rorb_in_read_long", ret(JAVA_INT, ADDRESS), CRITICAL);
    static final MethodHandle inLength = fn("rorb_in_read_length", ret(JAVA_INT, ADDRESS, JAVA_LONG), CRITICAL);
    static final MethodHandle inArray = fn("rorb_in_read_array",
            none(ADDRESS, JAVA_INT, ADDRESS, JAVA_LONG), CRITICAL_HEAP);
    static final MethodHandle inFailed = fn("rorb_in_failed", ret(JAVA_INT, ADDRESS), CRITICAL);

    /** An upcall stub of the static method name of the class of lookup,
     * alive for good. */
    static MemorySegment upcall(MethodHandles.Lookup lookup, String name, FunctionDescriptor fd) {
        try {
            MethodHandle target = lookup.findStatic(lookup.lookupClass(), name, fd.toMethodType());
            return LINKER.upcallStub(target, fd, Arena.global());
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    static String utf8(MemorySegment data, long len) {
        if (len == 0) {
            return "";
        }
        return new String(data.reinterpret(len).toArray(java.lang.foreign.ValueLayout.JAVA_BYTE),
                StandardCharsets.UTF_8);
    }

    /** Throws the exception in exc unless status is OK, and clears it. */
    static void check(int status, MemorySegment exc) {
        if (status == OK) {
            return;
        }
        String id = "status " + status;
        if (status == SYSTEM_EXCEPTION) {
            MemorySegment p = exc.get(ADDRESS, EXCEPTION.byteOffset(MemoryLayout.PathElement.groupElement("repository_id")));
            long len = exc.get(JAVA_LONG, EXCEPTION.byteOffset(MemoryLayout.PathElement.groupElement("repository_id_len")));
            id = utf8(p, len);
        }
        try {
            exceptionClear.invokeExact(exc);
        } catch (Throwable t) {
            throw new AssertionError(t);
        }
        throw new SystemError(id);
    }

    /** t as an unchecked exception (MethodHandle.invokeExact throws Throwable). */
    static RuntimeException rethrow(Throwable t) {
        if (t instanceof RuntimeException r) {
            throw r;
        }
        if (t instanceof Error e) {
            throw e;
        }
        throw new IllegalStateException(t);
    }

    /** A CORBA system exception (the prototype keeps only its id). */
    static final class SystemError extends RuntimeException {
        SystemError(String id) {
            super(id);
        }
    }
}
