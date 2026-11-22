// The streams the generated code writes arguments to and reads them from:
// views of the rorb_out / rorb_in of a C ABI callback, valid only during that
// callback. Errors on them are sticky and reported when the callback returns.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Runtime.InteropServices;
using Rustorb.Native;

namespace Rustorb
{
    /// <summary>An output stream of rustorb (the arguments of a request, a reply,
    /// the value of an any). Used by the generated code.</summary>
    public readonly unsafe struct CdrOutput
    {
        internal readonly nint ptr;

        internal CdrOutput(nint ptr)
        {
            this.ptr = ptr;
        }

        public void write_boolean(bool v) => Ffi.rorb_out_write_boolean(ptr, v ? 1 : 0);
        public void write_octet(byte v) => Ffi.rorb_out_write_octet(ptr, v);
        public void write_short(short v) => Ffi.rorb_out_write_short(ptr, v);
        public void write_ushort(ushort v) => Ffi.rorb_out_write_ushort(ptr, v);
        public void write_long(int v) => Ffi.rorb_out_write_long(ptr, v);
        public void write_ulong(uint v) => Ffi.rorb_out_write_ulong(ptr, v);
        public void write_longlong(long v) => Ffi.rorb_out_write_longlong(ptr, v);
        public void write_ulonglong(ulong v) => Ffi.rorb_out_write_ulonglong(ptr, v);
        public void write_float(float v) => Ffi.rorb_out_write_float(ptr, v);
        public void write_double(double v) => Ffi.rorb_out_write_double(ptr, v);
        public void write_int8(sbyte v) => Ffi.rorb_out_write_octet(ptr, (byte)v);
        public void write_uint8(byte v) => Ffi.rorb_out_write_octet(ptr, v);

        /// <summary>An IDL char: one byte (ISO 8859-1); a char above U+00FF is
        /// DATA_CONVERSION.</summary>
        public void write_char(char v)
        {
            if (v > 0xFF)
            {
                throw new Corba.DATA_CONVERSION($"char U+{(int)v:X4} is not in ISO 8859-1", 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
            Ffi.rorb_out_write_char(ptr, (byte)v);
        }

        public void write_wchar(char v) => Ffi.rorb_out_write_wchar(ptr, v);

        public void write_longdouble(decimal v)
        {
            LongDouble.FromDecimal(v, out ulong hi, out ulong lo);
            Ffi.rorb_out_write_longdouble(ptr, hi, lo);
        }

        public void write_string(string v)
        {
            if (v == null)
            {
                throw new Corba.BAD_PARAM("a null string", 0, Corba.CompletionStatus.COMPLETED_NO);
            }
            byte[] b = Strings.ToUtf8(v);
            fixed (byte* p = b)
            {
                Ffi.rorb_out_write_string(ptr, p, (nuint)b.Length);
            }
        }

        public void write_wstring(string v)
        {
            if (v == null)
            {
                throw new Corba.BAD_PARAM("a null wstring", 0, Corba.CompletionStatus.COMPLETED_NO);
            }
            fixed (char* p = v)
            {
                Ffi.rorb_out_write_wstring(ptr, (ushort*)p, (nuint)v.Length);
            }
        }

        /// <summary>An object reference; null is a nil reference. An object that
        /// is not a reference of rustorb (a servant, say) is BAD_PARAM.</summary>
        public void write_Object(Corba.IObject? v)
        {
            if (v == null)
            {
                Ffi.rorb_out_write_object(ptr, 0);
                return;
            }
            if (v is not Corba.Object o)
            {
                throw new Corba.BAD_PARAM("not an object reference of rustorb: " + v.GetType(), 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
            Ffi.rorb_out_write_object(ptr, o._handle.Raw);
            GC.KeepAlive(o);
        }

        public void write_TypeCode(Corba.TypeCode v)
        {
            if (v == null)
            {
                throw new Corba.BAD_TYPECODE("a null TypeCode", 0, Corba.CompletionStatus.COMPLETED_NO);
            }
            Ffi.rorb_out_write_typecode(ptr, v._handle.Raw);
            GC.KeepAlive(v);
        }

        public void write_any(Corba.Any v)
        {
            if (v == null)
            {
                throw new Corba.BAD_PARAM("a null any", 0, Corba.CompletionStatus.COMPLETED_NO);
            }
            Ffi.rorb_out_write_any(ptr, v._handle.Raw);
            GC.KeepAlive(v);
        }

        /// <summary>A value (null, an instance written before in the message
        /// as an indirection, or the state of the instance).</summary>
        public void write_value(Corba.IValueBase? v) => Values.Write(this, v);

        /// <summary>An abstract interface: an object reference or a value.</summary>
        public void write_abstract(object? v) => Values.WriteAbstract(this, v);

        /// <summary>A fixed&lt;digits, scale&gt;: a value with more fraction digits than
        /// scale (not zeros) or more integer digits than digits - scale is
        /// BAD_PARAM (it is not rounded).</summary>
        public void write_fixed(decimal v, ushort digits, short scale)
        {
            byte[] d = Fixed.Digits(v, digits, scale);
            fixed (byte* p = d)
            {
                Ffi.rorb_out_write_fixed(ptr, p, (nuint)d.Length, v < 0 ? 1 : 0);
            }
        }

        private void Array<T>(int kind, ReadOnlySpan<T> v) where T : unmanaged
        {
            fixed (T* p = v)
            {
                Ffi.rorb_out_write_array(ptr, kind, (nint)p, (nuint)v.Length);
            }
        }

        public void write_octet_array(ReadOnlySpan<byte> v) => Array(Prim.OCTET, v);
        public void write_short_array(ReadOnlySpan<short> v) => Array(Prim.SHORT, v);
        public void write_ushort_array(ReadOnlySpan<ushort> v) => Array(Prim.USHORT, v);
        public void write_long_array(ReadOnlySpan<int> v) => Array(Prim.LONG, v);
        public void write_ulong_array(ReadOnlySpan<uint> v) => Array(Prim.ULONG, v);
        public void write_longlong_array(ReadOnlySpan<long> v) => Array(Prim.LONGLONG, v);
        public void write_ulonglong_array(ReadOnlySpan<ulong> v) => Array(Prim.ULONGLONG, v);
        public void write_float_array(ReadOnlySpan<float> v) => Array(Prim.FLOAT, v);
        public void write_double_array(ReadOnlySpan<double> v) => Array(Prim.DOUBLE, v);

        public void write_int8_array(ReadOnlySpan<sbyte> v) =>
            write_octet_array(MemoryMarshal.Cast<sbyte, byte>(v));

        public void write_boolean_array(ReadOnlySpan<bool> v)
        {
            foreach (bool b in v)
            {
                write_boolean(b);
            }
        }

        public void write_char_array(ReadOnlySpan<char> v)
        {
            foreach (char c in v)
            {
                write_char(c);
            }
        }

        public void write_wchar_array(ReadOnlySpan<char> v)
        {
            foreach (char c in v)
            {
                write_wchar(c);
            }
        }

        public bool failed => Ffi.rorb_out_failed(ptr) != 0;
    }

    /// <summary>An input stream of rustorb. Used by the generated code.</summary>
    public readonly unsafe struct CdrInput
    {
        internal readonly nint ptr;

        internal CdrInput(nint ptr)
        {
            this.ptr = ptr;
        }

        public bool read_boolean() => Ffi.rorb_in_read_boolean(ptr) != 0;
        public byte read_octet() => Ffi.rorb_in_read_octet(ptr);
        public char read_char() => (char)Ffi.rorb_in_read_char(ptr);
        public char read_wchar() => (char)Ffi.rorb_in_read_wchar(ptr);
        public short read_short() => Ffi.rorb_in_read_short(ptr);
        public ushort read_ushort() => Ffi.rorb_in_read_ushort(ptr);
        public int read_long() => Ffi.rorb_in_read_long(ptr);
        public uint read_ulong() => Ffi.rorb_in_read_ulong(ptr);
        public long read_longlong() => Ffi.rorb_in_read_longlong(ptr);
        public ulong read_ulonglong() => Ffi.rorb_in_read_ulonglong(ptr);
        public float read_float() => Ffi.rorb_in_read_float(ptr);
        public double read_double() => Ffi.rorb_in_read_double(ptr);
        public sbyte read_int8() => (sbyte)Ffi.rorb_in_read_octet(ptr);
        public byte read_uint8() => Ffi.rorb_in_read_octet(ptr);

        /// <summary>A long double; one outside the range of decimal is
        /// DATA_CONVERSION, and digits past its precision are rounded.</summary>
        public decimal read_longdouble()
        {
            ulong hi, lo;
            Ffi.rorb_in_read_longdouble(ptr, &hi, &lo);
            if (failed)
            {
                return 0;
            }
            return LongDouble.ToDecimal(hi, lo);
        }

        public string read_string()
        {
            nuint len;
            byte* p = Ffi.rorb_in_read_string(ptr, &len);
            return p == null ? "" : Strings.FromUtf8(p, len);
        }

        public string read_wstring()
        {
            nuint len;
            ushort* p = Ffi.rorb_in_read_wstring(ptr, &len);
            return p == null || len == 0 ? "" : new string((char*)p, 0, checked((int)len));
        }

        /// <summary>An object reference; null for a nil one.</summary>
        public Corba.Object? read_Object()
        {
            nint h = Ffi.rorb_in_read_object(ptr);
            return h == 0 ? null : new Corba.Object(new ObjectHandle(h));
        }

        public Corba.TypeCode read_TypeCode()
        {
            nint h = Ffi.rorb_in_read_typecode(ptr);
            return h == 0 ? TypeCodes.Simple(Corba.TCKind.tk_null) : new Corba.TypeCode(new TypeCodeHandle(h));
        }

        public Corba.Any read_any()
        {
            nint h = Ffi.rorb_in_read_any(ptr);
            return h == 0 ? new Corba.Any() : new Corba.Any(new AnyHandle(h));
        }

        /// <summary>A value: null, an instance read before in the message, or
        /// a new instance made by the value factory of its repository id
        /// (of expected when the value is sent without one).</summary>
        public Corba.IValueBase? read_value(string? expected) => Values.Read(this, expected);

        /// <summary>An abstract interface: an object reference (Corba.Object)
        /// or a value.</summary>
        public object? read_abstract() => Values.ReadAbstract(this);

        public decimal read_fixed(ushort digits, short scale)
        {
            var d = new byte[digits];
            int negative;
            fixed (byte* p = d)
            {
                negative = Ffi.rorb_in_read_fixed(ptr, p, (nuint)d.Length);
            }
            if (failed)
            {
                return 0;
            }
            return Fixed.FromDigits(d, scale, negative != 0);
        }

        /// <summary>A sequence length, checked against the bytes left for
        /// elements of at least minElementSize bytes.</summary>
        public int read_length(int minElementSize)
        {
            uint n = Ffi.rorb_in_read_length(ptr, (nuint)minElementSize);
            return n > int.MaxValue ? 0 : (int)n;
        }

        /// <summary>Marks the stream as failed (MARSHAL), for a value the
        /// caller rejects.</summary>
        public void fail() => Ffi.rorb_in_fail(ptr);

        public bool failed => Ffi.rorb_in_failed(ptr) != 0;

        private void Array<T>(int kind, Span<T> v) where T : unmanaged
        {
            fixed (T* p = v)
            {
                Ffi.rorb_in_read_array(ptr, kind, (nint)p, (nuint)v.Length);
            }
        }

        public void read_octet_array(Span<byte> v) => Array(Prim.OCTET, v);
        public void read_short_array(Span<short> v) => Array(Prim.SHORT, v);
        public void read_ushort_array(Span<ushort> v) => Array(Prim.USHORT, v);
        public void read_long_array(Span<int> v) => Array(Prim.LONG, v);
        public void read_ulong_array(Span<uint> v) => Array(Prim.ULONG, v);
        public void read_longlong_array(Span<long> v) => Array(Prim.LONGLONG, v);
        public void read_ulonglong_array(Span<ulong> v) => Array(Prim.ULONGLONG, v);
        public void read_float_array(Span<float> v) => Array(Prim.FLOAT, v);
        public void read_double_array(Span<double> v) => Array(Prim.DOUBLE, v);
        public void read_int8_array(Span<sbyte> v) => read_octet_array(MemoryMarshal.Cast<sbyte, byte>(v));

        public void read_boolean_array(Span<bool> v)
        {
            for (int i = 0; i < v.Length; i++)
            {
                v[i] = read_boolean();
            }
        }

        public void read_char_array(Span<char> v)
        {
            for (int i = 0; i < v.Length; i++)
            {
                v[i] = read_char();
            }
        }

        public void read_wchar_array(Span<char> v)
        {
            for (int i = 0; i < v.Length; i++)
            {
                v[i] = read_wchar();
            }
        }
    }

    /// <summary>Array helpers for the generated code.</summary>
    public static class Arrays
    {
        /// <summary>An array of n elements, each made by make (for the
        /// default values of IDL arrays of non-primitive types).</summary>
        public static T[] Make<T>(int n, Func<T> make)
        {
            var a = new T[n];
            for (int i = 0; i < n; i++)
            {
                a[i] = make();
            }
            return a;
        }
    }

    /// <summary>Sequence helpers for the generated code.</summary>
    public static class Seq
    {
        /// <summary>The elements of a sequence as a span (without a copy for an
        /// Omg.Types.Sequence).</summary>
        public static ReadOnlySpan<T> Span<T>(Omg.Types.ISequence<T> s)
        {
            if (s is List<T> l)
            {
                return CollectionsMarshal.AsSpan(l);
            }
            return s.ToArray();
        }

        /// <summary>A sequence of n default elements, to read into.</summary>
        public static Omg.Types.Sequence<T> Sized<T>(int n)
        {
            var s = new Omg.Types.Sequence<T>(n);
            CollectionsMarshal.SetCount(s, n);
            return s;
        }

        /// <summary>The span of a sequence made by Sized.</summary>
        public static Span<T> Writable<T>(Omg.Types.Sequence<T> s) => CollectionsMarshal.AsSpan(s);

        /// <summary>Checks the length of a value against the bound of its type
        /// before it is sent (BAD_PARAM).</summary>
        public static void CheckBound(int length, uint bound)
        {
            if (bound != 0 && (uint)length > bound)
            {
                throw new Corba.BAD_PARAM($"length {length} exceeds the bound {bound}", 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
        }

        /// <summary>Checks the length of an array value before it is sent
        /// (BAD_PARAM).</summary>
        public static void CheckLength(Array? a, int length)
        {
            if (a == null)
            {
                throw new Corba.BAD_PARAM("a null array", 0, Corba.CompletionStatus.COMPLETED_NO);
            }
            if (a.Length != length)
            {
                throw new Corba.BAD_PARAM($"an array of {a.Length} elements where {length} are needed", 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
        }

        /// <summary>Checks a value that must not be null before it is sent
        /// (BAD_PARAM).</summary>
        public static T NotNull<T>(T? v) where T : class
        {
            if (v == null)
            {
                throw new Corba.BAD_PARAM("a null value of type " + typeof(T).Name, 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
            return v;
        }

        /// <summary>Whether two sequences have equal elements.</summary>
        public static bool Equal<T>(IList<T>? a, IList<T>? b)
        {
            if (ReferenceEquals(a, b))
            {
                return true;
            }
            if (a == null || b == null || a.Count != b.Count)
            {
                return false;
            }
            var cmp = EqualityComparer<T>.Default;
            for (int i = 0; i < a.Count; i++)
            {
                if (!ElementEqual(a[i], b[i], cmp))
                {
                    return false;
                }
            }
            return true;
        }

        /// <summary>Whether two arrays (of any rank, nested) have equal elements.</summary>
        public static bool ArrayEqual(Array? a, Array? b)
        {
            if (ReferenceEquals(a, b))
            {
                return true;
            }
            if (a == null || b == null || a.Length != b.Length)
            {
                return false;
            }
            for (int i = 0; i < a.Length; i++)
            {
                if (!ElementEqual(a.GetValue(i), b.GetValue(i), EqualityComparer<object?>.Default))
                {
                    return false;
                }
            }
            return true;
        }

        private static bool ElementEqual<T>(T x, T y, IEqualityComparer<T> cmp)
        {
            if (x is Array ax && y is Array ay)
            {
                return ArrayEqual(ax, ay);
            }
            if (x is System.Collections.IList lx && y is System.Collections.IList ly && x is not string)
            {
                if (lx.Count != ly.Count)
                {
                    return false;
                }
                for (int i = 0; i < lx.Count; i++)
                {
                    if (!ElementEqual(lx[i], ly[i], EqualityComparer<object?>.Default))
                    {
                        return false;
                    }
                }
                return true;
            }
            return cmp.Equals(x, y);
        }

        /// <summary>A hash of the elements of a sequence or array.</summary>
        public static int Hash(System.Collections.IEnumerable? e)
        {
            if (e == null)
            {
                return 0;
            }
            var h = new HashCode();
            foreach (object? o in e)
            {
                h.Add(o is System.Collections.IEnumerable inner and not string ? Hash(inner) : o?.GetHashCode() ?? 0);
            }
            return h.ToHashCode();
        }
    }

    /// <summary>IDL fixed as decimal.</summary>
    internal static class Fixed
    {
        internal static byte[] Digits(decimal v, ushort digits, short scale)
        {
            if (scale < 0 || scale > digits || digits > 31)
            {
                throw new Corba.BAD_PARAM($"fixed<{digits},{scale}> is not a valid type", 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
            decimal a = Math.Abs(v);
            if (scale <= 28 && decimal.Round(a, scale) != a)
            {
                throw new Corba.BAD_PARAM($"{v} has more than {scale} fraction digits", 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
            string s = a.ToString("F" + scale, CultureInfo.InvariantCulture).Replace(".", "").TrimStart('0');
            if (s.Length > digits)
            {
                throw new Corba.BAD_PARAM($"{v} does not fit fixed<{digits},{scale}>", 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
            var d = new byte[digits];
            int off = digits - s.Length;
            for (int i = 0; i < s.Length; i++)
            {
                d[off + i] = (byte)(s[i] - '0');
            }
            return d;
        }

        internal static decimal FromDigits(byte[] d, short scale, bool negative)
        {
            var sb = new System.Text.StringBuilder(d.Length + 2);
            if (negative)
            {
                sb.Append('-');
            }
            int intDigits = d.Length - scale;
            for (int i = 0; i < d.Length; i++)
            {
                if (i == intDigits)
                {
                    if (i == 0)
                    {
                        sb.Append('0');
                    }
                    sb.Append('.');
                }
                sb.Append((char)('0' + d[i]));
            }
            if (d.Length == 0)
            {
                sb.Append('0');
            }
            try
            {
                return decimal.Parse(sb.ToString(), NumberStyles.Number, CultureInfo.InvariantCulture);
            }
            catch (OverflowException)
            {
                throw new Corba.DATA_CONVERSION("a fixed value out of the range of decimal", 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
        }
    }
}
