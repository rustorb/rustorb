// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** CORBA::PolicyError, raised by ORB.create_policy. */
public final class PolicyError extends UserException {
    /** A PolicyErrorCode: BAD_POLICY.value and the others. */
    public short reason;

    public PolicyError() {
        super("IDL:omg.org/CORBA/PolicyError:1.0");
    }

    public PolicyError(short reason) {
        this();
        this.reason = reason;
    }

    public PolicyError(String detail, short reason) {
        super(detail);
        this.reason = reason;
    }
}
