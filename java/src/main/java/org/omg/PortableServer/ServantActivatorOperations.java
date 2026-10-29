// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** The operations of PortableServer::ServantActivator. */
public interface ServantActivatorOperations extends ServantManagerOperations {
    Servant incarnate(byte[] oid, POA adapter) throws ForwardRequest;

    void etherealize(byte[] oid, POA adapter, Servant serv, boolean cleanup_in_progress,
            boolean remaining_activations);
}
