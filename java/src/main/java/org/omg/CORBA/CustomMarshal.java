// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The marshaling of a custom valuetype: the implementation writes the
 * state in marshal and reads it back in unmarshal, in the same order. */
public interface CustomMarshal {
    void marshal(DataOutputStream os);

    void unmarshal(DataInputStream is);
}
