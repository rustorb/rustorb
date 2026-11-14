// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/**
 * A DSI servant: it receives every request of its objects in invoke,
 * without a skeleton. The first id of _all_interfaces(null, null) is the
 * repository id it is activated with.
 *
 * invoke reads the arguments and sets the result or a user exception on
 * the ServerRequest; an exception it throws is the reply instead: a
 * SystemException as itself, an org.omg.CORBA.UnknownUserException as
 * set_exception of its any, and any other UNKNOWN.
 */
public abstract class DynamicImplementation extends Servant {
    public abstract void invoke(org.omg.CORBA.ServerRequest request);
}
