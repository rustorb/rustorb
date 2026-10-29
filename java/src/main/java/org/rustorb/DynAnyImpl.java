// DynamicAny::DynAny and the derived interfaces, on the DynAny of the
// rustorb core (an rorb_dynany handle). Values cross the C ABI as anys of
// the core (AnyImpl). See docs/dynany.md.
//
// One class per derived interface, chosen by the kind of the type (of()).
// Components share their state with the DynAny they belong to, so two
// DynAnyImpls can be the same DynAny: _is_equivalent tells.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.Any;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.UserException;
import org.omg.CORBA.portable.InputStream;
import org.omg.DynamicAny.DynAny;
import org.omg.DynamicAny.DynAnyPackage.InvalidValue;
import org.omg.DynamicAny.DynAnyPackage.TypeMismatch;
import org.omg.DynamicAny.NameDynAnyPair;
import org.omg.DynamicAny.NameValuePair;

class DynAnyImpl extends org.omg.CORBA.LocalObject implements DynAny {
    private final Owned handle;
    private final String[] ids;

    private DynAnyImpl(long handle, String... ids) {
        this.handle = Owned.register(this, handle, Native::dynRelease);
        this.ids = ids;
    }

    /** Takes a handle; null for 0. */
    static DynAnyImpl of(long h) {
        if (h == 0) {
            return null;
        }
        switch (Native.dynKind(h)) {
            case TCKind._tk_fixed:
                return new Fixed(h);
            case TCKind._tk_enum:
                return new Enum(h);
            case TCKind._tk_struct:
            case TCKind._tk_except:
                return new Struct(h);
            case TCKind._tk_union:
                return new Union(h);
            case TCKind._tk_sequence:
                return new Sequence(h);
            case TCKind._tk_array:
                return new Array(h);
            case TCKind._tk_value:
            case TCKind._tk_event:
                return new Value(h);
            case TCKind._tk_value_box:
                return new ValueBox(h);
            default:
                return new DynAnyImpl(h);
        }
    }

    static DynAnyImpl cast(DynAny d) {
        if (d instanceof DynAnyImpl) {
            return (DynAnyImpl) d;
        }
        throw new org.omg.CORBA.BAD_PARAM(d == null ? "null DynAny" : "not a rustorb DynAny", 0,
                org.omg.CORBA.CompletionStatus.COMPLETED_NO);
    }

    long h() {
        return handle.get();
    }

    @Override
    public boolean _is_a(String id) {
        for (String i : ids) {
            if (i.equals(id)) {
                return true;
            }
        }
        return id.equals("IDL:omg.org/DynamicAny/DynAny:1.0")
                || id.equals("IDL:omg.org/CORBA/LocalObject:1.0")
                || id.equals("IDL:omg.org/CORBA/Object:1.0");
    }

    @Override
    public boolean _is_equivalent(org.omg.CORBA.Object other) {
        if (!(other instanceof DynAnyImpl)) {
            return false;
        }
        DynAnyImpl o = (DynAnyImpl) other;
        try {
            return Native.dynSame(h(), o.h());
        } finally {
            Reference.reachabilityFence(this);
            Reference.reachabilityFence(o);
        }
    }

    // --- calls: the user exceptions an operation declares pass, the others
    // are unexpected ---------------------------------------------------------

    @FunctionalInterface
    interface Op<T> {
        T run() throws UserException;
    }

    @FunctionalInterface
    interface VoidOp {
        void run() throws UserException;
    }

    final <T> T plain(Op<T> op) {
        try {
            return op.run();
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    final void plain(VoidOp op) {
        plain(() -> {
            op.run();
            return null;
        });
    }

    final <T> T tm(Op<T> op) throws TypeMismatch {
        try {
            return op.run();
        } catch (TypeMismatch e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    final void tm(VoidOp op) throws TypeMismatch {
        tm(() -> {
            op.run();
            return null;
        });
    }

    final <T> T iv(Op<T> op) throws InvalidValue {
        try {
            return op.run();
        } catch (InvalidValue e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    final void iv(VoidOp op) throws InvalidValue {
        iv(() -> {
            op.run();
            return null;
        });
    }

    final <T> T both(Op<T> op) throws TypeMismatch, InvalidValue {
        try {
            return op.run();
        } catch (TypeMismatch | InvalidValue e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    final void both(VoidOp op) throws TypeMismatch, InvalidValue {
        both(() -> {
            op.run();
            return null;
        });
    }

    static long any(Any a) {
        return AnyImpl.cast(a).handle();
    }

    static long dyn(DynAny d) {
        return cast(d).h();
    }

    // --- DynAny --------------------------------------------------------------

    @Override
    public TypeCode type() {
        return plain(() -> TypeCodeImpl.of(Native.dynType(h())));
    }

    @Override
    public void assign(DynAny dyn_any) throws TypeMismatch {
        tm(() -> {
            try {
                Native.dynAssign(h(), dyn(dyn_any));
            } finally {
                Reference.reachabilityFence(dyn_any);
            }
        });
    }

    @Override
    public void from_any(Any value) throws TypeMismatch, InvalidValue {
        both(() -> {
            try {
                Native.dynFromAny(h(), any(value));
            } finally {
                Reference.reachabilityFence(value);
            }
        });
    }

    @Override
    public Any to_any() {
        return plain(() -> AnyImpl.of(Native.dynToAny(h())));
    }

    @Override
    public boolean equal(DynAny dyn_any) {
        return plain(() -> {
            try {
                return Native.dynEqual(h(), dyn(dyn_any));
            } finally {
                Reference.reachabilityFence(dyn_any);
            }
        });
    }

    @Override
    public void destroy() {
        plain(() -> Native.dynDestroy(h()));
    }

    @Override
    public DynAny copy() {
        return plain(() -> of(Native.dynCopy(h())));
    }

    /** insert_<type>: a's value as a value of the kind. */
    private void insert(int kind, AnyImpl a) throws TypeMismatch, InvalidValue {
        both(() -> {
            try {
                Native.dynInsert(h(), kind, a.handle());
            } finally {
                Reference.reachabilityFence(a);
            }
        });
    }

    /** get_<type>: an any of the kind's type. */
    private AnyImpl get(int kind) throws TypeMismatch, InvalidValue {
        return both(() -> AnyImpl.of(Native.dynGet(h(), kind)));
    }

    /** insert_<type>_seq: elements writes length elements of the kind. */
    private void insertSeq(int kind, int length, Anys.Writer elements)
            throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        TypeCode tc = TypeCodes.sequence(TypeCodes.primitive(TCKind.from_int(kind)), 0);
        a.insert(tc, out -> {
            out.write_ulong(length);
            elements.write(out);
        }, false);
        both(() -> {
            try {
                Native.dynInsertSeq(h(), kind, a.handle());
            } finally {
                Reference.reachabilityFence(a);
            }
        });
    }

    interface ArrayReader<T> {
        T read(InputStream in, int length);
    }

    private <T> T getSeq(int kind, ArrayReader<T> elements) throws TypeMismatch, InvalidValue {
        AnyImpl a = both(() -> AnyImpl.of(Native.dynGetSeq(h(), kind)));
        return a.extract(a.type(), in -> elements.read(in, in.read_ulong()), false);
    }

    @Override
    public void insert_boolean(boolean value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_boolean(value);
        insert(TCKind._tk_boolean, a);
    }

    @Override
    public boolean get_boolean() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_boolean).extract_boolean();
    }

    @Override
    public void insert_octet(byte value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_octet(value);
        insert(TCKind._tk_octet, a);
    }

    @Override
    public byte get_octet() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_octet).extract_octet();
    }

    @Override
    public void insert_char(char value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_char(value);
        insert(TCKind._tk_char, a);
    }

    @Override
    public char get_char() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_char).extract_char();
    }

    @Override
    public void insert_short(short value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_short(value);
        insert(TCKind._tk_short, a);
    }

    @Override
    public short get_short() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_short).extract_short();
    }

    @Override
    public void insert_ushort(short value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_ushort(value);
        insert(TCKind._tk_ushort, a);
    }

    @Override
    public short get_ushort() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_ushort).extract_ushort();
    }

    @Override
    public void insert_long(int value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_long(value);
        insert(TCKind._tk_long, a);
    }

    @Override
    public int get_long() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_long).extract_long();
    }

    @Override
    public void insert_ulong(int value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_ulong(value);
        insert(TCKind._tk_ulong, a);
    }

    @Override
    public int get_ulong() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_ulong).extract_ulong();
    }

    @Override
    public void insert_float(float value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_float(value);
        insert(TCKind._tk_float, a);
    }

    @Override
    public float get_float() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_float).extract_float();
    }

    @Override
    public void insert_double(double value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_double(value);
        insert(TCKind._tk_double, a);
    }

    @Override
    public double get_double() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_double).extract_double();
    }

    @Override
    public void insert_string(String value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_string(value);
        insert(TCKind._tk_string, a);
    }

    @Override
    public String get_string() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_string).extract_string();
    }

    @Override
    public void insert_longlong(long value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_longlong(value);
        insert(TCKind._tk_longlong, a);
    }

    @Override
    public long get_longlong() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_longlong).extract_longlong();
    }

    @Override
    public void insert_ulonglong(long value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_ulonglong(value);
        insert(TCKind._tk_ulonglong, a);
    }

    @Override
    public long get_ulonglong() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_ulonglong).extract_ulonglong();
    }

    @Override
    public void insert_wchar(char value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_wchar(value);
        insert(TCKind._tk_wchar, a);
    }

    @Override
    public char get_wchar() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_wchar).extract_wchar();
    }

    @Override
    public void insert_wstring(String value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_wstring(value);
        insert(TCKind._tk_wstring, a);
    }

    @Override
    public String get_wstring() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_wstring).extract_wstring();
    }

    @Override
    public void insert_any(Any value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_any(value);
        insert(TCKind._tk_any, a);
    }

    @Override
    public Any get_any() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_any).extract_any();
    }

    @Override
    public void insert_reference(org.omg.CORBA.Object value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_Object(value);
        insert(TCKind._tk_objref, a);
    }

    @Override
    public org.omg.CORBA.Object get_reference() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_objref).extract_Object();
    }

    @Override
    public void insert_typecode(TypeCode value) throws TypeMismatch, InvalidValue {
        AnyImpl a = new AnyImpl();
        a.insert_TypeCode(value);
        insert(TCKind._tk_TypeCode, a);
    }

    @Override
    public TypeCode get_typecode() throws TypeMismatch, InvalidValue {
        return get(TCKind._tk_TypeCode).extract_TypeCode();
    }

    @Override
    public void insert_dyn_any(DynAny value) throws TypeMismatch, InvalidValue {
        both(() -> {
            try {
                Native.dynInsertDynAny(h(), dyn(value));
            } finally {
                Reference.reachabilityFence(value);
            }
        });
    }

    @Override
    public DynAny get_dyn_any() throws TypeMismatch, InvalidValue {
        return both(() -> of(Native.dynGetDynAny(h())));
    }

    /** Written with the type of the current component (or of this
     * DynAny), read with the factory of the instance's type. */
    @Override
    public void insert_val(java.io.Serializable value) throws TypeMismatch, InvalidValue {
        TypeCode tc = both(() -> TypeCodeImpl.of(Native.dynTargetType(h())));
        AnyImpl a = new AnyImpl();
        a.insert(tc, out -> ((org.omg.CORBA_2_3.portable.OutputStream) out).write_value(value),
                false);
        insert(TCKind._tk_value, a);
    }

    @Override
    public java.io.Serializable get_val() throws TypeMismatch, InvalidValue {
        AnyImpl a = get(TCKind._tk_value);
        return a.extract(a.type(),
                in -> ((org.omg.CORBA_2_3.portable.InputStream) in).read_value(), false);
    }

    /** Abstract interfaces are not supported: NO_IMPLEMENT. */
    @Override
    public void insert_abstract(org.omg.CORBA.Object value) {
        plain(() -> Native.dynNoImplement(h()));
    }

    @Override
    public org.omg.CORBA.Object get_abstract() {
        plain(() -> Native.dynNoImplement(h()));
        return null;
    }

    @Override
    public void insert_boolean_seq(boolean[] value) throws TypeMismatch, InvalidValue {
        insertSeq(TCKind._tk_boolean, value.length, out -> out.write_boolean_array(value, 0, value.length));
    }

    @Override
    public boolean[] get_boolean_seq() throws TypeMismatch, InvalidValue {
        return getSeq(TCKind._tk_boolean, (in, n) -> {
            boolean[] v = new boolean[n];
            in.read_boolean_array(v, 0, n);
            return v;
        });
    }

    @Override
    public void insert_octet_seq(byte[] value) throws TypeMismatch, InvalidValue {
        insertSeq(TCKind._tk_octet, value.length, out -> out.write_octet_array(value, 0, value.length));
    }

    @Override
    public byte[] get_octet_seq() throws TypeMismatch, InvalidValue {
        return getSeq(TCKind._tk_octet, (in, n) -> {
            byte[] v = new byte[n];
            in.read_octet_array(v, 0, n);
            return v;
        });
    }

    @Override
    public void insert_char_seq(char[] value) throws TypeMismatch, InvalidValue {
        insertSeq(TCKind._tk_char, value.length, out -> out.write_char_array(value, 0, value.length));
    }

    @Override
    public char[] get_char_seq() throws TypeMismatch, InvalidValue {
        return getSeq(TCKind._tk_char, (in, n) -> {
            char[] v = new char[n];
            in.read_char_array(v, 0, n);
            return v;
        });
    }

    @Override
    public void insert_short_seq(short[] value) throws TypeMismatch, InvalidValue {
        insertSeq(TCKind._tk_short, value.length, out -> out.write_short_array(value, 0, value.length));
    }

    @Override
    public short[] get_short_seq() throws TypeMismatch, InvalidValue {
        return getSeq(TCKind._tk_short, (in, n) -> {
            short[] v = new short[n];
            in.read_short_array(v, 0, n);
            return v;
        });
    }

    @Override
    public void insert_ushort_seq(short[] value) throws TypeMismatch, InvalidValue {
        insertSeq(TCKind._tk_ushort, value.length, out -> out.write_ushort_array(value, 0, value.length));
    }

    @Override
    public short[] get_ushort_seq() throws TypeMismatch, InvalidValue {
        return getSeq(TCKind._tk_ushort, (in, n) -> {
            short[] v = new short[n];
            in.read_ushort_array(v, 0, n);
            return v;
        });
    }

    @Override
    public void insert_long_seq(int[] value) throws TypeMismatch, InvalidValue {
        insertSeq(TCKind._tk_long, value.length, out -> out.write_long_array(value, 0, value.length));
    }

    @Override
    public int[] get_long_seq() throws TypeMismatch, InvalidValue {
        return getSeq(TCKind._tk_long, (in, n) -> {
            int[] v = new int[n];
            in.read_long_array(v, 0, n);
            return v;
        });
    }

    @Override
    public void insert_ulong_seq(int[] value) throws TypeMismatch, InvalidValue {
        insertSeq(TCKind._tk_ulong, value.length, out -> out.write_ulong_array(value, 0, value.length));
    }

    @Override
    public int[] get_ulong_seq() throws TypeMismatch, InvalidValue {
        return getSeq(TCKind._tk_ulong, (in, n) -> {
            int[] v = new int[n];
            in.read_ulong_array(v, 0, n);
            return v;
        });
    }

    @Override
    public void insert_float_seq(float[] value) throws TypeMismatch, InvalidValue {
        insertSeq(TCKind._tk_float, value.length, out -> out.write_float_array(value, 0, value.length));
    }

    @Override
    public float[] get_float_seq() throws TypeMismatch, InvalidValue {
        return getSeq(TCKind._tk_float, (in, n) -> {
            float[] v = new float[n];
            in.read_float_array(v, 0, n);
            return v;
        });
    }

    @Override
    public void insert_double_seq(double[] value) throws TypeMismatch, InvalidValue {
        insertSeq(TCKind._tk_double, value.length, out -> out.write_double_array(value, 0, value.length));
    }

    @Override
    public double[] get_double_seq() throws TypeMismatch, InvalidValue {
        return getSeq(TCKind._tk_double, (in, n) -> {
            double[] v = new double[n];
            in.read_double_array(v, 0, n);
            return v;
        });
    }

    @Override
    public void insert_longlong_seq(long[] value) throws TypeMismatch, InvalidValue {
        insertSeq(TCKind._tk_longlong, value.length, out -> out.write_longlong_array(value, 0, value.length));
    }

    @Override
    public long[] get_longlong_seq() throws TypeMismatch, InvalidValue {
        return getSeq(TCKind._tk_longlong, (in, n) -> {
            long[] v = new long[n];
            in.read_longlong_array(v, 0, n);
            return v;
        });
    }

    @Override
    public void insert_ulonglong_seq(long[] value) throws TypeMismatch, InvalidValue {
        insertSeq(TCKind._tk_ulonglong, value.length, out -> out.write_ulonglong_array(value, 0, value.length));
    }

    @Override
    public long[] get_ulonglong_seq() throws TypeMismatch, InvalidValue {
        return getSeq(TCKind._tk_ulonglong, (in, n) -> {
            long[] v = new long[n];
            in.read_ulonglong_array(v, 0, n);
            return v;
        });
    }

    @Override
    public void insert_wchar_seq(char[] value) throws TypeMismatch, InvalidValue {
        insertSeq(TCKind._tk_wchar, value.length, out -> out.write_wchar_array(value, 0, value.length));
    }

    @Override
    public char[] get_wchar_seq() throws TypeMismatch, InvalidValue {
        return getSeq(TCKind._tk_wchar, (in, n) -> {
            char[] v = new char[n];
            in.read_wchar_array(v, 0, n);
            return v;
        });
    }

    @Override
    public boolean seek(int index) {
        return plain(() -> Native.dynSeek(h(), index));
    }

    @Override
    public void rewind() {
        plain(() -> Native.dynRewind(h()));
    }

    @Override
    public boolean next() {
        return plain(() -> Native.dynNext(h()));
    }

    @Override
    public int component_count() {
        return plain(() -> Native.dynComponentCount(h()));
    }

    @Override
    public DynAny current_component() throws TypeMismatch {
        return tm(() -> of(Native.dynCurrentComponent(h())));
    }

    // --- members of DynStruct and DynValue ----------------------------------

    final String currentMemberName() throws TypeMismatch, InvalidValue {
        return both(() -> Native.dynCurrentMemberName(h()));
    }

    final TCKind currentMemberKind() throws TypeMismatch, InvalidValue {
        return both(() -> TCKind.from_int(Native.dynCurrentMemberKind(h())));
    }

    final NameDynAnyPair[] membersAsDynAny() throws InvalidValue {
        return iv(() -> {
            int n = Native.dynMemberCount(h());
            NameDynAnyPair[] m = new NameDynAnyPair[n];
            String[] name = new String[1];
            for (int i = 0; i < n; i++) {
                DynAnyImpl d = of(Native.dynMemberAt(h(), i, name));
                m[i] = new NameDynAnyPair(name[0], d);
            }
            return m;
        });
    }

    final NameValuePair[] members() throws InvalidValue {
        NameDynAnyPair[] m = membersAsDynAny();
        NameValuePair[] v = new NameValuePair[m.length];
        for (int i = 0; i < m.length; i++) {
            v[i] = new NameValuePair(m[i].id, m[i].value.to_any());
        }
        return v;
    }

    final void setMembers(NameValuePair[] value) throws TypeMismatch, InvalidValue {
        String[] names = new String[value.length];
        Any[] anys = new Any[value.length];
        long[] h = new long[value.length];
        for (int i = 0; i < value.length; i++) {
            names[i] = value[i].id;
            anys[i] = value[i].value;
            h[i] = any(anys[i]);
        }
        both(() -> {
            try {
                Native.dynSetMembers(h(), names, h, false);
            } finally {
                Reference.reachabilityFence(anys);
            }
        });
    }

    final void setMembersAsDynAny(NameDynAnyPair[] value) throws TypeMismatch, InvalidValue {
        String[] names = new String[value.length];
        DynAny[] dyns = new DynAny[value.length];
        long[] h = new long[value.length];
        for (int i = 0; i < value.length; i++) {
            names[i] = value[i].id;
            dyns[i] = value[i].value;
            h[i] = dyn(dyns[i]);
        }
        both(() -> {
            try {
                Native.dynSetMembers(h(), names, h, true);
            } finally {
                Reference.reachabilityFence(dyns);
            }
        });
    }

    // --- elements of DynSequence and DynArray -------------------------------

    final DynAny[] elementsAsDynAny() {
        return plain(() -> {
            int n = Native.dynElementCount(h());
            DynAny[] e = new DynAny[n];
            for (int i = 0; i < n; i++) {
                e[i] = of(Native.dynElementAt(h(), i));
            }
            return e;
        });
    }

    final Any[] elements() {
        DynAny[] d = elementsAsDynAny();
        Any[] a = new Any[d.length];
        for (int i = 0; i < d.length; i++) {
            a[i] = d[i].to_any();
        }
        return a;
    }

    final void setElements(Any[] value) throws TypeMismatch, InvalidValue {
        long[] h = new long[value.length];
        for (int i = 0; i < value.length; i++) {
            h[i] = any(value[i]);
        }
        both(() -> {
            try {
                Native.dynSetElements(h(), h, false);
            } finally {
                Reference.reachabilityFence(value);
            }
        });
    }

    final void setElementsAsDynAny(DynAny[] value) throws TypeMismatch, InvalidValue {
        long[] h = new long[value.length];
        for (int i = 0; i < value.length; i++) {
            h[i] = dyn(value[i]);
        }
        both(() -> {
            try {
                Native.dynSetElements(h(), h, true);
            } finally {
                Reference.reachabilityFence(value);
            }
        });
    }

    // --- the derived interfaces ---------------------------------------------

    static final class Fixed extends DynAnyImpl implements org.omg.DynamicAny.DynFixed {
        Fixed(long h) {
            super(h, "IDL:omg.org/DynamicAny/DynFixed:1.0");
        }

        @Override
        public String get_value() {
            return plain(() -> Native.dynGetFixedValue(h()));
        }

        @Override
        public boolean set_value(String val) throws TypeMismatch, InvalidValue {
            return both(() -> Native.dynSetFixedValue(h(), val));
        }
    }

    static final class Enum extends DynAnyImpl implements org.omg.DynamicAny.DynEnum {
        Enum(long h) {
            super(h, "IDL:omg.org/DynamicAny/DynEnum:1.0");
        }

        @Override
        public String get_as_string() {
            return plain(() -> Native.dynGetAsString(h()));
        }

        @Override
        public void set_as_string(String value) throws InvalidValue {
            iv(() -> Native.dynSetAsString(h(), value));
        }

        @Override
        public int get_as_ulong() {
            return plain(() -> Native.dynGetAsUlong(h()));
        }

        @Override
        public void set_as_ulong(int value) throws InvalidValue {
            iv(() -> Native.dynSetAsUlong(h(), value));
        }
    }

    static final class Struct extends DynAnyImpl implements org.omg.DynamicAny.DynStruct {
        Struct(long h) {
            super(h, "IDL:omg.org/DynamicAny/DynStruct:1.0");
        }

        @Override
        public String current_member_name() throws TypeMismatch, InvalidValue {
            return currentMemberName();
        }

        @Override
        public TCKind current_member_kind() throws TypeMismatch, InvalidValue {
            return currentMemberKind();
        }

        @Override
        public NameValuePair[] get_members() {
            return plain(this::members);
        }

        @Override
        public void set_members(NameValuePair[] value) throws TypeMismatch, InvalidValue {
            setMembers(value);
        }

        @Override
        public NameDynAnyPair[] get_members_as_dyn_any() {
            return plain(this::membersAsDynAny);
        }

        @Override
        public void set_members_as_dyn_any(NameDynAnyPair[] value)
                throws TypeMismatch, InvalidValue {
            setMembersAsDynAny(value);
        }
    }

    static final class Union extends DynAnyImpl implements org.omg.DynamicAny.DynUnion {
        Union(long h) {
            super(h, "IDL:omg.org/DynamicAny/DynUnion:1.0");
        }

        @Override
        public DynAny get_discriminator() {
            return plain(() -> of(Native.dynGetDiscriminator(h())));
        }

        @Override
        public void set_discriminator(DynAny d) throws TypeMismatch {
            tm(() -> {
                try {
                    Native.dynSetDiscriminator(h(), dyn(d));
                } finally {
                    Reference.reachabilityFence(d);
                }
            });
        }

        @Override
        public void set_to_default_member() throws TypeMismatch {
            tm(() -> Native.dynSetToDefaultMember(h()));
        }

        @Override
        public void set_to_no_active_member() throws TypeMismatch {
            tm(() -> Native.dynSetToNoActiveMember(h()));
        }

        @Override
        public boolean has_no_active_member() {
            return plain(() -> Native.dynHasNoActiveMember(h()));
        }

        @Override
        public TCKind discriminator_kind() {
            return plain(() -> TCKind.from_int(Native.dynDiscriminatorKind(h())));
        }

        @Override
        public TCKind member_kind() throws InvalidValue {
            return iv(() -> TCKind.from_int(Native.dynMemberKind(h())));
        }

        @Override
        public DynAny member() throws InvalidValue {
            return iv(() -> of(Native.dynMember(h())));
        }

        @Override
        public String member_name() throws InvalidValue {
            return iv(() -> Native.dynMemberName(h()));
        }
    }

    static final class Sequence extends DynAnyImpl implements org.omg.DynamicAny.DynSequence {
        Sequence(long h) {
            super(h, "IDL:omg.org/DynamicAny/DynSequence:1.0");
        }

        @Override
        public int get_length() {
            return plain(() -> Native.dynGetLength(h()));
        }

        @Override
        public void set_length(int len) throws InvalidValue {
            iv(() -> Native.dynSetLength(h(), len));
        }

        @Override
        public Any[] get_elements() {
            return elements();
        }

        @Override
        public void set_elements(Any[] value) throws TypeMismatch, InvalidValue {
            setElements(value);
        }

        @Override
        public DynAny[] get_elements_as_dyn_any() {
            return elementsAsDynAny();
        }

        @Override
        public void set_elements_as_dyn_any(DynAny[] value) throws TypeMismatch, InvalidValue {
            setElementsAsDynAny(value);
        }
    }

    static final class Array extends DynAnyImpl implements org.omg.DynamicAny.DynArray {
        Array(long h) {
            super(h, "IDL:omg.org/DynamicAny/DynArray:1.0");
        }

        @Override
        public Any[] get_elements() {
            return elements();
        }

        @Override
        public void set_elements(Any[] value) throws TypeMismatch, InvalidValue {
            setElements(value);
        }

        @Override
        public DynAny[] get_elements_as_dyn_any() {
            return elementsAsDynAny();
        }

        @Override
        public void set_elements_as_dyn_any(DynAny[] value) throws TypeMismatch, InvalidValue {
            setElementsAsDynAny(value);
        }
    }

    abstract static class ValueCommon extends DynAnyImpl
            implements org.omg.DynamicAny.DynValueCommon {
        ValueCommon(long h, String id) {
            super(h, id, "IDL:omg.org/DynamicAny/DynValueCommon:1.0");
        }

        @Override
        public boolean is_null() {
            return plain(() -> Native.dynIsNull(h()));
        }

        @Override
        public void set_to_null() {
            plain(() -> Native.dynSetToNull(h()));
        }

        @Override
        public void set_to_value() {
            plain(() -> Native.dynSetToValue(h()));
        }
    }

    static final class Value extends ValueCommon implements org.omg.DynamicAny.DynValue {
        Value(long h) {
            super(h, "IDL:omg.org/DynamicAny/DynValue:1.0");
        }

        @Override
        public String current_member_name() throws TypeMismatch, InvalidValue {
            return currentMemberName();
        }

        @Override
        public TCKind current_member_kind() throws TypeMismatch, InvalidValue {
            return currentMemberKind();
        }

        @Override
        public NameValuePair[] get_members() throws InvalidValue {
            return members();
        }

        @Override
        public void set_members(NameValuePair[] value) throws TypeMismatch, InvalidValue {
            setMembers(value);
        }

        @Override
        public NameDynAnyPair[] get_members_as_dyn_any() throws InvalidValue {
            return membersAsDynAny();
        }

        @Override
        public void set_members_as_dyn_any(NameDynAnyPair[] value)
                throws TypeMismatch, InvalidValue {
            setMembersAsDynAny(value);
        }
    }

    static final class ValueBox extends ValueCommon implements org.omg.DynamicAny.DynValueBox {
        ValueBox(long h) {
            super(h, "IDL:omg.org/DynamicAny/DynValueBox:1.0");
        }

        @Override
        public Any get_boxed_value() throws InvalidValue {
            return iv(() -> AnyImpl.of(Native.dynGetBoxedValue(h())));
        }

        @Override
        public void set_boxed_value(Any boxed) throws TypeMismatch {
            tm(() -> {
                try {
                    Native.dynSetBoxedValue(h(), any(boxed));
                } finally {
                    Reference.reachabilityFence(boxed);
                }
            });
        }

        @Override
        public DynAny get_boxed_value_as_dyn_any() throws InvalidValue {
            return iv(() -> of(Native.dynGetBoxedValueAsDynAny(h())));
        }

        @Override
        public void set_boxed_value_as_dyn_any(DynAny boxed) throws TypeMismatch {
            tm(() -> {
                try {
                    Native.dynSetBoxedValueAsDynAny(h(), dyn(boxed));
                } finally {
                    Reference.reachabilityFence(boxed);
                }
            });
        }
    }
}
