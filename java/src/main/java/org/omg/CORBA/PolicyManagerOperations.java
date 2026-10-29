// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The operations of CORBA::PolicyManager. */
public interface PolicyManagerOperations {
    /** The overrides of the types ts, or all of them when ts is empty. */
    Policy[] get_policy_overrides(int[] ts);

    /**
     * Replaces (SET_OVERRIDE) or adds to (ADD_OVERRIDE) the overrides.
     * NO_PERMISSION for a policy that is not a client side override,
     * BAD_PARAM for two policies of one type.
     */
    void set_policy_overrides(Policy[] policies, SetOverrideType set_add) throws InvalidPolicies;
}
