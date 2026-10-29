// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** PortableServer::ForwardRequest: raised by a servant manager to send the
 * client to another object. */
public final class ForwardRequest extends org.omg.CORBA.UserException {
    public org.omg.CORBA.Object forward_reference;

    public ForwardRequest() {
        super("IDL:omg.org/PortableServer/ForwardRequest:1.0");
    }

    public ForwardRequest(org.omg.CORBA.Object forward_reference) {
        this();
        this.forward_reference = forward_reference;
    }

    public ForwardRequest(String reason, org.omg.CORBA.Object forward_reference) {
        super(reason);
        this.forward_reference = forward_reference;
    }
}
