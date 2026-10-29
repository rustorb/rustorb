// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** PortableServer::RequestProcessingPolicyValue */
public class RequestProcessingPolicyValue implements org.omg.CORBA.portable.IDLEntity {
    public static final int _USE_ACTIVE_OBJECT_MAP_ONLY = 0;
    public static final int _USE_DEFAULT_SERVANT = 1;
    public static final int _USE_SERVANT_MANAGER = 2;

    public static final RequestProcessingPolicyValue USE_ACTIVE_OBJECT_MAP_ONLY = new RequestProcessingPolicyValue(_USE_ACTIVE_OBJECT_MAP_ONLY);
    public static final RequestProcessingPolicyValue USE_DEFAULT_SERVANT = new RequestProcessingPolicyValue(_USE_DEFAULT_SERVANT);
    public static final RequestProcessingPolicyValue USE_SERVANT_MANAGER = new RequestProcessingPolicyValue(_USE_SERVANT_MANAGER);

    private final int value;

    protected RequestProcessingPolicyValue(int value) {
        this.value = value;
    }

    public int value() {
        return value;
    }

    public static RequestProcessingPolicyValue from_int(int i) {
        switch (i) {
            case _USE_ACTIVE_OBJECT_MAP_ONLY:
                return USE_ACTIVE_OBJECT_MAP_ONLY;
            case _USE_DEFAULT_SERVANT:
                return USE_DEFAULT_SERVANT;
            case _USE_SERVANT_MANAGER:
                return USE_SERVANT_MANAGER;
            default:
                throw new org.omg.CORBA.BAD_PARAM();
        }
    }

    private java.lang.Object readResolve() {
        return from_int(value);
    }
}
