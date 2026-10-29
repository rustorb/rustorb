// A reference whose interface is not known (CORBA::Object).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.portable.ObjectImpl;

final class ObjectStub extends ObjectImpl {
    private static final String[] OBJECT_IDS = {"IDL:omg.org/CORBA/Object:1.0"};

    /** Takes an object handle; null for 0 (nil). */
    static ObjectStub of(long handle, OrbImpl orb) {
        if (handle == 0) {
            return null;
        }
        ObjectStub s = new ObjectStub();
        s._set_delegate(new ObjectDelegateImpl(handle, orb));
        return s;
    }

    @Override
    public String[] _ids() {
        String id = ((ObjectDelegateImpl) _get_delegate()).typeId();
        return id.isEmpty() ? OBJECT_IDS.clone() : new String[] {id};
    }
}
