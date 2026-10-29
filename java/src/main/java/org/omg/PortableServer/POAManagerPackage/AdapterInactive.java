// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer.POAManagerPackage;

/** PortableServer::POAManager::AdapterInactive */
public final class AdapterInactive extends org.omg.CORBA.UserException {
    public AdapterInactive() {
        super("IDL:omg.org/PortableServer/POAManager/AdapterInactive:1.0");
    }

    public AdapterInactive(String reason) {
        super(reason);
    }
}
