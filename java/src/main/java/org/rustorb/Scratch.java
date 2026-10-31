// Native memory for the arguments and results of one call of the C ABI: a
// stack per thread. A call takes its mark, allocates, and resets to the
// mark when it returns; callbacks on the same thread nest on top.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import static java.lang.foreign.ValueLayout.JAVA_BYTE;
import static java.lang.foreign.ValueLayout.JAVA_CHAR;
import static java.lang.foreign.ValueLayout.JAVA_INT;
import static java.lang.foreign.ValueLayout.JAVA_LONG;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;

final class Scratch {
    private static final long SIZE = 16 * 1024;
    private static final ThreadLocal<Scratch> STACK = ThreadLocal.withInitial(Scratch::new);

    // Freed when the thread's Scratch is collected.
    private final MemorySegment memory = Arena.ofAuto().allocate(SIZE, 16);
    private final long base = memory.address();
    private long top;
    /** Arenas of what did not fit, closed by reset. */
    private final ArrayList<Arena> large = new ArrayList<>();

    private Scratch() {}

    static Scratch get() {
        return STACK.get();
    }

    /** What reset takes to free what is allocated after this. */
    long mark() {
        return ((long) large.size() << 32) | top;
    }

    void reset(long mark) {
        top = mark & 0xffffffffL;
        int n = (int) (mark >>> 32);
        while (large.size() > n) {
            large.remove(large.size() - 1).close();
        }
    }

    /** The address of size zeroed bytes, aligned to 8. */
    long alloc(long size) {
        long off = (top + 7) & ~7L;
        if (off + size <= SIZE) {
            top = off + size;
            memory.asSlice(off, size).fill((byte) 0);
            return base + off;
        }
        Arena a = Arena.ofConfined();
        large.add(a);
        return a.allocate(Math.max(size, 1), 8).address();
    }

    /** A zeroed long (or pointer, or size_t) for a result. */
    long word() {
        return alloc(8);
    }

    static long getLong(long p) {
        return Library.MEMORY.get(JAVA_LONG, p);
    }

    static int getInt(long p) {
        return Library.MEMORY.get(JAVA_INT, p);
    }

    static void putLong(long p, long v) {
        Library.MEMORY.set(JAVA_LONG, p, v);
    }

    /** s as UTF-8 (NUL terminated); its byte length is utf8Length. */
    long utf8(String s) {
        byte[] b = s.getBytes(StandardCharsets.UTF_8);
        lastLength = b.length;
        long p = alloc(b.length + 1);
        MemorySegment.copy(b, 0, Library.MEMORY, JAVA_BYTE, p, b.length);
        return p;
    }

    /** The byte length of the last string utf8 wrote. */
    long lastLength;

    /** The UTF-16 units of s. */
    long utf16(String s) {
        int n = s.length();
        long p = alloc(2L * n);
        MemorySegment.copy(s.toCharArray(), 0, Library.MEMORY, JAVA_CHAR, p, n);
        return p;
    }

    /** An array of longs. */
    long longs(long[] v, int n) {
        long p = alloc(8L * n);
        MemorySegment.copy(v, 0, Library.MEMORY, JAVA_LONG, p, n);
        return p;
    }

    /** An array of ints. */
    long ints(int[] v, int n) {
        long p = alloc(4L * n);
        MemorySegment.copy(v, 0, Library.MEMORY, JAVA_INT, p, n);
        return p;
    }

    /** The bytes of v. */
    long bytes(byte[] v) {
        long p = alloc(v.length);
        MemorySegment.copy(v, 0, Library.MEMORY, JAVA_BYTE, p, v.length);
        return p;
    }
}
