// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The base of local objects (POA, POAManager, Current...). */
public class LocalObject implements Object {
    public LocalObject() {}

    @Override
    public boolean _is_equivalent(Object that) {
        return this == that;
    }

    @Override
    public boolean _non_existent() {
        return false;
    }

    @Override
    public int _hash(int maximum) {
        int h = System.identityHashCode(this) & 0x7fffffff;
        return maximum > 0 ? h % maximum : h;
    }

    @Override
    public boolean _is_a(String repository_id) {
        throw new NO_IMPLEMENT(4, CompletionStatus.COMPLETED_NO);
    }

    @Override
    public Object _duplicate() {
        return this;
    }

    @Override
    public void _release() {}

    @Override
    public Object _get_interface_def() {
        throw new NO_IMPLEMENT(4, CompletionStatus.COMPLETED_NO);
    }

    public ORB _orb() {
        throw new NO_IMPLEMENT(4, CompletionStatus.COMPLETED_NO);
    }
}
