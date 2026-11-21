// Corba.Any (IDL4 to C# A.1.4) on rorb_any, and the insertion and
// extraction the generated Helpers use (Rustorb.Anys).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using Rustorb.Native;

namespace Corba
{
    /// <summary>CORBA::Any: a TypeCode and a value of that type. The insert
    /// and extract pairs of A.1.4, with those the mapping leaves out (string,
    /// wstring, TypeCode, fixed, long double) added; values of user types go
    /// through the generated Helpers.</summary>
    public sealed class Any : IEquatable<Any>
    {
        internal AnyHandle _handle;

        /// <summary>An empty any (tk_null).</summary>
        public Any()
        {
            _handle = new AnyHandle(Ffi.rorb_any_new());
        }

        internal Any(AnyHandle handle)
        {
            _handle = handle;
        }

        /// <summary>The TypeCode of the value. Setting an equivalent TypeCode (an
        /// alias) keeps the value; another one leaves the any without a value
        /// (rustorb).</summary>
        public TypeCode type
        {
            get
            {
                var t = new TypeCode(new TypeCodeHandle(Ffi.rorb_any_type(_handle.Raw)));
                GC.KeepAlive(this);
                return t;
            }
            set => SetType(value);
        }

        // Setting the type (rustorb, like Any.type(TypeCode) of the Java
        // mapping): an equivalent TypeCode (an alias) keeps the value; another
        // one leaves the any without a value of that type.
        private unsafe void SetType(TypeCode tc)
        {
            RorbException exc;
            int st = Ffi.rorb_any_set_type(_handle.Raw, tc._handle.Raw, &exc);
            if (st != Status.OK)
            {
                Ffi.rorb_exception_clear(&exc);
                _handle = new AnyHandle(Ffi.rorb_any_typed(tc._handle.Raw));
            }
            GC.KeepAlive(tc);
        }

        /// <summary>A copy of the any.</summary>
        public Any copy()
        {
            var a = new Any(new AnyHandle(Ffi.rorb_any_clone(_handle.Raw)));
            GC.KeepAlive(this);
            return a;
        }

        private void Put(TCKind kind, Action<Rustorb.CdrOutput> w) =>
            Rustorb.Anys.Insert(this, Rustorb.TypeCodes.Simple(kind), w);

        private T Get<T>(TCKind kind, Func<Rustorb.CdrInput, T> r) =>
            Rustorb.Anys.Extract(this, Rustorb.TypeCodes.Simple(kind), r);

        public void insert_short(short value) => Put(TCKind.tk_short, o => o.write_short(value));
        public short extract_short() => Get(TCKind.tk_short, i => i.read_short());
        public void insert_long(int value) => Put(TCKind.tk_long, o => o.write_long(value));
        public int extract_long() => Get(TCKind.tk_long, i => i.read_long());
        public void insert_longlong(long value) => Put(TCKind.tk_longlong, o => o.write_longlong(value));
        public long extract_longlong() => Get(TCKind.tk_longlong, i => i.read_longlong());
        public void insert_ushort(ushort value) => Put(TCKind.tk_ushort, o => o.write_ushort(value));
        public ushort extract_ushort() => Get(TCKind.tk_ushort, i => i.read_ushort());
        public void insert_ulong(uint value) => Put(TCKind.tk_ulong, o => o.write_ulong(value));
        public uint extract_ulong() => Get(TCKind.tk_ulong, i => i.read_ulong());
        public void insert_ulonglong(ulong value) => Put(TCKind.tk_ulonglong, o => o.write_ulonglong(value));
        public ulong extract_ulonglong() => Get(TCKind.tk_ulonglong, i => i.read_ulonglong());
        public void insert_float(float value) => Put(TCKind.tk_float, o => o.write_float(value));
        public float extract_float() => Get(TCKind.tk_float, i => i.read_float());
        public void insert_double(double value) => Put(TCKind.tk_double, o => o.write_double(value));
        public double extract_double() => Get(TCKind.tk_double, i => i.read_double());
        public void insert_boolean(bool value) => Put(TCKind.tk_boolean, o => o.write_boolean(value));
        public bool extract_boolean() => Get(TCKind.tk_boolean, i => i.read_boolean());
        public void insert_char(char value) => Put(TCKind.tk_char, o => o.write_char(value));
        public char extract_char() => Get(TCKind.tk_char, i => i.read_char());
        public void insert_wchar(char value) => Put(TCKind.tk_wchar, o => o.write_wchar(value));
        public char extract_wchar() => Get(TCKind.tk_wchar, i => i.read_wchar());
        public void insert_octet(byte value) => Put(TCKind.tk_octet, o => o.write_octet(value));
        public byte extract_octet() => Get(TCKind.tk_octet, i => i.read_octet());
        public void insert_longdouble(decimal value) => Put(TCKind.tk_longdouble, o => o.write_longdouble(value));
        public decimal extract_longdouble() => Get(TCKind.tk_longdouble, i => i.read_longdouble());

        public void insert_any(Any value)
        {
            if (value == null)
            {
                throw new BAD_PARAM("a null any", 0, CompletionStatus.COMPLETED_NO);
            }
            Put(TCKind.tk_any, o => o.write_any(value));
        }

        public Any extract_any() => Get(TCKind.tk_any, i => i.read_any());

        public void insert_TypeCode(TypeCode value) => Put(TCKind.tk_TypeCode, o => o.write_TypeCode(value));
        public TypeCode extract_TypeCode() => Get(TCKind.tk_TypeCode, i => i.read_TypeCode());

        public void insert_string(string value) => Put(TCKind.tk_string, o => o.write_string(value));

        public string extract_string() =>
            Rustorb.Anys.ExtractKind(this, TCKind.tk_string, i => i.read_string());

        public void insert_wstring(string value) => Put(TCKind.tk_wstring, o => o.write_wstring(value));

        public string extract_wstring() =>
            Rustorb.Anys.ExtractKind(this, TCKind.tk_wstring, i => i.read_wstring());

        /// <summary>An object reference with the TypeCode of CORBA::Object.</summary>
        public void insert_object(Object? value) =>
            insert_object(value, Rustorb.TypeCodes.Interface(TCKind.tk_objref, "IDL:omg.org/CORBA/Object:1.0", "Object"));

        /// <summary>An object reference with the TypeCode tc (of an interface).</summary>
        public void insert_object(IObject? value, TypeCode tc) => Rustorb.Anys.Insert(this, tc, o => o.write_Object(value));

        public Object? extract_object() =>
            Rustorb.Anys.ExtractKind(this, TCKind.tk_objref, i => i.read_Object());

        /// <summary>A value with the TypeCode tc (of a valuetype or a value
        /// box; insert_Value of the Java mapping).</summary>
        public void insert_Value(IValueBase? value, TypeCode tc) => Rustorb.Anys.Insert(this, tc, o => o.write_value(value));

        /// <summary>The value of an any of a valuetype or value box.</summary>
        public IValueBase? extract_Value()
        {
            TCKind k = type.unaliased().kind();
            if (k != TCKind.tk_value && k != TCKind.tk_value_box && k != TCKind.tk_event)
            {
                throw new BAD_OPERATION("the any does not hold a value", 0, CompletionStatus.COMPLETED_NO);
            }
            string id = type.unaliased().id();
            return Rustorb.Anys.ExtractKind(this, k, i => i.read_value(id));
        }

        /// <summary>A fixed with the TypeCode of its own digits and scale.</summary>
        public void insert_fixed(decimal value)
        {
            int[] bits = decimal.GetBits(value);
            short scale = (short)((bits[3] >> 16) & 0xFF);
            string s = Math.Abs(value).ToString(System.Globalization.CultureInfo.InvariantCulture)
                .Replace(".", "").TrimStart('0');
            ushort digits = (ushort)Math.Max(Math.Max(s.Length, scale), 1);
            insert_fixed(value, Rustorb.TypeCodes.Fixed(digits, scale));
        }

        /// <summary>A fixed with the TypeCode tc (tk_fixed or an alias of one).</summary>
        public void insert_fixed(decimal value, TypeCode tc)
        {
            TypeCode u = tc.unaliased();
            if (u.kind() != TCKind.tk_fixed)
            {
                throw new BAD_INV_ORDER("insert_fixed with a TypeCode that is not tk_fixed", 0,
                    CompletionStatus.COMPLETED_NO);
            }
            ushort d = u.fixed_digits();
            short s = u.fixed_scale();
            Rustorb.Anys.Insert(this, tc, o => o.write_fixed(value, d, s));
        }

        public decimal extract_fixed()
        {
            TypeCode u = type.unaliased();
            if (u.kind() != TCKind.tk_fixed)
            {
                throw new BAD_OPERATION("the any does not hold a fixed", 0, CompletionStatus.COMPLETED_NO);
            }
            ushort d = u.fixed_digits();
            short s = u.fixed_scale();
            return Rustorb.Anys.ExtractKind(this, TCKind.tk_fixed, i => i.read_fixed(d, s));
        }

        public bool Equals(Any? other)
        {
            if (other is null)
            {
                return false;
            }
            bool r = Ffi.rorb_any_equal(_handle.Raw, other._handle.Raw) != 0;
            GC.KeepAlive(this);
            GC.KeepAlive(other);
            return r;
        }

        public override bool Equals(object? obj) => obj is Any a && Equals(a);
        public override int GetHashCode() => type.GetHashCode();
        public override string ToString() => $"Any({type})";
    }
}

namespace Rustorb
{
    /// <summary>Values in anys, for the generated Helpers.</summary>
    public static unsafe class Anys
    {
        /// <summary>Puts the value write writes, of type tc, in the any.</summary>
        public static void Insert(Corba.Any any, Corba.TypeCode tc, Action<CdrOutput> write)
        {
            var c = new Call { write = write };
            nint result = Callbacks.With(c, ctx =>
            {
                nint r;
                RorbException exc;
                int st = Ffi.rorb_any_insert(tc._handle.Raw, (nint)Callbacks.Write, ctx, &r, &exc);
                if (st == Status.CALLBACK_FAILED)
                {
                    Ffi.rorb_exception_clear(&exc);
                    c.ThrowError();
                }
                Errors.Check(st, ref exc);
                return r;
            });
            GC.KeepAlive(tc);
            any._handle = new AnyHandle(result);
        }

        /// <summary>Reads the value of the any, which must have a TypeCode
        /// equivalent to tc (else BAD_OPERATION).</summary>
        public static T Extract<T>(Corba.Any any, Corba.TypeCode tc, Func<CdrInput, T> read)
        {
            if (!any.type.equivalent(tc))
            {
                throw new Corba.BAD_OPERATION($"the any holds {any.type}, not {tc}", 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
            return Read(any, read);
        }

        /// <summary>Reads the value of an any whose type (without aliases) has
        /// the given kind (bounded strings, any object reference...).</summary>
        public static T ExtractKind<T>(Corba.Any any, Corba.TCKind kind, Func<CdrInput, T> read)
        {
            if (any.type.unaliased().kind() != kind)
            {
                throw new Corba.BAD_OPERATION($"the any holds {any.type}, not a {kind}", 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
            return Read(any, read);
        }

        private static T Read<T>(Corba.Any any, Func<CdrInput, T> read)
        {
            T value = default!;
            var c = new Call { read = i => value = read(i) };
            Callbacks.With(c, ctx =>
            {
                RorbException exc;
                int st = Ffi.rorb_any_extract(any._handle.Raw, (nint)Callbacks.Read, ctx, &exc);
                if (st == Status.CALLBACK_FAILED)
                {
                    Ffi.rorb_exception_clear(&exc);
                    c.ThrowError();
                }
                Errors.Check(st, ref exc);
                return 0;
            });
            GC.KeepAlive(any);
            return value;
        }
    }
}
