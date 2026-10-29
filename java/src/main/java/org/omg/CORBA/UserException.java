// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The base of the exceptions declared in IDL. */
public abstract class UserException extends Exception
        implements org.omg.CORBA.portable.IDLEntity {
    protected UserException() {}

    protected UserException(String reason) {
        super(reason);
    }
}
