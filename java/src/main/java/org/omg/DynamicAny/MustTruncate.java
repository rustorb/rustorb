// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** DynamicAny::MustTruncate */
public final class MustTruncate extends org.omg.CORBA.UserException {
    public MustTruncate() {
        super("IDL:omg.org/DynamicAny/MustTruncate:1.0");
    }

    public MustTruncate(String reason) {
        super(reason);
    }
}
