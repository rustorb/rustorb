// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception INVALID_TRANSACTION. */
public final class INVALID_TRANSACTION extends SystemException {
    public INVALID_TRANSACTION() {
        this("");
    }

    public INVALID_TRANSACTION(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public INVALID_TRANSACTION(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public INVALID_TRANSACTION(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
