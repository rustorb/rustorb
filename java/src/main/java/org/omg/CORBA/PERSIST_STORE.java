// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception PERSIST_STORE. */
public final class PERSIST_STORE extends SystemException {
    public PERSIST_STORE() {
        this("");
    }

    public PERSIST_STORE(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public PERSIST_STORE(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public PERSIST_STORE(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
