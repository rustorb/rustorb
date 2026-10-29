// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer.POAManagerPackage;

/** PortableServer::POAManager::State */
public class State implements org.omg.CORBA.portable.IDLEntity {
    public static final int _HOLDING = 0;
    public static final int _ACTIVE = 1;
    public static final int _DISCARDING = 2;
    public static final int _INACTIVE = 3;

    public static final State HOLDING = new State(_HOLDING);
    public static final State ACTIVE = new State(_ACTIVE);
    public static final State DISCARDING = new State(_DISCARDING);
    public static final State INACTIVE = new State(_INACTIVE);

    private final int value;

    protected State(int value) {
        this.value = value;
    }

    public int value() {
        return value;
    }

    public static State from_int(int i) {
        switch (i) {
            case _HOLDING:
                return HOLDING;
            case _ACTIVE:
                return ACTIVE;
            case _DISCARDING:
                return DISCARDING;
            case _INACTIVE:
                return INACTIVE;
            default:
                throw new org.omg.CORBA.BAD_PARAM();
        }
    }

    private java.lang.Object readResolve() {
        return from_int(value);
    }
}
