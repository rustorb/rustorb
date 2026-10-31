// An output stream of the C ABI (rorb_out), valid during the callback that
// got it. Only what the comparison needs.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb.ffm;

import java.lang.foreign.MemorySegment;

public final class Out {
    MemorySegment handle;

    Out(MemorySegment handle) {
        this.handle = handle;
    }

    public void writeLong(int v) {
        try {
            Ffi.outLong.invokeExact(handle, v);
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }

    /** A sequence<octet>: the length and the bytes, read in place. */
    public void writeOctets(byte[] v) {
        try {
            Ffi.outUlong.invokeExact(handle, v.length);
            Ffi.outArray.invokeExact(handle, Ffi.PRIM_OCTET, MemorySegment.ofArray(v), (long) v.length);
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }
}
