// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The holder of org.omg.CORBA.Object, for out and inout parameters. */
public final class ObjectHolder implements org.omg.CORBA.portable.Streamable {
    public org.omg.CORBA.Object value;

    public ObjectHolder() {
        value = null;
    }

    public ObjectHolder(org.omg.CORBA.Object initial) {
        value = initial;
    }

    @Override
    public void _read(org.omg.CORBA.portable.InputStream in) {
        value = in.read_Object();
    }

    @Override
    public void _write(org.omg.CORBA.portable.OutputStream out) {
        out.write_Object(value);
    }

    @Override
    public TypeCode _type() {
        return ORB.init().create_interface_tc("IDL:omg.org/CORBA/Object:1.0", "Object");
    }
}
