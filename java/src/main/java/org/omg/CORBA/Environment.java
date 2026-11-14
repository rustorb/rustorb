// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;


/** CORBA::Environment: the exception of a Request (null for none). */
public abstract class Environment {
    public abstract Exception exception();

    public abstract void exception(Exception except);

    public abstract void clear();
}