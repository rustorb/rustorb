// The DII lists (NVList, NamedValue, ExceptionList, ContextList,
// Environment) and the ORB's operations on requests: what
// org.omg.CORBA.ORB calls. See docs/dii-dsi.md.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;

import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.Bounds;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.ContextList;
import org.omg.CORBA.Environment;
import org.omg.CORBA.ExceptionList;
import org.omg.CORBA.NVList;
import org.omg.CORBA.NamedValue;
import org.omg.CORBA.Request;
import org.omg.CORBA.TypeCode;

public final class Dii {
    private Dii() {}

    // --- the lists --------------------------------------------------------------

    static final class NamedValueImpl extends NamedValue {
        private final String name;
        private final Any value;
        private final int flags;

        NamedValueImpl(String name, Any value, int flags) {
            this.name = name == null ? "" : name;
            this.value = value;
            this.flags = flags;
        }

        @Override
        public String name() {
            return name;
        }

        @Override
        public Any value() {
            return value;
        }

        @Override
        public int flags() {
            return flags;
        }
    }

    /** A list of items, the base of the lists below. */
    private static class Items<T> {
        final List<T> items = new ArrayList<>();

        public synchronized int count() {
            return items.size();
        }

        synchronized T add(T item) {
            items.add(item);
            return item;
        }

        public synchronized T item(int index) throws Bounds {
            if (index < 0 || index >= items.size()) {
                throw new Bounds();
            }
            return items.get(index);
        }

        public synchronized void remove(int index) throws Bounds {
            if (index < 0 || index >= items.size()) {
                throw new Bounds();
            }
            items.remove(index);
        }

        /** A copy of the items. */
        synchronized List<T> snapshot() {
            return new ArrayList<>(items);
        }
    }

    static final class NVListImpl extends NVList {
        private final Items<NamedValue> items = new Items<>();

        @Override
        public int count() {
            return items.count();
        }

        @Override
        public NamedValue add(int flags) {
            return items.add(new NamedValueImpl("", new AnyImpl(), flags));
        }

        @Override
        public NamedValue add_item(String item_name, int flags) {
            return items.add(new NamedValueImpl(item_name, new AnyImpl(), flags));
        }

        @Override
        public NamedValue add_value(String item_name, Any val, int flags) {
            return items.add(new NamedValueImpl(item_name, val == null ? new AnyImpl() : val,
                    flags));
        }

        @Override
        public NamedValue item(int index) throws Bounds {
            return items.item(index);
        }

        @Override
        public void remove(int index) throws Bounds {
            items.remove(index);
        }

        List<NamedValue> snapshot() {
            return items.snapshot();
        }
    }

    static final class ExceptionListImpl extends ExceptionList {
        private final Items<TypeCode> items = new Items<>();

        @Override
        public int count() {
            return items.count();
        }

        @Override
        public void add(TypeCode exc) {
            items.add(TypeCodeImpl.cast(exc));
        }

        @Override
        public TypeCode item(int index) throws Bounds {
            return items.item(index);
        }

        @Override
        public void remove(int index) throws Bounds {
            items.remove(index);
        }

        List<TypeCode> snapshot() {
            return items.snapshot();
        }
    }

    static final class ContextListImpl extends ContextList {
        private final Items<String> items = new Items<>();

        @Override
        public int count() {
            return items.count();
        }

        @Override
        public void add(String ctx) {
            if (ctx == null) {
                throw new BAD_PARAM("null context name", 0, CompletionStatus.COMPLETED_NO);
            }
            items.add(ctx);
        }

        @Override
        public String item(int index) throws Bounds {
            return items.item(index);
        }

        @Override
        public void remove(int index) throws Bounds {
            items.remove(index);
        }
    }

    static final class EnvironmentImpl extends Environment {
        private Exception exception;

        @Override
        public synchronized Exception exception() {
            return exception;
        }

        @Override
        public synchronized void exception(Exception except) {
            exception = except;
        }

        @Override
        public synchronized void clear() {
            exception = null;
        }
    }

    /** The items of a list of rustorb (BAD_PARAM for another). */
    static List<NamedValue> items(NVList list) {
        if (list instanceof NVListImpl) {
            return ((NVListImpl) list).snapshot();
        }
        throw new BAD_PARAM(list == null ? "null NVList" : "not an NVList of rustorb", 0,
                CompletionStatus.COMPLETED_NO);
    }

    static List<TypeCode> items(ExceptionList list) {
        if (list instanceof ExceptionListImpl) {
            return ((ExceptionListImpl) list).snapshot();
        }
        throw new BAD_PARAM(list == null ? "null ExceptionList" : "not an ExceptionList of rustorb",
                0, CompletionStatus.COMPLETED_NO);
    }

    // --- ORB --------------------------------------------------------------------

    public static NVList createList() {
        return new NVListImpl();
    }

    public static NamedValue createNamedValue(String name, Any value, int flags) {
        return new NamedValueImpl(name, value == null ? new AnyImpl() : value, flags);
    }

    public static ExceptionList createExceptionList() {
        return new ExceptionListImpl();
    }

    public static ContextList createContextList() {
        return new ContextListImpl();
    }

    public static Environment createEnvironment() {
        return new EnvironmentImpl();
    }

    private static RequestImpl cast(Request r) {
        if (r instanceof RequestImpl) {
            return (RequestImpl) r;
        }
        throw new BAD_PARAM(r == null ? "null request" : "not a request of rustorb", 0,
                CompletionStatus.COMPLETED_NO);
    }

    public static void sendMultipleOneway(Request[] requests) {
        if (requests == null) {
            throw new BAD_PARAM("null requests", 0, CompletionStatus.COMPLETED_NO);
        }
        for (Request r : requests) {
            cast(r).send_oneway();
        }
    }

    public static void sendMultipleDeferred(Request[] requests) {
        if (requests == null) {
            throw new BAD_PARAM("null requests", 0, CompletionStatus.COMPLETED_NO);
        }
        for (Request r : requests) {
            cast(r).send_deferred();
        }
    }

    // --- the deferred requests of each ORB -----------------------------------------

    /** The requests sent deferred and not yet taken, for each ORB, in the
     * order sent. */
    private static final Map<OrbImpl, List<RequestImpl>> DEFERRED = new IdentityHashMap<>();

    static void deferred(OrbImpl orb, RequestImpl r) {
        synchronized (DEFERRED) {
            DEFERRED.computeIfAbsent(orb, o -> new ArrayList<>()).add(r);
        }
    }

    static void taken(OrbImpl orb, RequestImpl r) {
        synchronized (DEFERRED) {
            List<RequestImpl> l = DEFERRED.get(orb);
            if (l != null) {
                l.remove(r);
                if (l.isEmpty()) {
                    DEFERRED.remove(orb);
                }
            }
        }
    }

    private static List<RequestImpl> outstanding(OrbImpl orb) {
        synchronized (DEFERRED) {
            List<RequestImpl> l = DEFERRED.get(orb);
            return l == null ? new ArrayList<>() : new ArrayList<>(l);
        }
    }

    /** ORB.poll_next_response. */
    static boolean pollNext(OrbImpl orb) {
        for (RequestImpl r : outstanding(orb)) {
            if (r.ready()) {
                return true;
            }
        }
        return false;
    }

    /** ORB.get_next_response: looks at the outstanding requests every
     * millisecond (there is no notice of the order replies arrive in). A
     * system exception of the call is in the request's env(). */
    static Request getNext(OrbImpl orb) {
        while (true) {
            List<RequestImpl> l = outstanding(orb);
            if (l.isEmpty()) {
                throw new BAD_INV_ORDER("no request was sent deferred", 0,
                        CompletionStatus.COMPLETED_NO);
            }
            for (RequestImpl r : l) {
                if (r.ready() && r.take()) {
                    return r;
                }
            }
            try {
                Thread.sleep(1);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                throw new org.omg.CORBA.TRANSIENT("interrupted", 0,
                        CompletionStatus.COMPLETED_MAYBE);
            }
        }
    }
}
