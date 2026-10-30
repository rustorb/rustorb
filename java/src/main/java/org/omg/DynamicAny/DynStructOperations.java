// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** The operations of DynamicAny::DynStruct. */
public interface DynStructOperations extends DynAnyOperations {
    String current_member_name()
            throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch,
            org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    org.omg.CORBA.TCKind current_member_kind()
            throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch,
            org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    NameValuePair[] get_members();
    void set_members(NameValuePair[] value)
            throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch,
            org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    NameDynAnyPair[] get_members_as_dyn_any();
    void set_members_as_dyn_any(NameDynAnyPair[] value)
            throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch,
            org.omg.DynamicAny.DynAnyPackage.InvalidValue;
}
