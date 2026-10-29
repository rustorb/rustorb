// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

import org.omg.PortableServer.portable.Delegate;

/** The base of servants; skeletons (FooPOA) extend it. */
public abstract class Servant {
    private transient Delegate _delegate;

    /** The servant's delegate, made by rustorb when first needed. */
    public final synchronized Delegate _get_delegate() {
        if (_delegate == null) {
            _delegate = org.rustorb.ServantDelegateImpl.create(this);
        }
        return _delegate;
    }

    public final synchronized void _set_delegate(Delegate delegate) {
        _delegate = delegate;
    }

    /** The reference of the servant: in a request on it, the target;
     * otherwise implicitly activated in _default_POA(). */
    public final org.omg.CORBA.Object _this_object() {
        return _get_delegate().this_object(this);
    }

    public final org.omg.CORBA.Object _this_object(org.omg.CORBA.ORB orb) {
        return _this_object();
    }

    public final org.omg.CORBA.ORB _orb() {
        return _get_delegate().orb(this);
    }

    public final POA _poa() {
        return _get_delegate().poa(this);
    }

    public final byte[] _object_id() {
        return _get_delegate().object_id(this);
    }

    /** The RootPOA of the first ORB initialised. */
    public POA _default_POA() {
        return _get_delegate().default_POA(this);
    }

    public boolean _is_a(String repository_id) {
        return _get_delegate().is_a(this, repository_id);
    }

    public boolean _non_existent() {
        return _get_delegate().non_existent(this);
    }

    public org.omg.CORBA.Object _get_interface_def() {
        return _get_delegate().get_interface_def(this);
    }

    /** The repository ids of the most derived interface and its bases. */
    public abstract String[] _all_interfaces(POA poa, byte[] objectId);
}
