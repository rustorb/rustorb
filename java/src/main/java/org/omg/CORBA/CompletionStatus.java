// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** Whether the operation completed before a system exception. */
public class CompletionStatus implements org.omg.CORBA.portable.IDLEntity {
    public static final int _COMPLETED_YES = 0;
    public static final int _COMPLETED_NO = 1;
    public static final int _COMPLETED_MAYBE = 2;

    public static final CompletionStatus COMPLETED_YES = new CompletionStatus(_COMPLETED_YES);
    public static final CompletionStatus COMPLETED_NO = new CompletionStatus(_COMPLETED_NO);
    public static final CompletionStatus COMPLETED_MAYBE = new CompletionStatus(_COMPLETED_MAYBE);

    private final int value;

    private CompletionStatus(int value) {
        this.value = value;
    }

    public int value() {
        return value;
    }

    public static CompletionStatus from_int(int i) {
        switch (i) {
            case _COMPLETED_YES:
                return COMPLETED_YES;
            case _COMPLETED_NO:
                return COMPLETED_NO;
            case _COMPLETED_MAYBE:
                return COMPLETED_MAYBE;
            default:
                throw new BAD_PARAM();
        }
    }

    private java.lang.Object readResolve() {
        return from_int(value);
    }
}
