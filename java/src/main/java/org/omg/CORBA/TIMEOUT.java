// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception TIMEOUT. */
public final class TIMEOUT extends SystemException {
    public TIMEOUT() {
        this("");
    }

    public TIMEOUT(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public TIMEOUT(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public TIMEOUT(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
