// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The base of the CORBA system exceptions. */
public abstract class SystemException extends RuntimeException {
    public int minor;
    public CompletionStatus completed;

    protected SystemException(String reason, int minor, CompletionStatus completed) {
        super(reason);
        this.minor = minor;
        this.completed = completed;
    }

    @Override
    public String toString() {
        String c;
        switch (completed.value()) {
            case CompletionStatus._COMPLETED_YES:
                c = "Yes";
                break;
            case CompletionStatus._COMPLETED_NO:
                c = "No";
                break;
            default:
                c = "Maybe";
                break;
        }
        String detail = getMessage();
        return getClass().getName() + ":" + (detail == null || detail.isEmpty() ? "" : " " + detail)
                + "  minor code: 0x" + Integer.toHexString(minor) + "  completed: " + c;
    }
}
