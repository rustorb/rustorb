// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** The operations of DynamicAny::DynValueBox. */
public interface DynValueBoxOperations extends DynValueCommonOperations {
    org.omg.CORBA.Any get_boxed_value() throws org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    void set_boxed_value(org.omg.CORBA.Any boxed) throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch;
    DynAny get_boxed_value_as_dyn_any() throws org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    void set_boxed_value_as_dyn_any(DynAny boxed) throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch;
}
