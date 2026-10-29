// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception IMP_LIMIT. */
public final class IMP_LIMIT extends SystemException {
    public IMP_LIMIT() {
        this("");
    }

    public IMP_LIMIT(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public IMP_LIMIT(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public IMP_LIMIT(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
