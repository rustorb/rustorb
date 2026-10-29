// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** PortableServer::LifespanPolicyValue */
public class LifespanPolicyValue implements org.omg.CORBA.portable.IDLEntity {
    public static final int _TRANSIENT = 0;
    public static final int _PERSISTENT = 1;

    public static final LifespanPolicyValue TRANSIENT = new LifespanPolicyValue(_TRANSIENT);
    public static final LifespanPolicyValue PERSISTENT = new LifespanPolicyValue(_PERSISTENT);

    private final int value;

    protected LifespanPolicyValue(int value) {
        this.value = value;
    }

    public int value() {
        return value;
    }

    public static LifespanPolicyValue from_int(int i) {
        switch (i) {
            case _TRANSIENT:
                return TRANSIENT;
            case _PERSISTENT:
                return PERSISTENT;
            default:
                throw new org.omg.CORBA.BAD_PARAM();
        }
    }

    private java.lang.Object readResolve() {
        return from_int(value);
    }
}
