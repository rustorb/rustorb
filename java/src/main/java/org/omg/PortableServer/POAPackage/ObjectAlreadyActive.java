// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer.POAPackage;

/** PortableServer::POA::ObjectAlreadyActive */
public final class ObjectAlreadyActive extends org.omg.CORBA.UserException {
    public ObjectAlreadyActive() {
        super("IDL:omg.org/PortableServer/POA/ObjectAlreadyActive:1.0");
    }

    public ObjectAlreadyActive(String reason) {
        super(reason);
    }
}
