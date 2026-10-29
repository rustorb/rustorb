// An any of the rustorb core (an rorb_any handle, replaced on each insert).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.BAD_OPERATION;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.DATA_CONVERSION;
import org.omg.CORBA.NO_IMPLEMENT;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.portable.InputStream;
import org.omg.CORBA.portable.OutputStream;
import org.omg.CORBA.portable.Streamable;

final class AnyImpl extends Any {
    private final Owned handle;
    /** The Streamable of insert_Streamable, for extract_Streamable. */
    private Streamable streamable;

    AnyImpl() {
        this(Native.anyNew());
    }

    private AnyImpl(long handle) {
        this.handle = Owned.register(this, handle, Native::anyRelease);
    }

    /** Takes a handle; null for 0. */
    static AnyImpl of(long handle) {
        return handle == 0 ? null : new AnyImpl(handle);
    }

    long handle() {
        return handle.get();
    }

    static AnyImpl cast(Any a) {
        if (a instanceof AnyImpl) {
            return (AnyImpl) a;
        }
        throw new BAD_PARAM(a == null ? "null any" : "not a rustorb any", 0,
                CompletionStatus.COMPLETED_NO);
    }

    private static final class WriterCall extends Call {
        final Anys.Writer writer;

        WriterCall(Anys.Writer writer, boolean exception) {
            this.writer = writer;
            this.exceptionMode = exception ? Bridge.SKIP_ID : 0;
        }

        @Override
        public void write(OutputStream out) {
            writer.write(out);
        }
    }

    private static final class ReaderCall<T> extends Call {
        final Anys.Reader<T> reader;
        T value;

        ReaderCall(Anys.Reader<T> reader, String id) {
            this.reader = reader;
            this.injectId = id;
        }

        @Override
        public void read(InputStream in) {
            value = reader.read(in);
        }
    }

    void insert(TypeCode tc, Anys.Writer w, boolean exception) {
        TypeCodeImpl t = TypeCodeImpl.cast(tc);
        WriterCall call = new WriterCall(w, exception);
        call.orb = OrbImpl.defaultOrb();
        long h;
        try {
            h = Native.anyInsert(t.handle(), call);
        } finally {
            Reference.reachabilityFence(t);
        }
        Bridge.rethrow(call);
        handle.set(h);
        streamable = null;
    }

    <T> T extract(TypeCode tc, Anys.Reader<T> r, boolean exception) {
        TypeCodeImpl t = TypeCodeImpl.cast(tc);
        if (!type().equivalent(t)) {
            throw Anys.badOperation();
        }
        String id = null;
        if (exception) {
            try {
                id = t.id();
            } catch (org.omg.CORBA.TypeCodePackage.BadKind e) {
                throw Anys.badOperation();
            }
        }
        ReaderCall<T> call = new ReaderCall<>(r, id);
        call.orb = OrbImpl.defaultOrb();
        try {
            Native.anyExtract(handle(), call);
        } finally {
            Reference.reachabilityFence(this);
        }
        Bridge.rethrow(call);
        return call.value;
    }

    private void checkKind(int kind) {
        TypeCodeImpl t = (TypeCodeImpl) type();
        if (t.unaliasedKind() != kind) {
            throw Anys.badOperation();
        }
    }

    private <T> T extractKind(int kind, Anys.Reader<T> r) {
        checkKind(kind);
        ReaderCall<T> call = new ReaderCall<>(r, null);
        call.orb = OrbImpl.defaultOrb();
        try {
            Native.anyExtract(handle(), call);
        } finally {
            Reference.reachabilityFence(this);
        }
        Bridge.rethrow(call);
        return call.value;
    }

    private void insertKind(int kind, Anys.Writer w) {
        insert(TypeCodes.primitive(TCKind.from_int(kind)), w, false);
    }

    @Override
    public boolean equal(Any a) {
        AnyImpl other = cast(a);
        try {
            return Native.anyEqual(handle(), other.handle());
        } finally {
            Reference.reachabilityFence(this);
            Reference.reachabilityFence(other);
        }
    }

    @Override
    public boolean equals(Object o) {
        return o instanceof AnyImpl && equal((AnyImpl) o);
    }

    @Override
    public int hashCode() {
        return type().kind().value();
    }

    @Override
    public TypeCode type() {
        try {
            return TypeCodeImpl.of(Native.anyType(handle()));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public void type(TypeCode t) {
        TypeCodeImpl tc = TypeCodeImpl.cast(t);
        int kind = tc.unaliasedKind();
        try {
            if (kind == TCKind._tk_null || kind == TCKind._tk_void) {
                insert(tc, out -> {}, false);
            } else {
                Native.anySetType(handle(), tc.handle());
            }
        } finally {
            Reference.reachabilityFence(tc);
        }
    }

    @Override
    public void read_value(InputStream is, TypeCode t) {
        throw new NO_IMPLEMENT();
    }

    @Override
    public void write_value(OutputStream os) {
        throw new NO_IMPLEMENT();
    }

    @Override
    public OutputStream create_output_stream() {
        throw new NO_IMPLEMENT();
    }

    @Override
    public InputStream create_input_stream() {
        throw new NO_IMPLEMENT();
    }

    @Override
    public short extract_short() {
        return extractKind(TCKind._tk_short, InputStream::read_short);
    }

    @Override
    public void insert_short(short s) {
        insertKind(TCKind._tk_short, out -> out.write_short(s));
    }

    @Override
    public int extract_long() {
        return extractKind(TCKind._tk_long, InputStream::read_long);
    }

    @Override
    public void insert_long(int l) {
        insertKind(TCKind._tk_long, out -> out.write_long(l));
    }

    @Override
    public long extract_longlong() {
        return extractKind(TCKind._tk_longlong, InputStream::read_longlong);
    }

    @Override
    public void insert_longlong(long l) {
        insertKind(TCKind._tk_longlong, out -> out.write_longlong(l));
    }

    @Override
    public short extract_ushort() {
        return extractKind(TCKind._tk_ushort, InputStream::read_ushort);
    }

    @Override
    public void insert_ushort(short s) {
        insertKind(TCKind._tk_ushort, out -> out.write_ushort(s));
    }

    @Override
    public int extract_ulong() {
        return extractKind(TCKind._tk_ulong, InputStream::read_ulong);
    }

    @Override
    public void insert_ulong(int l) {
        insertKind(TCKind._tk_ulong, out -> out.write_ulong(l));
    }

    @Override
    public long extract_ulonglong() {
        return extractKind(TCKind._tk_ulonglong, InputStream::read_ulonglong);
    }

    @Override
    public void insert_ulonglong(long l) {
        insertKind(TCKind._tk_ulonglong, out -> out.write_ulonglong(l));
    }

    @Override
    public float extract_float() {
        return extractKind(TCKind._tk_float, InputStream::read_float);
    }

    @Override
    public void insert_float(float f) {
        insertKind(TCKind._tk_float, out -> out.write_float(f));
    }

    @Override
    public double extract_double() {
        return extractKind(TCKind._tk_double, InputStream::read_double);
    }

    @Override
    public void insert_double(double d) {
        insertKind(TCKind._tk_double, out -> out.write_double(d));
    }

    @Override
    public boolean extract_boolean() {
        return extractKind(TCKind._tk_boolean, InputStream::read_boolean);
    }

    @Override
    public void insert_boolean(boolean b) {
        insertKind(TCKind._tk_boolean, out -> out.write_boolean(b));
    }

    @Override
    public char extract_char() {
        return extractKind(TCKind._tk_char, InputStream::read_char);
    }

    @Override
    public void insert_char(char c) {
        if (c > 0xff) {
            throw new DATA_CONVERSION("char out of range", 0, CompletionStatus.COMPLETED_NO);
        }
        insertKind(TCKind._tk_char, out -> out.write_char(c));
    }

    @Override
    public char extract_wchar() {
        return extractKind(TCKind._tk_wchar, InputStream::read_wchar);
    }

    @Override
    public void insert_wchar(char c) {
        insertKind(TCKind._tk_wchar, out -> out.write_wchar(c));
    }

    @Override
    public byte extract_octet() {
        return extractKind(TCKind._tk_octet, InputStream::read_octet);
    }

    @Override
    public void insert_octet(byte b) {
        insertKind(TCKind._tk_octet, out -> out.write_octet(b));
    }

    @Override
    public Any extract_any() {
        return extractKind(TCKind._tk_any, InputStream::read_any);
    }

    @Override
    public void insert_any(Any a) {
        cast(a);
        insertKind(TCKind._tk_any, out -> out.write_any(a));
    }

    @Override
    public org.omg.CORBA.Object extract_Object() {
        return extractKind(TCKind._tk_objref, InputStream::read_Object);
    }

    @Override
    public void insert_Object(org.omg.CORBA.Object o) {
        String id = "IDL:omg.org/CORBA/Object:1.0";
        String name = "Object";
        if (o instanceof org.omg.CORBA.portable.ObjectImpl) {
            String[] ids = ((org.omg.CORBA.portable.ObjectImpl) o)._ids();
            if (ids.length > 0 && !(o instanceof ObjectStub)) {
                id = ids[0];
                name = "";
            }
        }
        insert_Object(o, TypeCodes.iface(TCKind._tk_objref, id, name));
    }

    @Override
    public void insert_Object(org.omg.CORBA.Object o, TypeCode t) {
        insert(t, out -> out.write_Object(o), false);
    }

    @Override
    public String extract_string() {
        return extractKind(TCKind._tk_string, InputStream::read_string);
    }

    @Override
    public void insert_string(String s) {
        insertKind(TCKind._tk_string, out -> out.write_string(s));
    }

    @Override
    public String extract_wstring() {
        return extractKind(TCKind._tk_wstring, InputStream::read_wstring);
    }

    @Override
    public void insert_wstring(String s) {
        insertKind(TCKind._tk_wstring, out -> out.write_wstring(s));
    }

    @Override
    public TypeCode extract_TypeCode() {
        return extractKind(TCKind._tk_TypeCode, InputStream::read_TypeCode);
    }

    @Override
    public void insert_TypeCode(TypeCode t) {
        TypeCodeImpl.cast(t);
        insertKind(TCKind._tk_TypeCode, out -> out.write_TypeCode(t));
    }

    @Override
    public java.math.BigDecimal extract_fixed() {
        checkKind(TCKind._tk_fixed);
        TypeCode t = unaliased(type());
        try {
            short digits = t.fixed_digits();
            short scale = t.fixed_scale();
            return extractKind(TCKind._tk_fixed, in -> in.read_fixed(digits, scale));
        } catch (org.omg.CORBA.TypeCodePackage.BadKind e) {
            throw Anys.badOperation();
        }
    }

    /** Inserts value as a fixed of its own digits and scale. */
    @Override
    public void insert_fixed(java.math.BigDecimal value) {
        if (value.scale() < 0) {
            value = value.setScale(0);
        }
        int digits = Math.max(1, Math.max(value.precision(), value.scale()));
        if (digits > 31) {
            throw new BAD_PARAM("fixed value with more than 31 digits", 0,
                    CompletionStatus.COMPLETED_NO);
        }
        insert_fixed(value, TypeCodes.fixed((short) digits, (short) value.scale()));
    }

    /** Inserts value as a fixed of type (BAD_INV_ORDER if it is not a
     * fixed type, BAD_PARAM if the value does not fit). */
    @Override
    public void insert_fixed(java.math.BigDecimal value, TypeCode type) {
        TypeCode t = unaliased(TypeCodeImpl.cast(type));
        short digits;
        short scale;
        try {
            if (t.kind().value() != TCKind._tk_fixed) {
                throw new BAD_INV_ORDER("not a fixed TypeCode", 0, CompletionStatus.COMPLETED_NO);
            }
            digits = t.fixed_digits();
            scale = t.fixed_scale();
        } catch (org.omg.CORBA.TypeCodePackage.BadKind e) {
            throw new BAD_INV_ORDER("not a fixed TypeCode", 0, CompletionStatus.COMPLETED_NO);
        }
        insert(type, out -> out.write_fixed(value, digits, scale), false);
    }

    private static TypeCode unaliased(TypeCode t) {
        try {
            while (t.kind().value() == TCKind._tk_alias) {
                t = t.content_type();
            }
        } catch (org.omg.CORBA.TypeCodePackage.BadKind e) {
            throw Anys.badOperation();
        }
        return t;
    }

    @Override
    public Streamable extract_Streamable() {
        if (streamable == null) {
            throw new org.omg.CORBA.BAD_INV_ORDER("no Streamable was inserted", 0,
                    CompletionStatus.COMPLETED_NO);
        }
        return streamable;
    }

    @Override
    public void insert_Streamable(Streamable s) {
        insert(s._type(), s::_write, false);
        streamable = s;
    }

    @Override
    public String toString() {
        return "Any(" + type() + ")";
    }

}
