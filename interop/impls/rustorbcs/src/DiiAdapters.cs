// The rustorbcs-dii implementation: the .NET client and server of this
// directory with every call of the basic suite made through the DII and the
// Echo servant run through the DSI.
//
// With RUSTORB_DII set, the client wraps the reference it tests in DiiEcho,
// whose operations build a Corba.Request from the TypeCodes of their
// parameters, and the server activates its servant as DsiEcho: a
// PortableServer.DynamicImplementation that reads the arguments with
// Corba.ServerRequest.arguments and calls the static servant's operation.
// The messages are those of the static stubs and skeletons. (The callback
// objects of the client, and the calls the servant makes, stay static.)
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

#nullable enable

using System;
using Corba;
using InteropTest;
using Omg.Types;
using Record = InteropTest.Record;
using TypeCode = Corba.TypeCode;

namespace Interop
{
    /// <summary>How a value of type T goes in and out of an any.</summary>
    internal sealed class Codec<T>
    {
        internal readonly Func<TypeCode> Tc;
        internal readonly Action<Any, T> Insert;
        internal readonly Func<Any, T> Extract;

        internal Codec(Func<TypeCode> tc, Action<Any, T> insert, Func<Any, T> extract)
        {
            Tc = tc;
            Insert = insert;
            Extract = extract;
        }

        internal Any Of(T v)
        {
            var a = new Any();
            Insert(a, v);
            return a;
        }
    }

    internal static class Codecs
    {
        private static TypeCode P(TCKind k) => Rustorb.TypeCodes.Simple(k);

        internal static readonly Codec<bool> Boolean = new(() => P(TCKind.tk_boolean), (a, v) => a.insert_boolean(v), a => a.extract_boolean());
        internal static readonly Codec<byte> Octet = new(() => P(TCKind.tk_octet), (a, v) => a.insert_octet(v), a => a.extract_octet());
        internal static readonly Codec<char> Char = new(() => P(TCKind.tk_char), (a, v) => a.insert_char(v), a => a.extract_char());
        internal static readonly Codec<char> WChar = new(() => P(TCKind.tk_wchar), (a, v) => a.insert_wchar(v), a => a.extract_wchar());
        internal static readonly Codec<short> Short = new(() => P(TCKind.tk_short), (a, v) => a.insert_short(v), a => a.extract_short());
        internal static readonly Codec<ushort> UShort = new(() => P(TCKind.tk_ushort), (a, v) => a.insert_ushort(v), a => a.extract_ushort());
        internal static readonly Codec<int> Long = new(() => P(TCKind.tk_long), (a, v) => a.insert_long(v), a => a.extract_long());
        internal static readonly Codec<uint> ULong = new(() => P(TCKind.tk_ulong), (a, v) => a.insert_ulong(v), a => a.extract_ulong());
        internal static readonly Codec<long> LongLong = new(() => P(TCKind.tk_longlong), (a, v) => a.insert_longlong(v), a => a.extract_longlong());
        internal static readonly Codec<ulong> ULongLong = new(() => P(TCKind.tk_ulonglong), (a, v) => a.insert_ulonglong(v), a => a.extract_ulonglong());
        internal static readonly Codec<float> Float = new(() => P(TCKind.tk_float), (a, v) => a.insert_float(v), a => a.extract_float());
        internal static readonly Codec<double> Double = new(() => P(TCKind.tk_double), (a, v) => a.insert_double(v), a => a.extract_double());
        internal static readonly Codec<string> String = new(() => P(TCKind.tk_string), (a, v) => a.insert_string(v), a => a.extract_string());
        internal static readonly Codec<string> WString = new(() => P(TCKind.tk_wstring), (a, v) => a.insert_wstring(v), a => a.extract_wstring());
        internal static readonly Codec<Color> Color = new(ColorHelper.type, ColorHelper.insert, ColorHelper.extract);
        internal static readonly Codec<Point> Point = new(PointHelper.type, PointHelper.insert, PointHelper.extract);
        internal static readonly Codec<Record> Record = new(RecordHelper.type, RecordHelper.insert, RecordHelper.extract);
        internal static readonly Codec<ISequence<Record>> Records = new(RecordSeqHelper.type, RecordSeqHelper.insert, RecordSeqHelper.extract);
        internal static readonly Codec<Value> Value = new(ValueHelper.type, ValueHelper.insert, ValueHelper.extract);
        internal static readonly Codec<Shade> Shade = new(ShadeHelper.type, ShadeHelper.insert, ShadeHelper.extract);
        internal static readonly Codec<int[]> LongArray = new(LongArrayHelper.type, LongArrayHelper.insert, LongArrayHelper.extract);
        internal static readonly Codec<double[][]> Matrix = new(MatrixHelper.type, MatrixHelper.insert, MatrixHelper.extract);
        internal static readonly Codec<ISequence<int>> LongSeq = new(LongSeqHelper.type, LongSeqHelper.insert, LongSeqHelper.extract);
        internal static readonly Codec<ISequence<string>> StringSeq = new(StringSeqHelper.type, StringSeqHelper.insert, StringSeqHelper.extract);
        internal static readonly Codec<ISequence<byte>> OctetSeq = new(OctetSeqHelper.type, OctetSeqHelper.insert, OctetSeqHelper.extract);
        internal static readonly Codec<IEcho?> Echo = new(EchoHelper.type, EchoHelper.insert, EchoHelper.extract);
        internal static readonly Codec<ICallback?> Callback = new(CallbackHelper.type, CallbackHelper.insert, CallbackHelper.extract);
    }

    // --- the client side ----------------------------------------------------------

    /// <summary>A call of an operation through a Corba.Request.</summary>
    internal sealed class Call
    {
        private readonly Request req;

        internal Call(Corba.Object target, string op)
        {
            req = target._request(op);
        }

        internal Call In<T>(Codec<T> c, T v)
        {
            c.Insert(req.add_in_arg(), v);
            return this;
        }

        internal Call InOut<T>(Codec<T> c, T v)
        {
            c.Insert(req.add_inout_arg(), v);
            return this;
        }

        /// <summary>The value only gives the type.</summary>
        internal Call Out<T>(Codec<T> c)
        {
            req.add_out_arg().type = c.Tc();
            return this;
        }

        internal Call Returns<T>(Codec<T> c)
        {
            req.set_return_type(c.Tc());
            return this;
        }

        /// <summary>Invokes, raising the user exceptions of the list as themselves.</summary>
        internal Call Invoke(params (Func<TypeCode> tc, Func<Any, System.Exception> extract)[] exceptions)
        {
            foreach (var e in exceptions)
            {
                req.exceptions.add(e.tc());
            }
            try
            {
                req.invoke();
            }
            catch (UnknownUserException u)
            {
                foreach (var e in exceptions)
                {
                    if (u.except.type.equal(e.tc()))
                    {
                        throw e.extract(u.except);
                    }
                }
                throw;
            }
            return this;
        }

        internal void Oneway() => req.send_oneway();

        internal T Result<T>(Codec<T> c) => c.Extract(req.return_value());

        internal T Arg<T>(Codec<T> c, uint i) => c.Extract(req.arguments.item(i).value);
    }

    /// <summary>Echo through the DII.</summary>
    internal sealed class DiiEcho : Corba.Object, IEcho
    {
        private static readonly (Func<TypeCode>, Func<Any, System.Exception>) Oops = (OopsHelper.type, a => OopsHelper.extract(a));
        private static readonly (Func<TypeCode>, Func<Any, System.Exception>) DivideByZero = (DivideByZeroHelper.type, a => DivideByZeroHelper.extract(a));

        internal DiiEcho(Corba.Object reference) : base(reference)
        {
        }

        internal static IEcho? Wrap(IEcho? e) => e == null ? null : new DiiEcho((Corba.Object)e);

        private T Echo<T>(string op, Codec<T> c, T v) => new Call(this, op).In(c, v).Returns(c).Invoke().Result(c);

        public bool echo_boolean(bool v) => Echo("echo_boolean", Codecs.Boolean, v);
        public byte echo_octet(byte v) => Echo("echo_octet", Codecs.Octet, v);
        public char echo_char(char v) => Echo("echo_char", Codecs.Char, v);
        public char echo_wchar(char v) => Echo("echo_wchar", Codecs.WChar, v);
        public short echo_short(short v) => Echo("echo_short", Codecs.Short, v);
        public ushort echo_ushort(ushort v) => Echo("echo_ushort", Codecs.UShort, v);
        public int echo_long(int v) => Echo("echo_long", Codecs.Long, v);
        public uint echo_ulong(uint v) => Echo("echo_ulong", Codecs.ULong, v);
        public long echo_longlong(long v) => Echo("echo_longlong", Codecs.LongLong, v);
        public ulong echo_ulonglong(ulong v) => Echo("echo_ulonglong", Codecs.ULongLong, v);
        public float echo_float(float v) => Echo("echo_float", Codecs.Float, v);
        public double echo_double(double v) => Echo("echo_double", Codecs.Double, v);
        public string echo_string(string v) => Echo("echo_string", Codecs.String, v);
        public string echo_wstring(string v) => Echo("echo_wstring", Codecs.WString, v);
        public Color echo_color(Color v) => Echo("echo_color", Codecs.Color, v);
        public Point echo_point(Point v) => Echo("echo_point", Codecs.Point, v);
        public Record echo_record(Record v) => Echo("echo_record", Codecs.Record, v);
        public ISequence<Record> echo_records(ISequence<Record> v) => Echo("echo_records", Codecs.Records, v);
        public Value echo_value(Value v) => Echo("echo_value", Codecs.Value, v);
        public Shade echo_shade(Shade v) => Echo("echo_shade", Codecs.Shade, v);
        public int[] echo_long_array(int[] v) => Echo("echo_long_array", Codecs.LongArray, v);
        public double[][] echo_matrix(double[][] v) => Echo("echo_matrix", Codecs.Matrix, v);
        public ISequence<int> echo_long_seq(ISequence<int> v) => Echo("echo_long_seq", Codecs.LongSeq, v);
        public ISequence<string> echo_string_seq(ISequence<string> v) => Echo("echo_string_seq", Codecs.StringSeq, v);
        public ISequence<byte> echo_octets(ISequence<byte> v) => Echo("echo_octets", Codecs.OctetSeq, v);

        public int swap(ref int a, ref int b)
        {
            var c = new Call(this, "swap").InOut(Codecs.Long, a).InOut(Codecs.Long, b).Returns(Codecs.Long).Invoke();
            a = c.Arg(Codecs.Long, 0);
            b = c.Arg(Codecs.Long, 1);
            return c.Result(Codecs.Long);
        }

        public int divide(int a, int b, out int remainder)
        {
            var c = new Call(this, "divide").In(Codecs.Long, a).In(Codecs.Long, b).Out(Codecs.Long)
                .Returns(Codecs.Long).Invoke(DivideByZero);
            remainder = c.Arg(Codecs.Long, 2);
            return c.Result(Codecs.Long);
        }

        public void make(int x, out Point p, out Record r)
        {
            var c = new Call(this, "make").In(Codecs.Long, x).Out(Codecs.Point).Out(Codecs.Record).Invoke();
            p = c.Arg(Codecs.Point, 1);
            r = c.Arg(Codecs.Record, 2);
        }

        public void fail(int code, string reason) =>
            new Call(this, "fail").In(Codecs.Long, code).In(Codecs.String, reason).Invoke(Oops);

        public void fail_system() => new Call(this, "fail_system").Invoke();

        public int counter
        {
            get => new Call(this, "_get_counter").Returns(Codecs.Long).Invoke().Result(Codecs.Long);
            set => new Call(this, "_set_counter").In(Codecs.Long, value).Invoke();
        }

        public string name => new Call(this, "_get_name").Returns(Codecs.String).Invoke().Result(Codecs.String);

        public void post(string message) => new Call(this, "post").In(Codecs.String, message).Oneway();

        public ISequence<string> posted() =>
            new Call(this, "posted").Returns(Codecs.StringSeq).Invoke().Result(Codecs.StringSeq);

        public IEcho? self_ref() =>
            Wrap(new Call(this, "self_ref").Returns(Codecs.Echo).Invoke().Result(Codecs.Echo));

        public IEcho? echo_ref(IEcho? other) =>
            Wrap(new Call(this, "echo_ref").In(Codecs.Echo, other).Returns(Codecs.Echo).Invoke().Result(Codecs.Echo));

        public bool is_nil(IEcho? other) =>
            new Call(this, "is_nil").In(Codecs.Echo, other).Returns(Codecs.Boolean).Invoke().Result(Codecs.Boolean);

        public string call_back(ICallback? cb, string message) =>
            new Call(this, "call_back").In(Codecs.Callback, cb).In(Codecs.String, message).Returns(Codecs.String)
                .Invoke().Result(Codecs.String);

        public void register_callback(ICallback? cb) => new Call(this, "register_callback").In(Codecs.Callback, cb).Invoke();

        public string trigger(string message) =>
            new Call(this, "trigger").In(Codecs.String, message).Returns(Codecs.String).Invoke(Oops)
                .Result(Codecs.String);

        public void shutdown() => new Call(this, "shutdown").Oneway();
    }

    // --- the server side ----------------------------------------------------------

    /// <summary>Reads the parameters of an operation and sets its results
    /// through a Corba.ServerRequest.</summary>
    internal sealed class Serve
    {
        private readonly ServerRequest r;
        private readonly NVList parameters = new();

        internal Serve(ServerRequest r)
        {
            this.r = r;
        }

        private Serve Add<T>(Codec<T> c, uint flags)
        {
            var a = new Any();
            a.type = c.Tc();
            parameters.add_value("", a, flags);
            return this;
        }

        internal Serve In<T>(Codec<T> c) => Add(c, Corba.Constants.ARG_IN);
        internal Serve InOut<T>(Codec<T> c) => Add(c, Corba.Constants.ARG_INOUT);
        internal Serve Out<T>(Codec<T> c) => Add(c, Corba.Constants.ARG_OUT);

        internal Serve Read()
        {
            r.arguments(parameters);
            return this;
        }

        internal T Arg<T>(Codec<T> c, uint i) => c.Extract(parameters.item(i).value);
        internal void Set<T>(Codec<T> c, uint i, T v) => c.Insert(parameters.item(i).value, v);
        internal void Result<T>(Codec<T> c, T v) => r.set_result(c.Of(v));

        /// <summary>Runs f, replying with the user exceptions it raises.</summary>
        internal void Run(Action f)
        {
            try
            {
                f();
            }
            catch (Oops e)
            {
                var a = new Any();
                OopsHelper.insert(a, e);
                r.set_exception(a);
            }
            catch (DivideByZero e)
            {
                var a = new Any();
                DivideByZeroHelper.insert(a, e);
                r.set_exception(a);
            }
        }
    }

    /// <summary>The Echo servant behind the DSI.</summary>
    internal sealed class DsiEcho : PortableServer.DynamicImplementation
    {
        private readonly EchoImpl impl;

        internal DsiEcho(EchoImpl impl, PortableServer.Current current)
        {
            this.impl = impl;
            impl.Self = () => EchoHelper.narrow(current.get_reference())!;
        }

        public override string _primary_interface(byte[]? oid, PortableServer.POA? poa) => EchoHelper.id();

        private static bool Echo<T>(ServerRequest r, Codec<T> c, Func<T, T> f)
        {
            var s = new Serve(r).In(c).Read();
            s.Result(c, f(s.Arg(c, 0)));
            return true;
        }

        public override void invoke(ServerRequest r)
        {
            if (!Dispatch(r))
            {
                throw new BAD_OPERATION("", 0x4f4d0002, CompletionStatus.COMPLETED_NO);
            }
        }

        private bool Dispatch(ServerRequest r)
        {
            switch (r.operation)
            {
                case "echo_boolean": return Echo(r, Codecs.Boolean, impl.echo_boolean);
                case "echo_octet": return Echo(r, Codecs.Octet, impl.echo_octet);
                case "echo_char": return Echo(r, Codecs.Char, impl.echo_char);
                case "echo_wchar": return Echo(r, Codecs.WChar, impl.echo_wchar);
                case "echo_short": return Echo(r, Codecs.Short, impl.echo_short);
                case "echo_ushort": return Echo(r, Codecs.UShort, impl.echo_ushort);
                case "echo_long": return Echo(r, Codecs.Long, impl.echo_long);
                case "echo_ulong": return Echo(r, Codecs.ULong, impl.echo_ulong);
                case "echo_longlong": return Echo(r, Codecs.LongLong, impl.echo_longlong);
                case "echo_ulonglong": return Echo(r, Codecs.ULongLong, impl.echo_ulonglong);
                case "echo_float": return Echo(r, Codecs.Float, impl.echo_float);
                case "echo_double": return Echo(r, Codecs.Double, impl.echo_double);
                case "echo_string": return Echo(r, Codecs.String, impl.echo_string);
                case "echo_wstring": return Echo(r, Codecs.WString, impl.echo_wstring);
                case "echo_color": return Echo(r, Codecs.Color, impl.echo_color);
                case "echo_point": return Echo(r, Codecs.Point, impl.echo_point);
                case "echo_record": return Echo(r, Codecs.Record, impl.echo_record);
                case "echo_records": return Echo(r, Codecs.Records, impl.echo_records);
                case "echo_value": return Echo(r, Codecs.Value, impl.echo_value);
                case "echo_shade": return Echo(r, Codecs.Shade, impl.echo_shade);
                case "echo_long_array": return Echo(r, Codecs.LongArray, impl.echo_long_array);
                case "echo_matrix": return Echo(r, Codecs.Matrix, impl.echo_matrix);
                case "echo_long_seq": return Echo(r, Codecs.LongSeq, impl.echo_long_seq);
                case "echo_string_seq": return Echo(r, Codecs.StringSeq, impl.echo_string_seq);
                case "echo_octets": return Echo(r, Codecs.OctetSeq, impl.echo_octets);
                case "echo_ref": return Echo(r, Codecs.Echo, impl.echo_ref);
            }
            var s = new Serve(r);
            switch (r.operation)
            {
                case "swap":
                    {
                        s.InOut(Codecs.Long).InOut(Codecs.Long).Read();
                        int a = s.Arg(Codecs.Long, 0), b = s.Arg(Codecs.Long, 1);
                        int sum = impl.swap(ref a, ref b);
                        s.Set(Codecs.Long, 0, a);
                        s.Set(Codecs.Long, 1, b);
                        s.Result(Codecs.Long, sum);
                        return true;
                    }
                case "divide":
                    s.In(Codecs.Long).In(Codecs.Long).Out(Codecs.Long).Read();
                    s.Run(() =>
                    {
                        int q = impl.divide(s.Arg(Codecs.Long, 0), s.Arg(Codecs.Long, 1), out int rem);
                        s.Set(Codecs.Long, 2, rem);
                        s.Result(Codecs.Long, q);
                    });
                    return true;
                case "make":
                    {
                        s.In(Codecs.Long).Out(Codecs.Point).Out(Codecs.Record).Read();
                        impl.make(s.Arg(Codecs.Long, 0), out Point p, out Record rec);
                        s.Set(Codecs.Point, 1, p);
                        s.Set(Codecs.Record, 2, rec);
                        return true;
                    }
                case "fail":
                    s.In(Codecs.Long).In(Codecs.String).Read();
                    s.Run(() => impl.fail(s.Arg(Codecs.Long, 0), s.Arg(Codecs.String, 1)));
                    return true;
                case "fail_system":
                    s.Read();
                    impl.fail_system();
                    return true;
                case "_get_counter":
                    s.Read();
                    s.Result(Codecs.Long, impl.counter);
                    return true;
                case "_set_counter":
                    s.In(Codecs.Long).Read();
                    impl.counter = s.Arg(Codecs.Long, 0);
                    return true;
                case "_get_name":
                    s.Read();
                    s.Result(Codecs.String, impl.name);
                    return true;
                case "post":
                    s.In(Codecs.String).Read();
                    impl.post(s.Arg(Codecs.String, 0));
                    return true;
                case "posted":
                    s.Read();
                    s.Result(Codecs.StringSeq, impl.posted());
                    return true;
                case "self_ref":
                    s.Read();
                    s.Result(Codecs.Echo, impl.self_ref());
                    return true;
                case "is_nil":
                    s.In(Codecs.Echo).Read();
                    s.Result(Codecs.Boolean, impl.is_nil(s.Arg(Codecs.Echo, 0)));
                    return true;
                case "call_back":
                    s.In(Codecs.Callback).In(Codecs.String).Read();
                    s.Result(Codecs.String, impl.call_back(s.Arg(Codecs.Callback, 0), s.Arg(Codecs.String, 1)));
                    return true;
                case "register_callback":
                    s.In(Codecs.Callback).Read();
                    impl.register_callback(s.Arg(Codecs.Callback, 0));
                    return true;
                case "trigger":
                    s.In(Codecs.String).Read();
                    s.Run(() => s.Result(Codecs.String, impl.trigger(s.Arg(Codecs.String, 0))));
                    return true;
                case "shutdown":
                    s.Read();
                    impl.shutdown();
                    return true;
                default:
                    return false;
            }
        }
    }

    internal static class Dii
    {
        internal static bool Enabled
        {
            get
            {
                string? v = System.Environment.GetEnvironmentVariable("RUSTORB_DII");
                return !string.IsNullOrEmpty(v) && v != "0";
            }
        }
    }
}
