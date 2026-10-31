// An input stream of the C ABI (rorb_in), valid during the callback that
// got it. Only what the comparison needs.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb.ffm;

import java.lang.foreign.MemorySegment;

public final class In {
    MemorySegment handle;

    In(MemorySegment handle) {
        this.handle = handle;
    }

    public int readLong() {
        try {
            return (int) Ffi.inLong.invokeExact(handle);
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }

    /** A sequence<octet>, its length checked against the bytes left. */
    public byte[] readOctets() {
        try {
            int n = (int) Ffi.inLength.invokeExact(handle, 1L);
            byte[] v = new byte[n];
            Ffi.inArray.invokeExact(handle, Ffi.PRIM_OCTET, MemorySegment.ofArray(v), (long) n);
            return v;
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }

    /** A failed read is MARSHAL, as the stream records it. */
    boolean failed() {
        try {
            return (int) Ffi.inFailed.invokeExact(handle) != 0;
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }
}
