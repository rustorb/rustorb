// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception TRANSACTION_ROLLEDBACK. */
public final class TRANSACTION_ROLLEDBACK extends SystemException {
    public TRANSACTION_ROLLEDBACK() {
        this("");
    }

    public TRANSACTION_ROLLEDBACK(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public TRANSACTION_ROLLEDBACK(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public TRANSACTION_ROLLEDBACK(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
