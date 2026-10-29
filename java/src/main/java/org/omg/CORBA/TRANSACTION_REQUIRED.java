// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception TRANSACTION_REQUIRED. */
public final class TRANSACTION_REQUIRED extends SystemException {
    public TRANSACTION_REQUIRED() {
        this("");
    }

    public TRANSACTION_REQUIRED(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public TRANSACTION_REQUIRED(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public TRANSACTION_REQUIRED(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
