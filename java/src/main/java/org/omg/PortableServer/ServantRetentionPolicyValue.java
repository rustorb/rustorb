// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** PortableServer::ServantRetentionPolicyValue */
public class ServantRetentionPolicyValue implements org.omg.CORBA.portable.IDLEntity {
    public static final int _RETAIN = 0;
    public static final int _NON_RETAIN = 1;

    public static final ServantRetentionPolicyValue RETAIN = new ServantRetentionPolicyValue(_RETAIN);
    public static final ServantRetentionPolicyValue NON_RETAIN = new ServantRetentionPolicyValue(_NON_RETAIN);

    private final int value;

    protected ServantRetentionPolicyValue(int value) {
        this.value = value;
    }

    public int value() {
        return value;
    }

    public static ServantRetentionPolicyValue from_int(int i) {
        switch (i) {
            case _RETAIN:
                return RETAIN;
            case _NON_RETAIN:
                return NON_RETAIN;
            default:
                throw new org.omg.CORBA.BAD_PARAM();
        }
    }

    private java.lang.Object readResolve() {
        return from_int(value);
    }
}
