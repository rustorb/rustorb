// DynamicAny (CORBA 3.0 chapter 9) on the DynAny of the rustorb core: the
// local interfaces IDynAny, IDynFixed, IDynEnum, IDynStruct, IDynUnion,
// IDynSequence, IDynArray, IDynValueCommon, IDynValue, IDynValueBox and
// IDynAnyFactory as the IDL4 to C# mapping writes local interfaces, with
// rustorb's implementation (the classes DynAny, DynStruct...). Values cross
// the C ABI as anys. See docs/dynany.md.
//
// One class per derived interface, chosen by the kind of the type.
// Components share their state with the DynAny they belong to, so two
// objects can be the same DynAny: _is_equivalent tells.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using Rustorb.Native;

namespace DynamicAny
{
    /// <summary>DynamicAny::MustTruncate.</summary>
    public class MustTruncate : Corba.UserException
    {
        public override string _id() => "IDL:omg.org/DynamicAny/MustTruncate:1.0";
    }

    /// <summary>DynamicAny::NameValuePair.</summary>
    public class NameValuePair
    {
        public string id { get; set; } = "";
        public Corba.Any value { get; set; } = new Corba.Any();
        public NameValuePair() { }
        public NameValuePair(string id, Corba.Any value) { this.id = id; this.value = value; }
    }

    /// <summary>DynamicAny::NameDynAnyPair.</summary>
    public class NameDynAnyPair
    {
        public string id { get; set; } = "";
        public IDynAny? value { get; set; }
        public NameDynAnyPair() { }
        public NameDynAnyPair(string id, IDynAny? value) { this.id = id; this.value = value; }
    }

    /// <summary>DynamicAny::DynAny.</summary>
    public interface IDynAny : Corba.IObject
    {
        Corba.TypeCode type();
        void assign(IDynAny dyn_any);
        void from_any(Corba.Any value);
        Corba.Any to_any();
        bool equal(IDynAny dyn_any);
        void destroy();
        IDynAny copy();
        void insert_boolean(bool value);
        bool get_boolean();
        void insert_octet(byte value);
        byte get_octet();
        void insert_char(char value);
        char get_char();
        void insert_short(short value);
        short get_short();
        void insert_ushort(ushort value);
        ushort get_ushort();
        void insert_long(int value);
        int get_long();
        void insert_ulong(uint value);
        uint get_ulong();
        void insert_float(float value);
        float get_float();
        void insert_double(double value);
        double get_double();
        void insert_longlong(long value);
        long get_longlong();
        void insert_ulonglong(ulong value);
        ulong get_ulonglong();
        void insert_longdouble(decimal value);
        decimal get_longdouble();
        void insert_wchar(char value);
        char get_wchar();
        void insert_string(string value);
        string get_string();
        void insert_wstring(string value);
        string get_wstring();
        void insert_typecode(Corba.TypeCode value);
        Corba.TypeCode get_typecode();
        void insert_any(Corba.Any value);
        Corba.Any get_any();
        void insert_boolean_seq(Omg.Types.ISequence<bool> value);
        Omg.Types.ISequence<bool> get_boolean_seq();
        void insert_octet_seq(Omg.Types.ISequence<byte> value);
        Omg.Types.ISequence<byte> get_octet_seq();
        void insert_char_seq(Omg.Types.ISequence<char> value);
        Omg.Types.ISequence<char> get_char_seq();
        void insert_short_seq(Omg.Types.ISequence<short> value);
        Omg.Types.ISequence<short> get_short_seq();
        void insert_ushort_seq(Omg.Types.ISequence<ushort> value);
        Omg.Types.ISequence<ushort> get_ushort_seq();
        void insert_long_seq(Omg.Types.ISequence<int> value);
        Omg.Types.ISequence<int> get_long_seq();
        void insert_ulong_seq(Omg.Types.ISequence<uint> value);
        Omg.Types.ISequence<uint> get_ulong_seq();
        void insert_float_seq(Omg.Types.ISequence<float> value);
        Omg.Types.ISequence<float> get_float_seq();
        void insert_double_seq(Omg.Types.ISequence<double> value);
        Omg.Types.ISequence<double> get_double_seq();
        void insert_longlong_seq(Omg.Types.ISequence<long> value);
        Omg.Types.ISequence<long> get_longlong_seq();
        void insert_ulonglong_seq(Omg.Types.ISequence<ulong> value);
        Omg.Types.ISequence<ulong> get_ulonglong_seq();
        void insert_longdouble_seq(Omg.Types.ISequence<decimal> value);
        Omg.Types.ISequence<decimal> get_longdouble_seq();
        void insert_wchar_seq(Omg.Types.ISequence<char> value);
        Omg.Types.ISequence<char> get_wchar_seq();
        void insert_reference(Corba.IObject? value);
        Corba.IObject? get_reference();
        void insert_dyn_any(IDynAny value);
        IDynAny get_dyn_any();
        void insert_val(object? value);
        object? get_val();
        void insert_abstract(object? value);
        object? get_abstract();
        bool seek(int index);
        void rewind();
        bool next();
        uint component_count();
        IDynAny? current_component();
    }

    /// <summary>DynamicAny::DynFixed.</summary>
    public interface IDynFixed : IDynAny
    {
        string get_value();
        bool set_value(string val);
    }

    /// <summary>DynamicAny::DynEnum.</summary>
    public interface IDynEnum : IDynAny
    {
        string get_as_string();
        void set_as_string(string value);
        uint get_as_ulong();
        void set_as_ulong(uint value);
    }

    /// <summary>DynamicAny::DynStruct (structs and exceptions).</summary>
    public interface IDynStruct : IDynAny
    {
        string current_member_name();
        Corba.TCKind current_member_kind();
        Omg.Types.ISequence<NameValuePair> get_members();
        void set_members(Omg.Types.ISequence<NameValuePair> value);
        Omg.Types.ISequence<NameDynAnyPair> get_members_as_dyn_any();
        void set_members_as_dyn_any(Omg.Types.ISequence<NameDynAnyPair> value);
    }

    /// <summary>DynamicAny::DynUnion.</summary>
    public interface IDynUnion : IDynAny
    {
        IDynAny get_discriminator();
        void set_discriminator(IDynAny d);
        void set_to_default_member();
        void set_to_no_active_member();
        bool has_no_active_member();
        Corba.TCKind discriminator_kind();
        Corba.TCKind member_kind();
        IDynAny member();
        string member_name();
    }

    /// <summary>DynamicAny::DynSequence.</summary>
    public interface IDynSequence : IDynAny
    {
        uint get_length();
        void set_length(uint len);
        Omg.Types.ISequence<Corba.Any> get_elements();
        void set_elements(Omg.Types.ISequence<Corba.Any> value);
        Omg.Types.ISequence<IDynAny> get_elements_as_dyn_any();
        void set_elements_as_dyn_any(Omg.Types.ISequence<IDynAny> value);
    }

    /// <summary>DynamicAny::DynArray.</summary>
    public interface IDynArray : IDynAny
    {
        Omg.Types.ISequence<Corba.Any> get_elements();
        void set_elements(Omg.Types.ISequence<Corba.Any> value);
        Omg.Types.ISequence<IDynAny> get_elements_as_dyn_any();
        void set_elements_as_dyn_any(Omg.Types.ISequence<IDynAny> value);
    }

    /// <summary>DynamicAny::DynValueCommon.</summary>
    public interface IDynValueCommon : IDynAny
    {
        bool is_null();
        void set_to_null();
        void set_to_value();
    }

    /// <summary>DynamicAny::DynValue.</summary>
    public interface IDynValue : IDynValueCommon
    {
        string current_member_name();
        Corba.TCKind current_member_kind();
        Omg.Types.ISequence<NameValuePair> get_members();
        void set_members(Omg.Types.ISequence<NameValuePair> value);
        Omg.Types.ISequence<NameDynAnyPair> get_members_as_dyn_any();
        void set_members_as_dyn_any(Omg.Types.ISequence<NameDynAnyPair> value);
    }

    /// <summary>DynamicAny::DynValueBox.</summary>
    public interface IDynValueBox : IDynValueCommon
    {
        Corba.Any get_boxed_value();
        void set_boxed_value(Corba.Any boxed);
        IDynAny get_boxed_value_as_dyn_any();
        void set_boxed_value_as_dyn_any(IDynAny boxed);
    }

    /// <summary>DynamicAny::DynAnyFactory (the initial reference
    /// "DynAnyFactory").</summary>
    public interface IDynAnyFactory : Corba.IObject
    {
        IDynAny create_dyn_any(Corba.Any value);
        IDynAny create_dyn_any_from_type_code(Corba.TypeCode type);
        IDynAny create_dyn_any_without_truncation(Corba.Any value);
        Omg.Types.ISequence<IDynAny> create_multiple_dyn_anys(Omg.Types.ISequence<Corba.Any> values, bool allow_truncate);
        Omg.Types.ISequence<Corba.Any> create_multiple_anys(Omg.Types.ISequence<IDynAny> values);
    }

    /// <summary>rustorb's DynAny, and DynamicAny::DynAny's exceptions. A
    /// DynAny of a type with no derived interface (basic types, any,
    /// TypeCode, object references...) is one of this class; the others
    /// are of the derived classes.</summary>
    public unsafe class DynAny : Corba.LocalObject, IDynAny
    {
        /// <summary>DynamicAny::DynAny::InvalidValue.</summary>
        public class InvalidValue : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/DynamicAny/DynAny/InvalidValue:1.0";
        }

        /// <summary>DynamicAny::DynAny::TypeMismatch.</summary>
        public class TypeMismatch : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/DynamicAny/DynAny/TypeMismatch:1.0";
        }

        internal readonly DynAnyHandle _handle;
        private readonly string[] ids;

        internal DynAny(DynAnyHandle handle, params string[] ids)
        {
            _handle = handle;
            this.ids = ids;
        }

        protected override string[] _ids()
        {
            var all = new string[ids.Length + 2];
            ids.CopyTo(all, 0);
            all[ids.Length] = "IDL:omg.org/DynamicAny/DynAny:1.0";
            all[ids.Length + 1] = "IDL:omg.org/CORBA/LocalObject:1.0";
            return all;
        }

        /// <summary>Takes a handle: the object of the class of its kind; null for 0.</summary>
        internal static DynAny? Of(nint h)
        {
            if (h == 0)
            {
                return null;
            }
            var handle = new DynAnyHandle(h);
            switch ((Corba.TCKind)Ffi.rorb_dynany_kind(h))
            {
                case Corba.TCKind.tk_fixed: return new DynFixed(handle);
                case Corba.TCKind.tk_enum: return new DynEnum(handle);
                case Corba.TCKind.tk_struct:
                case Corba.TCKind.tk_except: return new DynStruct(handle);
                case Corba.TCKind.tk_union: return new DynUnion(handle);
                case Corba.TCKind.tk_sequence: return new DynSequence(handle);
                case Corba.TCKind.tk_array: return new DynArray(handle);
                case Corba.TCKind.tk_value:
                case Corba.TCKind.tk_event: return new DynValue(handle);
                case Corba.TCKind.tk_value_box: return new DynValueBox(handle);
                default: return new DynAny(handle);
            }
        }

        internal static DynAny NotNull(nint h) => Of(h) ?? throw new Corba.INTERNAL("a null DynAny", 0,
            Corba.CompletionStatus.COMPLETED_MAYBE);

        internal static DynAny Cast(IDynAny? d) => d as DynAny ?? throw new Corba.BAD_PARAM(
            d == null ? "a null DynAny" : "not a DynAny of rustorb", 0, Corba.CompletionStatus.COMPLETED_NO);

        internal nint H => _handle.Raw;

        public override bool _is_equivalent(Corba.IObject? other_object)
        {
            if (other_object is not DynAny o)
            {
                return false;
            }
            bool r = Ffi.rorb_dynany_same(H, o.H) != 0;
            GC.KeepAlive(this);
            GC.KeepAlive(o);
            return r;
        }

        // --- calls ------------------------------------------------------------

        internal void Do(delegate*<nint, RorbException*, int> f)
        {
            RorbException exc;
            Errors.Check(f(H, &exc), ref exc);
            GC.KeepAlive(this);
        }

        internal uint U32(delegate*<nint, uint*, RorbException*, int> f)
        {
            uint v;
            RorbException exc;
            Errors.Check(f(H, &v, &exc), ref exc);
            GC.KeepAlive(this);
            return v;
        }

        internal bool Bool(delegate*<nint, int*, RorbException*, int> f)
        {
            int v;
            RorbException exc;
            Errors.Check(f(H, &v, &exc), ref exc);
            GC.KeepAlive(this);
            return v != 0;
        }

        internal nint Ptr(delegate*<nint, nint*, RorbException*, int> f)
        {
            nint v;
            RorbException exc;
            Errors.Check(f(H, &v, &exc), ref exc);
            GC.KeepAlive(this);
            return v;
        }

        internal string Str(delegate*<nint, RorbBytes*, RorbException*, int> f)
        {
            RorbBytes b;
            RorbException exc;
            Errors.Check(f(H, &b, &exc), ref exc);
            GC.KeepAlive(this);
            return Strings.Take(b);
        }

        internal void WithOther(delegate*<nint, nint, RorbException*, int> f, nint other, object keep)
        {
            RorbException exc;
            Errors.Check(f(H, other, &exc), ref exc);
            GC.KeepAlive(this);
            GC.KeepAlive(keep);
        }

        internal static Corba.Any AnyOf(nint h) => new Corba.Any(new AnyHandle(h));

        internal static nint AnyRaw(Corba.Any? a) =>
            (a ?? throw new Corba.BAD_PARAM("a null any", 0, Corba.CompletionStatus.COMPLETED_NO))._handle.Raw;

        // --- DynAny -----------------------------------------------------------

        public Corba.TypeCode type() =>
            new Corba.TypeCode(new TypeCodeHandle(Ptr(&Ffi.rorb_dynany_type)));

        public void assign(IDynAny dyn_any)
        {
            DynAny o = Cast(dyn_any);
            WithOther(&Ffi.rorb_dynany_assign, o.H, o);
        }

        public void from_any(Corba.Any value) => WithOther(&Ffi.rorb_dynany_from_any, AnyRaw(value), value);

        public Corba.Any to_any() => AnyOf(Ptr(&Ffi.rorb_dynany_to_any));

        public bool equal(IDynAny dyn_any)
        {
            DynAny o = Cast(dyn_any);
            int r;
            RorbException exc;
            Errors.Check(Ffi.rorb_dynany_equal(H, o.H, &r, &exc), ref exc);
            GC.KeepAlive(this);
            GC.KeepAlive(o);
            return r != 0;
        }

        public void destroy() => Do(&Ffi.rorb_dynany_destroy);

        public IDynAny copy() => NotNull(Ptr(&Ffi.rorb_dynany_copy));

        /// <summary>insert_&lt;type&gt;: the value of a as a value of kind.</summary>
        private void Insert(Corba.TCKind kind, Corba.Any a)
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_dynany_insert(H, (uint)kind, a._handle.Raw, &exc), ref exc);
            GC.KeepAlive(this);
            GC.KeepAlive(a);
        }

        /// <summary>get_&lt;type&gt;: an any of kind's type.</summary>
        private Corba.Any Get(Corba.TCKind kind)
        {
            nint a;
            RorbException exc;
            Errors.Check(Ffi.rorb_dynany_get(H, (uint)kind, &a, &exc), ref exc);
            GC.KeepAlive(this);
            return AnyOf(a);
        }

        private void InsertSeq(Corba.TCKind kind, int count, Action<Rustorb.CdrOutput> elements)
        {
            var a = new Corba.Any();
            Corba.TypeCode tc = Rustorb.TypeCodes.Sequence(Rustorb.TypeCodes.Simple(kind), 0);
            Rustorb.Anys.Insert(a, tc, o =>
            {
                o.write_ulong((uint)count);
                elements(o);
            });
            RorbException exc;
            Errors.Check(Ffi.rorb_dynany_insert_seq(H, (uint)kind, a._handle.Raw, &exc), ref exc);
            GC.KeepAlive(this);
            GC.KeepAlive(a);
        }

        private delegate void SpanReader<T>(Rustorb.CdrInput i, Span<T> s);

        private Omg.Types.ISequence<T> GetSeq<T>(Corba.TCKind kind, int size, SpanReader<T> elements)
        {
            nint h;
            RorbException exc;
            Errors.Check(Ffi.rorb_dynany_get_seq(H, (uint)kind, &h, &exc), ref exc);
            GC.KeepAlive(this);
            Corba.Any a = AnyOf(h);
            return Rustorb.Anys.ExtractKind(a, Corba.TCKind.tk_sequence, i =>
            {
                int n = i.read_length(size);
                var s = Rustorb.Seq.Sized<T>(n);
                elements(i, Rustorb.Seq.Writable(s));
                return (Omg.Types.ISequence<T>)s;
            });
        }

        public void insert_boolean(bool value)
        {
            var a = new Corba.Any();
            a.insert_boolean(value);
            Insert(Corba.TCKind.tk_boolean, a);
        }

        public bool get_boolean() => Get(Corba.TCKind.tk_boolean).extract_boolean();

        public void insert_octet(byte value)
        {
            var a = new Corba.Any();
            a.insert_octet(value);
            Insert(Corba.TCKind.tk_octet, a);
        }

        public byte get_octet() => Get(Corba.TCKind.tk_octet).extract_octet();

        public void insert_char(char value)
        {
            var a = new Corba.Any();
            a.insert_char(value);
            Insert(Corba.TCKind.tk_char, a);
        }

        public char get_char() => Get(Corba.TCKind.tk_char).extract_char();

        public void insert_short(short value)
        {
            var a = new Corba.Any();
            a.insert_short(value);
            Insert(Corba.TCKind.tk_short, a);
        }

        public short get_short() => Get(Corba.TCKind.tk_short).extract_short();

        public void insert_ushort(ushort value)
        {
            var a = new Corba.Any();
            a.insert_ushort(value);
            Insert(Corba.TCKind.tk_ushort, a);
        }

        public ushort get_ushort() => Get(Corba.TCKind.tk_ushort).extract_ushort();

        public void insert_long(int value)
        {
            var a = new Corba.Any();
            a.insert_long(value);
            Insert(Corba.TCKind.tk_long, a);
        }

        public int get_long() => Get(Corba.TCKind.tk_long).extract_long();

        public void insert_ulong(uint value)
        {
            var a = new Corba.Any();
            a.insert_ulong(value);
            Insert(Corba.TCKind.tk_ulong, a);
        }

        public uint get_ulong() => Get(Corba.TCKind.tk_ulong).extract_ulong();

        public void insert_float(float value)
        {
            var a = new Corba.Any();
            a.insert_float(value);
            Insert(Corba.TCKind.tk_float, a);
        }

        public float get_float() => Get(Corba.TCKind.tk_float).extract_float();

        public void insert_double(double value)
        {
            var a = new Corba.Any();
            a.insert_double(value);
            Insert(Corba.TCKind.tk_double, a);
        }

        public double get_double() => Get(Corba.TCKind.tk_double).extract_double();

        public void insert_longlong(long value)
        {
            var a = new Corba.Any();
            a.insert_longlong(value);
            Insert(Corba.TCKind.tk_longlong, a);
        }

        public long get_longlong() => Get(Corba.TCKind.tk_longlong).extract_longlong();

        public void insert_ulonglong(ulong value)
        {
            var a = new Corba.Any();
            a.insert_ulonglong(value);
            Insert(Corba.TCKind.tk_ulonglong, a);
        }

        public ulong get_ulonglong() => Get(Corba.TCKind.tk_ulonglong).extract_ulonglong();

        public void insert_longdouble(decimal value)
        {
            var a = new Corba.Any();
            a.insert_longdouble(value);
            Insert(Corba.TCKind.tk_longdouble, a);
        }

        public decimal get_longdouble() => Get(Corba.TCKind.tk_longdouble).extract_longdouble();

        public void insert_wchar(char value)
        {
            var a = new Corba.Any();
            a.insert_wchar(value);
            Insert(Corba.TCKind.tk_wchar, a);
        }

        public char get_wchar() => Get(Corba.TCKind.tk_wchar).extract_wchar();

        public void insert_string(string value)
        {
            var a = new Corba.Any();
            a.insert_string(value);
            Insert(Corba.TCKind.tk_string, a);
        }

        public string get_string() => Get(Corba.TCKind.tk_string).extract_string();

        public void insert_wstring(string value)
        {
            var a = new Corba.Any();
            a.insert_wstring(value);
            Insert(Corba.TCKind.tk_wstring, a);
        }

        public string get_wstring() => Get(Corba.TCKind.tk_wstring).extract_wstring();

        public void insert_typecode(Corba.TypeCode value)
        {
            var a = new Corba.Any();
            a.insert_TypeCode(value);
            Insert(Corba.TCKind.tk_TypeCode, a);
        }

        public Corba.TypeCode get_typecode() => Get(Corba.TCKind.tk_TypeCode).extract_TypeCode();

        public void insert_any(Corba.Any value)
        {
            var a = new Corba.Any();
            a.insert_any(value);
            Insert(Corba.TCKind.tk_any, a);
        }

        public Corba.Any get_any() => Get(Corba.TCKind.tk_any).extract_any();

        public void insert_boolean_seq(Omg.Types.ISequence<bool> value) =>
            InsertSeq(Corba.TCKind.tk_boolean, value.Count, o => o.write_boolean_array(Rustorb.Seq.Span(value)));

        public Omg.Types.ISequence<bool> get_boolean_seq() =>
            GetSeq<bool>(Corba.TCKind.tk_boolean, 1, (i, s) => i.read_boolean_array(s));

        public void insert_octet_seq(Omg.Types.ISequence<byte> value) =>
            InsertSeq(Corba.TCKind.tk_octet, value.Count, o => o.write_octet_array(Rustorb.Seq.Span(value)));

        public Omg.Types.ISequence<byte> get_octet_seq() =>
            GetSeq<byte>(Corba.TCKind.tk_octet, 1, (i, s) => i.read_octet_array(s));

        public void insert_char_seq(Omg.Types.ISequence<char> value) =>
            InsertSeq(Corba.TCKind.tk_char, value.Count, o => o.write_char_array(Rustorb.Seq.Span(value)));

        public Omg.Types.ISequence<char> get_char_seq() =>
            GetSeq<char>(Corba.TCKind.tk_char, 1, (i, s) => i.read_char_array(s));

        public void insert_short_seq(Omg.Types.ISequence<short> value) =>
            InsertSeq(Corba.TCKind.tk_short, value.Count, o => o.write_short_array(Rustorb.Seq.Span(value)));

        public Omg.Types.ISequence<short> get_short_seq() =>
            GetSeq<short>(Corba.TCKind.tk_short, 2, (i, s) => i.read_short_array(s));

        public void insert_ushort_seq(Omg.Types.ISequence<ushort> value) =>
            InsertSeq(Corba.TCKind.tk_ushort, value.Count, o => o.write_ushort_array(Rustorb.Seq.Span(value)));

        public Omg.Types.ISequence<ushort> get_ushort_seq() =>
            GetSeq<ushort>(Corba.TCKind.tk_ushort, 2, (i, s) => i.read_ushort_array(s));

        public void insert_long_seq(Omg.Types.ISequence<int> value) =>
            InsertSeq(Corba.TCKind.tk_long, value.Count, o => o.write_long_array(Rustorb.Seq.Span(value)));

        public Omg.Types.ISequence<int> get_long_seq() =>
            GetSeq<int>(Corba.TCKind.tk_long, 4, (i, s) => i.read_long_array(s));

        public void insert_ulong_seq(Omg.Types.ISequence<uint> value) =>
            InsertSeq(Corba.TCKind.tk_ulong, value.Count, o => o.write_ulong_array(Rustorb.Seq.Span(value)));

        public Omg.Types.ISequence<uint> get_ulong_seq() =>
            GetSeq<uint>(Corba.TCKind.tk_ulong, 4, (i, s) => i.read_ulong_array(s));

        public void insert_float_seq(Omg.Types.ISequence<float> value) =>
            InsertSeq(Corba.TCKind.tk_float, value.Count, o => o.write_float_array(Rustorb.Seq.Span(value)));

        public Omg.Types.ISequence<float> get_float_seq() =>
            GetSeq<float>(Corba.TCKind.tk_float, 4, (i, s) => i.read_float_array(s));

        public void insert_double_seq(Omg.Types.ISequence<double> value) =>
            InsertSeq(Corba.TCKind.tk_double, value.Count, o => o.write_double_array(Rustorb.Seq.Span(value)));

        public Omg.Types.ISequence<double> get_double_seq() =>
            GetSeq<double>(Corba.TCKind.tk_double, 8, (i, s) => i.read_double_array(s));

        public void insert_longlong_seq(Omg.Types.ISequence<long> value) =>
            InsertSeq(Corba.TCKind.tk_longlong, value.Count, o => o.write_longlong_array(Rustorb.Seq.Span(value)));

        public Omg.Types.ISequence<long> get_longlong_seq() =>
            GetSeq<long>(Corba.TCKind.tk_longlong, 8, (i, s) => i.read_longlong_array(s));

        public void insert_ulonglong_seq(Omg.Types.ISequence<ulong> value) =>
            InsertSeq(Corba.TCKind.tk_ulonglong, value.Count, o => o.write_ulonglong_array(Rustorb.Seq.Span(value)));

        public Omg.Types.ISequence<ulong> get_ulonglong_seq() =>
            GetSeq<ulong>(Corba.TCKind.tk_ulonglong, 8, (i, s) => i.read_ulonglong_array(s));

        public void insert_longdouble_seq(Omg.Types.ISequence<decimal> value) =>
            InsertSeq(Corba.TCKind.tk_longdouble, value.Count, o => { foreach (decimal x in value) o.write_longdouble(x); });

        public Omg.Types.ISequence<decimal> get_longdouble_seq() =>
            GetSeq<decimal>(Corba.TCKind.tk_longdouble, 8, (i, s) => { for (int k = 0; k < s.Length; k++) s[k] = i.read_longdouble(); });

        public void insert_wchar_seq(Omg.Types.ISequence<char> value) =>
            InsertSeq(Corba.TCKind.tk_wchar, value.Count, o => o.write_wchar_array(Rustorb.Seq.Span(value)));

        public Omg.Types.ISequence<char> get_wchar_seq() =>
            GetSeq<char>(Corba.TCKind.tk_wchar, 1, (i, s) => i.read_wchar_array(s));

        public void insert_reference(Corba.IObject? value)
        {
            var a = new Corba.Any();
            a.insert_object(value, Rustorb.TypeCodes.Interface(Corba.TCKind.tk_objref, "IDL:omg.org/CORBA/Object:1.0", "Object"));
            Insert(Corba.TCKind.tk_objref, a);
        }

        public Corba.IObject? get_reference() => Get(Corba.TCKind.tk_objref).extract_object();

        public void insert_dyn_any(IDynAny value)
        {
            DynAny o = Cast(value);
            WithOther(&Ffi.rorb_dynany_insert_dyn_any, o.H, o);
        }

        public IDynAny get_dyn_any() => NotNull(Ptr(&Ffi.rorb_dynany_get_dyn_any));

        /// <summary>Valuetypes are not in the .NET binding yet: NO_IMPLEMENT
        /// (from_any and to_any take values).</summary>
        public void insert_val(object? value) =>
            throw new Corba.NO_IMPLEMENT("insert_val: valuetypes are not in the .NET binding yet", 0,
                Corba.CompletionStatus.COMPLETED_NO);

        public object? get_val() =>
            throw new Corba.NO_IMPLEMENT("get_val: valuetypes are not in the .NET binding yet", 0,
                Corba.CompletionStatus.COMPLETED_NO);

        /// <summary>Abstract interfaces are not supported: NO_IMPLEMENT.</summary>
        public void insert_abstract(object? value) => Do(&Ffi.rorb_dynany_no_implement);

        public object? get_abstract()
        {
            Do(&Ffi.rorb_dynany_no_implement);
            return null;
        }

        public bool seek(int index)
        {
            int r;
            RorbException exc;
            Errors.Check(Ffi.rorb_dynany_seek(H, index, &r, &exc), ref exc);
            GC.KeepAlive(this);
            return r != 0;
        }

        public void rewind() => Do(&Ffi.rorb_dynany_rewind);

        public bool next() => Bool(&Ffi.rorb_dynany_next);

        public uint component_count() => U32(&Ffi.rorb_dynany_component_count);

        public IDynAny? current_component() => Of(Ptr(&Ffi.rorb_dynany_current_component));

        // --- members of DynStruct and DynValue ----------------------------------

        internal string CurrentMemberName() => Str(&Ffi.rorb_dynany_current_member_name);

        internal Corba.TCKind CurrentMemberKind() => (Corba.TCKind)U32(&Ffi.rorb_dynany_current_member_kind);

        internal Omg.Types.ISequence<NameDynAnyPair> MembersAsDynAny()
        {
            uint n = U32(&Ffi.rorb_dynany_member_count);
            var m = new Omg.Types.Sequence<NameDynAnyPair>((int)n);
            for (uint i = 0; i < n; i++)
            {
                RorbBytes name;
                nint d;
                RorbException exc;
                Errors.Check(Ffi.rorb_dynany_member_at(H, i, &name, &d, &exc), ref exc);
                m.Add(new NameDynAnyPair(Strings.Take(name), Of(d)));
            }
            GC.KeepAlive(this);
            return m;
        }

        internal Omg.Types.ISequence<NameValuePair> Members()
        {
            var m = MembersAsDynAny();
            var v = new Omg.Types.Sequence<NameValuePair>(m.Count);
            foreach (NameDynAnyPair p in m)
            {
                v.Add(new NameValuePair(p.id, p.value!.to_any()));
            }
            return v;
        }

        private void SetMembers(string[] names, nint[] values, bool dyn, object keep)
        {
            var bytes = new byte[names.Length][];
            var lens = new nuint[names.Length];
            var pins = new System.Runtime.InteropServices.GCHandle[names.Length];
            var ptrs = new byte*[names.Length];
            try
            {
                for (int i = 0; i < names.Length; i++)
                {
                    bytes[i] = Strings.ToUtf8(names[i] ?? "");
                    lens[i] = (nuint)bytes[i].Length;
                    pins[i] = System.Runtime.InteropServices.GCHandle.Alloc(bytes[i],
                        System.Runtime.InteropServices.GCHandleType.Pinned);
                    ptrs[i] = (byte*)pins[i].AddrOfPinnedObject();
                }
                RorbException exc;
                int st;
                fixed (byte** np = ptrs)
                fixed (nuint* lp = lens)
                fixed (nint* vp = values)
                {
                    st = dyn
                        ? Ffi.rorb_dynany_set_members_as_dyn_any(H, (nuint)names.Length, np, lp, vp, &exc)
                        : Ffi.rorb_dynany_set_members(H, (nuint)names.Length, np, lp, vp, &exc);
                }
                Errors.Check(st, ref exc);
            }
            finally
            {
                foreach (var p in pins)
                {
                    if (p.IsAllocated)
                    {
                        p.Free();
                    }
                }
                GC.KeepAlive(this);
                GC.KeepAlive(keep);
            }
        }

        internal void SetMembers(Omg.Types.ISequence<NameValuePair> value)
        {
            var names = new string[value.Count];
            var raw = new nint[value.Count];
            for (int i = 0; i < value.Count; i++)
            {
                names[i] = value[i].id;
                raw[i] = AnyRaw(value[i].value);
            }
            SetMembers(names, raw, false, value);
        }

        internal void SetMembersAsDynAny(Omg.Types.ISequence<NameDynAnyPair> value)
        {
            var names = new string[value.Count];
            var raw = new nint[value.Count];
            for (int i = 0; i < value.Count; i++)
            {
                names[i] = value[i].id;
                raw[i] = Cast(value[i].value).H;
            }
            SetMembers(names, raw, true, value);
        }

        // --- elements of DynSequence and DynArray -------------------------------

        internal Omg.Types.ISequence<IDynAny> ElementsAsDynAny()
        {
            uint n = U32(&Ffi.rorb_dynany_element_count);
            var e = new Omg.Types.Sequence<IDynAny>((int)n);
            for (uint i = 0; i < n; i++)
            {
                nint d;
                RorbException exc;
                Errors.Check(Ffi.rorb_dynany_element_at(H, i, &d, &exc), ref exc);
                e.Add(NotNull(d));
            }
            GC.KeepAlive(this);
            return e;
        }

        internal Omg.Types.ISequence<Corba.Any> Elements()
        {
            var d = ElementsAsDynAny();
            var a = new Omg.Types.Sequence<Corba.Any>(d.Count);
            foreach (IDynAny x in d)
            {
                a.Add(x.to_any());
            }
            return a;
        }

        private void SetElements(nint[] raw, bool dyn, object keep)
        {
            RorbException exc;
            int st;
            fixed (nint* vp = raw)
            {
                st = dyn
                    ? Ffi.rorb_dynany_set_elements_as_dyn_any(H, (nuint)raw.Length, vp, &exc)
                    : Ffi.rorb_dynany_set_elements(H, (nuint)raw.Length, vp, &exc);
            }
            GC.KeepAlive(this);
            GC.KeepAlive(keep);
            Errors.Check(st, ref exc);
        }

        internal void SetElements(Omg.Types.ISequence<Corba.Any> value) =>
            SetElements(System.Linq.Enumerable.ToArray(System.Linq.Enumerable.Select(value, AnyRaw)), false, value);

        internal void SetElementsAsDynAny(Omg.Types.ISequence<IDynAny> value) =>
            SetElements(System.Linq.Enumerable.ToArray(System.Linq.Enumerable.Select(value, d => Cast(d).H)), true, value);
    }

    public sealed unsafe class DynFixed : DynAny, IDynFixed
    {
        internal DynFixed(DynAnyHandle h) : base(h, "IDL:omg.org/DynamicAny/DynFixed:1.0") { }

        public string get_value() => Str(&Ffi.rorb_dynany_get_fixed_value);

        public bool set_value(string val)
        {
            byte[] b = Strings.ToUtf8(val);
            int r;
            RorbException exc;
            fixed (byte* p = b)
            {
                Errors.Check(Ffi.rorb_dynany_set_fixed_value(H, p, (nuint)b.Length, &r, &exc), ref exc);
            }
            GC.KeepAlive(this);
            return r != 0;
        }
    }

    public sealed unsafe class DynEnum : DynAny, IDynEnum
    {
        internal DynEnum(DynAnyHandle h) : base(h, "IDL:omg.org/DynamicAny/DynEnum:1.0") { }

        public string get_as_string() => Str(&Ffi.rorb_dynany_get_as_string);

        public void set_as_string(string value)
        {
            byte[] b = Strings.ToUtf8(value);
            RorbException exc;
            fixed (byte* p = b)
            {
                Errors.Check(Ffi.rorb_dynany_set_as_string(H, p, (nuint)b.Length, &exc), ref exc);
            }
            GC.KeepAlive(this);
        }

        public uint get_as_ulong() => U32(&Ffi.rorb_dynany_get_as_ulong);

        public void set_as_ulong(uint value)
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_dynany_set_as_ulong(H, value, &exc), ref exc);
            GC.KeepAlive(this);
        }
    }

    public sealed class DynStruct : DynAny, IDynStruct
    {
        internal DynStruct(DynAnyHandle h) : base(h, "IDL:omg.org/DynamicAny/DynStruct:1.0") { }

        public string current_member_name() => CurrentMemberName();
        public Corba.TCKind current_member_kind() => CurrentMemberKind();
        public Omg.Types.ISequence<NameValuePair> get_members() => Members();
        public void set_members(Omg.Types.ISequence<NameValuePair> value) => SetMembers(value);
        public Omg.Types.ISequence<NameDynAnyPair> get_members_as_dyn_any() => MembersAsDynAny();
        public void set_members_as_dyn_any(Omg.Types.ISequence<NameDynAnyPair> value) => SetMembersAsDynAny(value);
    }

    public sealed unsafe class DynUnion : DynAny, IDynUnion
    {
        internal DynUnion(DynAnyHandle h) : base(h, "IDL:omg.org/DynamicAny/DynUnion:1.0") { }

        public IDynAny get_discriminator() => NotNull(Ptr(&Ffi.rorb_dynany_get_discriminator));

        public void set_discriminator(IDynAny d)
        {
            DynAny o = Cast(d);
            WithOther(&Ffi.rorb_dynany_set_discriminator, o.H, o);
        }

        public void set_to_default_member() => Do(&Ffi.rorb_dynany_set_to_default_member);
        public void set_to_no_active_member() => Do(&Ffi.rorb_dynany_set_to_no_active_member);
        public bool has_no_active_member() => Bool(&Ffi.rorb_dynany_has_no_active_member);
        public Corba.TCKind discriminator_kind() => (Corba.TCKind)U32(&Ffi.rorb_dynany_discriminator_kind);
        public Corba.TCKind member_kind() => (Corba.TCKind)U32(&Ffi.rorb_dynany_member_kind);
        public IDynAny member() => NotNull(Ptr(&Ffi.rorb_dynany_member));
        public string member_name() => Str(&Ffi.rorb_dynany_member_name);
    }

    public sealed unsafe class DynSequence : DynAny, IDynSequence
    {
        internal DynSequence(DynAnyHandle h) : base(h, "IDL:omg.org/DynamicAny/DynSequence:1.0") { }

        public uint get_length() => U32(&Ffi.rorb_dynany_get_length);

        public void set_length(uint len)
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_dynany_set_length(H, len, &exc), ref exc);
            GC.KeepAlive(this);
        }

        public Omg.Types.ISequence<Corba.Any> get_elements() => Elements();
        public void set_elements(Omg.Types.ISequence<Corba.Any> value) => SetElements(value);
        public Omg.Types.ISequence<IDynAny> get_elements_as_dyn_any() => ElementsAsDynAny();
        public void set_elements_as_dyn_any(Omg.Types.ISequence<IDynAny> value) => SetElementsAsDynAny(value);
    }

    public sealed class DynArray : DynAny, IDynArray
    {
        internal DynArray(DynAnyHandle h) : base(h, "IDL:omg.org/DynamicAny/DynArray:1.0") { }

        public Omg.Types.ISequence<Corba.Any> get_elements() => Elements();
        public void set_elements(Omg.Types.ISequence<Corba.Any> value) => SetElements(value);
        public Omg.Types.ISequence<IDynAny> get_elements_as_dyn_any() => ElementsAsDynAny();
        public void set_elements_as_dyn_any(Omg.Types.ISequence<IDynAny> value) => SetElementsAsDynAny(value);
    }

    public abstract unsafe class DynValueCommon : DynAny, IDynValueCommon
    {
        internal DynValueCommon(DynAnyHandle h, string id) : base(h, id, "IDL:omg.org/DynamicAny/DynValueCommon:1.0") { }

        public bool is_null() => Bool(&Ffi.rorb_dynany_is_null);
        public void set_to_null() => Do(&Ffi.rorb_dynany_set_to_null);
        public void set_to_value() => Do(&Ffi.rorb_dynany_set_to_value);
    }

    public sealed class DynValue : DynValueCommon, IDynValue
    {
        internal DynValue(DynAnyHandle h) : base(h, "IDL:omg.org/DynamicAny/DynValue:1.0") { }

        public string current_member_name() => CurrentMemberName();
        public Corba.TCKind current_member_kind() => CurrentMemberKind();
        public Omg.Types.ISequence<NameValuePair> get_members() => Members();
        public void set_members(Omg.Types.ISequence<NameValuePair> value) => SetMembers(value);
        public Omg.Types.ISequence<NameDynAnyPair> get_members_as_dyn_any() => MembersAsDynAny();
        public void set_members_as_dyn_any(Omg.Types.ISequence<NameDynAnyPair> value) => SetMembersAsDynAny(value);
    }

    public sealed unsafe class DynValueBox : DynValueCommon, IDynValueBox
    {
        internal DynValueBox(DynAnyHandle h) : base(h, "IDL:omg.org/DynamicAny/DynValueBox:1.0") { }

        public Corba.Any get_boxed_value() => AnyOf(Ptr(&Ffi.rorb_dynany_get_boxed_value));
        public void set_boxed_value(Corba.Any boxed) => WithOther(&Ffi.rorb_dynany_set_boxed_value, AnyRaw(boxed), boxed);
        public IDynAny get_boxed_value_as_dyn_any() => NotNull(Ptr(&Ffi.rorb_dynany_get_boxed_value_as_dyn_any));

        public void set_boxed_value_as_dyn_any(IDynAny boxed)
        {
            DynAny o = Cast(boxed);
            WithOther(&Ffi.rorb_dynany_set_boxed_value_as_dyn_any, o.H, o);
        }
    }

    /// <summary>rustorb's DynAnyFactory (ORB.resolve_initial_references
    /// ("DynAnyFactory")), and DynamicAny::DynAnyFactory's exception.</summary>
    public sealed unsafe class DynAnyFactory : Corba.LocalObject, IDynAnyFactory
    {
        /// <summary>DynamicAny::DynAnyFactory::InconsistentTypeCode.</summary>
        public class InconsistentTypeCode : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/DynamicAny/DynAnyFactory/InconsistentTypeCode:1.0";
        }

        internal static readonly DynAnyFactory Instance = new DynAnyFactory();

        private DynAnyFactory() { }

        protected override string[] _ids() =>
            new[] { "IDL:omg.org/DynamicAny/DynAnyFactory:1.0", "IDL:omg.org/CORBA/LocalObject:1.0" };

        private static IDynAny Create(Corba.Any value, bool truncate)
        {
            nint d;
            RorbException exc;
            Errors.Check(Ffi.rorb_dynany_create(DynAny.AnyRaw(value), truncate ? 1 : 0, &d, &exc), ref exc);
            GC.KeepAlive(value);
            return DynAny.NotNull(d);
        }

        public IDynAny create_dyn_any(Corba.Any value) => Create(value, true);

        public IDynAny create_dyn_any_without_truncation(Corba.Any value) => Create(value, false);

        public IDynAny create_dyn_any_from_type_code(Corba.TypeCode type)
        {
            if (type == null)
            {
                throw new Corba.BAD_PARAM("a null TypeCode", 0, Corba.CompletionStatus.COMPLETED_NO);
            }
            nint d;
            RorbException exc;
            Errors.Check(Ffi.rorb_dynany_from_typecode(type._handle.Raw, &d, &exc), ref exc);
            GC.KeepAlive(type);
            return DynAny.NotNull(d);
        }

        public Omg.Types.ISequence<IDynAny> create_multiple_dyn_anys(Omg.Types.ISequence<Corba.Any> values,
            bool allow_truncate)
        {
            var r = new Omg.Types.Sequence<IDynAny>(values.Count);
            foreach (Corba.Any a in values)
            {
                r.Add(Create(a, allow_truncate));
            }
            return r;
        }

        public Omg.Types.ISequence<Corba.Any> create_multiple_anys(Omg.Types.ISequence<IDynAny> values)
        {
            var r = new Omg.Types.Sequence<Corba.Any>(values.Count);
            foreach (IDynAny d in values)
            {
                r.Add(d.to_any());
            }
            return r;
        }
    }

    /// <summary>narrow for the local DynamicAny interfaces: the object itself
    /// when it implements the interface, else BAD_PARAM.</summary>
    internal static class Narrowing
    {
        internal static T? Narrow<T>(Corba.IObject? obj) where T : class
        {
            if (obj == null)
            {
                return null;
            }
            return obj as T ?? throw new Corba.BAD_PARAM($"the object is not a {typeof(T).Name}", 0,
                Corba.CompletionStatus.COMPLETED_NO);
        }
    }

    public static class DynAnyHelper { public static IDynAny? narrow(Corba.IObject? obj) => Narrowing.Narrow<IDynAny>(obj); }
    public static class DynFixedHelper { public static IDynFixed? narrow(Corba.IObject? obj) => Narrowing.Narrow<IDynFixed>(obj); }
    public static class DynEnumHelper { public static IDynEnum? narrow(Corba.IObject? obj) => Narrowing.Narrow<IDynEnum>(obj); }
    public static class DynStructHelper { public static IDynStruct? narrow(Corba.IObject? obj) => Narrowing.Narrow<IDynStruct>(obj); }
    public static class DynUnionHelper { public static IDynUnion? narrow(Corba.IObject? obj) => Narrowing.Narrow<IDynUnion>(obj); }
    public static class DynSequenceHelper { public static IDynSequence? narrow(Corba.IObject? obj) => Narrowing.Narrow<IDynSequence>(obj); }
    public static class DynArrayHelper { public static IDynArray? narrow(Corba.IObject? obj) => Narrowing.Narrow<IDynArray>(obj); }
    public static class DynValueCommonHelper { public static IDynValueCommon? narrow(Corba.IObject? obj) => Narrowing.Narrow<IDynValueCommon>(obj); }
    public static class DynValueHelper { public static IDynValue? narrow(Corba.IObject? obj) => Narrowing.Narrow<IDynValue>(obj); }
    public static class DynValueBoxHelper { public static IDynValueBox? narrow(Corba.IObject? obj) => Narrowing.Narrow<IDynValueBox>(obj); }
    public static class DynAnyFactoryHelper { public static IDynAnyFactory? narrow(Corba.IObject? obj) => Narrowing.Narrow<IDynAnyFactory>(obj); }
}
