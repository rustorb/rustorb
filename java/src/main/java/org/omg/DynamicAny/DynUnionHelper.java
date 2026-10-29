// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** The Helper of DynamicAny::DynUnion, a local interface. */
public abstract class DynUnionHelper {
    public static String id() {
        return "IDL:omg.org/DynamicAny/DynUnion:1.0";
    }

    public static DynUnion narrow(org.omg.CORBA.Object obj) {
        if (obj == null) {
            return null;
        }
        if (obj instanceof DynUnion) {
            return (DynUnion) obj;
        }
        throw new org.omg.CORBA.BAD_PARAM();
    }

    public static DynUnion unchecked_narrow(org.omg.CORBA.Object obj) {
        return narrow(obj);
    }
}
