// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA_2_3;

import org.omg.CORBA.portable.ValueFactory;

/** The ORB with the value factory registry. The factories are shared by the
 * ORBs of the process. */
public abstract class ORB extends org.omg.CORBA.ORB {
    /** Returns the factory replaced, or null. */
    public ValueFactory register_value_factory(String id, ValueFactory factory) {
        return org.rustorb.Values.register(id, factory);
    }

    public void unregister_value_factory(String id) {
        org.rustorb.Values.unregister(id);
    }

    /** The factory for id: registered, or found by its class name (the
     * class of the type followed by DefaultFactory); null if none. */
    public ValueFactory lookup_value_factory(String id) {
        return org.rustorb.Values.lookup(id);
    }
}
