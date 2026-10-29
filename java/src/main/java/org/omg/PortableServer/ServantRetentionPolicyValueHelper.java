// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** The helper of PortableServer::ServantRetentionPolicyValue, for the anys of ORB.create_policy. */
public abstract class ServantRetentionPolicyValueHelper {
    private static volatile org.omg.CORBA.TypeCode typeCode;

    public static String id() {
        return "IDL:omg.org/PortableServer/ServantRetentionPolicyValue:2.3";
    }

    public static org.omg.CORBA.TypeCode type() {
        if (typeCode == null) {
            typeCode = org.omg.CORBA.ORB.init().create_enum_tc(id(), "ServantRetentionPolicyValue",
                    new String[] {"RETAIN", "NON_RETAIN"});
        }
        return typeCode;
    }

    public static void insert(org.omg.CORBA.Any a, ServantRetentionPolicyValue that) {
        org.rustorb.Anys.insert(a, type(), out -> write(out, that));
    }

    public static ServantRetentionPolicyValue extract(org.omg.CORBA.Any a) {
        return org.rustorb.Anys.extract(a, type(), ServantRetentionPolicyValueHelper::read);
    }

    public static ServantRetentionPolicyValue read(org.omg.CORBA.portable.InputStream in) {
        return ServantRetentionPolicyValue.from_int(in.read_ulong());
    }

    public static void write(org.omg.CORBA.portable.OutputStream out, ServantRetentionPolicyValue value) {
        out.write_ulong(value.value());
    }
}
