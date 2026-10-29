// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer.POAPackage;

/** PortableServer::POA::NoServant */
public final class NoServant extends org.omg.CORBA.UserException {
    public NoServant() {
        super("IDL:omg.org/PortableServer/POA/NoServant:1.0");
    }

    public NoServant(String reason) {
        super(reason);
    }
}
