// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer.CurrentPackage;

/** PortableServer::Current::NoContext */
public final class NoContext extends org.omg.CORBA.UserException {
    public NoContext() {
        super("IDL:omg.org/PortableServer/Current/NoContext:1.0");
    }

    public NoContext(String reason) {
        super(reason);
    }
}
