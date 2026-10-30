// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** The Helper of DynamicAny::DynSequence, a local interface. */
public abstract class DynSequenceHelper {
    public static String id() {
        return "IDL:omg.org/DynamicAny/DynSequence:1.0";
    }

    public static DynSequence narrow(org.omg.CORBA.Object obj) {
        if (obj == null) {
            return null;
        }
        if (obj instanceof DynSequence) {
            return (DynSequence) obj;
        }
        throw new org.omg.CORBA.BAD_PARAM();
    }

    public static DynSequence unchecked_narrow(org.omg.CORBA.Object obj) {
        return narrow(obj);
    }
}
