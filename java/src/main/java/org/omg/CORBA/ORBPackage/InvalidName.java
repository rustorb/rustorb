// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.ORBPackage;

/** CORBA::ORB::InvalidName */
public final class InvalidName extends org.omg.CORBA.UserException {
    public InvalidName() {
        super("IDL:omg.org/CORBA/ORB/InvalidName:1.0");
    }

    public InvalidName(String reason) {
        super(reason);
    }
}
