// An ORB, its RootPOA and object references, on the C ABI. Only what the
// comparison needs; handles are released by close().
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb.ffm;

import static java.lang.foreign.ValueLayout.ADDRESS;
import static java.lang.foreign.ValueLayout.JAVA_INT;
import static java.lang.foreign.ValueLayout.JAVA_LONG;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.nio.charset.StandardCharsets;

public final class Orb implements AutoCloseable {
    static {
        try {
            int abi = (int) Ffi.abiVersion.invokeExact();
            if (abi != 1) {
                throw new UnsatisfiedLinkError("librustorb_ffi has ABI version " + abi + ", not 1");
            }
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }

    private final MemorySegment orb;
    private final MemorySegment poa;

    /** rorb_orb_init with args, and the RootPOA, activated. */
    public Orb(String... args) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment argv = arena.allocate(ADDRESS, args.length + 1L);
            for (int i = 0; i < args.length; i++) {
                argv.setAtIndex(ADDRESS, i, arena.allocateFrom(args[i]));
            }
            MemorySegment argc = arena.allocateFrom(JAVA_INT, args.length);
            MemorySegment out = arena.allocate(ADDRESS);
            MemorySegment exc = arena.allocate(Ffi.EXCEPTION);
            Ffi.check((int) Ffi.orbInit.invokeExact(argc, argv, out, exc), exc);
            orb = out.get(ADDRESS, 0);
            Ffi.check((int) Ffi.orbRootPoa.invokeExact(orb, out, exc), exc);
            poa = out.get(ADDRESS, 0);
            MemorySegment manager = (MemorySegment) Ffi.poaTheManager.invokeExact(poa);
            Ffi.check((int) Ffi.poaManagerActivate.invokeExact(manager, exc), exc);
            Ffi.poaManagerRelease.invokeExact(manager);
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }

    /** A reference to the servant, activated in the RootPOA (rorb_object*). */
    public MemorySegment activate(Servant servant) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = arena.allocate(ADDRESS);
            MemorySegment exc = arena.allocate(Ffi.EXCEPTION);
            Ffi.check((int) Ffi.poaServantToReference.invokeExact(poa, servant.peer(), out, exc), exc);
            return out.get(ADDRESS, 0);
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }

    public String objectToString(MemorySegment obj) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment bytes = arena.allocate(Ffi.BYTES);
            MemorySegment exc = arena.allocate(Ffi.EXCEPTION);
            Ffi.check((int) Ffi.orbObjectToString.invokeExact(orb, obj, bytes, exc), exc);
            String s = Ffi.utf8(bytes.get(ADDRESS, 0), bytes.get(JAVA_LONG, 8));
            Ffi.bytesFree.invokeExact(bytes);
            return s;
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }

    public MemorySegment stringToObject(String s) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment out = arena.allocate(ADDRESS);
            MemorySegment exc = arena.allocate(Ffi.EXCEPTION);
            long len = s.getBytes(StandardCharsets.UTF_8).length;
            Ffi.check((int) Ffi.orbStringToObject.invokeExact(orb, arena.allocateFrom(s), len, out, exc), exc);
            return out.get(ADDRESS, 0);
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }

    public static void release(MemorySegment obj) {
        try {
            Ffi.objectRelease.invokeExact(obj);
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }

    /** rorb_orb_run: blocks until shutdown. */
    public void run() {
        try {
            Ffi.orbRun.invokeExact(orb);
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }

    public void shutdown(boolean wait) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment exc = arena.allocate(Ffi.EXCEPTION);
            Ffi.check((int) Ffi.orbShutdown.invokeExact(orb, wait ? 1 : 0, exc), exc);
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }

    @Override
    public void close() {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment exc = arena.allocate(Ffi.EXCEPTION);
            Ffi.check((int) Ffi.orbDestroy.invokeExact(orb, exc), exc);
            Ffi.poaRelease.invokeExact(poa);
            Ffi.orbRelease.invokeExact(orb);
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }
}
