// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception OBJECT_NOT_EXIST. */
public final class OBJECT_NOT_EXIST extends SystemException {
    public OBJECT_NOT_EXIST() {
        this("");
    }

    public OBJECT_NOT_EXIST(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public OBJECT_NOT_EXIST(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public OBJECT_NOT_EXIST(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
