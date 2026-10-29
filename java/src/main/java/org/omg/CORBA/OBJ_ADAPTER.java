// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception OBJ_ADAPTER. */
public final class OBJ_ADAPTER extends SystemException {
    public OBJ_ADAPTER() {
        this("");
    }

    public OBJ_ADAPTER(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public OBJ_ADAPTER(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public OBJ_ADAPTER(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
