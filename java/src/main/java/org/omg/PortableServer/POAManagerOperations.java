// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

import org.omg.PortableServer.POAManagerPackage.AdapterInactive;
import org.omg.PortableServer.POAManagerPackage.State;

/** The operations of PortableServer::POAManager. */
public interface POAManagerOperations {
    void activate() throws AdapterInactive;

    void hold_requests(boolean wait_for_completion) throws AdapterInactive;

    void discard_requests(boolean wait_for_completion) throws AdapterInactive;

    void deactivate(boolean etherealize_objects, boolean wait_for_completion)
            throws AdapterInactive;

    State get_state();

    String get_id();
}
