// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** An object reference (CORBA::Object). null is the nil reference. */
public interface Object {
    boolean _is_a(String repositoryIdentifier);

    boolean _is_equivalent(Object other);

    boolean _non_existent();

    int _hash(int maximum);

    Object _duplicate();

    void _release();

    Object _get_interface_def();

    /**
     * The effective policy of the type (rustorb reads no policies from
     * IORs, so the client policy); null if there is none.
     */
    Policy _get_policy(int policy_type);

    /** A new reference to the object with the overrides policies. */
    Object _set_policy_override(Policy[] policies, SetOverrideType set_add);

    /**
     * The policy of the type for calls on this reference: its override,
     * else the calling thread's, else the ORB's; null if there is none.
     */
    Policy _get_client_policy(int type);

    /** The overrides of this reference of the types, all when empty. */
    Policy[] _get_policy_overrides(int[] types);

    /** Binds the reference with a LocateRequest: true, or the failure. */
    boolean _validate_connection(PolicyListHolder inconsistent_policies);
}
