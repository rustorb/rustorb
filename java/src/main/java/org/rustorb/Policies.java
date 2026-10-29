// The seven POA policies and Messaging::RelativeRoundtripTimeoutPolicy, and
// their form for the core: a type and a value (rorb_each_policy).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.NO_PERMISSION;
import org.omg.CORBA.Policy;
import org.omg.Messaging.RELATIVE_RT_TIMEOUT_POLICY_TYPE;
import org.omg.Messaging.RelativeRoundtripTimeoutPolicy;
import org.omg.PortableServer.*;

final class Policies {
    private Policies() {}

    /** The policy of a type and value of the core. */
    static Policy make(int type, long value) {
        int v = (int) value;
        switch (type) {
            case 16:
                return new Thread(ThreadPolicyValue.from_int(v));
            case 17:
                return new Lifespan(LifespanPolicyValue.from_int(v));
            case 18:
                return new IdUniqueness(IdUniquenessPolicyValue.from_int(v));
            case 19:
                return new IdAssignment(IdAssignmentPolicyValue.from_int(v));
            case 20:
                return new ImplicitActivation(ImplicitActivationPolicyValue.from_int(v));
            case 21:
                return new ServantRetention(ServantRetentionPolicyValue.from_int(v));
            case 22:
                return new RequestProcessing(RequestProcessingPolicyValue.from_int(v));
            case RELATIVE_RT_TIMEOUT_POLICY_TYPE.value:
                return new RelativeRoundtripTimeout(value);
            default:
                throw new org.omg.CORBA.INTERNAL("policy type " + type, 0,
                        CompletionStatus.COMPLETED_NO);
        }
    }

    /** The policies of the pairs (type, value) a native getter returns. */
    static Policy[] fromPairs(long[] pairs) {
        Policy[] list = new Policy[pairs.length / 2];
        for (int i = 0; i < list.length; i++) {
            list[i] = make((int) pairs[2 * i], pairs[2 * i + 1]);
        }
        return list;
    }

    /**
     * The types and values of a policy list for the core: BAD_PARAM for
     * null, NO_PERMISSION for a policy that is not rustorb's (it cannot
     * override).
     */
    static final class Raw {
        final int[] types;
        final long[] values;

        Raw(Policy[] policies) {
            if (policies == null) {
                throw new org.omg.CORBA.BAD_PARAM();
            }
            types = new int[policies.length];
            values = new long[policies.length];
            for (int i = 0; i < policies.length; i++) {
                Policy p = policies[i];
                if (p instanceof Base) {
                    types[i] = ((Base) p).type;
                    values[i] = ((Base) p).value;
                } else if (p instanceof RelativeRoundtripTimeout) {
                    types[i] = RELATIVE_RT_TIMEOUT_POLICY_TYPE.value;
                    values[i] = ((RelativeRoundtripTimeout) p).relative_expiry();
                } else if (p == null) {
                    throw new org.omg.CORBA.BAD_PARAM();
                } else {
                    throw new NO_PERMISSION(0, CompletionStatus.COMPLETED_NO);
                }
            }
        }
    }

    static final class RelativeRoundtripTimeout extends org.omg.CORBA.LocalObject
            implements RelativeRoundtripTimeoutPolicy {
        private final long relativeExpiry;

        RelativeRoundtripTimeout(long relativeExpiry) {
            this.relativeExpiry = relativeExpiry;
        }

        @Override
        public long relative_expiry() {
            return relativeExpiry;
        }

        @Override
        public int policy_type() {
            return RELATIVE_RT_TIMEOUT_POLICY_TYPE.value;
        }

        @Override
        public Policy copy() {
            return new RelativeRoundtripTimeout(relativeExpiry);
        }

        @Override
        public void destroy() {}
    }

    /** A policy: its type (the policy id) and value (the enumerator). */
    abstract static class Base extends org.omg.CORBA.LocalObject implements Policy {
        final int type;
        final int value;

        Base(int type, int value) {
            this.type = type;
            this.value = value;
        }

        @Override
        public int policy_type() {
            return type;
        }

        @Override
        public void destroy() {}
    }

    static final class Thread extends Base implements ThreadPolicy {
        Thread(ThreadPolicyValue v) {
            super(THREAD_POLICY_ID.value, v.value());
        }

        @Override
        public ThreadPolicyValue value() {
            return ThreadPolicyValue.from_int(value);
        }

        @Override
        public Policy copy() {
            return new Thread(value());
        }
    }

    static final class Lifespan extends Base implements LifespanPolicy {
        Lifespan(LifespanPolicyValue v) {
            super(LIFESPAN_POLICY_ID.value, v.value());
        }

        @Override
        public LifespanPolicyValue value() {
            return LifespanPolicyValue.from_int(value);
        }

        @Override
        public Policy copy() {
            return new Lifespan(value());
        }
    }

    static final class IdUniqueness extends Base implements IdUniquenessPolicy {
        IdUniqueness(IdUniquenessPolicyValue v) {
            super(ID_UNIQUENESS_POLICY_ID.value, v.value());
        }

        @Override
        public IdUniquenessPolicyValue value() {
            return IdUniquenessPolicyValue.from_int(value);
        }

        @Override
        public Policy copy() {
            return new IdUniqueness(value());
        }
    }

    static final class IdAssignment extends Base implements IdAssignmentPolicy {
        IdAssignment(IdAssignmentPolicyValue v) {
            super(ID_ASSIGNMENT_POLICY_ID.value, v.value());
        }

        @Override
        public IdAssignmentPolicyValue value() {
            return IdAssignmentPolicyValue.from_int(value);
        }

        @Override
        public Policy copy() {
            return new IdAssignment(value());
        }
    }

    static final class ImplicitActivation extends Base implements ImplicitActivationPolicy {
        ImplicitActivation(ImplicitActivationPolicyValue v) {
            super(IMPLICIT_ACTIVATION_POLICY_ID.value, v.value());
        }

        @Override
        public ImplicitActivationPolicyValue value() {
            return ImplicitActivationPolicyValue.from_int(value);
        }

        @Override
        public Policy copy() {
            return new ImplicitActivation(value());
        }
    }

    static final class ServantRetention extends Base implements ServantRetentionPolicy {
        ServantRetention(ServantRetentionPolicyValue v) {
            super(SERVANT_RETENTION_POLICY_ID.value, v.value());
        }

        @Override
        public ServantRetentionPolicyValue value() {
            return ServantRetentionPolicyValue.from_int(value);
        }

        @Override
        public Policy copy() {
            return new ServantRetention(value());
        }
    }

    static final class RequestProcessing extends Base implements RequestProcessingPolicy {
        RequestProcessing(RequestProcessingPolicyValue v) {
            super(REQUEST_PROCESSING_POLICY_ID.value, v.value());
        }

        @Override
        public RequestProcessingPolicyValue value() {
            return RequestProcessingPolicyValue.from_int(value);
        }

        @Override
        public Policy copy() {
            return new RequestProcessing(value());
        }
    }
}
