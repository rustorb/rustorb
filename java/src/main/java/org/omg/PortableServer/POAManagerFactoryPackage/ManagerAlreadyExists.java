// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer.POAManagerFactoryPackage;

/** PortableServer::POAManagerFactory::ManagerAlreadyExists */
public final class ManagerAlreadyExists extends org.omg.CORBA.UserException {
    public ManagerAlreadyExists() {
        super("IDL:omg.org/PortableServer/POAManagerFactory/ManagerAlreadyExists:1.0");
    }

    public ManagerAlreadyExists(String reason) {
        super(reason);
    }
}
