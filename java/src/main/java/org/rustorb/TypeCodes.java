// TypeCode constructors (used by org.omg.CORBA.ORB) and anys.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.StructMember;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.UnionMember;

public final class TypeCodes {
    private TypeCodes() {}

    static {
        Native.ensureLoaded();
    }

    private static final TypeCodeImpl[] PRIMITIVES = new TypeCodeImpl[37];

    private static BAD_PARAM badParam(String why) {
        return new BAD_PARAM(why, 0, CompletionStatus.COMPLETED_NO);
    }

    /** A TypeCode without parameters (string and wstring unbounded). */
    public static TypeCode primitive(TCKind kind) {
        int k = kind.value();
        synchronized (PRIMITIVES) {
            if (PRIMITIVES[k] == null) {
                PRIMITIVES[k] = TypeCodeImpl.of(Native.tcSimple(k));
            }
            return PRIMITIVES[k];
        }
    }

    public static Any newAny() {
        return new AnyImpl();
    }

    public static TypeCode string(boolean wide, int bound) {
        return TypeCodeImpl.of(Native.tcString(wide, bound));
    }

    public static TypeCode fixed(short digits, short scale) {
        return TypeCodeImpl.of(Native.tcFixed(digits, scale));
    }

    public static TypeCode sequence(TypeCode content, int bound) {
        TypeCodeImpl c = TypeCodeImpl.cast(content);
        try {
            return TypeCodeImpl.of(Native.tcSequence(c.handle(), bound));
        } finally {
            Reference.reachabilityFence(c);
        }
    }

    public static TypeCode array(TypeCode content, int length) {
        TypeCodeImpl c = TypeCodeImpl.cast(content);
        try {
            return TypeCodeImpl.of(Native.tcArray(c.handle(), length));
        } finally {
            Reference.reachabilityFence(c);
        }
    }

    public static TypeCode alias(int kind, String id, String name, TypeCode content) {
        TypeCodeImpl c = TypeCodeImpl.cast(content);
        try {
            return TypeCodeImpl.of(Native.tcAlias(kind, id, name, c.handle()));
        } finally {
            Reference.reachabilityFence(c);
        }
    }

    public static TypeCode iface(int kind, String id, String name) {
        return TypeCodeImpl.of(Native.tcInterface(kind, id, name));
    }

    public static TypeCode struct(int kind, String id, String name, StructMember[] members) {
        if (members == null) {
            throw badParam("null members");
        }
        String[] names = new String[members.length];
        TypeCodeImpl[] types = new TypeCodeImpl[members.length];
        long[] handles = new long[members.length];
        for (int i = 0; i < members.length; i++) {
            names[i] = members[i].name;
            types[i] = TypeCodeImpl.cast(members[i].type);
            handles[i] = types[i].handle();
        }
        try {
            return TypeCodeImpl.of(Native.tcStruct(kind, id, name, names, handles));
        } finally {
            Reference.reachabilityFence(types);
        }
    }

    public static TypeCode union(String id, String name, TypeCode discriminator,
            UnionMember[] members) {
        if (members == null) {
            throw badParam("null members");
        }
        TypeCodeImpl d = TypeCodeImpl.cast(discriminator);
        String[] names = new String[members.length];
        TypeCodeImpl[] types = new TypeCodeImpl[members.length];
        long[] handles = new long[members.length];
        long[] labels = new long[members.length];
        int defaultIndex = -1;
        long[] label = new long[1];
        for (int i = 0; i < members.length; i++) {
            names[i] = members[i].name;
            types[i] = TypeCodeImpl.cast(members[i].type);
            handles[i] = types[i].handle();
            AnyImpl l = AnyImpl.cast(members[i].label);
            // The default member has the octet 0 as its label.
            if (l.type().kind().value() == TCKind._tk_octet) {
                if (defaultIndex >= 0) {
                    throw badParam("two default members");
                }
                defaultIndex = i;
                continue;
            }
            if (!Native.anyLabel(l.handle(), label)) {
                throw badParam("a label that is not of a discriminator type");
            }
            Reference.reachabilityFence(l);
            labels[i] = label[0];
        }
        try {
            return TypeCodeImpl.of(Native.tcUnion(id, name, d.handle(), labels, defaultIndex,
                    names, handles));
        } finally {
            Reference.reachabilityFence(d);
            Reference.reachabilityFence(types);
        }
    }

    public static TypeCode enumeration(String id, String name, String[] members) {
        if (members == null) {
            throw badParam("null members");
        }
        return TypeCodeImpl.of(Native.tcEnum(id, name, members));
    }

    public static TypeCode value(String id, String name, short modifier, TypeCode base,
            org.omg.CORBA.ValueMember[] members) {
        if (members == null) {
            throw badParam("null members");
        }
        TypeCodeImpl b = null;
        if (base != null && base.kind().value() != TCKind._tk_null) {
            b = TypeCodeImpl.cast(base);
        }
        String[] names = new String[members.length];
        TypeCodeImpl[] types = new TypeCodeImpl[members.length];
        long[] handles = new long[members.length];
        short[] access = new short[members.length];
        for (int i = 0; i < members.length; i++) {
            names[i] = members[i].name;
            types[i] = TypeCodeImpl.cast(members[i].type);
            handles[i] = types[i].handle();
            access[i] = members[i].access;
        }
        try {
            return TypeCodeImpl.of(Native.tcValue(id, name, modifier, b == null ? 0 : b.handle(),
                    names, handles, access));
        } finally {
            Reference.reachabilityFence(types);
            Reference.reachabilityFence(b);
        }
    }

    public static TypeCode recursive(String id) {
        return TypeCodeImpl.of(Native.tcRecursive(id));
    }
}
