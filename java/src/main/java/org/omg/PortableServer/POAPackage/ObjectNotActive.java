// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer.POAPackage;

/** PortableServer::POA::ObjectNotActive */
public final class ObjectNotActive extends org.omg.CORBA.UserException {
    public ObjectNotActive() {
        super("IDL:omg.org/PortableServer/POA/ObjectNotActive:1.0");
    }

    public ObjectNotActive(String reason) {
        super(reason);
    }
}
