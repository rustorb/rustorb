// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

import org.omg.PortableServer.CurrentPackage.NoContext;

/** The operations of PortableServer::Current: the request being run. */
public interface CurrentOperations extends org.omg.CORBA.CurrentOperations {
    POA get_POA() throws NoContext;

    byte[] get_object_id() throws NoContext;

    org.omg.CORBA.Object get_reference() throws NoContext;

    Servant get_servant() throws NoContext;
}
