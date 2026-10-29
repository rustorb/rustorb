// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception REBIND. */
public final class REBIND extends SystemException {
    public REBIND() {
        this("");
    }

    public REBIND(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public REBIND(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public REBIND(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
