// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** PortableServer::IdUniquenessPolicyValue */
public class IdUniquenessPolicyValue implements org.omg.CORBA.portable.IDLEntity {
    public static final int _UNIQUE_ID = 0;
    public static final int _MULTIPLE_ID = 1;

    public static final IdUniquenessPolicyValue UNIQUE_ID = new IdUniquenessPolicyValue(_UNIQUE_ID);
    public static final IdUniquenessPolicyValue MULTIPLE_ID = new IdUniquenessPolicyValue(_MULTIPLE_ID);

    private final int value;

    protected IdUniquenessPolicyValue(int value) {
        this.value = value;
    }

    public int value() {
        return value;
    }

    public static IdUniquenessPolicyValue from_int(int i) {
        switch (i) {
            case _UNIQUE_ID:
                return UNIQUE_ID;
            case _MULTIPLE_ID:
                return MULTIPLE_ID;
            default:
                throw new org.omg.CORBA.BAD_PARAM();
        }
    }

    private java.lang.Object readResolve() {
        return from_int(value);
    }
}
