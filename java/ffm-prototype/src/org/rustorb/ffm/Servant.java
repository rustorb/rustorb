// A servant: the C ABI calls dispatch through a vtable whose self is a
// number naming the servant in a table. The table holds the servant while
// the Rust side has references to it (add_ref / remove_ref).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb.ffm;

import static java.lang.foreign.ValueLayout.ADDRESS;
import static java.lang.foreign.ValueLayout.JAVA_INT;
import static java.lang.foreign.ValueLayout.JAVA_LONG;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.MemoryLayout.PathElement;
import java.lang.foreign.MemorySegment;
import java.lang.invoke.MethodHandles;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicLong;

public abstract class Servant {
    private static final AtomicLong NEXT = new AtomicLong(1);
    private static final ConcurrentHashMap<Long, Entry> TABLE = new ConcurrentHashMap<>();

    private static final class Entry {
        final Servant servant;
        int refs;

        Entry(Servant servant) {
            this.servant = servant;
        }
    }

    private static final MemorySegment VTABLE = vtable();

    private static MemorySegment vtable() {
        MemorySegment v = Arena.global().allocate(Ffi.SERVANT_VTABLE);
        set(v, "dispatch", Ffi.upcall(MethodHandles.lookup(), "dispatch",
                FunctionDescriptor.of(JAVA_INT, ADDRESS, ADDRESS, JAVA_LONG, ADDRESS, ADDRESS)));
        set(v, "is_a", Ffi.upcall(MethodHandles.lookup(), "isA",
                FunctionDescriptor.of(JAVA_INT, ADDRESS, ADDRESS, JAVA_LONG)));
        set(v, "add_ref", Ffi.upcall(MethodHandles.lookup(), "addRef", FunctionDescriptor.ofVoid(ADDRESS)));
        set(v, "remove_ref", Ffi.upcall(MethodHandles.lookup(), "removeRef", FunctionDescriptor.ofVoid(ADDRESS)));
        return v;
    }

    private static void set(MemorySegment v, String field, MemorySegment fn) {
        Ffi.SERVANT_VTABLE.varHandle(PathElement.groupElement(field)).set(v, 0L, fn);
    }

    private final long id = NEXT.getAndIncrement();
    private final Entry entry = new Entry(this);
    private MemorySegment peer;

    /** The repository id of the most derived interface. */
    protected abstract String repositoryId();

    /** Runs op: reads the arguments from in and writes the reply to reply.output(). */
    protected abstract void invoke(String op, In in, Reply reply);

    /** The rorb_servant* of this servant (made once). */
    synchronized MemorySegment peer() {
        if (peer == null) {
            // Kept in the table from here: the Rust side adds a reference
            // in rorb_servant_get, before it returns.
            TABLE.put(id, entry);
            try (Arena arena = Arena.ofConfined()) {
                byte[] rid = repositoryId().getBytes(StandardCharsets.UTF_8);
                peer = (MemorySegment) Ffi.servantGet.invokeExact(MemorySegment.ofAddress(id),
                        arena.allocateFrom(repositoryId()), (long) rid.length, VTABLE);
            } catch (Throwable t) {
                throw Ffi.rethrow(t);
            }
        }
        return peer;
    }

    private static Servant of(MemorySegment self) {
        Entry e = TABLE.get(self.address());
        return e == null ? null : e.servant;
    }

    private static final byte[] UNKNOWN = "IDL:omg.org/CORBA/UNKNOWN:1.0".getBytes(StandardCharsets.UTF_8);
    private static final MemorySegment UNKNOWN_ID = Arena.global().allocateFrom(
            "IDL:omg.org/CORBA/UNKNOWN:1.0");

    private static int dispatch(MemorySegment self, MemorySegment op, long opLen, MemorySegment in,
            MemorySegment reply) {
        try {
            Servant s = of(self);
            s.invoke(Ffi.utf8(op, opLen), new In(in), new Reply(reply));
        } catch (Throwable t) {
            try {
                // COMPLETED_MAYBE
                Ffi.serverReplySystemException.invokeExact(reply, UNKNOWN_ID, (long) UNKNOWN.length, 0, 2);
            } catch (Throwable ignored) {
                // Nothing else to do.
            }
        }
        return Ffi.OK;
    }

    private static int isA(MemorySegment self, MemorySegment id, long len) {
        try {
            return of(self).repositoryId().equals(Ffi.utf8(id, len)) ? 1 : 0;
        } catch (Throwable t) {
            return 0;
        }
    }

    private static void addRef(MemorySegment self) {
        Entry e = TABLE.get(self.address());
        if (e != null) {
            synchronized (e) {
                e.refs++;
            }
        }
    }

    private static void removeRef(MemorySegment self) {
        Entry e = TABLE.get(self.address());
        if (e != null) {
            synchronized (e) {
                if (--e.refs == 0) {
                    TABLE.remove(self.address());
                }
            }
        }
    }
}
