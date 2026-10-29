// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** Reads and writes the value of a value box. */
public interface BoxedValueHelper {
    java.io.Serializable read_value(InputStream is);

    void write_value(OutputStream os, java.io.Serializable value);

    String get_id();
}
