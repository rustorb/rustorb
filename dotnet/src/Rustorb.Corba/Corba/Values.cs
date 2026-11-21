// Valuetypes (CORBA 3.4 part 1, 14 and part 2, 9.3.4) in the .NET binding:
// Corba.ValueBase, value factories, custom marshaling, and the reading and
// writing of values on the streams of rustorb (Rustorb.Values).
//
// IDL4 to C# 7.6 maps a valuetype to an abstract class with its state
// members as properties (private ones protected), its operations as
// abstract methods and its factories as abstract void methods marked
// [Omg.Types.Factory]; the rest (the base class, factories, boxes, custom
// marshaling) is rustorb's, after the C++ and Java mappings.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

#nullable enable

using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Threading;
using Rustorb.Native;

namespace Corba
{
    /// <summary>What every value is (CORBA::ValueBase); an abstract
    /// valuetype is an interface that extends it.</summary>
    public interface IValueBase
    {
        /// <summary>The repository ids the value is sent with: that of its
        /// type, then those of the bases it is truncatable to.</summary>
        string[] _truncatable_ids();
    }

    /// <summary>The base class of the classes of valuetypes and value
    /// boxes. The generated classes write and read their state with
    /// _write and _read.</summary>
    public abstract class ValueBase : IValueBase
    {
        public abstract string[] _truncatable_ids();

        /// <summary>Writes the state members (those of the bases first).</summary>
        public virtual void _write(Rustorb.CdrOutput output)
        {
        }

        /// <summary>Reads the state members (those of the bases first).</summary>
        public virtual void _read(Rustorb.CdrInput input)
        {
        }
    }

    /// <summary>Makes the instances the ORB reads values into (CORBA 3.4
    /// part 1, 14.4.4; create_for_unmarshal of the C++ mapping). The ORB
    /// then reads the state into the instance.</summary>
    public interface IValueFactory
    {
        IValueBase create_for_unmarshal();
    }

    /// <summary>A value factory from a function.</summary>
    public sealed class ValueFactory : IValueFactory
    {
        private readonly Func<IValueBase> make;

        public ValueFactory(Func<IValueBase> make)
        {
            this.make = make ?? throw new BAD_PARAM("a null function", 0, CompletionStatus.COMPLETED_NO);
        }

        public IValueBase create_for_unmarshal() => make();
    }

    /// <summary>A custom valuetype (custom valuetype V): its class
    /// implements marshal and unmarshal, which write and read the state
    /// (CORBA 3.4 part 1, 14.4.7). Its values are sent chunked.</summary>
    public interface ICustomMarshal
    {
        void marshal(DataOutputStream os);
        void unmarshal(DataInputStream @is);
    }

    /// <summary>CORBA::DataOutputStream: what a custom value writes its
    /// state with. Valid only during marshal.</summary>
    public sealed class DataOutputStream
    {
        private readonly Rustorb.CdrOutput o;

        internal DataOutputStream(Rustorb.CdrOutput o)
        {
            this.o = o;
        }

        public void write_any(Any value) => o.write_any(value);
        public void write_boolean(bool value) => o.write_boolean(value);
        public void write_char(char value) => o.write_char(value);
        public void write_wchar(char value) => o.write_wchar(value);
        public void write_octet(byte value) => o.write_octet(value);
        public void write_short(short value) => o.write_short(value);
        public void write_ushort(ushort value) => o.write_ushort(value);
        public void write_long(int value) => o.write_long(value);
        public void write_ulong(uint value) => o.write_ulong(value);
        public void write_longlong(long value) => o.write_longlong(value);
        public void write_ulonglong(ulong value) => o.write_ulonglong(value);
        public void write_float(float value) => o.write_float(value);
        public void write_double(double value) => o.write_double(value);
        public void write_longdouble(decimal value) => o.write_longdouble(value);
        public void write_string(string value) => o.write_string(value);
        public void write_wstring(string value) => o.write_wstring(value);
        public void write_Object(IObject? value) => o.write_Object(value);
        public void write_Abstract(object? value) => o.write_abstract(value);
        public void write_Value(IValueBase? value) => o.write_value(value);
        public void write_TypeCode(TypeCode value) => o.write_TypeCode(value);

        private static ReadOnlySpan<T> Slice<T>(IList<T> seq, uint offset, uint length)
        {
            if (seq == null)
            {
                throw new BAD_PARAM("a null sequence", 0, CompletionStatus.COMPLETED_NO);
            }
            if ((ulong)offset + length > (ulong)seq.Count)
            {
                throw new BAD_PARAM("offset and length beyond the sequence", 0, CompletionStatus.COMPLETED_NO);
            }
            var a = new T[length];
            for (int i = 0; i < length; i++)
            {
                a[i] = seq[(int)offset + i];
            }
            return a;
        }

        public void write_any_array(IList<Any> seq, uint offset, uint length)
        {
            foreach (Any a in Slice(seq, offset, length))
            {
                o.write_any(a);
            }
        }

        public void write_boolean_array(IList<bool> seq, uint offset, uint length) => o.write_boolean_array(Slice(seq, offset, length));
        public void write_char_array(IList<char> seq, uint offset, uint length) => o.write_char_array(Slice(seq, offset, length));
        public void write_wchar_array(IList<char> seq, uint offset, uint length) => o.write_wchar_array(Slice(seq, offset, length));
        public void write_octet_array(IList<byte> seq, uint offset, uint length) => o.write_octet_array(Slice(seq, offset, length));
        public void write_short_array(IList<short> seq, uint offset, uint length) => o.write_short_array(Slice(seq, offset, length));
        public void write_ushort_array(IList<ushort> seq, uint offset, uint length) => o.write_ushort_array(Slice(seq, offset, length));
        public void write_long_array(IList<int> seq, uint offset, uint length) => o.write_long_array(Slice(seq, offset, length));
        public void write_ulong_array(IList<uint> seq, uint offset, uint length) => o.write_ulong_array(Slice(seq, offset, length));
        public void write_longlong_array(IList<long> seq, uint offset, uint length) => o.write_longlong_array(Slice(seq, offset, length));
        public void write_ulonglong_array(IList<ulong> seq, uint offset, uint length) => o.write_ulonglong_array(Slice(seq, offset, length));
        public void write_float_array(IList<float> seq, uint offset, uint length) => o.write_float_array(Slice(seq, offset, length));
        public void write_double_array(IList<double> seq, uint offset, uint length) => o.write_double_array(Slice(seq, offset, length));

        public void write_longdouble_array(IList<decimal> seq, uint offset, uint length)
        {
            foreach (decimal d in Slice(seq, offset, length))
            {
                o.write_longdouble(d);
            }
        }
    }

    /// <summary>CORBA::DataInputStream: what a custom value reads its state
    /// with. Valid only during unmarshal.</summary>
    public sealed class DataInputStream
    {
        private readonly Rustorb.CdrInput i;

        internal DataInputStream(Rustorb.CdrInput i)
        {
            this.i = i;
        }

        public Any read_any() => i.read_any();
        public bool read_boolean() => i.read_boolean();
        public char read_char() => i.read_char();
        public char read_wchar() => i.read_wchar();
        public byte read_octet() => i.read_octet();
        public short read_short() => i.read_short();
        public ushort read_ushort() => i.read_ushort();
        public int read_long() => i.read_long();
        public uint read_ulong() => i.read_ulong();
        public long read_longlong() => i.read_longlong();
        public ulong read_ulonglong() => i.read_ulonglong();
        public float read_float() => i.read_float();
        public double read_double() => i.read_double();
        public decimal read_longdouble() => i.read_longdouble();
        public string read_string() => i.read_string();
        public string read_wstring() => i.read_wstring();
        public Object? read_Object() => i.read_Object();
        public object? read_Abstract() => i.read_abstract();
        public IValueBase? read_Value() => i.read_value(null);
        public TypeCode read_TypeCode() => i.read_TypeCode();

        private delegate void SpanReader<T>(Span<T> s);

        private static void Fill<T>(IList<T> seq, uint offset, uint length, SpanReader<T> read)
        {
            if (seq == null)
            {
                throw new BAD_PARAM("a null sequence", 0, CompletionStatus.COMPLETED_NO);
            }
            var a = new T[length];
            read(a);
            for (int k = 0; k < length; k++)
            {
                int at = (int)offset + k;
                if (at < seq.Count)
                {
                    seq[at] = a[k];
                }
                else
                {
                    seq.Add(a[k]);
                }
            }
        }

        // The elements are stored at seq[offset...], the sequence grown as
        // needed.
        public void read_boolean_array(IList<bool> seq, uint offset, uint length) { var s = i; Fill(seq, offset, length, x => s.read_boolean_array(x)); }
        public void read_char_array(IList<char> seq, uint offset, uint length) { var s = i; Fill(seq, offset, length, x => s.read_char_array(x)); }
        public void read_wchar_array(IList<char> seq, uint offset, uint length) { var s = i; Fill(seq, offset, length, x => s.read_wchar_array(x)); }
        public void read_octet_array(IList<byte> seq, uint offset, uint length) { var s = i; Fill(seq, offset, length, x => s.read_octet_array(x)); }
        public void read_short_array(IList<short> seq, uint offset, uint length) { var s = i; Fill(seq, offset, length, x => s.read_short_array(x)); }
        public void read_ushort_array(IList<ushort> seq, uint offset, uint length) { var s = i; Fill(seq, offset, length, x => s.read_ushort_array(x)); }
        public void read_long_array(IList<int> seq, uint offset, uint length) { var s = i; Fill(seq, offset, length, x => s.read_long_array(x)); }
        public void read_ulong_array(IList<uint> seq, uint offset, uint length) { var s = i; Fill(seq, offset, length, x => s.read_ulong_array(x)); }
        public void read_longlong_array(IList<long> seq, uint offset, uint length) { var s = i; Fill(seq, offset, length, x => s.read_longlong_array(x)); }
        public void read_ulonglong_array(IList<ulong> seq, uint offset, uint length) { var s = i; Fill(seq, offset, length, x => s.read_ulonglong_array(x)); }
        public void read_float_array(IList<float> seq, uint offset, uint length) { var s = i; Fill(seq, offset, length, x => s.read_float_array(x)); }
        public void read_double_array(IList<double> seq, uint offset, uint length) { var s = i; Fill(seq, offset, length, x => s.read_double_array(x)); }

        public void read_any_array(IList<Any> seq, uint offset, uint length)
        {
            var s = i;
            Fill(seq, offset, length, x =>
            {
                for (int k = 0; k < x.Length; k++) x[k] = s.read_any();
            });
        }

        public void read_longdouble_array(IList<decimal> seq, uint offset, uint length)
        {
            var s = i;
            Fill(seq, offset, length, x =>
            {
                for (int k = 0; k < x.Length; k++) x[k] = s.read_longdouble();
            });
        }
    }
}

namespace Rustorb
{
    /// <summary>The value factories and valuetypes of the process, by
    /// repository id, and the reading and writing of values. ORB
    /// register_value_factory and the generated module initialisers (default
    /// factories, TypeCodes) fill it.</summary>
    public static unsafe class Values
    {
        private static readonly ConcurrentDictionary<string, Corba.IValueFactory> factories = new();
        private static readonly ConcurrentDictionary<string, Corba.IValueFactory> defaults = new();
        private static readonly ConcurrentDictionary<string, Func<Corba.TypeCode>> types = new();
        private static int resolverSet;

        /// <summary>ORB::register_value_factory: the factory replaced, or null.</summary>
        public static Corba.IValueFactory? Register(string id, Corba.IValueFactory factory)
        {
            if (id == null || factory == null)
            {
                throw new Corba.BAD_PARAM("a null repository id or factory", 0, Corba.CompletionStatus.COMPLETED_NO);
            }
            Corba.IValueFactory? old = null;
            factories.AddOrUpdate(id, factory, (_, o) =>
            {
                old = o;
                return factory;
            });
            return old;
        }

        public static void Unregister(string id)
        {
            if (id != null)
            {
                factories.TryRemove(id, out _);
            }
        }

        /// <summary>The factory registered for id, else the default one of the
        /// generated code; null if there is none.</summary>
        public static Corba.IValueFactory? Lookup(string? id)
        {
            if (id == null)
            {
                return null;
            }
            if (factories.TryGetValue(id, out var f))
            {
                return f;
            }
            return defaults.TryGetValue(id, out f) ? f : null;
        }

        /// <summary>Generated code: the factory of a valuetype without
        /// operations (its OBV_ class) or of a value box.</summary>
        public static void Default(string id, Func<Corba.IValueBase> make) =>
            defaults[id] = new Corba.ValueFactory(make);

        /// <summary>Generated code: the TypeCode of a valuetype, which the core
        /// asks for when it reads a value of that type in an any where a base
        /// type is expected.</summary>
        public static void Type(string id, Func<Corba.TypeCode> type)
        {
            types[id] = type;
            if (Interlocked.Exchange(ref resolverSet, 1) == 0)
            {
                Ffi.rorb_set_value_type_resolver((nint)(delegate* unmanaged[Cdecl]<nint, byte*, nuint, nint>)&Resolve, 0);
            }
        }

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static nint Resolve(nint context, byte* id, nuint len)
        {
            try
            {
                if (!types.TryGetValue(Strings.FromUtf8(id, len), out var f))
                {
                    return 0;
                }
                Corba.TypeCode tc = f();
                nint h = Ffi.rorb_tc_clone(tc._handle.Raw);
                GC.KeepAlive(tc);
                return h;
            }
            catch (Exception)
            {
                return 0;
            }
        }

        // --- the instances the core keeps -----------------------------------

        /// <summary>Instances handed to the core by number, so that one
        /// instance put in several anys stays one: the core keeps a
        /// reference (counted here) while one of its anys holds the
        /// instance, and an instance has the same number while it is kept.
        /// The streams that read values keep the instances read the same
        /// way, for indirections.</summary>
        internal static class Hosts
        {
            private sealed class Entry
            {
                internal readonly nint id;
                internal readonly object value;
                internal int count;

                internal Entry(nint id, object value)
                {
                    this.id = id;
                    this.value = value;
                }
            }

            private static readonly object gate = new();
            private static readonly Dictionary<object, Entry> byValue = new(ReferenceEqualityComparer.Instance);
            private static readonly Dictionary<nint, Entry> byId = new();
            // Numbers apart from the keys of Key (1 << 62).
            private static long next = 1L << 61;

            internal static readonly delegate* unmanaged[Cdecl]<nint, void> ReleaseFn = &ReleaseCb;

            internal static nint Retain(object v)
            {
                lock (gate)
                {
                    if (!byValue.TryGetValue(v, out Entry? e))
                    {
                        e = new Entry((nint)(++next), v);
                        byValue[v] = e;
                        byId[e.id] = e;
                    }
                    e.count++;
                    return e.id;
                }
            }

            internal static void Release(nint id)
            {
                lock (gate)
                {
                    if (byId.TryGetValue(id, out Entry? e) && --e.count == 0)
                    {
                        byId.Remove(id);
                        byValue.Remove(e.value);
                    }
                }
            }

            internal static object? Get(nint id)
            {
                lock (gate)
                {
                    return byId.TryGetValue(id, out Entry? e) ? e.value : null;
                }
            }

            internal static int Count
            {
                get
                {
                    lock (gate) return byId.Count;
                }
            }

            [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
            private static void ReleaseCb(nint id) => Release(id);
        }

        /// <summary>How many instances the core and the streams keep (for
        /// tests).</summary>
        public static int KeptInstances => Hosts.Count;

        // Keys of the instances written: unique in the process and apart from
        // the addresses the core and C++ use, so that the streams of one
        // message never mix them up.
        private sealed class KeyBox
        {
            internal readonly ulong key = (1UL << 62) | (ulong)Interlocked.Increment(ref nextKey);
        }

        private static long nextKey;
        private static readonly ConditionalWeakTable<object, KeyBox> keys = new();

        private static ulong Key(object v) => keys.GetValue(v, _ => new KeyBox()).key;

        // The repository ids of a type as pinned UTF-8, made once per array.
        private sealed class Ids
        {
            internal readonly GCHandle[] pins;
            internal readonly nint[] ptrs;
            internal readonly nuint[] lens;

            internal Ids(string[] ids)
            {
                pins = new GCHandle[ids.Length];
                ptrs = new nint[ids.Length];
                lens = new nuint[ids.Length];
                for (int k = 0; k < ids.Length; k++)
                {
                    byte[] b = Strings.ToUtf8(ids[k] ?? throw new Corba.BAD_PARAM("a null repository id", 0,
                        Corba.CompletionStatus.COMPLETED_NO));
                    pins[k] = GCHandle.Alloc(b, GCHandleType.Pinned);
                    ptrs[k] = pins[k].AddrOfPinnedObject();
                    lens[k] = (nuint)b.Length;
                }
            }

            ~Ids()
            {
                foreach (GCHandle h in pins)
                {
                    if (h.IsAllocated) h.Free();
                }
            }
        }

        private static readonly ConditionalWeakTable<string[], Ids> idCache = new();

        /// <summary>Writes null, an indirection to an instance written before
        /// in the message, or the value with its repository ids (chunked when
        /// it is truncatable or custom).</summary>
        internal static void Write(CdrOutput o, Corba.IValueBase? v)
        {
            if (v == null)
            {
                Ffi.rorb_out_write_null_value(o.ptr);
                return;
            }
            var custom = v as Corba.ICustomMarshal;
            var plain = v as Corba.ValueBase;
            if (custom == null && plain == null)
            {
                throw new Corba.BAD_PARAM("not a value this ORB can write: " + v.GetType(), 0,
                    Corba.CompletionStatus.COMPLETED_MAYBE);
            }
            string[] idList = v._truncatable_ids();
            if (idList == null || idList.Length == 0)
            {
                throw new Corba.BAD_PARAM("a value without repository ids: " + v.GetType(), 0,
                    Corba.CompletionStatus.COMPLETED_MAYBE);
            }
            Ids ids = idCache.GetValue(idList, a => new Ids(a));
            int chunked = idList.Length > 1 || custom != null ? 1 : 0;
            int state;
            fixed (nint* ip = ids.ptrs)
            fixed (nuint* lp = ids.lens)
            {
                if (Ffi.rorb_out_keeps_hosts(o.ptr) != 0)
                {
                    // Inserting into an any: the core keeps the instance, by a
                    // number that stays the same while it does.
                    nint host = Hosts.Retain(v);
                    state = Ffi.rorb_out_begin_host_value(o.ptr, (ulong)host, host, (nint)Hosts.ReleaseFn,
                        (nuint)idList.Length, (byte**)ip, lp, chunked);
                }
                else
                {
                    state = Ffi.rorb_out_begin_value(o.ptr, Key(v), (nuint)idList.Length, (byte**)ip, lp, chunked);
                }
            }
            GC.KeepAlive(ids);
            if (state == 0)
            {
                return;
            }
            if (custom != null)
            {
                custom.marshal(new Corba.DataOutputStream(o));
            }
            else
            {
                plain!._write(o);
            }
            Ffi.rorb_out_end_value(o.ptr);
        }

        /// <summary>Reads null, an indirection or a new value: an instance made
        /// by the factory of the first repository id received that has one
        /// (expected when none is sent). Without a factory, the stream fails
        /// (MARSHAL).</summary>
        internal static Corba.IValueBase? Read(CdrInput i, string? expected)
        {
            nuint slot, count;
            int kind = Ffi.rorb_in_read_value_header(i.ptr, &slot, &count);
            if (kind == 1) // RORB_VALUE_INDIRECTION
            {
                nint host = Ffi.rorb_in_get_value(i.ptr, slot);
                var v = host == 0 ? null : Hosts.Get(host) as Corba.IValueBase;
                if (v == null)
                {
                    i.fail();
                }
                return v;
            }
            if (kind != 2) // RORB_VALUE_NEW
            {
                return null;
            }
            Corba.IValueFactory? factory = null;
            if (count == 0)
            {
                factory = Lookup(expected);
            }
            for (nuint k = 0; k < count && factory == null; k++)
            {
                nuint len;
                byte* id = Ffi.rorb_in_value_id(i.ptr, k, &len);
                factory = Lookup(Strings.FromUtf8(id, len));
            }
            if (factory == null)
            {
                i.fail();
                return null;
            }
            Corba.IValueBase made = factory.create_for_unmarshal();
            if (made == null)
            {
                i.fail();
                return null;
            }
            // Kept before the state is read, for cycles.
            nint kept = Hosts.Retain(made);
            if (Ffi.rorb_in_keeps_hosts(i.ptr) != 0)
            {
                // Reading an any: the core's instance keeps this one.
                Ffi.rorb_in_set_host_value(i.ptr, slot, (ulong)kept, kept, (nint)Hosts.ReleaseFn);
            }
            else
            {
                Ffi.rorb_in_set_value(i.ptr, slot, kept, (nint)Hosts.ReleaseFn);
            }
            if (made is Corba.ICustomMarshal custom)
            {
                custom.unmarshal(new Corba.DataInputStream(i));
            }
            else if (made is Corba.ValueBase plain)
            {
                plain._read(i);
            }
            else
            {
                i.fail();
                return null;
            }
            Ffi.rorb_in_end_value(i.ptr);
            return made;
        }

        /// <summary>An abstract interface: TRUE and a reference (null is a nil
        /// one), or FALSE and a value.</summary>
        internal static void WriteAbstract(CdrOutput o, object? v)
        {
            if (v == null || v is Corba.IObject)
            {
                o.write_boolean(true);
                o.write_Object((Corba.IObject?)v);
            }
            else if (v is Corba.IValueBase value)
            {
                o.write_boolean(false);
                Write(o, value);
            }
            else
            {
                throw new Corba.BAD_PARAM("neither an object nor a value: " + v.GetType(), 0,
                    Corba.CompletionStatus.COMPLETED_NO);
            }
        }

        internal static object? ReadAbstract(CdrInput i) => i.read_boolean() ? i.read_Object() : Read(i, null);
    }
}
