// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception TRANSACTION_MODE. */
public final class TRANSACTION_MODE extends SystemException {
    public TRANSACTION_MODE() {
        this("");
    }

    public TRANSACTION_MODE(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public TRANSACTION_MODE(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public TRANSACTION_MODE(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
