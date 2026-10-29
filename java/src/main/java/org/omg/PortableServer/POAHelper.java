// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** The Helper of PortableServer::POA. */
public abstract class POAHelper {
    private static final String _id = "IDL:omg.org/PortableServer/POA:1.0";

    public static String id() {
        return _id;
    }

    public static POA narrow(org.omg.CORBA.Object obj) {
        if (obj == null) {
            return null;
        }
        if (obj instanceof POA) {
            return (POA) obj;
        }
        throw new org.omg.CORBA.BAD_PARAM();
    }

    public static POA unchecked_narrow(org.omg.CORBA.Object obj) {
        return narrow(obj);
    }
}
