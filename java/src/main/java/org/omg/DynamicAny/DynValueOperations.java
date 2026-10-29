// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** The operations of DynamicAny::DynValue. */
public interface DynValueOperations extends DynValueCommonOperations {
    String current_member_name()
            throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch,
            org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    org.omg.CORBA.TCKind current_member_kind()
            throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch,
            org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    NameValuePair[] get_members() throws org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    void set_members(NameValuePair[] value)
            throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch,
            org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    NameDynAnyPair[] get_members_as_dyn_any() throws org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    void set_members_as_dyn_any(NameDynAnyPair[] value)
            throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch,
            org.omg.DynamicAny.DynAnyPackage.InvalidValue;
}
