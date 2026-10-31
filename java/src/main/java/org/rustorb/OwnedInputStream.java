// An input stream the caller owns (an owned rorb_in): the reply of a
// portable stub (ObjectImpl._invoke) or the stream of create_input_stream.
// Unlike the stream of a callback, it stays valid after the call that made
// it, until release() (ObjectImpl._releaseReply) or until it is collected,
// and each read throws the error it meets.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

final class OwnedInputStream extends NativeInputStream {
    private final Owned owned;

    /** Takes the handle h (not 0). */
    OwnedInputStream(long h, OrbImpl orb, String injectId) {
        super(h, orb, injectId);
        owned = Owned.register(this, h, Native::inRelease);
    }

    /** Releases the stream; reads after it are BAD_INV_ORDER. */
    void release() {
        invalidate();
        owned.run();
    }

    /** Throws the error of the last read. */
    private void check() {
        long h = owned.get();
        if (h != 0) {
            try {
                Native.inCheck(h);
            } finally {
                Reference.reachabilityFence(this);
            }
        }
    }

    @Override
    public boolean read_boolean() {
        boolean v = super.read_boolean();
        check();
        return v;
    }

    @Override
    public char read_char() {
        char v = super.read_char();
        check();
        return v;
    }

    @Override
    public char read_wchar() {
        char v = super.read_wchar();
        check();
        return v;
    }

    @Override
    public byte read_octet() {
        byte v = super.read_octet();
        check();
        return v;
    }

    @Override
    public short read_short() {
        short v = super.read_short();
        check();
        return v;
    }

    @Override
    public short read_ushort() {
        short v = super.read_ushort();
        check();
        return v;
    }

    @Override
    public int read_long() {
        int v = super.read_long();
        check();
        return v;
    }

    @Override
    public int read_ulong() {
        int v = super.read_ulong();
        check();
        return v;
    }

    @Override
    public long read_longlong() {
        long v = super.read_longlong();
        check();
        return v;
    }

    @Override
    public long read_ulonglong() {
        long v = super.read_ulonglong();
        check();
        return v;
    }

    @Override
    public float read_float() {
        float v = super.read_float();
        check();
        return v;
    }

    @Override
    public double read_double() {
        double v = super.read_double();
        check();
        return v;
    }

    @Override
    public double read_longdouble() {
        double v = super.read_longdouble();
        check();
        return v;
    }

    @Override
    public String read_string() {
        String v = super.read_string();
        check();
        return v;
    }

    @Override
    public String read_wstring() {
        String v = super.read_wstring();
        check();
        return v;
    }

    @Override
    public void read_octet_array(byte[] value, int offset, int length) {
        super.read_octet_array(value, offset, length);
        check();
    }

    @Override
    public void read_short_array(short[] value, int offset, int length) {
        super.read_short_array(value, offset, length);
        check();
    }

    @Override
    public void read_ushort_array(short[] value, int offset, int length) {
        super.read_ushort_array(value, offset, length);
        check();
    }

    @Override
    public void read_long_array(int[] value, int offset, int length) {
        super.read_long_array(value, offset, length);
        check();
    }

    @Override
    public void read_ulong_array(int[] value, int offset, int length) {
        super.read_ulong_array(value, offset, length);
        check();
    }

    @Override
    public void read_longlong_array(long[] value, int offset, int length) {
        super.read_longlong_array(value, offset, length);
        check();
    }

    @Override
    public void read_ulonglong_array(long[] value, int offset, int length) {
        super.read_ulonglong_array(value, offset, length);
        check();
    }

    @Override
    public void read_float_array(float[] value, int offset, int length) {
        super.read_float_array(value, offset, length);
        check();
    }

    @Override
    public void read_double_array(double[] value, int offset, int length) {
        super.read_double_array(value, offset, length);
        check();
    }

    @Override
    public org.omg.CORBA.Object read_Object() {
        org.omg.CORBA.Object v = super.read_Object();
        check();
        return v;
    }

    @Override
    public org.omg.CORBA.TypeCode read_TypeCode() {
        org.omg.CORBA.TypeCode v = super.read_TypeCode();
        check();
        return v;
    }

    @Override
    public org.omg.CORBA.Any read_any() {
        org.omg.CORBA.Any v = super.read_any();
        check();
        return v;
    }

    @Override
    long readAnyValue(TypeCodeImpl tc) {
        long v = super.readAnyValue(tc);
        check();
        return v;
    }

    @Override
    int readLength(int minElementSize) {
        int v = super.readLength(minElementSize);
        check();
        return v;
    }

    @Override
    public java.io.Serializable read_value() {
        java.io.Serializable v = super.read_value();
        check();
        return v;
    }

    @Override
    public java.io.Serializable read_value(String rep_id) {
        java.io.Serializable v = super.read_value(rep_id);
        check();
        return v;
    }

    @Override
    public java.io.Serializable read_value(Class<?> clz) {
        java.io.Serializable v = super.read_value(clz);
        check();
        return v;
    }

    @Override
    public java.io.Serializable read_value(org.omg.CORBA.portable.BoxedValueHelper factory) {
        java.io.Serializable v = super.read_value(factory);
        check();
        return v;
    }

    @Override
    public java.io.Serializable read_value(java.io.Serializable value) {
        java.io.Serializable v = super.read_value(value);
        check();
        return v;
    }
}
