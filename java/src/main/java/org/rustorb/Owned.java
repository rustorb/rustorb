// A handle of the C ABI owned by a Java object, released by a Cleaner when
// the object is unreachable (or earlier, by release()).
//
// Code that passes value to a native method keeps the owner reachable
// until the call returns (Reference.reachabilityFence), so that the
// Cleaner does not release the handle during the call.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Cleaner;
import java.util.concurrent.atomic.AtomicLong;

final class Owned implements Runnable {
    interface Releaser {
        void release(long handle);
    }

    private static final Cleaner CLEANER = Cleaner.create();

    /** Handles registered and not yet released (Diagnostics.liveHandles). */
    static final AtomicLong LIVE = new AtomicLong();

    /** The handle; replaced by set(). */
    private volatile long value;
    private final Releaser releaser;

    private Owned(long value, Releaser releaser) {
        this.value = value;
        this.releaser = releaser;
    }

    static Owned register(Object owner, long handle, Releaser releaser) {
        Owned o = new Owned(handle, releaser);
        if (handle != 0) {
            LIVE.incrementAndGet();
        }
        CLEANER.register(owner, o);
        return o;
    }

    long get() {
        return value;
    }

    /** Replaces the handle, releasing the old one. */
    synchronized void set(long handle) {
        long old = value;
        value = handle;
        if (old == handle) {
            return;
        }
        if (handle != 0) {
            LIVE.incrementAndGet();
        }
        if (old != 0) {
            LIVE.decrementAndGet();
            releaser.release(old);
        }
    }

    @Override
    public synchronized void run() {
        long v = value;
        value = 0;
        if (v != 0) {
            LIVE.decrementAndGet();
            releaser.release(v);
        }
    }
}
