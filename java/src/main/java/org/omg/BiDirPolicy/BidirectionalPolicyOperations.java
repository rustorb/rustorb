// Part of the IDL to Java mapping (org.omg.BiDirPolicy), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.BiDirPolicy;

/** The operations of BiDirPolicy::BidirectionalPolicy. */
public interface BidirectionalPolicyOperations extends org.omg.CORBA.PolicyOperations {
    /** NORMAL.value or BOTH.value. */
    short value();
}
