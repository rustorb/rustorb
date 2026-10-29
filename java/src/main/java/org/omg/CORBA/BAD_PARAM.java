// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception BAD_PARAM. */
public final class BAD_PARAM extends SystemException {
    public BAD_PARAM() {
        this("");
    }

    public BAD_PARAM(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public BAD_PARAM(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public BAD_PARAM(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
