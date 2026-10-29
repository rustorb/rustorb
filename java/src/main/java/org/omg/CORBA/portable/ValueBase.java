// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** A valuetype or value box instance. */
public interface ValueBase extends IDLEntity {
    /** The repository id of the type, then those of the bases it is
     * truncatable to. */
    String[] _truncatable_ids();
}
