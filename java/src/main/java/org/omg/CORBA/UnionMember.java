// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** A member of ORB.create_union_tc; the label of the default member is the
 * octet 0. */
public final class UnionMember implements org.omg.CORBA.portable.IDLEntity {
    public String name;
    public Any label;
    public TypeCode type;
    public IDLType type_def;

    public UnionMember() {}

    public UnionMember(String name, Any label, TypeCode type, IDLType type_def) {
        this.name = name;
        this.label = label;
        this.type = type;
        this.type_def = type_def;
    }
}
