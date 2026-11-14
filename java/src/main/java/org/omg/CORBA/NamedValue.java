// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;


/** CORBA::NamedValue: a name, a value and the flags of a parameter
 * (ARG_IN, ARG_OUT or ARG_INOUT). */
public abstract class NamedValue {
    public abstract String name();

    public abstract Any value();

    public abstract int flags();
}