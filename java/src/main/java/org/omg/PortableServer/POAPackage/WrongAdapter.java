// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer.POAPackage;

/** PortableServer::POA::WrongAdapter */
public final class WrongAdapter extends org.omg.CORBA.UserException {
    public WrongAdapter() {
        super("IDL:omg.org/PortableServer/POA/WrongAdapter:1.0");
    }

    public WrongAdapter(String reason) {
        super(reason);
    }
}
