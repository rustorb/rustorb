// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** PortableServer::AdapterActivator: implemented by extending
 * org.omg.CORBA.LocalObject. unknown_adapter runs on an upcall thread. */
public interface AdapterActivator extends AdapterActivatorOperations, org.omg.CORBA.Object,
        org.omg.CORBA.portable.IDLEntity {}
