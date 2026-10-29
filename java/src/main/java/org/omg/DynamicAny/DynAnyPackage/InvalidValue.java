// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny.DynAnyPackage;

/** DynamicAny::DynAny::InvalidValue */
public final class InvalidValue extends org.omg.CORBA.UserException {
    public InvalidValue() {
        super("IDL:omg.org/DynamicAny/DynAny/InvalidValue:1.0");
    }

    public InvalidValue(String reason) {
        super(reason);
    }
}
