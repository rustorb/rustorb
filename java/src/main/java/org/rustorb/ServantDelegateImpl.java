// The Delegate of a Java servant: owns its peer in librustorb_jni (the
// `self` of the C ABI), freed when the servant is unreachable.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.NO_IMPLEMENT;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.Servant;
import org.omg.PortableServer.portable.Delegate;

public final class ServantDelegateImpl implements Delegate {
    private final Owned peer;

    private ServantDelegateImpl(Servant servant) {
        peer = Owned.register(this, Native.servantPeerNew(servant), Native::servantPeerFree);
    }

    /** Called by Servant._get_delegate. */
    public static Delegate create(Servant servant) {
        return new ServantDelegateImpl(servant);
    }

    /** The C side servant (a new rorb_servant handle) of a Java servant;
     * the caller releases it. */
    static long rorbServant(Servant servant) {
        if (servant == null) {
            return 0;
        }
        Delegate d = servant._get_delegate();
        if (!(d instanceof ServantDelegateImpl)) {
            throw new org.omg.CORBA.OBJ_ADAPTER("not a rustorb servant", 0,
                    CompletionStatus.COMPLETED_NO);
        }
        String[] ids = servant._all_interfaces(null, null);
        String id = ids.length > 0 ? ids[0] : "";
        try {
            return Native.servantGet(((ServantDelegateImpl) d).peer.get(), id);
        } finally {
            Reference.reachabilityFence(d);
        }
    }

    @Override
    public org.omg.CORBA.ORB orb(Servant self) {
        return OrbImpl.defaultOrb();
    }

    @Override
    public org.omg.CORBA.Object this_object(Servant self) {
        long s = rorbServant(self);
        try {
            long obj = Native.servantCurrentReference(s);
            if (obj != 0) {
                return ObjectStub.of(obj, OrbImpl.defaultOrb());
            }
        } finally {
            Native.servantRelease(s);
        }
        try {
            return default_POA(self).servant_to_reference(self);
        } catch (org.omg.PortableServer.POAPackage.ServantNotActive
                | org.omg.PortableServer.POAPackage.WrongPolicy e) {
            throw new org.omg.CORBA.OBJ_ADAPTER("the servant cannot be activated implicitly", 0,
                    CompletionStatus.COMPLETED_NO);
        }
    }

    @Override
    public POA poa(Servant self) {
        try {
            if (PoaCurrentImpl.INSTANCE.get_servant() == self) {
                return PoaCurrentImpl.INSTANCE.get_POA();
            }
        } catch (org.omg.PortableServer.CurrentPackage.NoContext e) {
            // not in a request
        }
        throw new BAD_INV_ORDER("the servant is not running a request", 0,
                CompletionStatus.COMPLETED_NO);
    }

    @Override
    public byte[] object_id(Servant self) {
        try {
            if (PoaCurrentImpl.INSTANCE.get_servant() == self) {
                return PoaCurrentImpl.INSTANCE.get_object_id();
            }
        } catch (org.omg.PortableServer.CurrentPackage.NoContext e) {
            // not in a request
        }
        throw new BAD_INV_ORDER("the servant is not running a request", 0,
                CompletionStatus.COMPLETED_NO);
    }

    @Override
    public POA default_POA(Servant self) {
        return PoaImpl.of(Native.defaultRootPoa());
    }

    @Override
    public boolean is_a(Servant self, String repository_id) {
        return Bridge.isA(self, repository_id);
    }

    @Override
    public boolean non_existent(Servant self) {
        return false;
    }

    @Override
    public org.omg.CORBA.Object get_interface_def(Servant self) {
        throw new NO_IMPLEMENT();
    }
}
