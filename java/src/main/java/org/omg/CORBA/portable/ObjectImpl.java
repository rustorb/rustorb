// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** The base class of stubs: an object reference with its Delegate. */
public abstract class ObjectImpl implements org.omg.CORBA.Object {
    private transient Delegate __delegate;

    public Delegate _get_delegate() {
        if (__delegate == null) {
            throw new org.omg.CORBA.BAD_OPERATION("The delegate has not been set");
        }
        return __delegate;
    }

    public void _set_delegate(Delegate delegate) {
        __delegate = delegate;
    }

    /** The repository ids of the interface and its bases. */
    public abstract String[] _ids();

    @Override
    public org.omg.CORBA.Object _duplicate() {
        return _get_delegate().duplicate(this);
    }

    @Override
    public void _release() {
        _get_delegate().release(this);
    }

    @Override
    public boolean _is_a(String repository_id) {
        return _get_delegate().is_a(this, repository_id);
    }

    @Override
    public boolean _is_equivalent(org.omg.CORBA.Object that) {
        return _get_delegate().is_equivalent(this, that);
    }

    @Override
    public boolean _non_existent() {
        return _get_delegate().non_existent(this);
    }

    @Override
    public int _hash(int maximum) {
        return _get_delegate().hash(this, maximum);
    }

    @Override
    public org.omg.CORBA.Object _get_interface_def() {
        return _get_delegate().get_interface_def(this);
    }

    public org.omg.CORBA.ORB _orb() {
        return _get_delegate().orb(this);
    }

    public OutputStream _request(String operation, boolean responseExpected) {
        return _get_delegate().request(this, operation, responseExpected);
    }

    public InputStream _invoke(OutputStream output)
            throws ApplicationException, RemarshalException {
        return _get_delegate().invoke(this, output);
    }

    public void _releaseReply(InputStream input) {
        _get_delegate().releaseReply(this, input);
    }

    public boolean _is_local() {
        return _get_delegate().is_local(this);
    }

    public ServantObject _servant_preinvoke(String operation, Class<?> expectedType) {
        return _get_delegate().servant_preinvoke(this, operation, expectedType);
    }

    public void _servant_postinvoke(ServantObject servant) {
        _get_delegate().servant_postinvoke(this, servant);
    }

    @Override
    public String toString() {
        return __delegate != null ? __delegate.toString(this) : getClass().getName() + ":no delegate";
    }

    @Override
    public int hashCode() {
        return __delegate != null ? __delegate.hashCode(this) : super.hashCode();
    }

    @Override
    public boolean equals(java.lang.Object obj) {
        return __delegate != null ? __delegate.equals(this, obj) : this == obj;
    }
}
