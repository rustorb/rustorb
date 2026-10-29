// Part of the IDL to Java mapping (org.omg.Messaging), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.
package org.omg.Messaging;

/**
 * Messaging::RelativeRoundtripTimeoutPolicy: made by ORB.create_policy from
 * an unsigned long long (TimeBase::TimeT) in an any, and applied with
 * Object._set_policy_override, the ORBPolicyManager or the PolicyCurrent.
 */
public interface RelativeRoundtripTimeoutPolicy extends RelativeRoundtripTimeoutPolicyOperations,
        org.omg.CORBA.Policy, org.omg.CORBA.portable.IDLEntity {}
