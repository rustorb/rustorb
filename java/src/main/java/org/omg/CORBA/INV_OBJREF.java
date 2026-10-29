// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception INV_OBJREF. */
public final class INV_OBJREF extends SystemException {
    public INV_OBJREF() {
        this("");
    }

    public INV_OBJREF(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public INV_OBJREF(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public INV_OBJREF(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
