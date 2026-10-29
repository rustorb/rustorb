// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception CODESET_INCOMPATIBLE. */
public final class CODESET_INCOMPATIBLE extends SystemException {
    public CODESET_INCOMPATIBLE() {
        this("");
    }

    public CODESET_INCOMPATIBLE(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public CODESET_INCOMPATIBLE(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public CODESET_INCOMPATIBLE(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
