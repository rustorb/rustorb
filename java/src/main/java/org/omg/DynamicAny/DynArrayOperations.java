// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** The operations of DynamicAny::DynArray. */
public interface DynArrayOperations extends DynAnyOperations {
    org.omg.CORBA.Any[] get_elements();
    void set_elements(org.omg.CORBA.Any[] value)
            throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch,
            org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    DynAny[] get_elements_as_dyn_any();
    void set_elements_as_dyn_any(DynAny[] value)
            throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch,
            org.omg.DynamicAny.DynAnyPackage.InvalidValue;
}
