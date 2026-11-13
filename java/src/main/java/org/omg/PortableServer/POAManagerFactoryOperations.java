// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

import org.omg.PortableServer.POAManagerFactoryPackage.ManagerAlreadyExists;

/** The operations of PortableServer::POAManagerFactory. */
public interface POAManagerFactoryOperations {
    POAManager create_POAManager(String id, org.omg.CORBA.Policy[] policies)
            throws ManagerAlreadyExists, org.omg.CORBA.PolicyError;

    POAManager[] list();

    POAManager find(String id);
}
