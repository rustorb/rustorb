// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** The operations of DynamicAny::DynAnyFactory. */
public interface DynAnyFactoryOperations {
    DynAny create_dyn_any(org.omg.CORBA.Any value) throws org.omg.DynamicAny.DynAnyFactoryPackage.InconsistentTypeCode;
    DynAny create_dyn_any_from_type_code(org.omg.CORBA.TypeCode type) throws org.omg.DynamicAny.DynAnyFactoryPackage.InconsistentTypeCode;
    DynAny create_dyn_any_without_truncation(org.omg.CORBA.Any value)
            throws org.omg.DynamicAny.DynAnyFactoryPackage.InconsistentTypeCode, MustTruncate;
    DynAny[] create_multiple_dyn_anys(org.omg.CORBA.Any[] values, boolean allow_truncate)
            throws org.omg.DynamicAny.DynAnyFactoryPackage.InconsistentTypeCode, MustTruncate;
    org.omg.CORBA.Any[] create_multiple_anys(DynAny[] values);
}
