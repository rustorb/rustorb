// The Java instances the core's valuetype instances stand for.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

/**
 * Java instances handed to the core by number, so that one valuetype
 * instance put in several anys stays one (rorb_out_begin_host_value): the
 * core keeps a reference to each instance (a count here) while one of its
 * anys holds it, and an instance has the same number while it is kept, so
 * that inserting it again finds the core's instance.
 */
final class Hosts {
    private Hosts() {}

    private static final class Entry {
        final long id;
        final Object value;
        int count;

        Entry(long id, Object value) {
            this.id = id;
            this.value = value;
        }
    }

    private static final Object LOCK = new Object();
    private static final java.util.IdentityHashMap<Object, Entry> BY_VALUE =
            new java.util.IdentityHashMap<>();
    private static final java.util.HashMap<Long, Entry> BY_ID = new java.util.HashMap<>();
    /** Numbers apart from the keys of NativeOutputStream (1 << 62). */
    private static long next = 1L << 61;

    /** A new reference to v: its number, the same while it is kept. */
    static long retain(Object v) {
        synchronized (LOCK) {
            Entry e = BY_VALUE.get(v);
            if (e == null) {
                e = new Entry(++next, v);
                BY_VALUE.put(v, e);
                BY_ID.put(e.id, e);
            }
            e.count++;
            return e.id;
        }
    }

    /** Releases a reference from retain (the core does, from any thread). */
    static void release(long id) {
        synchronized (LOCK) {
            Entry e = BY_ID.get(id);
            if (e != null && --e.count == 0) {
                BY_ID.remove(id);
                BY_VALUE.remove(e.value);
            }
        }
    }

    /** The instance of a number kept, or null. */
    static Object get(long id) {
        synchronized (LOCK) {
            Entry e = BY_ID.get(id);
            return e == null ? null : e.value;
        }
    }

    /** How many instances are kept (for tests). */
    static int size() {
        synchronized (LOCK) {
            return BY_ID.size();
        }
    }
}
