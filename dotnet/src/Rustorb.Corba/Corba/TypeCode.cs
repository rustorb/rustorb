// Corba.TypeCode (IDL4 to C# A.1.2) on rorb_typecode, and the TypeCode
// constructors the generated code uses (Rustorb.TypeCodes).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Runtime.InteropServices;
using Rustorb.Native;

namespace Corba
{
    /// <summary>CORBA::TCKind.</summary>
    public enum TCKind
    {
        tk_null, tk_void, tk_short, tk_long, tk_ushort, tk_ulong, tk_float, tk_double,
        tk_boolean, tk_char, tk_octet, tk_any, tk_TypeCode, tk_Principal, tk_objref,
        tk_struct, tk_union, tk_enum, tk_string, tk_sequence, tk_array, tk_alias, tk_except,
        tk_longlong, tk_ulonglong, tk_longdouble, tk_wchar, tk_wstring, tk_fixed, tk_value,
        tk_value_box, tk_native, tk_abstract_interface, tk_local_interface, tk_component,
        tk_home, tk_event,
    }

    /// <summary>CORBA::TypeCode (IDL4 to C# A.1.2). Immutable.</summary>
    public unsafe class TypeCode : IEquatable<TypeCode>
    {
        internal readonly TypeCodeHandle _handle;

        internal TypeCode(TypeCodeHandle handle)
        {
            _handle = handle;
        }

        /// <summary>TypeCode::Bounds.</summary>
        public class Bounds : UserException
        {
            public override string _id() => "IDL:omg.org/CORBA/TypeCode/Bounds:1.0";
        }

        /// <summary>TypeCode::BadKind.</summary>
        public class BadKind : UserException
        {
            public override string _id() => "IDL:omg.org/CORBA/TypeCode/BadKind:1.0";
        }

        private nint H => _handle.Raw;

        private static TypeCode Wrap(nint h) => new TypeCode(new TypeCodeHandle(h));

        public bool equal(TypeCode tc)
        {
            bool r = Ffi.rorb_tc_equal(H, tc._handle.Raw) != 0;
            GC.KeepAlive(tc);
            GC.KeepAlive(this);
            return r;
        }

        public bool equivalent(TypeCode tc)
        {
            bool r = Ffi.rorb_tc_equivalent(H, tc._handle.Raw) != 0;
            GC.KeepAlive(tc);
            GC.KeepAlive(this);
            return r;
        }

        public TypeCode get_compact_typecode()
        {
            var t = Wrap(Ffi.rorb_tc_compact(H));
            GC.KeepAlive(this);
            return t;
        }

        public TCKind kind()
        {
            var k = (TCKind)Ffi.rorb_tc_kind(H);
            GC.KeepAlive(this);
            return k;
        }


        private string Str(delegate*<nint, byte**, nuint*, RorbException*, int> f)
        {
            byte* s;
            nuint len;
            RorbException exc;
            int st = f(H, &s, &len, &exc);
            Errors.Check(st, ref exc);
            string r = Strings.FromUtf8(s, len);
            GC.KeepAlive(this);
            return r;
        }

        public string id() => Str(&Ffi.rorb_tc_id);
        public string name() => Str(&Ffi.rorb_tc_name);

        public uint member_count()
        {
            uint n;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_member_count(H, &n, &exc), ref exc);
            GC.KeepAlive(this);
            return n;
        }

        public string member_name(uint index)
        {
            byte* s;
            nuint len;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_member_name(H, index, &s, &len, &exc), ref exc);
            string r = Strings.FromUtf8(s, len);
            GC.KeepAlive(this);
            return r;
        }

        public TypeCode member_type(uint index)
        {
            nint t;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_member_type(H, index, &t, &exc), ref exc);
            GC.KeepAlive(this);
            return Wrap(t);
        }

        public Any member_label(uint index)
        {
            nint a;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_member_label(H, index, &a, &exc), ref exc);
            GC.KeepAlive(this);
            return new Any(new AnyHandle(a));
        }

        public TypeCode discriminator_type()
        {
            nint t;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_discriminator_type(H, &t, &exc), ref exc);
            GC.KeepAlive(this);
            return Wrap(t);
        }

        public int default_index()
        {
            int i;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_default_index(H, &i, &exc), ref exc);
            GC.KeepAlive(this);
            return i;
        }

        public uint length()
        {
            uint n;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_length(H, &n, &exc), ref exc);
            GC.KeepAlive(this);
            return n;
        }

        public TypeCode content_type()
        {
            nint t;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_content_type(H, &t, &exc), ref exc);
            GC.KeepAlive(this);
            return Wrap(t);
        }

        public ushort fixed_digits()
        {
            ushort d;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_fixed_digits(H, &d, &exc), ref exc);
            GC.KeepAlive(this);
            return d;
        }

        public short fixed_scale()
        {
            short s;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_fixed_scale(H, &s, &exc), ref exc);
            GC.KeepAlive(this);
            return s;
        }

        public short member_visibility(uint index)
        {
            short v;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_member_visibility(H, index, &v, &exc), ref exc);
            GC.KeepAlive(this);
            return v;
        }

        public short type_modifier()
        {
            short v;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_type_modifier(H, &v, &exc), ref exc);
            GC.KeepAlive(this);
            return v;
        }

        /// <summary>The concrete base of a valuetype; null for none.</summary>
        public TypeCode? concrete_base_type()
        {
            nint t;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_concrete_base_type(H, &t, &exc), ref exc);
            GC.KeepAlive(this);
            return t == 0 ? null : Wrap(t);
        }

        /// <summary>The TypeCode without its aliases (rustorb).</summary>
        public TypeCode unaliased()
        {
            var t = Wrap(Ffi.rorb_tc_unaliased(H));
            GC.KeepAlive(this);
            return t;
        }

        public bool Equals(TypeCode? other) => other is not null && equal(other);
        public override bool Equals(object? obj) => obj is TypeCode t && equal(t);

        public override int GetHashCode()
        {
            TCKind k = kind();
            try
            {
                return HashCode.Combine(k, id());
            }
            catch (BadKind)
            {
                return k.GetHashCode();
            }
        }

        public override string ToString()
        {
            TCKind k = kind();
            try
            {
                return $"TypeCode({k}, {id()})";
            }
            catch (BadKind)
            {
                return $"TypeCode({k})";
            }
        }
    }
}

namespace Rustorb
{
    /// <summary>The TypeCode constructors of the generated code (the ORB's
    /// create_*_tc are the standard ones).</summary>
    public static unsafe class TypeCodes
    {
        private static readonly Corba.TypeCode?[] simple = new Corba.TypeCode?[37];

        private static Corba.TypeCode Wrap(nint h) => new Corba.TypeCode(new TypeCodeHandle(h));

        public static Corba.TypeCode Simple(Corba.TCKind kind)
        {
            Corba.TypeCode? t = simple[(int)kind];
            if (t != null)
            {
                return t;
            }
            nint h;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_simple((uint)kind, &h, &exc), ref exc);
            t = Wrap(h);
            simple[(int)kind] = t;
            return t;
        }

        public static Corba.TypeCode String(uint bound) => Str(0, bound);
        public static Corba.TypeCode WString(uint bound) => Str(1, bound);

        private static Corba.TypeCode Str(int wide, uint bound)
        {
            nint h;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_string(wide, bound, &h, &exc), ref exc);
            return Wrap(h);
        }

        public static Corba.TypeCode Fixed(ushort digits, short scale)
        {
            nint h;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_fixed(digits, scale, &h, &exc), ref exc);
            return Wrap(h);
        }

        public static Corba.TypeCode Sequence(Corba.TypeCode content, uint bound)
        {
            nint h;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_sequence(content._handle.Raw, bound, &h, &exc), ref exc);
            GC.KeepAlive(content);
            return Wrap(h);
        }

        public static Corba.TypeCode Array(Corba.TypeCode content, uint length)
        {
            nint h;
            RorbException exc;
            Errors.Check(Ffi.rorb_tc_array(content._handle.Raw, length, &h, &exc), ref exc);
            GC.KeepAlive(content);
            return Wrap(h);
        }

        public static Corba.TypeCode Alias(string id, string name, Corba.TypeCode content) =>
            AliasKind(Corba.TCKind.tk_alias, id, name, content);

        public static Corba.TypeCode ValueBox(string id, string name, Corba.TypeCode content) =>
            AliasKind(Corba.TCKind.tk_value_box, id, name, content);

        private static Corba.TypeCode AliasKind(Corba.TCKind kind, string id, string name, Corba.TypeCode content)
        {
            byte[] i = Strings.ToUtf8(id), n = Strings.ToUtf8(name);
            nint h;
            RorbException exc;
            fixed (byte* ip = i, np = n)
            {
                Errors.Check(Ffi.rorb_tc_alias((uint)kind, ip, (nuint)i.Length, np, (nuint)n.Length,
                    content._handle.Raw, &h, &exc), ref exc);
            }
            GC.KeepAlive(content);
            return Wrap(h);
        }

        /// <summary>tk_objref, tk_abstract_interface, tk_local_interface or tk_native.</summary>
        public static Corba.TypeCode Interface(Corba.TCKind kind, string id, string name)
        {
            byte[] i = Strings.ToUtf8(id), n = Strings.ToUtf8(name);
            nint h;
            RorbException exc;
            fixed (byte* ip = i, np = n)
            {
                Errors.Check(Ffi.rorb_tc_interface((uint)kind, ip, (nuint)i.Length, np, (nuint)n.Length, &h, &exc),
                    ref exc);
            }
            return Wrap(h);
        }

        public static Corba.TypeCode Struct(string id, string name, string[] names, Corba.TypeCode[] types) =>
            StructKind(Corba.TCKind.tk_struct, id, name, names, types);

        public static Corba.TypeCode Exception(string id, string name, string[] names, Corba.TypeCode[] types) =>
            StructKind(Corba.TCKind.tk_except, id, name, names, types);

        /// <summary>The UTF-8 bytes of names, pinned while they are used.</summary>
        private sealed class Names : IDisposable
        {
            internal readonly byte*[] ptrs;
            internal readonly nuint[] lens;
            private readonly GCHandle[] pins;

            internal Names(string[] names)
            {
                ptrs = new byte*[names.Length];
                lens = new nuint[names.Length];
                pins = new GCHandle[names.Length];
                for (int k = 0; k < names.Length; k++)
                {
                    byte[] b = Strings.ToUtf8(names[k]);
                    if (b.Length == 0)
                    {
                        b = new byte[1];
                        lens[k] = 0;
                    }
                    else
                    {
                        lens[k] = (nuint)b.Length;
                    }
                    pins[k] = GCHandle.Alloc(b, GCHandleType.Pinned);
                    ptrs[k] = (byte*)pins[k].AddrOfPinnedObject();
                }
            }

            public void Dispose()
            {
                foreach (GCHandle p in pins)
                {
                    p.Free();
                }
            }
        }

        private static nint[] Raw(Corba.TypeCode[] types)
        {
            var r = new nint[types.Length];
            for (int k = 0; k < types.Length; k++)
            {
                r[k] = types[k]._handle.Raw;
            }
            return r;
        }

        private static Corba.TypeCode StructKind(Corba.TCKind kind, string id, string name, string[] names,
            Corba.TypeCode[] types)
        {
            byte[] i = Strings.ToUtf8(id), n = Strings.ToUtf8(name);
            nint[] raw = Raw(types);
            nint h;
            RorbException exc;
            using (var ns = new Names(names))
            {
                fixed (byte* ip = i, np = n)
                fixed (byte** nps = ns.ptrs)
                fixed (nuint* nls = ns.lens)
                fixed (nint* ts = raw)
                {
                    Errors.Check(Ffi.rorb_tc_struct((uint)kind, ip, (nuint)i.Length, np, (nuint)n.Length,
                        (nuint)names.Length, nps, nls, ts, &h, &exc), ref exc);
                }
            }
            GC.KeepAlive(types);
            return Wrap(h);
        }

        /// <summary>A union: one entry per label (labels as rorb_tc_union takes
        /// them), defaultIndex the entry of the default member or -1.</summary>
        public static Corba.TypeCode Union(string id, string name, Corba.TypeCode discriminator, ulong[] labels,
            int defaultIndex, string[] names, Corba.TypeCode[] types)
        {
            byte[] i = Strings.ToUtf8(id), n = Strings.ToUtf8(name);
            nint[] raw = Raw(types);
            nint h;
            RorbException exc;
            using (var ns = new Names(names))
            {
                fixed (byte* ip = i, np = n)
                fixed (byte** nps = ns.ptrs)
                fixed (nuint* nls = ns.lens)
                fixed (nint* ts = raw)
                fixed (ulong* ls = labels)
                {
                    Errors.Check(Ffi.rorb_tc_union(ip, (nuint)i.Length, np, (nuint)n.Length,
                        discriminator._handle.Raw, (nuint)names.Length, ls, defaultIndex, nps, nls, ts, &h, &exc),
                        ref exc);
                }
            }
            GC.KeepAlive(types);
            GC.KeepAlive(discriminator);
            return Wrap(h);
        }

        public static Corba.TypeCode Enum(string id, string name, string[] names)
        {
            byte[] i = Strings.ToUtf8(id), n = Strings.ToUtf8(name);
            nint h;
            RorbException exc;
            using (var ns = new Names(names))
            {
                fixed (byte* ip = i, np = n)
                fixed (byte** nps = ns.ptrs)
                fixed (nuint* nls = ns.lens)
                {
                    Errors.Check(Ffi.rorb_tc_enum(ip, (nuint)i.Length, np, (nuint)n.Length, (nuint)names.Length,
                        nps, nls, &h, &exc), ref exc);
                }
            }
            return Wrap(h);
        }

        /// <summary>A placeholder for a recursive struct or union (ORB::create_recursive_tc).</summary>
        public static Corba.TypeCode Recursive(string id)
        {
            byte[] i = Strings.ToUtf8(id);
            nint h;
            RorbException exc;
            fixed (byte* ip = i)
            {
                Errors.Check(Ffi.rorb_tc_recursive(ip, (nuint)i.Length, &h, &exc), ref exc);
            }
            return Wrap(h);
        }
    }
}
