// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** PortableServer::ServantManager. Not supported by the Java binding yet:
 * set_servant_manager raises NO_IMPLEMENT. */
public interface ServantManager extends ServantManagerOperations, org.omg.CORBA.Object,
        org.omg.CORBA.portable.IDLEntity {}
