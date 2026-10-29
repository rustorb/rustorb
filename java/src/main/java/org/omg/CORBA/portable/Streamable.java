// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** A Holder: a value that can be read and written with its TypeCode. */
public interface Streamable {
    void _read(InputStream istream);
    void _write(OutputStream ostream);
    org.omg.CORBA.TypeCode _type();
}
