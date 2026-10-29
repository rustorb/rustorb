// Value factories and the valuetypes of generated classes, found by
// repository id.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;

import org.omg.CORBA.TypeCode;
import org.omg.CORBA.portable.ValueFactory;

public final class Values {
    private Values() {}

    private static final Map<String, ValueFactory> FACTORIES = new ConcurrentHashMap<>();
    /** Repository ids without a factory class, or without a Helper. */
    private static final Set<String> NO_FACTORY = ConcurrentHashMap.newKeySet();
    private static final Set<String> NO_HELPER = ConcurrentHashMap.newKeySet();
    /** Package prefixes ("a.b.") of generated code (rustorb-idl -Wbpackage). */
    private static final List<String> PACKAGES = new CopyOnWriteArrayList<>(List.of(""));

    public static ValueFactory register(String id, ValueFactory factory) {
        if (id == null) {
            throw new org.omg.CORBA.BAD_PARAM("null repository id", 0,
                    org.omg.CORBA.CompletionStatus.COMPLETED_NO);
        }
        NO_FACTORY.remove(id);
        return factory == null ? FACTORIES.remove(id) : FACTORIES.put(id, factory);
    }

    public static void unregister(String id) {
        if (id != null) {
            FACTORIES.remove(id);
        }
    }

    /** The registered factory of id, else an instance of the generated
     * class <type>DefaultFactory; null if there is none. */
    public static ValueFactory lookup(String id) {
        if (id == null) {
            return null;
        }
        ValueFactory f = FACTORIES.get(id);
        if (f != null || NO_FACTORY.contains(id)) {
            return f;
        }
        Class<?> c = generated(id, "DefaultFactory");
        if (c != null && ValueFactory.class.isAssignableFrom(c)) {
            try {
                f = (ValueFactory) c.getDeclaredConstructor().newInstance();
                ValueFactory old = FACTORIES.putIfAbsent(id, f);
                return old != null ? old : f;
            } catch (ReflectiveOperationException e) {
                // no usable constructor
            }
        }
        NO_FACTORY.add(id);
        return null;
    }

    /** Called by the static initialisers of Helpers generated with a
     * package prefix ("a.b."), so that their classes are found by id. */
    public static void addPackage(String prefix) {
        if (!PACKAGES.contains(prefix)) {
            PACKAGES.add(prefix);
            NO_FACTORY.clear();
            NO_HELPER.clear();
        }
    }

    /** The TypeCode of the valuetype id, from its generated Helper; null if
     * there is none. */
    static TypeCode typeCode(String id) {
        if (NO_HELPER.contains(id)) {
            return null;
        }
        Class<?> c = generated(id, "Helper");
        if (c != null) {
            try {
                Object tc = c.getMethod("type").invoke(null);
                if (tc instanceof TypeCode) {
                    return (TypeCode) tc;
                }
            } catch (ReflectiveOperationException | RuntimeException e) {
                // not a Helper
            }
        }
        NO_HELPER.add(id);
        return null;
    }

    /** Bridge.resolveValueType: a new handle of the TypeCode of id, or 0. */
    static long resolve(String id) {
        TypeCode tc = typeCode(id);
        if (!(tc instanceof TypeCodeImpl)) {
            return 0;
        }
        TypeCodeImpl t = (TypeCodeImpl) tc;
        try {
            return Native.tcClone(t.handle());
        } finally {
            Reference.reachabilityFence(t);
        }
    }

    /** The generated class of id with suffix ("Helper"), by the names of
     * the IDL to Java mapping. */
    private static Class<?> generated(String id, String suffix) {
        for (String name : classNames(id)) {
            for (String prefix : PACKAGES) {
                try {
                    return Class.forName(prefix + name + suffix, true, loader());
                } catch (ClassNotFoundException | LinkageError e) {
                    // try the next name
                }
            }
        }
        return null;
    }

    private static ClassLoader loader() {
        ClassLoader l = Thread.currentThread().getContextClassLoader();
        return l != null ? l : Values.class.getClassLoader();
    }

    /** Java class names an IDL repository id may stand for:
     * "IDL:M/I/V:1.0" is M.I.V or M.IPackage.V, and a pragma prefix with
     * dots ("omg.org") is reversed into packages (org.omg). */
    static List<String> classNames(String id) {
        List<String> out = new ArrayList<>();
        if (!id.startsWith("IDL:")) {
            return out;
        }
        int colon = id.lastIndexOf(':');
        String path = colon > 4 ? id.substring(4, colon) : id.substring(4);
        String[] parts = path.split("/");
        if (parts.length == 0) {
            return out;
        }
        List<String> names = new ArrayList<>();
        names.add(String.join(".", parts));
        if (parts[0].contains(".") && parts.length > 1) {
            String[] host = parts[0].split("\\.");
            StringBuilder b = new StringBuilder();
            for (int i = host.length - 1; i >= 0; i--) {
                b.append(host[i]).append('.');
            }
            String[] rest = java.util.Arrays.copyOfRange(parts, 1, parts.length);
            names.add(b + String.join(".", rest));
        }
        for (String n : names) {
            out.add(n);
            int dot = n.lastIndexOf('.');
            if (dot > 0) {
                out.add(n.substring(0, dot) + "Package" + n.substring(dot));
            }
        }
        return out;
    }
}
