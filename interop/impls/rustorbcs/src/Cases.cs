// The test cases of the .NET client, the same as those of the other clients
// (case names included), on the IDL4 to C# mapping.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Threading;
using InteropTest;
using Omg.Types;
using Record = InteropTest.Record;

namespace Interop
{
    public sealed class CheckFailed : Exception
    {
        public CheckFailed(string message) : base(message)
        {
        }
    }

    public sealed class Runner
    {
        private int passed;
        public readonly List<string> Failed = new List<string>();

        public void Run(string name, Action fn)
        {
            try
            {
                fn();
                ++passed;
                Console.WriteLine("ok   " + name);
            }
            catch (CheckFailed e)
            {
                Fail(name, e.Message);
            }
            catch (Corba.SystemException e)
            {
                Fail(name, "unexpected " + e.GetType().Name + " (minor " + e.minor + ", " + (int)e.completed + ")");
            }
            catch (Exception e)
            {
                if (Environment.GetEnvironmentVariable("INTEROP_TRACE") != null)
                {
                    Console.Error.WriteLine(e);
                }
                Fail(name, "unexpected " + e.GetType().FullName + ": " + e.Message);
            }
        }

        private void Fail(string name, string why)
        {
            Failed.Add(name);
            Console.WriteLine("FAIL " + name + ": " + why);
        }

        public void Summary()
        {
            Console.WriteLine();
            Console.WriteLine(passed + " passed, " + Failed.Count + " failed");
            if (Failed.Count > 0)
            {
                Console.WriteLine("failed: " + string.Join(", ", Failed));
            }
            Console.Out.Flush();
        }
    }

    public sealed class CallbackImpl : CallbackPOA
    {
        public readonly List<string> Received = new List<string>();

        public override string notify(string message)
        {
            lock (Received)
            {
                Received.Add(message);
            }
            return "ack:" + message;
        }
    }

    public static class Cases
    {
        private static void Check(bool cond, string what)
        {
            if (!cond)
            {
                throw new CheckFailed(what);
            }
        }

        private static CheckFailed NotRaised() => new CheckFailed("expected exception was not raised");

        private static void EchoValues<T>(Runner r, string op, Func<T, T> call, params T[] values)
        {
            for (int i = 0; i < values.Length; ++i)
            {
                T v = values[i];
                r.Run(op + "[" + i + "]", () =>
                {
                    T got = call(v);
                    Check(EqualityComparer<T>.Default.Equals(v, got), op + " returned " + got);
                });
            }
        }

        private static Record MakeRecord(int id, string name, Color c, Point pos, int[] values, bool active) =>
            new Record(id, name, c, pos, new Sequence<int>(values), active);

        /// <summary>Runs every case against echo; poa is where the callback
        /// object goes.</summary>
        public static void Run(Runner r, IEcho echo, PortableServer.POA poa)
        {
            // --- basic types ---
            EchoValues(r, "echo_boolean", echo.echo_boolean, true, false);
            EchoValues(r, "echo_octet", echo.echo_octet, (byte)0, (byte)0x7f, (byte)0xff);
            EchoValues(r, "echo_char", echo.echo_char, 'a', '\0', '~');
            EchoValues(r, "echo_wchar", echo.echo_wchar, 'a', 'あ', 'é');
            EchoValues(r, "echo_short", echo.echo_short, (short)0, short.MinValue, short.MaxValue);
            EchoValues(r, "echo_ushort", echo.echo_ushort, (ushort)0, ushort.MaxValue);
            EchoValues(r, "echo_long", echo.echo_long, 0, int.MinValue, int.MaxValue, InteropTest.Constants.MAGIC);
            EchoValues(r, "echo_ulong", echo.echo_ulong, 0u, uint.MaxValue);
            EchoValues(r, "echo_longlong", echo.echo_longlong, 0L, long.MinValue, long.MaxValue);
            EchoValues(r, "echo_ulonglong", echo.echo_ulonglong, 0UL, ulong.MaxValue);
            EchoValues(r, "echo_double", echo.echo_double, 0.0, -1.5, 1e300, double.Epsilon);
            EchoValues(r, "echo_string", echo.echo_string, "", "hello", new string('x', 10000), "café");
            // U+1F980 is sent as a surrogate pair, as the other clients do.
            EchoValues(r, "echo_wstring", echo.echo_wstring,
                "", "hello", "こんにちは", "🦀");
            EchoValues(r, "echo_float", echo.echo_float, 0.0f, -1.5f, float.MaxValue, 1.17549435e-38f);

            // --- constructed types ---
            var p = new Point(1.25, -2.5);
            Record rec = MakeRecord(7, "seven", Color.GREEN, p, new[] { 1, -2, 3 }, true);
            Record rec2 = MakeRecord(-1, "", Color.RED, new Point(0.0, 0.0), new int[0], false);

            r.Run("echo_color", () =>
            {
                foreach (Color c in new[] { Color.RED, Color.GREEN, Color.BLUE })
                {
                    Check(echo.echo_color(c) == c, "echo_color " + c);
                }
            });
            r.Run("echo_point", () => Check(echo.echo_point(p).Equals(p), "echo_point"));
            r.Run("echo_record", () => Check(echo.echo_record(rec).Equals(rec), "echo_record"));
            r.Run("echo_records", () =>
            {
                var seq = new Sequence<Record> { rec, rec2, rec };
                Check(echo.echo_records(seq).SequenceEqual(seq), "echo_records");
            });
            r.Run("echo_records[empty]", () => Check(echo.echo_records(new Sequence<Record>()).Count == 0, "empty"));
            // Large enough to be sent in several fragments, with doubles
            // after each fragment boundary; record i has a name of varying
            // length so that the doubles land at every alignment.
            r.Run("echo_records[large]", () =>
            {
                Color[] colors = { Color.RED, Color.GREEN, Color.BLUE };
                var seq = new Sequence<Record>();
                for (int i = 0; i < 400; ++i)
                {
                    seq.Add(MakeRecord(i, "r" + i, colors[i % 3], new Point(i + 0.5, -0.25 * i),
                        new[] { i, i + 1 }, i % 2 == 0));
                }
                Check(echo.echo_records(seq).SequenceEqual(seq), "echo_records[large]");
            });

            // Each value is made inside its case, so that a mapping that
            // refuses one (see known-failures.txt) fails only that case.
            var values = new List<Action<Value>>
            {
                v => v.l = -5,
                v => v.s = "union",
                v => v.p = p,
                v => v.Setseq(new Sequence<int> { 1, 2 }, 4),
                v => v.Setseq(new Sequence<int>(), 5),
                v => v.Setflag(true, 99),
                v => v.Setflag(false, 0),
            };
            for (int i = 0; i < values.Count; ++i)
            {
                Action<Value> set = values[i];
                r.Run("echo_value[" + i + "]", () =>
                {
                    var v = new Value();
                    set(v);
                    Check(echo.echo_value(v).Equals(v), "echo_value");
                });
            }

            var shades = new[] { new Shade(), new Shade(), new Shade() };
            shades[0].level = 200;
            shades[1].label = "leaf";
            shades[2]._default();
            for (int i = 0; i < shades.Length; ++i)
            {
                Shade s = shades[i];
                bool implicitDefault = i == 2;
                r.Run("echo_shade[" + i + "]", () =>
                {
                    if (implicitDefault)
                    {
                        Check(s.Discriminator == Color.BLUE, "implicit default");
                    }
                    Check(echo.echo_shade(s).Equals(s), "echo_shade");
                });
            }

            r.Run("echo_long_array", () =>
            {
                int[] a = { 1, -1, 2147483647, 0 };
                Check(echo.echo_long_array(a).SequenceEqual(a), "echo_long_array");
            });
            r.Run("echo_matrix", () =>
            {
                double[][] m = { new[] { 1.0, 2.0, 3.0 }, new[] { -4.0, 0.5, 1e-9 } };
                double[][] got = echo.echo_matrix(m);
                Check(got.Length == 2 && got[0].SequenceEqual(m[0]) && got[1].SequenceEqual(m[1]), "echo_matrix");
            });
            r.Run("echo_long_seq", () =>
            {
                var seq = new Sequence<int>(Enumerable.Range(0, 1000).Select(i => i - 500));
                Check(echo.echo_long_seq(seq).SequenceEqual(seq), "echo_long_seq");
            });
            r.Run("echo_long_seq[empty]", () => Check(echo.echo_long_seq(new Sequence<int>()).Count == 0, "empty"));
            r.Run("echo_string_seq", () =>
            {
                var seq = new Sequence<string> { "a", "", "ccc" };
                Check(echo.echo_string_seq(seq).SequenceEqual(seq), "echo_string_seq");
            });
            r.Run("echo_octets", () =>
            {
                var seq = new Sequence<byte>(Enumerable.Range(0, 256).Select(i => (byte)i));
                Check(echo.echo_octets(seq).SequenceEqual(seq), "echo_octets");
            });
            r.Run("echo_octets[large]", () =>
            {
                var seq = new Sequence<byte>(Enumerable.Range(0, 300000).Select(i => (byte)(i % 251)));
                Check(echo.echo_octets(seq).SequenceEqual(seq), "echo_octets[large]");
            });

            // --- parameter modes ---
            r.Run("swap", () =>
            {
                int a = 3, b = -10;
                Check(echo.swap(ref a, ref b) == -7, "swap result");
                Check(a == -10 && b == 3, "swap inout");
            });
            r.Run("divide", () =>
            {
                Check(echo.divide(17, 5, out int rem) == 3 && rem == 2, "17 / 5");
                Check(echo.divide(-17, 5, out rem) == -3 && rem == -2, "-17 / 5");
                try
                {
                    echo.divide(1, 0, out rem);
                    throw NotRaised();
                }
                catch (DivideByZero)
                {
                    // expected
                }
            });
            r.Run("make", () =>
            {
                echo.make(12, out Point pt, out Record rc);
                Check(pt.Equals(new Point(12.0, -12.0)), "make p");
                Check(rc.Equals(MakeRecord(12, "r12", Color.BLUE, new Point(12.0, -12.0), new[] { 12, 13 }, true)),
                    "make r");
            });

            // --- exceptions ---
            r.Run("fail", () =>
            {
                try
                {
                    echo.fail(7, "boom");
                    throw NotRaised();
                }
                catch (Oops e)
                {
                    Check(e.code == 7 && e.reason == "boom", "Oops members");
                }
            });
            r.Run("fail_system", () =>
            {
                try
                {
                    echo.fail_system();
                    throw NotRaised();
                }
                catch (Corba.BAD_PARAM e)
                {
                    Check(e.minor == 42 && e.completed == Corba.CompletionStatus.COMPLETED_NO,
                        "BAD_PARAM minor " + e.minor);
                }
            });

            // --- attributes ---
            r.Run("attribute counter", () =>
            {
                echo.counter = 41;
                Check(echo.counter == 41, "41");
                echo.counter = -1;
                Check(echo.counter == -1, "-1");
            });
            r.Run("attribute name", () => Check(echo.name == "rustorb-interop-echo", "name"));

            // --- object references ---
            r.Run("self_ref", () =>
            {
                IEcho self = echo.self_ref();
                Check(self._is_equivalent(echo), "equivalent");
                Check(self.echo_long(5) == 5, "callable");
            });
            r.Run("echo_ref", () =>
            {
                Check(echo.echo_ref(echo)._is_equivalent(echo), "echo_ref(echo)");
                Check(echo.echo_ref(null)._is_equivalent(echo), "echo_ref(nil)");
            });
            r.Run("is_nil", () =>
            {
                Check(echo.is_nil(null), "nil");
                Check(!echo.is_nil(echo), "non-nil");
            });

            // --- callbacks ---
            var cbServant = new CallbackImpl();
            byte[] oid = Common.Oid("Callback");
            poa.activate_object_with_id(oid, cbServant);
            ICallback cb = CallbackHelper.narrow(poa.id_to_reference(oid));

            r.Run("call_back", () => Check(echo.call_back(cb, "hi") == "ack:hi", "call_back"));
            r.Run("register_callback/trigger", () =>
            {
                echo.register_callback(cb);
                Check(echo.trigger("later") == "ack:later", "trigger");
                lock (cbServant.Received)
                {
                    Check(cbServant.Received.Count > 0 && cbServant.Received[^1] == "later", "received");
                }
            });

            // --- oneway ---
            // Last, as in the other clients (the number of posted() polls
            // depends on timing).
            r.Run("oneway post", () =>
            {
                echo.posted();
                var sent = new List<string> { "one", "two", "three" };
                foreach (string m in sent)
                {
                    echo.post(m);
                }
                var got = new List<string>();
                var clock = Stopwatch.StartNew();
                while (got.Count < sent.Count && clock.Elapsed.TotalSeconds < 5)
                {
                    got.AddRange(echo.posted());
                    Thread.Sleep(50);
                }
                // Oneway requests may be dispatched concurrently, so order
                // is not checked.
                got.Sort(StringComparer.Ordinal);
                sent.Sort(StringComparer.Ordinal);
                Check(got.SequenceEqual(sent), "posted [" + string.Join(", ", got) + "]");
            });
        }
    }
}
