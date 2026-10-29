// A POA of the rustorb core (an rorb_poa handle).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.NO_IMPLEMENT;
import org.omg.CORBA.UserException;
import org.omg.PortableServer.*;
import org.omg.PortableServer.POAPackage.*;

final class PoaImpl extends org.omg.CORBA.LocalObject implements POA {
    private final Owned handle;

    private PoaImpl(long handle) {
        this.handle = Owned.register(this, handle, Native::poaRelease);
    }

    /** Takes a handle; null for 0. */
    static PoaImpl of(long handle) {
        return handle == 0 ? null : new PoaImpl(handle);
    }

    long handle() {
        return handle.get();
    }

    private static OrbImpl orb() {
        return OrbImpl.defaultOrb();
    }

    @Override
    public boolean _is_a(String id) {
        return id.equals("IDL:omg.org/PortableServer/POA:1.0")
                || id.equals("IDL:omg.org/CORBA/LocalObject:1.0")
                || id.equals("IDL:omg.org/CORBA/Object:1.0");
    }

    @Override
    public boolean _is_equivalent(org.omg.CORBA.Object that) {
        if (!(that instanceof PoaImpl)) {
            return false;
        }
        // The same POA has the same name and parent.
        PoaImpl other = (PoaImpl) that;
        if (!the_name().equals(other.the_name())) {
            return false;
        }
        POA p = the_parent();
        POA q = other.the_parent();
        return p == null ? q == null : q != null && p._is_equivalent(q);
    }

    @Override
    public POA create_POA(String adapter_name, POAManager a_POAManager,
            org.omg.CORBA.Policy[] policies) throws AdapterAlreadyExists, InvalidPolicy {
        int n = policies == null ? 0 : policies.length;
        int[] types = new int[n];
        int[] values = new int[n];
        for (int i = 0; i < n; i++) {
            if (!(policies[i] instanceof Policies.Base)) {
                throw new InvalidPolicy((short) i);
            }
            Policies.Base p = (Policies.Base) policies[i];
            types[i] = p.type;
            values[i] = p.value;
        }
        PoaManagerImpl m = a_POAManager == null ? null : PoaManagerImpl.cast(a_POAManager);
        try {
            return of(Native.poaCreatePoa(handle(), adapter_name, m == null ? 0 : m.handle(), types,
                    values));
        } catch (AdapterAlreadyExists | InvalidPolicy e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
            Reference.reachabilityFence(m);
        }
    }

    @Override
    public POA find_POA(String adapter_name, boolean activate_it) throws AdapterNonExistent {
        try {
            return of(Native.poaFindPoa(handle(), adapter_name, activate_it));
        } catch (AdapterNonExistent e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public void destroy(boolean etherealize_objects, boolean wait_for_completion) {
        try {
            Native.poaDestroy(handle(), etherealize_objects, wait_for_completion);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public ThreadPolicy create_thread_policy(ThreadPolicyValue value) {
        return new Policies.Thread(value);
    }

    @Override
    public LifespanPolicy create_lifespan_policy(LifespanPolicyValue value) {
        return new Policies.Lifespan(value);
    }

    @Override
    public IdUniquenessPolicy create_id_uniqueness_policy(IdUniquenessPolicyValue value) {
        return new Policies.IdUniqueness(value);
    }

    @Override
    public IdAssignmentPolicy create_id_assignment_policy(IdAssignmentPolicyValue value) {
        return new Policies.IdAssignment(value);
    }

    @Override
    public ImplicitActivationPolicy create_implicit_activation_policy(
            ImplicitActivationPolicyValue value) {
        return new Policies.ImplicitActivation(value);
    }

    @Override
    public ServantRetentionPolicy create_servant_retention_policy(
            ServantRetentionPolicyValue value) {
        return new Policies.ServantRetention(value);
    }

    @Override
    public RequestProcessingPolicy create_request_processing_policy(
            RequestProcessingPolicyValue value) {
        return new Policies.RequestProcessing(value);
    }

    @Override
    public String the_name() {
        try {
            return Native.poaName(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public POA the_parent() {
        try {
            return of(Native.poaParent(handle()));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public POA[] the_children() {
        long[] children;
        try {
            children = Native.poaChildren(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
        POA[] result = new POA[children.length];
        for (int i = 0; i < children.length; i++) {
            result[i] = of(children[i]);
        }
        return result;
    }

    @Override
    public POAManager the_POAManager() {
        try {
            return PoaManagerImpl.of(Native.poaTheManager(handle()));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public AdapterActivator the_activator() {
        return null;
    }

    @Override
    public void the_activator(AdapterActivator newThe_activator) {
        throw new NO_IMPLEMENT("AdapterActivator is not supported by the Java binding yet", 0,
                CompletionStatus.COMPLETED_NO);
    }

    @Override
    public ServantManager get_servant_manager() throws WrongPolicy {
        return null;
    }

    @Override
    public void set_servant_manager(ServantManager imgr) throws WrongPolicy {
        throw new NO_IMPLEMENT("servant managers are not supported by the Java binding yet", 0,
                CompletionStatus.COMPLETED_NO);
    }

    @Override
    public Servant get_servant() throws NoServant, WrongPolicy {
        try {
            return (Servant) Native.poaGetServant(handle());
        } catch (NoServant | WrongPolicy e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public void set_servant(Servant p_servant) throws WrongPolicy {
        long s = ServantDelegateImpl.rorbServant(p_servant);
        try {
            Native.poaSetServant(handle(), s);
        } catch (WrongPolicy e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Native.servantRelease(s);
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public byte[] activate_object(Servant p_servant) throws ServantAlreadyActive, WrongPolicy {
        long s = ServantDelegateImpl.rorbServant(p_servant);
        try {
            return Native.poaActivateObject(handle(), s);
        } catch (ServantAlreadyActive | WrongPolicy e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Native.servantRelease(s);
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public void activate_object_with_id(byte[] id, Servant p_servant)
            throws ServantAlreadyActive, ObjectAlreadyActive, WrongPolicy {
        long s = ServantDelegateImpl.rorbServant(p_servant);
        try {
            Native.poaActivateObjectWithId(handle(), id, s);
        } catch (ServantAlreadyActive | ObjectAlreadyActive | WrongPolicy e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Native.servantRelease(s);
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public void deactivate_object(byte[] oid) throws ObjectNotActive, WrongPolicy {
        try {
            Native.poaDeactivateObject(handle(), oid);
        } catch (ObjectNotActive | WrongPolicy e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public org.omg.CORBA.Object create_reference(String intf) throws WrongPolicy {
        try {
            return ObjectStub.of(Native.poaCreateReference(handle(), intf), orb());
        } catch (WrongPolicy e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public org.omg.CORBA.Object create_reference_with_id(byte[] oid, String intf) {
        try {
            return ObjectStub.of(Native.poaCreateReferenceWithId(handle(), oid, intf), orb());
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public byte[] servant_to_id(Servant p_servant) throws ServantNotActive, WrongPolicy {
        long s = ServantDelegateImpl.rorbServant(p_servant);
        try {
            return Native.poaServantToId(handle(), s);
        } catch (ServantNotActive | WrongPolicy e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Native.servantRelease(s);
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public org.omg.CORBA.Object servant_to_reference(Servant p_servant)
            throws ServantNotActive, WrongPolicy {
        long s = ServantDelegateImpl.rorbServant(p_servant);
        try {
            return ObjectStub.of(Native.poaServantToReference(handle(), s), orb());
        } catch (ServantNotActive | WrongPolicy e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Native.servantRelease(s);
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public Servant reference_to_servant(org.omg.CORBA.Object reference)
            throws ObjectNotActive, WrongPolicy, WrongAdapter {
        ObjectDelegateImpl d = ObjectDelegateImpl.of(reference);
        try {
            return (Servant) Native.poaReferenceToServant(handle(), d.handle());
        } catch (ObjectNotActive | WrongPolicy | WrongAdapter e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
            Reference.reachabilityFence(d);
        }
    }

    @Override
    public byte[] reference_to_id(org.omg.CORBA.Object reference)
            throws WrongAdapter, WrongPolicy {
        ObjectDelegateImpl d = ObjectDelegateImpl.of(reference);
        try {
            return Native.poaReferenceToId(handle(), d.handle());
        } catch (WrongAdapter | WrongPolicy e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
            Reference.reachabilityFence(d);
        }
    }

    @Override
    public Servant id_to_servant(byte[] oid) throws ObjectNotActive, WrongPolicy {
        try {
            return (Servant) Native.poaIdToServant(handle(), oid);
        } catch (ObjectNotActive | WrongPolicy e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public org.omg.CORBA.Object id_to_reference(byte[] oid) throws ObjectNotActive, WrongPolicy {
        try {
            return ObjectStub.of(Native.poaIdToReference(handle(), oid), orb());
        } catch (ObjectNotActive | WrongPolicy e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public byte[] id() {
        throw new NO_IMPLEMENT();
    }
}
