// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** The operations of DynamicAny::DynEnum. */
public interface DynEnumOperations extends DynAnyOperations {
    String get_as_string();
    void set_as_string(String value) throws org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    int get_as_ulong();
    void set_as_ulong(int value) throws org.omg.DynamicAny.DynAnyPackage.InvalidValue;
}
