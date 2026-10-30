// The Delegate of an object reference of the rustorb core (an rorb_object
// handle). Stubs of the same reference share it.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.MARSHAL;
import org.omg.CORBA.UNKNOWN;
import org.omg.CORBA.portable.ApplicationException;
import org.omg.CORBA.portable.Delegate;
import org.omg.CORBA.portable.InputStream;
import org.omg.CORBA.portable.ObjectImpl;
import org.omg.CORBA.portable.OutputStream;

final class ObjectDelegateImpl extends Delegate {
    private final Owned handle;
    final OrbImpl orb;

    ObjectDelegateImpl(long handle, OrbImpl orb) {
        this.handle = Owned.register(this, handle, Native::objectRelease);
        this.orb = orb;
    }

    long handle() {
        return handle.get();
    }

    /** The delegate of a reference of rustorb. */
    static ObjectDelegateImpl of(org.omg.CORBA.Object obj) {
        if (obj instanceof ObjectImpl) {
            Delegate d = ((ObjectImpl) obj)._get_delegate();
            if (d instanceof ObjectDelegateImpl) {
                return (ObjectDelegateImpl) d;
            }
        }
        if (obj instanceof org.omg.CORBA.LocalObject) {
            // CORBA 3: local objects cannot be marshaled (minor 4).
            throw new MARSHAL("a local object", 4, CompletionStatus.COMPLETED_NO);
        }
        throw new BAD_PARAM("not an object reference of rustorb", 0, CompletionStatus.COMPLETED_NO);
    }

    @Override
    public boolean is_a(org.omg.CORBA.Object self, String repository_id) {
        try {
            return Native.objectIsA(handle(), repository_id);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public boolean non_existent(org.omg.CORBA.Object self) {
        try {
            return Native.objectNonExistent(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public boolean is_equivalent(org.omg.CORBA.Object self, org.omg.CORBA.Object rhs) {
        if (rhs == null) {
            return false;
        }
        if (!(rhs instanceof ObjectImpl)
                || !(((ObjectImpl) rhs)._get_delegate() instanceof ObjectDelegateImpl)) {
            return false;
        }
        ObjectDelegateImpl other = (ObjectDelegateImpl) ((ObjectImpl) rhs)._get_delegate();
        try {
            return Native.objectIsEquivalent(handle(), other.handle());
        } finally {
            Reference.reachabilityFence(this);
            Reference.reachabilityFence(other);
        }
    }

    @Override
    public int hash(org.omg.CORBA.Object self, int max) {
        try {
            return Native.objectHash(handle(), max);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public org.omg.CORBA.ORB orb(org.omg.CORBA.Object self) {
        return orb;
    }

    @Override
    public org.omg.CORBA.Policy get_policy(org.omg.CORBA.Object self, int policy_type) {
        return get_client_policy(self, policy_type);
    }

    @Override
    public org.omg.CORBA.Object set_policy_override(org.omg.CORBA.Object self,
            org.omg.CORBA.Policy[] policies, org.omg.CORBA.SetOverrideType set_add) {
        Policies.Raw raw = new Policies.Raw(policies);
        try {
            return ObjectStub.of(Native.objectSetPolicyOverrides(handle(), raw.types, raw.values,
                    set_add == org.omg.CORBA.SetOverrideType.ADD_OVERRIDE), orb);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public org.omg.CORBA.Policy get_client_policy(org.omg.CORBA.Object self, int type) {
        long[] value = new long[1];
        try {
            return Native.objectClientPolicy(handle(), type, value)
                    ? Policies.make(type, value[0]) : null;
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public org.omg.CORBA.Policy[] get_policy_overrides(org.omg.CORBA.Object self, int[] types) {
        try {
            return Policies.fromPairs(Native.objectPolicyOverrides(handle(), types));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public boolean validate_connection(org.omg.CORBA.Object self,
            org.omg.CORBA.PolicyListHolder inconsistent_policies) {
        if (inconsistent_policies != null) {
            inconsistent_policies.value = new org.omg.CORBA.Policy[0];
        }
        try {
            return Native.objectValidateConnection(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    // --- portable stubs ------------------------------------------------------

    /** The stream of ObjectImpl._request: the arguments, kept until
     * _invoke writes them into the request. */
    private static final class RequestStream extends PortableOutputStream {
        final ObjectDelegateImpl target;
        final String operation;
        final boolean responseExpected;

        RequestStream(ObjectDelegateImpl target, String operation, boolean responseExpected) {
            super(target.orb);
            this.target = target;
            this.operation = operation;
            this.responseExpected = responseExpected;
        }
    }

    @Override
    public OutputStream request(org.omg.CORBA.Object self, String operation,
            boolean responseExpected) {
        if (operation == null) {
            throw new BAD_PARAM("null operation", 0, CompletionStatus.COMPLETED_NO);
        }
        return new RequestStream(this, operation, responseExpected);
    }

    /**
     * Sends the request and returns the reply stream (null for a oneway
     * call). A user exception is an ApplicationException whose stream
     * gives its repository id to the first read_string, so that the
     * Helper reads it. The core follows LOCATION_FORWARD itself, so there
     * is no RemarshalException.
     */
    @Override
    public InputStream invoke(org.omg.CORBA.Object self, OutputStream output)
            throws ApplicationException {
        if (!(output instanceof RequestStream) || ((RequestStream) output).target != this) {
            throw new BAD_PARAM("not a stream of _request on this object", 0,
                    CompletionStatus.COMPLETED_NO);
        }
        RequestStream r = (RequestStream) output;
        Call call = r.replay(orb);
        long h;
        try {
            h = Native.objectInvokeStream(handle(), r.operation, !r.responseExpected, -1, call);
        } finally {
            Reference.reachabilityFence(this);
        }
        Bridge.rethrow(call);
        if (h == 0) {
            if (r.responseExpected) {
                throw new UNKNOWN("no reply", 0, CompletionStatus.COMPLETED_MAYBE);
            }
            return null;
        }
        OwnedInputStream in = new OwnedInputStream(h, orb, null);
        String id = Native.inExceptionId(h);
        if (id != null) {
            in.inject(id);
            throw new ApplicationException(id, in);
        }
        return in;
    }

    @Override
    public void releaseReply(org.omg.CORBA.Object self, InputStream input) {
        if (input instanceof OwnedInputStream) {
            ((OwnedInputStream) input).release();
        }
    }

    /** The repository id in the IOR (may be empty). */
    String typeId() {
        try {
            return Native.objectTypeId(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public String toString(org.omg.CORBA.Object self) {
        return self.getClass().getName() + "[" + typeId() + "]";
    }

    @Override
    public int hashCode(org.omg.CORBA.Object self) {
        return hash(self, Integer.MAX_VALUE);
    }

    @Override
    public boolean equals(org.omg.CORBA.Object self, java.lang.Object obj) {
        return obj instanceof org.omg.CORBA.Object && is_equivalent(self, (org.omg.CORBA.Object) obj);
    }
}
