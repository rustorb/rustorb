// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** A CDR input stream, as generated Helpers read values from it. */
public abstract class InputStream extends java.io.InputStream {
    public abstract boolean read_boolean();
    public abstract char read_char();
    public abstract char read_wchar();
    public abstract byte read_octet();
    public abstract short read_short();
    public abstract short read_ushort();
    public abstract int read_long();
    public abstract int read_ulong();
    public abstract long read_longlong();
    public abstract long read_ulonglong();
    public abstract float read_float();
    public abstract double read_double();
    public abstract String read_string();
    public abstract String read_wstring();

    public abstract void read_boolean_array(boolean[] value, int offset, int length);
    public abstract void read_char_array(char[] value, int offset, int length);
    public abstract void read_wchar_array(char[] value, int offset, int length);
    public abstract void read_octet_array(byte[] value, int offset, int length);
    public abstract void read_short_array(short[] value, int offset, int length);
    public abstract void read_ushort_array(short[] value, int offset, int length);
    public abstract void read_long_array(int[] value, int offset, int length);
    public abstract void read_ulong_array(int[] value, int offset, int length);
    public abstract void read_longlong_array(long[] value, int offset, int length);
    public abstract void read_ulonglong_array(long[] value, int offset, int length);
    public abstract void read_float_array(float[] value, int offset, int length);
    public abstract void read_double_array(double[] value, int offset, int length);

    public abstract org.omg.CORBA.Object read_Object();
    public abstract org.omg.CORBA.TypeCode read_TypeCode();
    public abstract org.omg.CORBA.Any read_any();

    /**
     * Reads an object reference as an instance of clz: a stub class (with
     * a public constructor without arguments) or an interface.
     */
    public org.omg.CORBA.Object read_Object(Class<?> clz) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    @Override
    public int read() throws java.io.IOException {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    /** Reads a fixed of any number of digits, up to its sign, as an
     * integer (scale 0). */
    public java.math.BigDecimal read_fixed() {
        StringBuilder text = new StringBuilder();
        for (;;) {
            int b = read_octet() & 0xff;
            int high = b >> 4;
            int low = b & 0xf;
            if (high > 9 || text.length() >= 32) {
                throw badFixed();
            }
            text.append((char) ('0' + high));
            if (low > 9) {
                return fixedValue(text, low, (short) 0);
            }
            text.append((char) ('0' + low));
        }
    }

    /** Reads a fixed&lt;digits, scale&gt;; a malformed digit or sign is MARSHAL. */
    public java.math.BigDecimal read_fixed(short digits, short scale) {
        if (digits < 1 || digits > 31 || scale < 0 || scale > digits) {
            throw new org.omg.CORBA.BAD_PARAM("not a fixed type: fixed<" + digits + ", "
                    + scale + ">", 0, org.omg.CORBA.CompletionStatus.COMPLETED_NO);
        }
        byte[] in = new byte[digits / 2 + 1];
        read_octet_array(in, 0, in.length);
        StringBuilder text = new StringBuilder(digits + 1);
        for (int n = digits % 2 == 0 ? 1 : 0; n < in.length * 2 - 1; n++) {
            int d = (in[n / 2] >> (n % 2 == 0 ? 4 : 0)) & 0xf;
            if (d > 9) {
                throw badFixed();
            }
            text.append((char) ('0' + d));
        }
        return fixedValue(text, in[in.length - 1] & 0xf, scale);
    }

    private static java.math.BigDecimal fixedValue(StringBuilder digits, int sign, short scale) {
        if (sign != 0xc && sign != 0xd && sign != 0xf) {
            throw badFixed();
        }
        java.math.BigInteger unscaled = new java.math.BigInteger(digits.toString());
        return new java.math.BigDecimal(sign == 0xd ? unscaled.negate() : unscaled, scale);
    }

    private static org.omg.CORBA.MARSHAL badFixed() {
        return new org.omg.CORBA.MARSHAL("malformed fixed", 0,
                org.omg.CORBA.CompletionStatus.COMPLETED_MAYBE);
    }

    public org.omg.CORBA.ORB orb() {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }
}
