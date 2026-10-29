// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** CORBA::InvalidPolicies */
public final class InvalidPolicies extends UserException {
    public short[] indices;

    public InvalidPolicies() {
        super("IDL:omg.org/CORBA/InvalidPolicies:1.0");
        indices = new short[0];
    }

    public InvalidPolicies(short[] indices) {
        this();
        this.indices = indices;
    }

    public InvalidPolicies(String detail, short[] indices) {
        super(detail);
        this.indices = indices;
    }
}
