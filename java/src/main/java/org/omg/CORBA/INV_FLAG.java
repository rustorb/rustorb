// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception INV_FLAG. */
public final class INV_FLAG extends SystemException {
    public INV_FLAG() {
        this("");
    }

    public INV_FLAG(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public INV_FLAG(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public INV_FLAG(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
