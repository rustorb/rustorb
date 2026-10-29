// Part of the IDL to Java mapping (org.omg.PortableServer), written for
// rustorb from the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.PortableServer;

/** PortableServer::ThreadPolicyValue */
public class ThreadPolicyValue implements org.omg.CORBA.portable.IDLEntity {
    public static final int _ORB_CTRL_MODEL = 0;
    public static final int _SINGLE_THREAD_MODEL = 1;
    public static final int _MAIN_THREAD_MODEL = 2;

    public static final ThreadPolicyValue ORB_CTRL_MODEL = new ThreadPolicyValue(_ORB_CTRL_MODEL);
    public static final ThreadPolicyValue SINGLE_THREAD_MODEL = new ThreadPolicyValue(_SINGLE_THREAD_MODEL);
    public static final ThreadPolicyValue MAIN_THREAD_MODEL = new ThreadPolicyValue(_MAIN_THREAD_MODEL);

    private final int value;

    protected ThreadPolicyValue(int value) {
        this.value = value;
    }

    public int value() {
        return value;
    }

    public static ThreadPolicyValue from_int(int i) {
        switch (i) {
            case _ORB_CTRL_MODEL:
                return ORB_CTRL_MODEL;
            case _SINGLE_THREAD_MODEL:
                return SINGLE_THREAD_MODEL;
            case _MAIN_THREAD_MODEL:
                return MAIN_THREAD_MODEL;
            default:
                throw new org.omg.CORBA.BAD_PARAM();
        }
    }

    private java.lang.Object readResolve() {
        return from_int(value);
    }
}
