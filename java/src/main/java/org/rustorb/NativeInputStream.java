// The input stream of a callback (an rorb_in), valid until the callback
// returns.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.TCKind;

class NativeInputStream extends org.omg.CORBA_2_3.portable.InputStream {
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

    /** Gives id to the next read_string (the repository id of a user
     * exception the stream is past). */
    void inject(String id) {
        injectId = id;
    }

    /** Reads a value of type tc into a new any handle (0 after an error). */
    long readAnyValue(TypeCodeImpl tc) {
        try {
            return Native.inAnyValue(in(), tc.handle());
        } finally {
            java.lang.ref.Reference.reachabilityFence(tc);
        }
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

    // --- valuetypes ---------------------------------------------------------

    /** The instances read, by slot, for indirections. */
    private java.util.Map<Long, java.io.Serializable> values;
    /** The slot of the value a factory is making, until it calls
     * read_value(Serializable). */
    private long pendingSlot = -1;

    @Override
    public java.io.Serializable read_value() {
        return readValue(null, null);
    }

    @Override
    public java.io.Serializable read_value(String rep_id) {
        return readValue(rep_id, null);
    }

    @Override
    public java.io.Serializable read_value(Class<?> clz) {
        return readValue(null, null);
    }

    @Override
    public java.io.Serializable read_value(org.omg.CORBA.portable.BoxedValueHelper factory) {
        return readValue(factory.get_id(), factory);
    }

    @Override
    public java.io.Serializable read_value(java.io.Serializable value) {
        keep(value);
        if (!(value instanceof org.omg.CORBA.portable.StreamableValue)) {
            throw new org.omg.CORBA.MARSHAL("not a StreamableValue: " + value, 0,
                    CompletionStatus.COMPLETED_MAYBE);
        }
        ((org.omg.CORBA.portable.StreamableValue) value)._read(this);
        return value;
    }

    /** Keeps value for the slot being read. */
    private void keep(java.io.Serializable value) {
        if (pendingSlot >= 0) {
            if (values == null) {
                values = new java.util.HashMap<>();
            }
            values.put(pendingSlot, value);
            pendingSlot = -1;
        }
    }

    /** Reads null, an indirection or a new value: a value box with helper,
     * else an instance made by the factory of the first repository id
     * received that has one (expected when none is sent). Without a
     * factory, the stream fails (MARSHAL). */
    private java.io.Serializable readValue(String expected,
            org.omg.CORBA.portable.BoxedValueHelper helper) {
        long[] slotCount = new long[2];
        int kind = Native.inValueHeader(in(), slotCount);
        long slot = slotCount[0];
        if (kind == Native.VALUE_INDIRECTION) {
            java.io.Serializable v = values == null ? null : values.get(slot);
            if (v == null) {
                Native.inFail(in());
            }
            return v;
        }
        if (kind != Native.VALUE_NEW) {
            return null;
        }
        java.io.Serializable v;
        if (helper != null) {
            v = helper.read_value(this);
            pendingSlot = slot;
            keep(v);
        } else {
            int count = (int) slotCount[1];
            org.omg.CORBA.portable.ValueFactory factory = null;
            if (count == 0) {
                factory = Values.lookup(expected);
            }
            for (int i = 0; i < count && factory == null; i++) {
                factory = Values.lookup(Native.inValueId(in(), i));
            }
            if (factory == null) {
                Native.inFail(in());
                return null;
            }
            long outer = pendingSlot;
            pendingSlot = slot;
            v = factory.read_value(this);
            if (pendingSlot == slot) {
                keep(v);
            }
            pendingSlot = outer;
        }
        Native.inEndValue(in());
        return v;
    }
}
