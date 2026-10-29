// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** DynamicAny::NameDynAnyPair */
public final class NameDynAnyPair implements org.omg.CORBA.portable.IDLEntity {
    public String id;
    public DynAny value;

    public NameDynAnyPair() {}

    public NameDynAnyPair(String id, DynAny value) {
        this.id = id;
        this.value = value;
    }
}
