// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer.POAPackage;

/** PortableServer::POA::InvalidPolicy */
public final class InvalidPolicy extends org.omg.CORBA.UserException {
    public short index;

    public InvalidPolicy() {
        super("IDL:omg.org/PortableServer/POA/InvalidPolicy:1.0");
    }

    public InvalidPolicy(short index) {
        this();
        this.index = index;
    }

    public InvalidPolicy(String reason, short index) {
        super(reason);
        this.index = index;
    }
}
