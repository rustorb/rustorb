// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.TypeCodePackage;

/** CORBA::TypeCode::BadKind */
public final class BadKind extends org.omg.CORBA.UserException {
    public BadKind() {
        super("IDL:omg.org/CORBA/TypeCode/BadKind:1.0");
    }

    public BadKind(String reason) {
        super(reason);
    }
}
