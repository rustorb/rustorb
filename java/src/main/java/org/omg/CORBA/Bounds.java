// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** CORBA::Bounds */
public final class Bounds extends UserException {
    public Bounds() {
        super("IDL:omg.org/CORBA/Bounds:1.0");
    }

    public Bounds(String reason) {
        super(reason);
    }
}
