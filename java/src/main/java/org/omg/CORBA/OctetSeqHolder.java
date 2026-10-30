// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The holder of CORBA::OctetSeq, for out and inout parameters. */
public final class OctetSeqHolder implements org.omg.CORBA.portable.Streamable {
    public byte[] value;

    public OctetSeqHolder() {
    }

    public OctetSeqHolder(byte[] initial) {
        value = initial;
    }

    @Override
    public void _read(org.omg.CORBA.portable.InputStream in) {
        value = new byte[in.read_ulong()];
        in.read_octet_array(value, 0, value.length);
    }

    @Override
    public void _write(org.omg.CORBA.portable.OutputStream out) {
        out.write_ulong(value.length);
        out.write_octet_array(value, 0, value.length);
    }

    @Override
    public TypeCode _type() {
        ORB orb = ORB.init();
        return orb.create_sequence_tc(0, orb.get_primitive_tc(TCKind.tk_octet));
    }
}
