// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception INTF_REPOS. */
public final class INTF_REPOS extends SystemException {
    public INTF_REPOS() {
        this("");
    }

    public INTF_REPOS(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public INTF_REPOS(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public INTF_REPOS(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
