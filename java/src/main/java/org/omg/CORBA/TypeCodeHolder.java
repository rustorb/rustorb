// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The holder of TypeCode, for out and inout parameters. */
public final class TypeCodeHolder implements org.omg.CORBA.portable.Streamable {
    public TypeCode value;

    public TypeCodeHolder() {
        value = null;
    }

    public TypeCodeHolder(TypeCode initial) {
        value = initial;
    }

    @Override
    public void _read(org.omg.CORBA.portable.InputStream in) {
        value = in.read_TypeCode();
    }

    @Override
    public void _write(org.omg.CORBA.portable.OutputStream out) {
        out.write_TypeCode(value);
    }

    @Override
    public TypeCode _type() {
        return ORB.init().get_primitive_tc(TCKind.tk_TypeCode);
    }
}
