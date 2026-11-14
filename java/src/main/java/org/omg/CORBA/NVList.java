// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;


/** CORBA::NVList: the parameters of a Request or a ServerRequest. */
public abstract class NVList {
    public abstract int count();

    /** Adds an item without a name, with a new any. */
    public abstract NamedValue add(int flags);

    /** Adds an item with a new any. */
    public abstract NamedValue add_item(String item_name, int flags);

    /** Adds an item holding val (not a copy of it). */
    public abstract NamedValue add_value(String item_name, Any val, int flags);

    public abstract NamedValue item(int index) throws Bounds;

    public abstract void remove(int index) throws Bounds;
}