// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** The operations of DynamicAny::DynUnion. */
public interface DynUnionOperations extends DynAnyOperations {
    DynAny get_discriminator();
    void set_discriminator(DynAny d) throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch;
    void set_to_default_member() throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch;
    void set_to_no_active_member() throws org.omg.DynamicAny.DynAnyPackage.TypeMismatch;
    boolean has_no_active_member();
    org.omg.CORBA.TCKind discriminator_kind();
    org.omg.CORBA.TCKind member_kind() throws org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    DynAny member() throws org.omg.DynamicAny.DynAnyPackage.InvalidValue;
    String member_name() throws org.omg.DynamicAny.DynAnyPackage.InvalidValue;
}
