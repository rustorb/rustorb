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
    /** The TypeCode type(t) set without a value of it (the next insert or
     * read_value gives one), else null. */
    private TypeCodeImpl pending;
    /** The stream of create_output_stream, whose writes become the value
     * when it is next used, else null. */
    private PortableOutputStream writer;

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

    /** The handle; BAD_OPERATION while the any has a TypeCode without a
     * value (see type(t)). */
    long handle() {
        if (writer != null) {
            PortableOutputStream w = writer;
            writer = null;
            TypeCodeImpl t = TypeCodeImpl.cast(type());
            insert(t, w::writeTo, t.unaliasedKind() == TCKind._tk_except);
        }
        if (pending != null) {
            throw new BAD_OPERATION("the any has no value of its TypeCode", 0,
                    CompletionStatus.COMPLETED_NO);
        }
        return handle.get();
    }

    /** Takes a new handle holding a value. */
    private void replace(long h) {
        handle.set(h);
        streamable = null;
        pending = null;
        writer = null;
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
        replace(h);
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
        if (pending != null) {
            return pending;
        }
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
            } else if (pending == null && writer == null && type().equivalent(tc)) {
                Native.anySetType(handle(), tc.handle());
            } else {
                // The value is lost: an insert, read_value or the writes on
                // the stream of create_output_stream give the new one
                // (portable Helpers set the type first).
                pending = tc;
                streamable = null;
            }
        } finally {
            Reference.reachabilityFence(tc);
        }
    }

    /** A new any with the same TypeCode and value. */
    AnyImpl copy() {
        try {
            return new AnyImpl(Native.anyClone(handle()));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    private static BAD_PARAM foreignStream() {
        return new BAD_PARAM("not a stream of rustorb", 0, CompletionStatus.COMPLETED_NO);
    }

    /** Reads a value of type t from is (a stream of rustorb). */
    @Override
    public void read_value(InputStream is, TypeCode t) {
        if (!(is instanceof NativeInputStream)) {
            throw foreignStream();
        }
        long h = ((NativeInputStream) is).readAnyValue(TypeCodeImpl.cast(t));
        if (h != 0) {
            replace(h);
        }
    }

    /** Writes the value, without the TypeCode. */
    @Override
    public void write_value(OutputStream os) {
        if (os instanceof NativeOutputStream) {
            ((NativeOutputStream) os).writeAnyValue(this);
        } else if (os instanceof PortableOutputStream) {
            AnyImpl a = copy();
            ((PortableOutputStream) os).record(o -> a.write_value(o));
        } else {
            throw foreignStream();
        }
    }

    /** A stream whose writes become the value, of the type the any then
     * has, when the any is next used (or that is given to read_value with
     * its create_input_stream()). */
    @Override
    public OutputStream create_output_stream() {
        writer = new PortableOutputStream(OrbImpl.defaultOrb());
        return writer;
    }

    /** A stream over the value (an exception starts with its repository
     * id). */
    @Override
    public InputStream create_input_stream() {
        long h;
        try {
            h = Native.anyInput(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
        return new OwnedInputStream(h, OrbImpl.defaultOrb(), null);
    }

    /** The TypeCode of a valuetype or value box without aliases;
     * BAD_OPERATION for other types. */
    private static TypeCode valueType(TypeCode t) {
        TypeCode u = unaliased(t);
        int kind = u.kind().value();
        if (kind != TCKind._tk_value && kind != TCKind._tk_value_box) {
            throw Anys.badOperation();
        }
        return u;
    }

    private static String idOf(TypeCode t) {
        try {
            return t.id();
        } catch (org.omg.CORBA.TypeCodePackage.BadKind e) {
            throw Anys.badOperation();
        }
    }

    @Override
    public java.io.Serializable extract_Value() {
        TypeCode u = valueType(type());
        String id = idOf(u);
        if (u.kind().value() == TCKind._tk_value_box) {
            org.omg.CORBA.portable.BoxedValueHelper helper = Values.boxHelper(id);
            if (helper == null) {
                throw new org.omg.CORBA.MARSHAL("no Helper for the value box " + id, 0,
                        CompletionStatus.COMPLETED_NO);
            }
            return extract(type(),
                    in -> ((org.omg.CORBA_2_3.portable.InputStream) in).read_value(helper), false);
        }
        return extract(type(),
                in -> ((org.omg.CORBA_2_3.portable.InputStream) in).read_value(id), false);
    }

    /** Inserts a StreamableValue (or null) with the TypeCode of its
     * class. */
    @Override
    public void insert_Value(java.io.Serializable v) {
        if (v instanceof org.omg.CORBA.portable.StreamableValue) {
            insert_Value(v, ((org.omg.CORBA.portable.StreamableValue) v)._type());
        } else {
            throw new org.omg.CORBA.MARSHAL("the TypeCode of "
                    + (v == null ? "null" : v.getClass().getName())
                    + " is not known; use insert_Value(v, type)", 0,
                    CompletionStatus.COMPLETED_NO);
        }
    }

    @Override
    public void insert_Value(java.io.Serializable v, TypeCode t) {
        TypeCode u = valueType(t);
        if (u.kind().value() == TCKind._tk_value_box) {
            String id = idOf(u);
            org.omg.CORBA.portable.BoxedValueHelper helper = Values.boxHelper(id);
            if (helper == null) {
                throw new org.omg.CORBA.MARSHAL("no Helper for the value box " + id, 0,
                        CompletionStatus.COMPLETED_NO);
            }
            insert(t, out -> ((org.omg.CORBA_2_3.portable.OutputStream) out).write_value(v, helper),
                    false);
        } else {
            insert(t, out -> ((org.omg.CORBA_2_3.portable.OutputStream) out).write_value(v), false);
        }
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
    public double extract_longdouble() {
        return extractKind(TCKind._tk_longdouble, InputStream::read_longdouble);
    }

    @Override
    public void insert_longdouble(double d) {
        insertKind(TCKind._tk_longdouble, out -> out.write_longdouble(d));
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
                name = nameOf(id);
            }
        }
        insert_Object(o, TypeCodes.iface(TCKind._tk_objref, id, name));
    }

    /** The last name of an IDL repository id ("IDL:M/I:1.0" is "I"), as
     * the TypeCodes of the interface's Helper have it; else "". */
    private static String nameOf(String id) {
        if (!id.startsWith("IDL:")) {
            return "";
        }
        int colon = id.lastIndexOf(':');
        String path = colon > 4 ? id.substring(4, colon) : id.substring(4);
        return path.substring(path.lastIndexOf('/') + 1);
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
