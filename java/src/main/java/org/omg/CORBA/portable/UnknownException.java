// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** A non-CORBA exception a servant threw, reported as UNKNOWN. */
public class UnknownException extends org.omg.CORBA.SystemException {
    public Throwable originalEx;

    public UnknownException(Throwable ex) {
        super("", 0, org.omg.CORBA.CompletionStatus.COMPLETED_MAYBE);
        originalEx = ex;
    }
}
