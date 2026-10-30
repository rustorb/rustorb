// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** The operations of DynamicAny::DynFixed. */
public interface DynFixedOperations extends DynAnyOperations {
    String get_value();
    boolean set_value(String val)
            throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch,
            org.omg.DynamicAny.DynAnyPackage.InvalidValue;
}
