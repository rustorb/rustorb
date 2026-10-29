// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** The helper of PortableServer::IdAssignmentPolicyValue, for the anys of ORB.create_policy. */
public abstract class IdAssignmentPolicyValueHelper {
    private static volatile org.omg.CORBA.TypeCode typeCode;

    public static String id() {
        return "IDL:omg.org/PortableServer/IdAssignmentPolicyValue:2.3";
    }

    public static org.omg.CORBA.TypeCode type() {
        if (typeCode == null) {
            typeCode = org.omg.CORBA.ORB.init().create_enum_tc(id(), "IdAssignmentPolicyValue",
                    new String[] {"USER_ID", "SYSTEM_ID"});
        }
        return typeCode;
    }

    public static void insert(org.omg.CORBA.Any a, IdAssignmentPolicyValue that) {
        org.rustorb.Anys.insert(a, type(), out -> write(out, that));
    }

    public static IdAssignmentPolicyValue extract(org.omg.CORBA.Any a) {
        return org.rustorb.Anys.extract(a, type(), IdAssignmentPolicyValueHelper::read);
    }

    public static IdAssignmentPolicyValue read(org.omg.CORBA.portable.InputStream in) {
        return IdAssignmentPolicyValue.from_int(in.read_ulong());
    }

    public static void write(org.omg.CORBA.portable.OutputStream out, IdAssignmentPolicyValue value) {
        out.write_ulong(value.value());
    }
}
