// The output stream of a callback (an rorb_out), valid until the callback
// returns.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.DATA_CONVERSION;
import org.omg.CORBA.MARSHAL;
import org.omg.CORBA.NO_IMPLEMENT;
import org.omg.CORBA.portable.InputStream;

final class NativeOutputStream extends org.omg.CORBA_2_3.portable.OutputStream {
    private long out;
    private final OrbImpl orb;
    /** An exception reply whose repository id is not written yet: the
     * first write_string names the exception and opens the stream. */
    private long reply;
    /** Drop the first string written (see Call.exceptionMode). */
    private boolean skipId;

    NativeOutputStream(long out, OrbImpl orb, boolean skipId) {
        this.out = out;
        this.orb = orb;
        this.skipId = skipId;
    }

    /** The stream of a user exception reply. */
    static NativeOutputStream exceptionReply(long reply, OrbImpl orb) {
        NativeOutputStream s = new NativeOutputStream(0, orb, false);
        s.reply = reply;
        return s;
    }

    /** Called when the callback returns. */
    void invalidate() {
        out = 0;
        reply = 0;
    }

    private long out() {
        if (out == 0) {
            if (reply != 0) {
                throw new MARSHAL("the repository id of the exception must be written first", 0,
                        CompletionStatus.COMPLETED_MAYBE);
            }
            throw new BAD_INV_ORDER("the stream is used outside its call", 0,
                    CompletionStatus.COMPLETED_MAYBE);
        }
        return out;
    }

    @Override
    public InputStream create_input_stream() {
        throw new NO_IMPLEMENT();
    }

    @Override
    public org.omg.CORBA.ORB orb() {
        return orb;
    }

    @Override
    public void write(int b) {
        write_octet((byte) b);
    }

    @Override
    public void write_boolean(boolean value) {
        Native.outBoolean(out(), value);
    }

    @Override
    public void write_char(char value) {
        if (value > 0xff) {
            throw new DATA_CONVERSION("char out of range", 0, CompletionStatus.COMPLETED_NO);
        }
        Native.outChar(out(), value);
    }

    @Override
    public void write_wchar(char value) {
        Native.outWchar(out(), value);
    }

    @Override
    public void write_octet(byte value) {
        Native.outOctet(out(), value);
    }

    @Override
    public void write_short(short value) {
        Native.outShort(out(), value);
    }

    @Override
    public void write_ushort(short value) {
        Native.outUshort(out(), value);
    }

    @Override
    public void write_long(int value) {
        Native.outLong(out(), value);
    }

    @Override
    public void write_ulong(int value) {
        Native.outUlong(out(), value);
    }

    @Override
    public void write_longlong(long value) {
        Native.outLonglong(out(), value);
    }

    @Override
    public void write_ulonglong(long value) {
        Native.outUlonglong(out(), value);
    }

    @Override
    public void write_float(float value) {
        Native.outFloat(out(), value);
    }

    @Override
    public void write_double(double value) {
        Native.outDouble(out(), value);
    }

    @Override
    public void write_string(String value) {
        if (out == 0 && reply != 0) {
            if (value == null) {
                throw new BAD_PARAM("null string", 0, CompletionStatus.COMPLETED_MAYBE);
            }
            out = Native.replyUserException(reply, value);
            reply = 0;
            return;
        }
        if (skipId) {
            skipId = false;
            return;
        }
        if (value == null) {
            throw new BAD_PARAM("null string", 0, CompletionStatus.COMPLETED_NO);
        }
        Native.outString(out(), value);
    }

    @Override
    public void write_wstring(String value) {
        if (value == null) {
            throw new BAD_PARAM("null wstring", 0, CompletionStatus.COMPLETED_NO);
        }
        Native.outWstring(out(), value);
    }

    @Override
    public void write_boolean_array(boolean[] value, int offset, int length) {
        for (int i = 0; i < length; i++) {
            write_boolean(value[offset + i]);
        }
    }

    @Override
    public void write_char_array(char[] value, int offset, int length) {
        for (int i = 0; i < length; i++) {
            write_char(value[offset + i]);
        }
    }

    @Override
    public void write_wchar_array(char[] value, int offset, int length) {
        for (int i = 0; i < length; i++) {
            write_wchar(value[offset + i]);
        }
    }

    @Override
    public void write_octet_array(byte[] value, int offset, int length) {
        Native.outArrayByte(out(), Native.PRIM_OCTET, value, offset, length);
    }

    @Override
    public void write_short_array(short[] value, int offset, int length) {
        Native.outArrayShort(out(), Native.PRIM_SHORT, value, offset, length);
    }

    @Override
    public void write_ushort_array(short[] value, int offset, int length) {
        Native.outArrayShort(out(), Native.PRIM_USHORT, value, offset, length);
    }

    @Override
    public void write_long_array(int[] value, int offset, int length) {
        Native.outArrayInt(out(), Native.PRIM_LONG, value, offset, length);
    }

    @Override
    public void write_ulong_array(int[] value, int offset, int length) {
        Native.outArrayInt(out(), Native.PRIM_ULONG, value, offset, length);
    }

    @Override
    public void write_longlong_array(long[] value, int offset, int length) {
        Native.outArrayLong(out(), Native.PRIM_LONGLONG, value, offset, length);
    }

    @Override
    public void write_ulonglong_array(long[] value, int offset, int length) {
        Native.outArrayLong(out(), Native.PRIM_ULONGLONG, value, offset, length);
    }

    @Override
    public void write_float_array(float[] value, int offset, int length) {
        Native.outArrayFloat(out(), Native.PRIM_FLOAT, value, offset, length);
    }

    @Override
    public void write_double_array(double[] value, int offset, int length) {
        Native.outArrayDouble(out(), Native.PRIM_DOUBLE, value, offset, length);
    }

    @Override
    public void write_Object(org.omg.CORBA.Object value) {
        if (value == null) {
            Native.outObject(out(), 0);
            return;
        }
        ObjectDelegateImpl d = ObjectDelegateImpl.of(value);
        try {
            Native.outObject(out(), d.handle());
        } finally {
            Reference.reachabilityFence(d);
        }
    }

    @Override
    public void write_abstract_interface(java.lang.Object obj) {
        Stubs.writeAbstract(this, obj);
    }

    @Override
    public void write_TypeCode(org.omg.CORBA.TypeCode value) {
        TypeCodeImpl tc = TypeCodeImpl.cast(value);
        try {
            Native.outTypeCode(out(), tc.handle());
        } finally {
            Reference.reachabilityFence(tc);
        }
    }

    @Override
    public void write_any(org.omg.CORBA.Any value) {
        AnyImpl a = AnyImpl.cast(value);
        try {
            Native.outAny(out(), a.handle());
        } finally {
            Reference.reachabilityFence(a);
        }
    }

    /** Writes the value of a, without its TypeCode. */
    void writeAnyValue(AnyImpl a) {
        try {
            Native.outAnyValue(out(), a.handle());
        } finally {
            Reference.reachabilityFence(a);
        }
    }

    // --- valuetypes ---------------------------------------------------------

    /** Keys of the instances written; keys are unique in the process (and
     * apart from the addresses the core and C++ use), so streams that
     * write the same message never mix them up. */
    private static final java.util.concurrent.atomic.AtomicLong NEXT_KEY =
            new java.util.concurrent.atomic.AtomicLong();
    private java.util.IdentityHashMap<Object, Long> keys;

    private long key(Object v) {
        if (keys == null) {
            keys = new java.util.IdentityHashMap<>();
        }
        return keys.computeIfAbsent(v, k -> (1L << 62) | NEXT_KEY.incrementAndGet());
    }

    @Override
    public void write_value(java.io.Serializable value) {
        writeValue(value, null);
    }

    @Override
    public void write_value(java.io.Serializable value, String repository_id) {
        writeValue(value, null);
    }

    @Override
    public void write_value(java.io.Serializable value, Class<?> clz) {
        writeValue(value, null);
    }

    @Override
    public void write_value(java.io.Serializable value,
            org.omg.CORBA.portable.BoxedValueHelper factory) {
        writeValue(value, factory);
    }

    /** Writes null, an indirection to an instance written before, or the
     * value with its repository ids (chunked when it is truncatable or
     * custom). */
    private void writeValue(java.io.Serializable value,
            org.omg.CORBA.portable.BoxedValueHelper helper) {
        if (value == null) {
            Native.outNullValue(out());
            return;
        }
        String[] ids;
        boolean custom = helper == null && value instanceof org.omg.CORBA.portable.CustomValue;
        if (helper != null) {
            ids = new String[] {helper.get_id()};
        } else if (custom) {
            ids = ((org.omg.CORBA.portable.CustomValue) value)._truncatable_ids();
        } else if (value instanceof org.omg.CORBA.portable.StreamableValue) {
            ids = ((org.omg.CORBA.portable.StreamableValue) value)._truncatable_ids();
        } else {
            throw new BAD_PARAM("not a value this ORB can write: " + value.getClass().getName(), 0,
                    CompletionStatus.COMPLETED_MAYBE);
        }
        // Custom values are always chunked.
        if (Native.outBeginValue(out(), key(value), ids, ids.length > 1 || custom)) {
            if (helper != null) {
                helper.write_value(this, value);
            } else if (custom) {
                ((org.omg.CORBA.portable.CustomValue) value).marshal(new CustomOutput(this));
            } else {
                ((org.omg.CORBA.portable.StreamableValue) value)._write(this);
            }
            Native.outEndValue(out());
        }
    }
}
