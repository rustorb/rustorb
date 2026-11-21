// The InteropTest::Echo servant of the .NET server, on the IDL4 to C#
// mapping. Behaves as the comments in idl/InteropTest.idl say.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System.Collections.Generic;
using InteropTest;
using Omg.Types;
using Record = InteropTest.Record;

namespace Interop
{
    public sealed class EchoImpl : EchoPOA
    {
        private readonly Corba.ORB orb;
        private readonly object gate = new object();
        private int count;
        private List<string> posts = new List<string>();
        private ICallback callback;

        public EchoImpl(Corba.ORB orb)
        {
            this.orb = orb;
        }

        // --- basic types ---
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

        // --- constructed types ---
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

        // --- parameter modes ---
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
            r = new Record(x, "r" + x, Color.BLUE, new Point(x, -x), new Sequence<int> { x, x + 1 }, true);
        }

        // --- exceptions ---
        public override void fail(int code, string reason) => throw new Oops(code, reason);

        public override void fail_system() =>
            throw new Corba.BAD_PARAM(42, Corba.CompletionStatus.COMPLETED_NO);

        // --- attributes ---
        public override int counter
        {
            get { lock (gate) { return count; } }
            set { lock (gate) { count = value; } }
        }

        public override string name => "rustorb-interop-echo";

        // --- oneway ---
        public override void post(string message)
        {
            lock (gate)
            {
                posts.Add(message);
            }
        }

        public override ISequence<string> posted()
        {
            lock (gate)
            {
                var r = new Sequence<string>(posts);
                posts = new List<string>();
                return r;
            }
        }

        // --- object references ---
        public override IEcho self_ref() => EchoHelper.narrow(_this_object());
        public override IEcho echo_ref(IEcho other) => other ?? self_ref();
        public override bool is_nil(IEcho other) => other == null;

        // --- callbacks ---
        public override string call_back(ICallback cb, string message) => cb.notify(message);

        public override void register_callback(ICallback cb)
        {
            lock (gate)
            {
                callback = cb;
            }
        }

        public override string trigger(string message)
        {
            ICallback cb;
            lock (gate)
            {
                cb = callback;
            }
            if (cb == null)
            {
                throw new Oops(1, "no callback");
            }
            return cb.notify(message);
        }

        // --- lifecycle ---
        public override void shutdown() => orb.shutdown(false);
    }
}
