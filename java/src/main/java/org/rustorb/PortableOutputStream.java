// An output stream that keeps what is written as calls to make again on
// another stream: the arguments of a request of a portable stub
// (ObjectImpl._request), written into the request when ObjectImpl._invoke
// sends it, and the streams of ORB.create_output_stream and
// Any.create_output_stream, read back with create_input_stream (a
// standalone stream with the native code sets).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.portable.InputStream;
import org.omg.CORBA_2_3.portable.OutputStream;

class PortableOutputStream extends OutputStream {
    /** One write, made again on a stream. */
    interface Op {
        void write(OutputStream out);
    }

    private final List<Op> ops = new ArrayList<>();
    /** The ORB of the stream (may be null: the default ORB). */
    final OrbImpl orb;

    PortableOutputStream(OrbImpl orb) {
        this.orb = orb;
    }

    void record(Op op) {
        ops.add(op);
    }

    /** Writes everything written so far on out. */
    void writeTo(org.omg.CORBA.portable.OutputStream out) {
        OutputStream o = (OutputStream) out;
        for (Op op : ops) {
            op.write(o);
        }
    }

    private static final class Replay extends Call {
        private final PortableOutputStream source;

        Replay(PortableOutputStream source) {
            this.source = source;
        }

        @Override
        public void write(org.omg.CORBA.portable.OutputStream out) {
            source.writeTo(out);
        }
    }

    /** A Call that writes what this stream holds. */
    Call replay(OrbImpl orb) {
        Call call = new Replay(this);
        call.orb = orb;
        return call;
    }

    @Override
    public InputStream create_input_stream() {
        OrbImpl o = orb != null ? orb : OrbImpl.defaultOrb();
        Call call = replay(o);
        long h;
        try {
            h = Native.inNew(o != null ? o.handle() : 0, call);
        } finally {
            Reference.reachabilityFence(o);
        }
        Bridge.rethrow(call);
        return new OwnedInputStream(h, o, null);
    }

    @Override
    public org.omg.CORBA.ORB orb() {
        return orb != null ? orb : OrbImpl.defaultOrb();
    }

    @Override
    public void write(int b) {
        write_octet((byte) b);
    }

    @Override
    public void write_boolean(boolean value) {
        record(o -> o.write_boolean(value));
    }

    @Override
    public void write_char(char value) {
        record(o -> o.write_char(value));
    }

    @Override
    public void write_wchar(char value) {
        record(o -> o.write_wchar(value));
    }

    @Override
    public void write_octet(byte value) {
        record(o -> o.write_octet(value));
    }

    @Override
    public void write_short(short value) {
        record(o -> o.write_short(value));
    }

    @Override
    public void write_ushort(short value) {
        record(o -> o.write_ushort(value));
    }

    @Override
    public void write_long(int value) {
        record(o -> o.write_long(value));
    }

    @Override
    public void write_ulong(int value) {
        record(o -> o.write_ulong(value));
    }

    @Override
    public void write_longlong(long value) {
        record(o -> o.write_longlong(value));
    }

    @Override
    public void write_ulonglong(long value) {
        record(o -> o.write_ulonglong(value));
    }

    @Override
    public void write_float(float value) {
        record(o -> o.write_float(value));
    }

    @Override
    public void write_double(double value) {
        record(o -> o.write_double(value));
    }

    @Override
    public void write_string(String value) {
        if (value == null) {
            throw new BAD_PARAM("null string", 0, CompletionStatus.COMPLETED_NO);
        }
        record(o -> o.write_string(value));
    }

    @Override
    public void write_wstring(String value) {
        if (value == null) {
            throw new BAD_PARAM("null wstring", 0, CompletionStatus.COMPLETED_NO);
        }
        record(o -> o.write_wstring(value));
    }

    // Arrays are copied: the caller may change them before the request is
    // sent.

    @Override
    public void write_boolean_array(boolean[] value, int offset, int length) {
        boolean[] v = Arrays.copyOfRange(value, offset, offset + length);
        record(o -> o.write_boolean_array(v, 0, v.length));
    }

    @Override
    public void write_char_array(char[] value, int offset, int length) {
        char[] v = Arrays.copyOfRange(value, offset, offset + length);
        record(o -> o.write_char_array(v, 0, v.length));
    }

    @Override
    public void write_wchar_array(char[] value, int offset, int length) {
        char[] v = Arrays.copyOfRange(value, offset, offset + length);
        record(o -> o.write_wchar_array(v, 0, v.length));
    }

    @Override
    public void write_octet_array(byte[] value, int offset, int length) {
        byte[] v = Arrays.copyOfRange(value, offset, offset + length);
        record(o -> o.write_octet_array(v, 0, v.length));
    }

    @Override
    public void write_short_array(short[] value, int offset, int length) {
        short[] v = Arrays.copyOfRange(value, offset, offset + length);
        record(o -> o.write_short_array(v, 0, v.length));
    }

    @Override
    public void write_ushort_array(short[] value, int offset, int length) {
        short[] v = Arrays.copyOfRange(value, offset, offset + length);
        record(o -> o.write_ushort_array(v, 0, v.length));
    }

    @Override
    public void write_long_array(int[] value, int offset, int length) {
        int[] v = Arrays.copyOfRange(value, offset, offset + length);
        record(o -> o.write_long_array(v, 0, v.length));
    }

    @Override
    public void write_ulong_array(int[] value, int offset, int length) {
        int[] v = Arrays.copyOfRange(value, offset, offset + length);
        record(o -> o.write_ulong_array(v, 0, v.length));
    }

    @Override
    public void write_longlong_array(long[] value, int offset, int length) {
        long[] v = Arrays.copyOfRange(value, offset, offset + length);
        record(o -> o.write_longlong_array(v, 0, v.length));
    }

    @Override
    public void write_ulonglong_array(long[] value, int offset, int length) {
        long[] v = Arrays.copyOfRange(value, offset, offset + length);
        record(o -> o.write_ulonglong_array(v, 0, v.length));
    }

    @Override
    public void write_float_array(float[] value, int offset, int length) {
        float[] v = Arrays.copyOfRange(value, offset, offset + length);
        record(o -> o.write_float_array(v, 0, v.length));
    }

    @Override
    public void write_double_array(double[] value, int offset, int length) {
        double[] v = Arrays.copyOfRange(value, offset, offset + length);
        record(o -> o.write_double_array(v, 0, v.length));
    }

    @Override
    public void write_Object(org.omg.CORBA.Object value) {
        if (value != null) {
            // Not a reference of rustorb (or a local object): fail now.
            ObjectDelegateImpl.of(value);
        }
        record(o -> o.write_Object(value));
    }

    @Override
    public void write_abstract_interface(java.lang.Object obj) {
        Stubs.writeAbstract(this, obj);
    }

    @Override
    public void write_TypeCode(org.omg.CORBA.TypeCode value) {
        TypeCodeImpl tc = TypeCodeImpl.cast(value);
        record(o -> o.write_TypeCode(tc));
    }

    @Override
    public void write_any(org.omg.CORBA.Any value) {
        AnyImpl a = AnyImpl.cast(value).copy();
        record(o -> o.write_any(a));
    }

    // Values are written when the stream is: the state then, and instances
    // written twice as indirections.

    @Override
    public void write_value(java.io.Serializable value) {
        record(o -> o.write_value(value));
    }

    @Override
    public void write_value(java.io.Serializable value, String repository_id) {
        record(o -> o.write_value(value, repository_id));
    }

    @Override
    public void write_value(java.io.Serializable value, Class<?> clz) {
        record(o -> o.write_value(value, clz));
    }

    @Override
    public void write_value(java.io.Serializable value,
            org.omg.CORBA.portable.BoxedValueHelper factory) {
        record(o -> o.write_value(value, factory));
    }
}
