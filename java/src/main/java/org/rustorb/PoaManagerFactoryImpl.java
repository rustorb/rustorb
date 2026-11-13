// The POAManagerFactory of the rustorb core, reached through a POA (an
// rorb_poa handle).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.NO_PERMISSION;
import org.omg.CORBA.Policy;
import org.omg.CORBA.PolicyError;
import org.omg.CORBA.UserException;
import org.omg.PortableServer.POAManager;
import org.omg.PortableServer.POAManagerFactory;
import org.omg.PortableServer.POAManagerFactoryPackage.ManagerAlreadyExists;

final class PoaManagerFactoryImpl extends org.omg.CORBA.LocalObject
        implements POAManagerFactory {
    private final Owned poa;

    /** Takes a POA handle. */
    PoaManagerFactoryImpl(long poa) {
        this.poa = Owned.register(this, poa, Native::poaRelease);
    }

    @Override
    public boolean _is_a(String id) {
        return id.equals("IDL:omg.org/PortableServer/POAManagerFactory:1.0")
                || id.equals("IDL:omg.org/CORBA/LocalObject:1.0")
                || id.equals("IDL:omg.org/CORBA/Object:1.0");
    }

    @Override
    public boolean _is_equivalent(org.omg.CORBA.Object that) {
        // One factory per ORB, and rustorb's Java binding has one ORB.
        return that instanceof PoaManagerFactoryImpl;
    }

    @Override
    public POAManager create_POAManager(String id, Policy[] policies)
            throws ManagerAlreadyExists, PolicyError {
        if (id == null) {
            throw new org.omg.CORBA.BAD_PARAM();
        }
        Policies.Raw raw;
        try {
            raw = new Policies.Raw(policies);
        } catch (NO_PERMISSION e) {
            // A policy that is not rustorb's.
            throw new PolicyError(org.omg.CORBA.BAD_POLICY.value);
        }
        try {
            return PoaManagerImpl.of(Native.pmfCreate(poa.get(), id, raw.types, raw.values));
        } catch (ManagerAlreadyExists | PolicyError e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public POAManager[] list() {
        try {
            long[] handles = Native.pmfList(poa.get());
            POAManager[] result = new POAManager[handles.length];
            for (int i = 0; i < handles.length; i++) {
                result[i] = PoaManagerImpl.of(handles[i]);
            }
            return result;
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public POAManager find(String id) {
        if (id == null) {
            throw new org.omg.CORBA.BAD_PARAM();
        }
        try {
            return PoaManagerImpl.of(Native.pmfFind(poa.get(), id));
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }
}
