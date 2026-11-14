// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;


/** CORBA::ContextList: the context names of a Request (rustorb sends no
 * contexts). */
public abstract class ContextList {
    public abstract int count();

    public abstract void add(String ctx);

    public abstract String item(int index) throws Bounds;

    public abstract void remove(int index) throws Bounds;
}