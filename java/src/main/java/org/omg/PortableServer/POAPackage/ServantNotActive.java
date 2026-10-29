// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer.POAPackage;

/** PortableServer::POA::ServantNotActive */
public final class ServantNotActive extends org.omg.CORBA.UserException {
    public ServantNotActive() {
        super("IDL:omg.org/PortableServer/POA/ServantNotActive:1.0");
    }

    public ServantNotActive(String reason) {
        super(reason);
    }
}
