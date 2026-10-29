// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** The Helper of PortableServer::Current. */
public abstract class CurrentHelper {
    public static String id() {
        return "IDL:omg.org/PortableServer/Current:1.0";
    }

    public static Current narrow(org.omg.CORBA.Object obj) {
        if (obj == null) {
            return null;
        }
        if (obj instanceof Current) {
            return (Current) obj;
        }
        throw new org.omg.CORBA.BAD_PARAM();
    }
}
