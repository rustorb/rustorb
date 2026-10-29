// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception INV_POLICY. */
public final class INV_POLICY extends SystemException {
    public INV_POLICY() {
        this("");
    }

    public INV_POLICY(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public INV_POLICY(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public INV_POLICY(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
