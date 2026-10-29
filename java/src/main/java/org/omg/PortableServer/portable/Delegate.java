// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer.portable;

import org.omg.PortableServer.POA;
import org.omg.PortableServer.Servant;

/** The ORB's side of a servant. */
public interface Delegate {
    org.omg.CORBA.ORB orb(Servant self);

    org.omg.CORBA.Object this_object(Servant self);

    POA poa(Servant self);

    byte[] object_id(Servant self);

    POA default_POA(Servant self);

    boolean is_a(Servant self, String repository_id);

    boolean non_existent(Servant self);

    org.omg.CORBA.Object get_interface_def(Servant self);
}
