// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** A member of ORB.create_value_tc: access is PRIVATE_MEMBER.value or
 * PUBLIC_MEMBER.value. (id, defined_in, version and type_def are for the
 * interface repository and not used.) */
public final class ValueMember implements org.omg.CORBA.portable.IDLEntity {
    public String name;
    public String id;
    public String defined_in;
    public String version;
    public TypeCode type;
    public IDLType type_def;
    public short access;

    public ValueMember() {}

    public ValueMember(String name, String id, String defined_in, String version, TypeCode type,
            IDLType type_def, short access) {
        this.name = name;
        this.id = id;
        this.defined_in = defined_in;
        this.version = version;
        this.type = type;
        this.type_def = type_def;
        this.access = access;
    }
}
