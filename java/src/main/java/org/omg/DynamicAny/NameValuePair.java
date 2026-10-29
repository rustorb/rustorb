// Part of the IDL to Java mapping (org.omg.DynamicAny), written for rustorb
// from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.DynamicAny;

/** DynamicAny::NameValuePair */
public final class NameValuePair implements org.omg.CORBA.portable.IDLEntity {
    public String id;
    public org.omg.CORBA.Any value;

    public NameValuePair() {}

    public NameValuePair(String id, org.omg.CORBA.Any value) {
        this.id = id;
        this.value = value;
    }
}
