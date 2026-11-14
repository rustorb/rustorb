// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;


/** A user exception that a DII request received, or that a DSI servant
 * raises: except holds it (an any of the exception TypeCode of the
 * request). */
public final class UnknownUserException extends UserException {
    private static final long serialVersionUID = 1L;

    public Any except;

    public UnknownUserException() {
        super("IDL:omg.org/CORBA/UnknownUserException:1.0");
    }

    public UnknownUserException(Any a) {
        this();
        except = a;
    }
}