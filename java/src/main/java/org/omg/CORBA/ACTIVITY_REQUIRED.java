// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception ACTIVITY_REQUIRED. */
public final class ACTIVITY_REQUIRED extends SystemException {
    public ACTIVITY_REQUIRED() {
        this("");
    }

    public ACTIVITY_REQUIRED(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public ACTIVITY_REQUIRED(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public ACTIVITY_REQUIRED(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
