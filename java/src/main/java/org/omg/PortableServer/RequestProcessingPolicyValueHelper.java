// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** The helper of PortableServer::RequestProcessingPolicyValue, for the anys of ORB.create_policy. */
public abstract class RequestProcessingPolicyValueHelper {
    private static volatile org.omg.CORBA.TypeCode typeCode;

    public static String id() {
        return "IDL:omg.org/PortableServer/RequestProcessingPolicyValue:2.3";
    }

    public static org.omg.CORBA.TypeCode type() {
        if (typeCode == null) {
            typeCode = org.omg.CORBA.ORB.init().create_enum_tc(id(), "RequestProcessingPolicyValue",
                    new String[] {"USE_ACTIVE_OBJECT_MAP_ONLY", "USE_DEFAULT_SERVANT", "USE_SERVANT_MANAGER"});
        }
        return typeCode;
    }

    public static void insert(org.omg.CORBA.Any a, RequestProcessingPolicyValue that) {
        org.rustorb.Anys.insert(a, type(), out -> write(out, that));
    }

    public static RequestProcessingPolicyValue extract(org.omg.CORBA.Any a) {
        return org.rustorb.Anys.extract(a, type(), RequestProcessingPolicyValueHelper::read);
    }

    public static RequestProcessingPolicyValue read(org.omg.CORBA.portable.InputStream in) {
        return RequestProcessingPolicyValue.from_int(in.read_ulong());
    }

    public static void write(org.omg.CORBA.portable.OutputStream out, RequestProcessingPolicyValue value) {
        out.write_ulong(value.value());
    }
}
