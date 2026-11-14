// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;


/** CORBA::ExceptionList: the TypeCodes of the user exceptions of a
 * Request. */
public abstract class ExceptionList {
    public abstract int count();

    public abstract void add(TypeCode exc);

    public abstract TypeCode item(int index) throws Bounds;

    public abstract void remove(int index) throws Bounds;
}