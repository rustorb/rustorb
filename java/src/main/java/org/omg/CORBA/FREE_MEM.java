// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception FREE_MEM. */
public final class FREE_MEM extends SystemException {
    public FREE_MEM() {
        this("");
    }

    public FREE_MEM(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public FREE_MEM(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public FREE_MEM(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
