// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception ACTIVITY_COMPLETED. */
public final class ACTIVITY_COMPLETED extends SystemException {
    public ACTIVITY_COMPLETED() {
        this("");
    }

    public ACTIVITY_COMPLETED(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public ACTIVITY_COMPLETED(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public ACTIVITY_COMPLETED(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
