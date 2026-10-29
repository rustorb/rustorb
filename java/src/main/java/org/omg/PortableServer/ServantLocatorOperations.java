// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

import org.omg.PortableServer.ServantLocatorPackage.CookieHolder;

/** The operations of PortableServer::ServantLocator. */
public interface ServantLocatorOperations extends ServantManagerOperations {
    Servant preinvoke(byte[] oid, POA adapter, String operation, CookieHolder the_cookie)
            throws ForwardRequest;

    void postinvoke(byte[] oid, POA adapter, String operation, java.lang.Object the_cookie,
            Servant the_servant);
}
