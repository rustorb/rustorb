// The reply of a request a servant runs (rorb_server_reply).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb.ffm;

import java.lang.foreign.MemorySegment;

public final class Reply {
    private final MemorySegment handle;

    Reply(MemorySegment handle) {
        this.handle = handle;
    }

    /** The stream of a normal reply. */
    public Out output() {
        try {
            return new Out((MemorySegment) Ffi.serverReplyOutput.invokeExact(handle));
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        }
    }
}
