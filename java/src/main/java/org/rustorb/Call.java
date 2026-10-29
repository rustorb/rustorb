// What one invocation (or one any insertion or extraction) writes and
// reads. Generated stubs subclass it for each operation.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.portable.InputStream;
import org.omg.CORBA.portable.OutputStream;

public abstract class Call {
    /** The ORB of the object references read. */
    OrbImpl orb;
    /** The first Java exception a callback threw. */
    Throwable error;
    /** The user exception readException returned. */
    org.omg.CORBA.UserException userException;
    /** Bridge.SKIP_ID: the stream drops the first string written (the
     * repository id of an exception put in an any). */
    int exceptionMode;
    /** Given to the first read_string of the stream (the repository id of
     * an exception taken from an any). */
    String injectId;

    /** Writes the in and inout arguments; may run more than once. */
    public void write(OutputStream out) {}

    /** Reads the result and the inout and out arguments. */
    public void read(InputStream in) {}

    /**
     * Reads the user exception {@code id} if the operation declares it
     * (the stream gives the repository id to the first read_string, so
     * that Helper.read works), else returns null.
     */
    public org.omg.CORBA.UserException readException(String id, InputStream in) {
        return null;
    }

    /** The user exception the call raised, or null. */
    public final org.omg.CORBA.UserException userException() {
        return userException;
    }
}
