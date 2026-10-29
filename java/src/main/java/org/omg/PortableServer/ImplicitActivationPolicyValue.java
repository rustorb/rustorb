// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** PortableServer::ImplicitActivationPolicyValue */
public class ImplicitActivationPolicyValue implements org.omg.CORBA.portable.IDLEntity {
    public static final int _IMPLICIT_ACTIVATION = 0;
    public static final int _NO_IMPLICIT_ACTIVATION = 1;

    public static final ImplicitActivationPolicyValue IMPLICIT_ACTIVATION = new ImplicitActivationPolicyValue(_IMPLICIT_ACTIVATION);
    public static final ImplicitActivationPolicyValue NO_IMPLICIT_ACTIVATION = new ImplicitActivationPolicyValue(_NO_IMPLICIT_ACTIVATION);

    private final int value;

    protected ImplicitActivationPolicyValue(int value) {
        this.value = value;
    }

    public int value() {
        return value;
    }

    public static ImplicitActivationPolicyValue from_int(int i) {
        switch (i) {
            case _IMPLICIT_ACTIVATION:
                return IMPLICIT_ACTIVATION;
            case _NO_IMPLICIT_ACTIVATION:
                return NO_IMPLICIT_ACTIVATION;
            default:
                throw new org.omg.CORBA.BAD_PARAM();
        }
    }

    private java.lang.Object readResolve() {
        return from_int(value);
    }
}
