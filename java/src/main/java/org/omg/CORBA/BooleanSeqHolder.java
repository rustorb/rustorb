// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The holder of CORBA::BooleanSeq, for out and inout parameters. */
public final class BooleanSeqHolder implements org.omg.CORBA.portable.Streamable {
    public boolean[] value;

    public BooleanSeqHolder() {
    }

    public BooleanSeqHolder(boolean[] initial) {
        value = initial;
    }

    @Override
    public void _read(org.omg.CORBA.portable.InputStream in) {
        value = new boolean[in.read_ulong()];
        in.read_boolean_array(value, 0, value.length);
    }

    @Override
    public void _write(org.omg.CORBA.portable.OutputStream out) {
        out.write_ulong(value.length);
        out.write_boolean_array(value, 0, value.length);
    }

    @Override
    public TypeCode _type() {
        ORB orb = ORB.init();
        return orb.create_sequence_tc(0, orb.get_primitive_tc(TCKind.tk_boolean));
    }
}
