// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The holder of CORBA::AnySeq, for out and inout parameters. */
public final class AnySeqHolder implements org.omg.CORBA.portable.Streamable {
    public Any[] value;

    public AnySeqHolder() {
    }

    public AnySeqHolder(Any[] initial) {
        value = initial;
    }

    @Override
    public void _read(org.omg.CORBA.portable.InputStream in) {
        value = new Any[in.read_ulong()];
        for (int i = 0; i < value.length; i++) {
            value[i] = in.read_any();
        }
    }

    @Override
    public void _write(org.omg.CORBA.portable.OutputStream out) {
        out.write_ulong(value.length);
        for (Any a : value) {
            out.write_any(a);
        }
    }

    @Override
    public TypeCode _type() {
        ORB orb = ORB.init();
        return orb.create_sequence_tc(0, orb.get_primitive_tc(TCKind.tk_any));
    }
}
