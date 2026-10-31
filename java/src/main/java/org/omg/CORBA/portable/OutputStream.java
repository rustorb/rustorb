// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** A CDR output stream, as generated Helpers write values to it. */
public abstract class OutputStream extends java.io.OutputStream {
    public abstract InputStream create_input_stream();

    public abstract void write_boolean(boolean value);
    public abstract void write_char(char value);
    public abstract void write_wchar(char value);
    public abstract void write_octet(byte value);
    public abstract void write_short(short value);
    public abstract void write_ushort(short value);
    public abstract void write_long(int value);
    public abstract void write_ulong(int value);
    public abstract void write_longlong(long value);
    public abstract void write_ulonglong(long value);
    public abstract void write_float(float value);
    public abstract void write_double(double value);
    public abstract void write_string(String value);
    public abstract void write_wstring(String value);

    public abstract void write_boolean_array(boolean[] value, int offset, int length);
    public abstract void write_char_array(char[] value, int offset, int length);
    public abstract void write_wchar_array(char[] value, int offset, int length);
    public abstract void write_octet_array(byte[] value, int offset, int length);
    public abstract void write_short_array(short[] value, int offset, int length);
    public abstract void write_ushort_array(short[] value, int offset, int length);
    public abstract void write_long_array(int[] value, int offset, int length);
    public abstract void write_ulong_array(int[] value, int offset, int length);
    public abstract void write_longlong_array(long[] value, int offset, int length);
    public abstract void write_ulonglong_array(long[] value, int offset, int length);
    public abstract void write_float_array(float[] value, int offset, int length);
    public abstract void write_double_array(double[] value, int offset, int length);

    public abstract void write_Object(org.omg.CORBA.Object value);
    public abstract void write_TypeCode(org.omg.CORBA.TypeCode value);
    public abstract void write_any(org.omg.CORBA.Any value);

    @Override
    public void write(int b) throws java.io.IOException {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    /** Writes value as a long double (rustorb: the Java mapping has no
     * long double); NO_IMPLEMENT on streams that cannot. */
    public void write_longdouble(double value) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    /** Writes value with the digits of its unscaled value (the scale is
     * not written; read it back with read_fixed()). */
    public void write_fixed(java.math.BigDecimal value) {
        if (value.scale() < 0) {
            value = value.setScale(0);
        }
        int digits = Math.max(1, Math.max(value.precision(), value.scale()));
        write_fixed(value, (short) digits, (short) value.scale());
    }

    /**
     * Writes value as a fixed&lt;digits, scale&gt;: packed decimal, two digits
     * an octet and the sign last. A value with more integer digits, or
     * more nonzero fraction digits, than the type has is BAD_PARAM.
     */
    public void write_fixed(java.math.BigDecimal value, short digits, short scale) {
        if (value == null) {
            throw badFixed("null fixed");
        }
        if (digits < 1 || digits > 31 || scale < 0 || scale > digits) {
            throw badFixed("not a fixed type: fixed<" + digits + ", " + scale + ">");
        }
        String text;
        try {
            text = value.setScale(scale, java.math.RoundingMode.UNNECESSARY)
                    .unscaledValue().abs().toString();
        } catch (ArithmeticException e) {
            throw badFixed("fixed value with more than " + scale + " fraction digits");
        }
        if (text.length() > digits) {
            throw badFixed("fixed value with more than " + digits + " digits");
        }
        // A leading zero digit when the count is even, then the sign.
        int nibbles = digits + (digits % 2 == 0 ? 2 : 1);
        byte[] out = new byte[nibbles / 2];
        int pad = nibbles - 1 - text.length();
        for (int i = 0; i < text.length(); i++) {
            int n = pad + i;
            out[n / 2] |= (byte) ((text.charAt(i) - '0') << (n % 2 == 0 ? 4 : 0));
        }
        out[out.length - 1] |= (byte) (value.signum() < 0 ? 0xd : 0xc);
        write_octet_array(out, 0, out.length);
    }

    private static org.omg.CORBA.BAD_PARAM badFixed(String message) {
        return new org.omg.CORBA.BAD_PARAM(message, 0,
                org.omg.CORBA.CompletionStatus.COMPLETED_NO);
    }

    public org.omg.CORBA.ORB orb() {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }
}
