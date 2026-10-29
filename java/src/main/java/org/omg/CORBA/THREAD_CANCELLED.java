// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception THREAD_CANCELLED. */
public final class THREAD_CANCELLED extends SystemException {
    public THREAD_CANCELLED() {
        this("");
    }

    public THREAD_CANCELLED(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public THREAD_CANCELLED(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public THREAD_CANCELLED(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
