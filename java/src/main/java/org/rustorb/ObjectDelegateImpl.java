// The Delegate of an object reference of the rustorb core (an rorb_object
// handle). Stubs of the same reference share it.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.MARSHAL;
import org.omg.CORBA.portable.Delegate;
import org.omg.CORBA.portable.ObjectImpl;

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
