// Part of the IDL to Java mapping (org.omg.Messaging), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
package org.omg.Messaging;

/** The operations of Messaging::RelativeRoundtripTimeoutPolicy. */
public interface RelativeRoundtripTimeoutPolicyOperations extends org.omg.CORBA.PolicyOperations {
    /** How long a call may take, as a TimeBase::TimeT (units of 100 ns). */
    long relative_expiry();
}
