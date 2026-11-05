// What org.omg.Messaging.ExceptionHolder calls (callback AMI, docs/ami.md).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

public final class Messaging {
    private Messaging() {}

    /** ExceptionHolder.raise_exception(_with_list) on the holder's state
     * (list null: any exception). */
    public static void raise(boolean isSystemException, boolean byteOrder, byte[] marshaledException,
            org.omg.CORBA.TypeCode[] list) throws org.omg.CORBA.UserException {
        Ami.raise(isSystemException, byteOrder, marshaledException, list);
    }
}
