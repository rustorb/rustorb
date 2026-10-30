// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** The operations of DynamicAny::DynValueCommon. */
public interface DynValueCommonOperations extends DynAnyOperations {
    boolean is_null();
    void set_to_null();
    void set_to_value();
}
