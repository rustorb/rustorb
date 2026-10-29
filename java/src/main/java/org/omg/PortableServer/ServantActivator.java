// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** PortableServer::ServantActivator, the servant manager of a POA with
 * RETAIN: implemented by extending org.omg.CORBA.LocalObject. Its methods
 * run on the ORB's upcall threads. */
public interface ServantActivator extends ServantActivatorOperations, ServantManager {}
