// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception NO_MEMORY. */
public final class NO_MEMORY extends SystemException {
    public NO_MEMORY() {
        this("");
    }

    public NO_MEMORY(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public NO_MEMORY(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public NO_MEMORY(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
