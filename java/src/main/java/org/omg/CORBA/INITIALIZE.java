// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception INITIALIZE. */
public final class INITIALIZE extends SystemException {
    public INITIALIZE() {
        this("");
    }

    public INITIALIZE(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public INITIALIZE(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public INITIALIZE(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
