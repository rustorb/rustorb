// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The holder of CORBA::PolicyList. */
public final class PolicyListHolder {
    public Policy[] value;

    public PolicyListHolder() {}

    public PolicyListHolder(Policy[] initial) {
        value = initial;
    }
}
