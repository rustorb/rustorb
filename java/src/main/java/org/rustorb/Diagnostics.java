// Counters of the rustorb Java runtime, for tests that look for leaks.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

/** Counters of the rustorb Java runtime, for leak tests. */
public final class Diagnostics {
    private Diagnostics() {}

    /**
     * The handles of the native library (ORBs, POAs, object references,
     * servants, TypeCodes, anys, DynAnys, input streams) that Java objects
     * hold now. A handle is released when its owner is garbage collected
     * (by a {@link java.lang.ref.Cleaner}) or destroyed.
     */
    public static long liveHandles() {
        return Owned.LIVE.get();
    }
}
