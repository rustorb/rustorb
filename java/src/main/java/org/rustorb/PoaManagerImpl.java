// A POAManager of the rustorb core (an rorb_poa_manager handle).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.UserException;
import org.omg.PortableServer.POAManager;
import org.omg.PortableServer.POAManagerPackage.AdapterInactive;
import org.omg.PortableServer.POAManagerPackage.State;

final class PoaManagerImpl extends org.omg.CORBA.LocalObject implements POAManager {
    private final Owned handle;

    private PoaManagerImpl(long handle) {
        this.handle = Owned.register(this, handle, Native::pmRelease);
    }

    static PoaManagerImpl of(long handle) {
        return handle == 0 ? null : new PoaManagerImpl(handle);
    }

    static PoaManagerImpl cast(POAManager m) {
        if (m instanceof PoaManagerImpl) {
            return (PoaManagerImpl) m;
        }
        throw new BAD_PARAM("not a rustorb POAManager", 0, CompletionStatus.COMPLETED_NO);
    }

    long handle() {
        return handle.get();
    }

    @Override
    public boolean _is_a(String id) {
        return id.equals("IDL:omg.org/PortableServer/POAManager:1.0")
                || id.equals("IDL:omg.org/CORBA/LocalObject:1.0")
                || id.equals("IDL:omg.org/CORBA/Object:1.0");
    }

    @Override
    public void activate() throws AdapterInactive {
        try {
            Native.pmActivate(handle());
        } catch (AdapterInactive e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public void hold_requests(boolean wait_for_completion) throws AdapterInactive {
        try {
            Native.pmHoldRequests(handle(), wait_for_completion);
        } catch (AdapterInactive e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public void discard_requests(boolean wait_for_completion) throws AdapterInactive {
        try {
            Native.pmDiscardRequests(handle(), wait_for_completion);
        } catch (AdapterInactive e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public void deactivate(boolean etherealize_objects, boolean wait_for_completion)
            throws AdapterInactive {
        try {
            Native.pmDeactivate(handle(), etherealize_objects, wait_for_completion);
        } catch (AdapterInactive e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public State get_state() {
        try {
            return State.from_int(Native.pmGetState(handle()));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public String get_id() {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }
}
