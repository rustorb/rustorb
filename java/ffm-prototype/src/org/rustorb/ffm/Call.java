// One invocation: what a generated stub would give rorb_object_invoke.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb.ffm;

public abstract class Call {
    Throwable error;

    /** Writes the arguments. */
    public void write(Out out) {}

    /** Reads the reply. */
    public void read(In in) {}
}
