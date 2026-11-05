// What one sendc_ operation of a generated stub (rustorb-idl -bjava -Wbami)
// writes, reads and hands to its AMI_<I>Handler (docs/ami.md).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

public abstract class SendcCall extends Call {
    /** Calls the handler's operation with what read() read. */
    public abstract void reply();

    /** Calls the handler's {@code <op>_excep} operation. */
    public abstract void excep(org.omg.Messaging.ExceptionHolder holder);
}
