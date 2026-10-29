// The input stream of a callback (an rorb_in), valid until the callback
// returns.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.TCKind;

final class NativeInputStream extends org.omg.CORBA.portable.InputStream {
    private long in;
    private final OrbImpl orb;
    /** The result of the first read_string (see Call.injectId). */
    private String injectId;

    NativeInputStream(long in, OrbImpl orb, String injectId) {
        this.in = in;
        this.orb = orb;
        this.injectId = injectId;
    }

    void invalidate() {
        in = 0;
    }

    long in() {
        if (in == 0) {
            throw new BAD_INV_ORDER("the stream is used outside its call", 0,
                    CompletionStatus.COMPLETED_MAYBE);
        }
        return in;
    }

    @Override
    public org.omg.CORBA.ORB orb() {
        return orb;
    }

    @Override
    public int read() {
        return read_octet() & 0xff;
    }

    @Override
    public boolean read_boolean() {
        return Native.inBoolean(in());
    }

    @Override
    public char read_char() {
        return Native.inChar(in());
    }

    @Override
    public char read_wchar() {
        return Native.inWchar(in());
    }

    @Override
    public byte read_octet() {
        return Native.inOctet(in());
    }

    @Override
    public short read_short() {
        return Native.inShort(in());
    }

    @Override
    public short read_ushort() {
        return Native.inUshort(in());
    }

    @Override
    public int read_long() {
        return Native.inLong(in());
    }

    @Override
    public int read_ulong() {
        return Native.inUlong(in());
    }

    @Override
    public long read_longlong() {
        return Native.inLonglong(in());
    }

    @Override
    public long read_ulonglong() {
        return Native.inUlonglong(in());
    }

    @Override
    public float read_float() {
        return Native.inFloat(in());
    }

    @Override
    public double read_double() {
        return Native.inDouble(in());
    }

    @Override
    public String read_string() {
        if (injectId != null) {
            String id = injectId;
            injectId = null;
            return id;
        }
        return Native.inString(in());
    }

    @Override
    public String read_wstring() {
        return Native.inWstring(in());
    }

    @Override
    public void read_boolean_array(boolean[] value, int offset, int length) {
        for (int i = 0; i < length; i++) {
            value[offset + i] = read_boolean();
        }
    }

    @Override
    public void read_char_array(char[] value, int offset, int length) {
        for (int i = 0; i < length; i++) {
            value[offset + i] = read_char();
        }
    }

    @Override
    public void read_wchar_array(char[] value, int offset, int length) {
        for (int i = 0; i < length; i++) {
            value[offset + i] = read_wchar();
        }
    }

    @Override
    public void read_octet_array(byte[] value, int offset, int length) {
        Native.inArrayByte(in(), Native.PRIM_OCTET, value, offset, length);
    }

    @Override
    public void read_short_array(short[] value, int offset, int length) {
        Native.inArrayShort(in(), Native.PRIM_SHORT, value, offset, length);
    }

    @Override
    public void read_ushort_array(short[] value, int offset, int length) {
        Native.inArrayShort(in(), Native.PRIM_USHORT, value, offset, length);
    }

    @Override
    public void read_long_array(int[] value, int offset, int length) {
        Native.inArrayInt(in(), Native.PRIM_LONG, value, offset, length);
    }

    @Override
    public void read_ulong_array(int[] value, int offset, int length) {
        Native.inArrayInt(in(), Native.PRIM_ULONG, value, offset, length);
    }

    @Override
    public void read_longlong_array(long[] value, int offset, int length) {
        Native.inArrayLong(in(), Native.PRIM_LONGLONG, value, offset, length);
    }

    @Override
    public void read_ulonglong_array(long[] value, int offset, int length) {
        Native.inArrayLong(in(), Native.PRIM_ULONGLONG, value, offset, length);
    }

    @Override
    public void read_float_array(float[] value, int offset, int length) {
        Native.inArrayFloat(in(), Native.PRIM_FLOAT, value, offset, length);
    }

    @Override
    public void read_double_array(double[] value, int offset, int length) {
        Native.inArrayDouble(in(), Native.PRIM_DOUBLE, value, offset, length);
    }

    @Override
    public org.omg.CORBA.Object read_Object() {
        return ObjectStub.of(Native.inObject(in()), orb);
    }

    @Override
    public org.omg.CORBA.Object read_Object(Class<?> clz) {
        org.omg.CORBA.Object o = read_Object();
        if (o == null) {
            return null;
        }
        return Stubs.narrowTo(o, clz);
    }

    @Override
    public org.omg.CORBA.TypeCode read_TypeCode() {
        TypeCodeImpl tc = TypeCodeImpl.of(Native.inTypeCode(in()));
        // After an error (reported when the callback returns).
        return tc != null ? tc : TypeCodes.primitive(TCKind.tk_null);
    }

    @Override
    public org.omg.CORBA.Any read_any() {
        AnyImpl a = AnyImpl.of(Native.inAny(in()));
        return a != null ? a : new AnyImpl();
    }

    /** A sequence length, checked against what is left in the stream. */
    int readLength(int minElementSize) {
        return Native.inLength(in(), minElementSize);
    }

}
