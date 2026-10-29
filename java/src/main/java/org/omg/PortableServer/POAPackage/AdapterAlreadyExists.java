// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer.POAPackage;

/** PortableServer::POA::AdapterAlreadyExists */
public final class AdapterAlreadyExists extends org.omg.CORBA.UserException {
    public AdapterAlreadyExists() {
        super("IDL:omg.org/PortableServer/POA/AdapterAlreadyExists:1.0");
    }

    public AdapterAlreadyExists(String reason) {
        super(reason);
    }
}
