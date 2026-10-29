// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception NO_IMPLEMENT. */
public final class NO_IMPLEMENT extends SystemException {
    public NO_IMPLEMENT() {
        this("");
    }

    public NO_IMPLEMENT(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public NO_IMPLEMENT(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public NO_IMPLEMENT(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
