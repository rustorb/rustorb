// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** PortableServer::IdAssignmentPolicyValue */
public class IdAssignmentPolicyValue implements org.omg.CORBA.portable.IDLEntity {
    public static final int _USER_ID = 0;
    public static final int _SYSTEM_ID = 1;

    public static final IdAssignmentPolicyValue USER_ID = new IdAssignmentPolicyValue(_USER_ID);
    public static final IdAssignmentPolicyValue SYSTEM_ID = new IdAssignmentPolicyValue(_SYSTEM_ID);

    private final int value;

    protected IdAssignmentPolicyValue(int value) {
        this.value = value;
    }

    public int value() {
        return value;
    }

    public static IdAssignmentPolicyValue from_int(int i) {
        switch (i) {
            case _USER_ID:
                return USER_ID;
            case _SYSTEM_ID:
                return SYSTEM_ID;
            default:
                throw new org.omg.CORBA.BAD_PARAM();
        }
    }

    private java.lang.Object readResolve() {
        return from_int(value);
    }
}
