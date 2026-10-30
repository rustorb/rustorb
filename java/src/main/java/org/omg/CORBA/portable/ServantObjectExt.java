// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** The servant of a colocated call, told how the call ended (not used by
 * rustorb, whose Delegate.servant_preinvoke returns null). */
public abstract class ServantObjectExt extends ServantObject {
    public abstract void normalCompletion();

    public abstract void exceptionalCompletion(Throwable thr);
}
