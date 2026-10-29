// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The CORBA system exception DATA_CONVERSION. */
public final class DATA_CONVERSION extends SystemException {
    public DATA_CONVERSION() {
        this("");
    }

    public DATA_CONVERSION(String detail) {
        this(detail, 0, CompletionStatus.COMPLETED_NO);
    }

    public DATA_CONVERSION(int minor, CompletionStatus completed) {
        this("", minor, completed);
    }

    public DATA_CONVERSION(String detail, int minor, CompletionStatus completed) {
        super(detail, minor, completed);
    }
}
