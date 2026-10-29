// The policy overrides of an ORB (ORBPolicyManager) and of the calling
// thread (PolicyCurrent).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.Policy;
import org.omg.CORBA.PolicyCurrent;
import org.omg.CORBA.PolicyManager;
import org.omg.CORBA.SetOverrideType;

final class PolicyManagerImpl {
    private PolicyManagerImpl() {}

    /** The ORB's overrides: the timeout overrides are its call timeout. */
    static final class OfOrb extends org.omg.CORBA.LocalObject implements PolicyManager {
        private final OrbImpl orb;

        OfOrb(OrbImpl orb) {
            this.orb = orb;
        }

        @Override
        public Policy[] get_policy_overrides(int[] ts) {
            try {
                return Policies.fromPairs(Native.orbPolicyOverrides(orb.handle(), ts));
            } finally {
                Reference.reachabilityFence(orb);
            }
        }

        @Override
        public void set_policy_overrides(Policy[] policies, SetOverrideType set_add) {
            Policies.Raw raw = new Policies.Raw(policies);
            try {
                Native.orbSetPolicyOverrides(orb.handle(), raw.types, raw.values,
                        set_add == SetOverrideType.ADD_OVERRIDE);
            } finally {
                Reference.reachabilityFence(orb);
            }
        }
    }

    /** The calling thread's overrides, for every ORB. */
    static final class Current extends org.omg.CORBA.LocalObject implements PolicyCurrent {
        static final Current INSTANCE = new Current();

        @Override
        public Policy[] get_policy_overrides(int[] ts) {
            return Policies.fromPairs(Native.threadPolicyOverrides(ts));
        }

        @Override
        public void set_policy_overrides(Policy[] policies, SetOverrideType set_add) {
            Policies.Raw raw = new Policies.Raw(policies);
            Native.setThreadPolicyOverrides(raw.types, raw.values,
                    set_add == SetOverrideType.ADD_OVERRIDE);
        }
    }
}
