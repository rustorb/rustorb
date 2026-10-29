// Octet-only CDR streams over a byte array, to test the stream methods
// that the org.omg.CORBA.portable base classes implement themselves.

package org.rustorb.test;

import java.io.ByteArrayOutputStream;

final class OctetStreams {
    private OctetStreams() {}

    static final class Out extends org.omg.CORBA.portable.OutputStream {
        final ByteArrayOutputStream bytes = new ByteArrayOutputStream();

        @Override public void write_octet(byte value) { bytes.write(value); }
        @Override public void write_octet_array(byte[] value, int offset, int length) {
            bytes.write(value, offset, length);
        }
        @Override public org.omg.CORBA.portable.InputStream create_input_stream() {
            return new In(bytes.toByteArray());
        }
        @Override public void write_boolean(boolean value) { throw new UnsupportedOperationException(); }
        @Override public void write_char(char value) { throw new UnsupportedOperationException(); }
        @Override public void write_wchar(char value) { throw new UnsupportedOperationException(); }
        @Override public void write_short(short value) { throw new UnsupportedOperationException(); }
        @Override public void write_ushort(short value) { throw new UnsupportedOperationException(); }
        @Override public void write_long(int value) { throw new UnsupportedOperationException(); }
        @Override public void write_ulong(int value) { throw new UnsupportedOperationException(); }
        @Override public void write_longlong(long value) { throw new UnsupportedOperationException(); }
        @Override public void write_ulonglong(long value) { throw new UnsupportedOperationException(); }
        @Override public void write_float(float value) { throw new UnsupportedOperationException(); }
        @Override public void write_double(double value) { throw new UnsupportedOperationException(); }
        @Override public void write_string(String value) { throw new UnsupportedOperationException(); }
        @Override public void write_wstring(String value) { throw new UnsupportedOperationException(); }
        @Override public void write_boolean_array(boolean[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void write_char_array(char[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void write_wchar_array(char[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void write_short_array(short[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void write_ushort_array(short[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void write_long_array(int[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void write_ulong_array(int[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void write_longlong_array(long[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void write_ulonglong_array(long[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void write_float_array(float[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void write_double_array(double[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void write_Object(org.omg.CORBA.Object value) { throw new UnsupportedOperationException(); }
        @Override public void write_TypeCode(org.omg.CORBA.TypeCode value) { throw new UnsupportedOperationException(); }
        @Override public void write_any(org.omg.CORBA.Any value) { throw new UnsupportedOperationException(); }
    }

    static final class In extends org.omg.CORBA.portable.InputStream {
        private final byte[] data;
        private int at;

        In(byte[] data) { this.data = data; }

        @Override public byte read_octet() {
            if (at >= data.length) {
                throw new org.omg.CORBA.MARSHAL();
            }
            return data[at++];
        }
        @Override public void read_octet_array(byte[] value, int offset, int length) {
            for (int i = 0; i < length; i++) {
                value[offset + i] = read_octet();
            }
        }
        @Override public boolean read_boolean() { throw new UnsupportedOperationException(); }
        @Override public char read_char() { throw new UnsupportedOperationException(); }
        @Override public char read_wchar() { throw new UnsupportedOperationException(); }
        @Override public short read_short() { throw new UnsupportedOperationException(); }
        @Override public short read_ushort() { throw new UnsupportedOperationException(); }
        @Override public int read_long() { throw new UnsupportedOperationException(); }
        @Override public int read_ulong() { throw new UnsupportedOperationException(); }
        @Override public long read_longlong() { throw new UnsupportedOperationException(); }
        @Override public long read_ulonglong() { throw new UnsupportedOperationException(); }
        @Override public float read_float() { throw new UnsupportedOperationException(); }
        @Override public double read_double() { throw new UnsupportedOperationException(); }
        @Override public String read_string() { throw new UnsupportedOperationException(); }
        @Override public String read_wstring() { throw new UnsupportedOperationException(); }
        @Override public void read_boolean_array(boolean[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void read_char_array(char[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void read_wchar_array(char[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void read_short_array(short[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void read_ushort_array(short[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void read_long_array(int[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void read_ulong_array(int[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void read_longlong_array(long[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void read_ulonglong_array(long[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void read_float_array(float[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public void read_double_array(double[] value, int offset, int length) { throw new UnsupportedOperationException(); }
        @Override public org.omg.CORBA.Object read_Object() { throw new UnsupportedOperationException(); }
        @Override public org.omg.CORBA.TypeCode read_TypeCode() { throw new UnsupportedOperationException(); }
        @Override public org.omg.CORBA.Any read_any() { throw new UnsupportedOperationException(); }
    }
}
