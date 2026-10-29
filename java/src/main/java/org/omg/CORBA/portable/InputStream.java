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

    public java.math.BigDecimal read_fixed() {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    public org.omg.CORBA.ORB orb() {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }
}
