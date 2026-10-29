// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/**
 * The holder of fixed, for out and inout parameters. Generated stubs and
 * skeletons read and write value with the digits and scale of the
 * parameter; _read and _write use those of the value itself.
 */
public final class FixedHolder implements org.omg.CORBA.portable.Streamable {
    public java.math.BigDecimal value;

    public FixedHolder() {
    }

    public FixedHolder(java.math.BigDecimal initial) {
        value = initial;
    }

    @Override
    public void _read(org.omg.CORBA.portable.InputStream in) {
        value = in.read_fixed();
    }

    @Override
    public void _write(org.omg.CORBA.portable.OutputStream out) {
        out.write_fixed(value);
    }

    @Override
    public TypeCode _type() {
        java.math.BigDecimal v = value.scale() < 0 ? value.setScale(0) : value;
        short digits = (short) Math.max(1, Math.max(v.precision(), v.scale()));
        return ORB.init().create_fixed_tc(digits, (short) v.scale());
    }
}
