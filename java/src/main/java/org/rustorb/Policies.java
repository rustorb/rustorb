// The seven POA policies.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.Policy;
import org.omg.PortableServer.*;

final class Policies {
    private Policies() {}

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
