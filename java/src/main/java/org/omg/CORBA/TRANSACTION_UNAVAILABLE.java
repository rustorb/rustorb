// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception TRANSACTION_UNAVAILABLE. */
public final class TRANSACTION_UNAVAILABLE extends SystemException {
    public TRANSACTION_UNAVAILABLE() {
        this("");
    }

    public TRANSACTION_UNAVAILABLE(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public TRANSACTION_UNAVAILABLE(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public TRANSACTION_UNAVAILABLE(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
