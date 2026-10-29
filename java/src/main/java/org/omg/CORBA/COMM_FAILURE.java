// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception COMM_FAILURE. */
public final class COMM_FAILURE extends SystemException {
    public COMM_FAILURE() {
        this("");
    }

    public COMM_FAILURE(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public COMM_FAILURE(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public COMM_FAILURE(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
