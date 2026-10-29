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

    public void write_fixed(java.math.BigDecimal value) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    public org.omg.CORBA.ORB orb() {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }
}
