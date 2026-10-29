// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception NO_PERMISSION. */
public final class NO_PERMISSION extends SystemException {
    public NO_PERMISSION() {
        this("");
    }

    public NO_PERMISSION(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public NO_PERMISSION(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public NO_PERMISSION(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
