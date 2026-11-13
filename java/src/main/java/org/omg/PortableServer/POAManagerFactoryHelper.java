// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** The Helper of PortableServer::POAManagerFactory. */
public abstract class POAManagerFactoryHelper {
    public static String id() {
        return "IDL:omg.org/PortableServer/POAManagerFactory:1.0";
    }

    public static POAManagerFactory narrow(org.omg.CORBA.Object obj) {
        if (obj == null) {
            return null;
        }
        if (obj instanceof POAManagerFactory) {
            return (POAManagerFactory) obj;
        }
        throw new org.omg.CORBA.BAD_PARAM();
    }
}
