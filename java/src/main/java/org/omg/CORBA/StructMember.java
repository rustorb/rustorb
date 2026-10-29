// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** A member of ORB.create_struct_tc and create_exception_tc. */
public final class StructMember implements org.omg.CORBA.portable.IDLEntity {
    public String name;
    public TypeCode type;
    public IDLType type_def;

    public StructMember() {}

    public StructMember(String name, TypeCode type, IDLType type_def) {
        this.name = name;
        this.type = type;
        this.type_def = type_def;
    }
}
