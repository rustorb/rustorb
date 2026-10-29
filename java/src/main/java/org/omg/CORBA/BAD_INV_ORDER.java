// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception BAD_INV_ORDER. */
public final class BAD_INV_ORDER extends SystemException {
    public BAD_INV_ORDER() {
        this("");
    }

    public BAD_INV_ORDER(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public BAD_INV_ORDER(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public BAD_INV_ORDER(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
