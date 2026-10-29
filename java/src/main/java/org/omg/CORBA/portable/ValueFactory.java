// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** Makes the instances of a valuetype that are read: read_value makes one
 * and reads its state with is.read_value(instance). */
public interface ValueFactory {
    java.io.Serializable read_value(org.omg.CORBA_2_3.portable.InputStream is);
}
