// Calls through the ORB, client and servant in one process, with the code
// rustorb-idl -bcsharp generates from interop/idl/InteropTest.idl.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using System.Linq;
using InteropTest;
using Omg.Types;
using Xunit;
using Record = InteropTest.Record;

namespace Rustorb.Tests
{
    public sealed class CallbackImpl : CallbackPOA
    {
        public override string notify(string message) => "ack:" + message;
    }

    public sealed class EchoImpl : EchoPOA
    {
        private readonly List<string> posts = new List<string>();
        private ICallback registered;

        public override bool echo_boolean(bool v) => v;
        public override byte echo_octet(byte v) => v;
        public override char echo_char(char v) => v;
        public override char echo_wchar(char v) => v;
        public override short echo_short(short v) => v;
        public override ushort echo_ushort(ushort v) => v;
        public override int echo_long(int v) => v;
        public override uint echo_ulong(uint v) => v;
        public override long echo_longlong(long v) => v;
        public override ulong echo_ulonglong(ulong v) => v;
        public override float echo_float(float v) => v;
        public override double echo_double(double v) => v;
        public override string echo_string(string v) => v;
        public override string echo_wstring(string v) => v;
        public override Color echo_color(Color v) => v;
        public override Point echo_point(Point v) => v;
        public override Record echo_record(Record v) => v;
        public override ISequence<Record> echo_records(ISequence<Record> v) => v;
        public override Value echo_value(Value v) => v;
        public override Shade echo_shade(Shade v) => v;
        public override int[] echo_long_array(int[] v) => v;
        public override double[][] echo_matrix(double[][] v) => v;
        public override ISequence<int> echo_long_seq(ISequence<int> v) => v;
        public override ISequence<string> echo_string_seq(ISequence<string> v) => v;
        public override ISequence<byte> echo_octets(ISequence<byte> v) => v;

        public override int swap(ref int a, ref int b)
        {
            int sum = a + b;
            (a, b) = (b, a);
            return sum;
        }

        public override int divide(int a, int b, out int remainder)
        {
            if (b == 0)
            {
                throw new DivideByZero();
            }
            remainder = a % b;
            return a / b;
        }

        public override void make(int x, out Point p, out Record r)
        {
            p = new Point(x, -x);
            r = new Record(x, "r" + x, Color.BLUE, p, new Sequence<int> { x, x + 1 }, true);
        }

        public override void fail(int code, string reason) => throw new Oops(code, reason);

        public override void fail_system() =>
            throw new Corba.BAD_PARAM(42, Corba.CompletionStatus.COMPLETED_NO);

        public override int counter { get; set; }
        public override string name => "rustorb-interop-echo";

        public override void post(string message)
        {
            lock (posts)
            {
                posts.Add(message);
            }
        }

        public override ISequence<string> posted()
        {
            lock (posts)
            {
                var r = new Sequence<string>(posts);
                posts.Clear();
                return r;
            }
        }

        public override IEcho self_ref() => EchoHelper.narrow(_this_object());
        public override IEcho echo_ref(IEcho other) => other ?? self_ref();
        public override bool is_nil(IEcho other) => other == null;
        public override string call_back(ICallback cb, string message) => cb.notify(message);
        public override void register_callback(ICallback cb) => registered = cb;

        public override string trigger(string message)
        {
            if (registered == null)
            {
                throw new Oops(1, "no callback");
            }
            return registered.notify(message);
        }

        public override void shutdown()
        {
        }
    }

    /// <summary>One ORB with an Echo servant for all the tests.</summary>
    public sealed class EchoFixture : IDisposable
    {
        public Corba.ORB Orb { get; }
        public PortableServer.POA Poa { get; }
        public IEcho Echo { get; }

        public EchoFixture()
        {
            Orb = Corba.ORB.init(new string[0]);
            Poa = PortableServer.POAHelper.narrow(Orb.resolve_initial_references("RootPOA"));
            Poa.the_POAManager.activate();
            Corba.Object o = Poa.servant_to_reference(new EchoImpl());
            // Through a string, as a client in another process would.
            Echo = EchoHelper.narrow(Orb.string_to_object(Orb.object_to_string(o)));
        }

        public void Dispose()
        {
            Orb.shutdown(true);
            Orb.destroy();
        }
    }

    public class EchoTests : IClassFixture<EchoFixture>
    {
        private readonly EchoFixture f;
        private IEcho Echo => f.Echo;

        public EchoTests(EchoFixture fixture)
        {
            f = fixture;
        }

        [Fact]
        public void Basic()
        {
            Assert.True(Echo.echo_boolean(true));
            Assert.Equal((byte)0xfe, Echo.echo_octet(0xfe));
            Assert.Equal('z', Echo.echo_char('z'));
            Assert.Equal('あ', Echo.echo_wchar('あ'));
            Assert.Equal(short.MinValue, Echo.echo_short(short.MinValue));
            Assert.Equal(ushort.MaxValue, Echo.echo_ushort(ushort.MaxValue));
            Assert.Equal(InteropTest.Constants.MAGIC, Echo.echo_long(InteropTest.Constants.MAGIC));
            Assert.Equal(uint.MaxValue, Echo.echo_ulong(uint.MaxValue));
            Assert.Equal(long.MinValue, Echo.echo_longlong(long.MinValue));
            Assert.Equal(ulong.MaxValue, Echo.echo_ulonglong(ulong.MaxValue));
            Assert.Equal(1.5f, Echo.echo_float(1.5f));
            Assert.Equal(Math.PI, Echo.echo_double(Math.PI));
            Assert.Equal("héllo", Echo.echo_string("héllo"));
            Assert.Equal("日本語 \U0001F600", Echo.echo_wstring("日本語 \U0001F600"));
        }

        [Fact]
        public void CharOutOfRange()
        {
            Assert.Throws<Corba.DATA_CONVERSION>(() => Echo.echo_char('あ'));
        }

        [Fact]
        public void Constructed()
        {
            Assert.Equal(Color.GREEN, Echo.echo_color(Color.GREEN));
            var p = new Point(1.5, -2.5);
            Assert.Equal(p, Echo.echo_point(p));
            var r = new Record(7, "seven", Color.RED, p, new Sequence<int> { 1, 2, 3 }, true);
            Record r2 = Echo.echo_record(r);
            Assert.Equal(r, r2);
            Assert.NotSame(r, r2);
            Assert.Equal(r.GetHashCode(), r2.GetHashCode());
            var rs = new Sequence<Record> { r, new Record() };
            Assert.Equal(rs, Echo.echo_records(rs));
            Assert.Equal(new[] { 1, 2, 3, 4 }, Echo.echo_long_array(new[] { 1, 2, 3, 4 }));
            var m = new[] { new[] { 1.0, 2.0, 3.0 }, new[] { 4.0, 5.0, 6.0 } };
            Assert.Equal(m, Echo.echo_matrix(m));
            Assert.Equal(new Sequence<int> { -1, 0, 1 }, Echo.echo_long_seq(new Sequence<int> { -1, 0, 1 }));
            Assert.Equal(new Sequence<string> { "a", "", "c" }, Echo.echo_string_seq(new Sequence<string> { "a", "", "c" }));
            var octets = new Sequence<byte>(Enumerable.Range(0, 1000).Select(i => (byte)i));
            Assert.Equal(octets, Echo.echo_octets(octets));
        }

        [Fact]
        public void Arrays()
        {
            // Checked before sending, like the other bindings of rustorb.
            Assert.Throws<Corba.BAD_PARAM>(() => Echo.echo_long_array(new[] { 1, 2, 3 }));
            Assert.Throws<Corba.BAD_PARAM>(() => Echo.echo_string(null));
        }

        [Fact]
        public void Unions()
        {
            var v = new Value();
            v.l = 5;
            Assert.Equal(5, Echo.echo_value(v).l);
            v.s = "five";
            Assert.Equal("five", Echo.echo_value(v).s);
            v.p = new Point(1, 2);
            Assert.Equal(new Point(1, 2), Echo.echo_value(v).p);
            v.Setseq(new Sequence<int> { 9, 8 }, 5);
            Value v2 = Echo.echo_value(v);
            Assert.Equal(5, v2.Discriminator);
            Assert.Equal(new Sequence<int> { 9, 8 }, v2.seq);
            Assert.Equal(v, v2);
            v.Setflag(true, 77);
            v2 = Echo.echo_value(v);
            Assert.Equal(77, v2.Discriminator);
            Assert.True(v2.flag);
            Assert.Throws<InvalidOperationException>(() => v2.l);
            Assert.Throws<ArgumentException>(() => v.Setl(1, 2));

            var sh = new Shade();
            sh.level = 3;
            Assert.Equal((byte)3, Echo.echo_shade(sh).level);
            sh.label = "green";
            Assert.Equal("green", Echo.echo_shade(sh).label);
            sh._default(Color.BLUE);
            Shade sh2 = Echo.echo_shade(sh);
            Assert.Equal(Color.BLUE, sh2.Discriminator);
        }

        [Fact]
        public void Modes()
        {
            int a = 3, b = 4;
            Assert.Equal(7, Echo.swap(ref a, ref b));
            Assert.Equal(4, a);
            Assert.Equal(3, b);
            Assert.Equal(-3, Echo.divide(-7, 2, out int rem));
            Assert.Equal(-1, rem);
            Assert.Throws<DivideByZero>(() => Echo.divide(1, 0, out _));
            Echo.make(6, out Point p, out Record r);
            Assert.Equal(new Point(6, -6), p);
            Assert.Equal(new Record(6, "r6", Color.BLUE, p, new Sequence<int> { 6, 7 }, true), r);
        }

        [Fact]
        public void Exceptions()
        {
            var e = Assert.Throws<Oops>(() => Echo.fail(13, "unlucky"));
            Assert.Equal(13, e.code);
            Assert.Equal("unlucky", e.reason);
            Assert.Equal("IDL:rustorb.org/InteropTest/Oops:1.0", e._id());
            var s = Assert.Throws<Corba.BAD_PARAM>(() => Echo.fail_system());
            Assert.Equal(42u, s.minor);
            Assert.Equal(Corba.CompletionStatus.COMPLETED_NO, s.completed);
        }

        [Fact]
        public void AttributesAndOneway()
        {
            Echo.counter = 41;
            Assert.Equal(41, Echo.counter);
            Assert.Equal("rustorb-interop-echo", Echo.name);
            Echo.posted();
            Echo.post("one");
            Echo.post("two");
            Assert.Equal(new Sequence<string> { "one", "two" }, Echo.posted());
        }

        [Fact]
        public void References()
        {
            IEcho self = Echo.self_ref();
            Assert.True(self._is_equivalent(Echo));
            Assert.True(Echo.is_nil(null));
            Assert.False(Echo.is_nil(Echo));
            Assert.True(Echo.echo_ref(null)._is_equivalent(Echo));
            Assert.True(Echo._is_a(EchoHelper.id()));
            Assert.False(Echo._is_a(CallbackHelper.id()));
            // narrow of an object of another type raises BAD_PARAM, as in Java.
            Assert.Throws<Corba.BAD_PARAM>(() => CallbackHelper.narrow(Echo));
            Assert.Null(CallbackHelper.narrow(null));
            Assert.NotNull(CallbackHelper.unchecked_narrow(Echo));
        }

        [Fact]
        public void Callbacks()
        {
            ICallback cb = CallbackHelper.narrow(f.Poa.servant_to_reference(new CallbackImpl()));
            Assert.Equal("ack:hi", Echo.call_back(cb, "hi"));
            Echo.register_callback(cb);
            Assert.Equal("ack:later", Echo.trigger("later"));
        }

        [Fact]
        public void Anys()
        {
            var a = new Corba.Any();
            var r = new Record(1, "one", Color.GREEN, new Point(), new Sequence<int>(), false);
            RecordHelper.insert(a, r);
            Assert.True(a.type.equal(RecordHelper.type()));
            Assert.Equal(r, RecordHelper.extract(a));
            Assert.Throws<Corba.BAD_OPERATION>(() => PointHelper.extract(a));
            var v = new Value();
            v.s = "x";
            ValueHelper.insert(a, v);
            Assert.Equal(v, ValueHelper.extract(a));
            Assert.Equal(Corba.TCKind.tk_union, ValueHelper.type().kind());
            Assert.Equal(6u, ValueHelper.type().member_count());
        }
    }
}
