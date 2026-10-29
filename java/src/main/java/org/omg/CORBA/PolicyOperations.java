// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The operations of CORBA::Policy. */
public interface PolicyOperations {
    int policy_type();

    Policy copy();

    void destroy();
}
