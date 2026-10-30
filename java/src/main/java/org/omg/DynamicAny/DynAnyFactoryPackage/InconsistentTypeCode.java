// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny.DynAnyFactoryPackage;

/** DynamicAny::DynAnyFactory::InconsistentTypeCode */
public final class InconsistentTypeCode extends org.omg.CORBA.UserException {
    public InconsistentTypeCode() {
        super("IDL:omg.org/DynamicAny/DynAnyFactory/InconsistentTypeCode:1.0");
    }

    public InconsistentTypeCode(String reason) {
        super(reason);
    }
}
