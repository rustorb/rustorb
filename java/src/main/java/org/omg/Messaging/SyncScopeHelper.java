// Part of the IDL to Java mapping (org.omg.Messaging), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
package org.omg.Messaging;

/** The helper of Messaging::SyncScope, an alias of short, for the anys of ORB.create_policy. */
public abstract class SyncScopeHelper {
    private static volatile org.omg.CORBA.TypeCode typeCode;

    public static String id() {
        return "IDL:omg.org/Messaging/SyncScope:1.0";
    }

    public static org.omg.CORBA.TypeCode type() {
        if (typeCode == null) {
            org.omg.CORBA.ORB orb = org.omg.CORBA.ORB.init();
            typeCode = orb.create_alias_tc(id(), "SyncScope",
                    orb.get_primitive_tc(org.omg.CORBA.TCKind.tk_short));
        }
        return typeCode;
    }

    public static void insert(org.omg.CORBA.Any a, short that) {
        org.rustorb.Anys.insert(a, type(), out -> write(out, that));
    }

    public static short extract(org.omg.CORBA.Any a) {
        return org.rustorb.Anys.extract(a, type(), SyncScopeHelper::read);
    }

    public static short read(org.omg.CORBA.portable.InputStream in) {
        return in.read_short();
    }

    public static void write(org.omg.CORBA.portable.OutputStream out, short value) {
        out.write_short(value);
    }
}
