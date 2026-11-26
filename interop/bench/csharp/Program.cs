// .NET program of the rustorb / omniORB comparison benchmark (interop/bench,
// docs/omniorb-comparison.md) on the rustorb .NET binding: the same cases,
// options and output as the Rust, C++ and Python programs.
//
//   bench serve  [ORB options]
//   bench client --ior IOR [options] [ORB options]
//   bench inproc [options] [ORB options]

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using System.Threading;
using Bench;
using Omg.Types;

public sealed class Impl : TargetPOA
{
    private readonly Corba.ORB orb;
    private int posted;

    public Impl(Corba.ORB orb) { this.orb = orb; }
    public override void ping() { }
    public override int echo_long(int v) => v;
    public override string echo_string(string v) => v;
    public override ISequence<byte> echo_octets(ISequence<byte> v) => v;
    public override uint put_octets(ISequence<byte> v) => (uint)v.Count;
    public override ISequence<byte> get_octets(uint n) =>
        new Sequence<byte>(Enumerable.Repeat((byte)7, (int)n));
    public override ISequence<Sample> echo_samples(ISequence<Sample> v) => v;
    public override Corba.Any echo_any(Corba.Any v) => v;
    public override void post(int v) { Interlocked.Increment(ref posted); }
    public override uint take_posted() => (uint)Interlocked.Exchange(ref posted, 0);
    public override void shutdown() { new Thread(() => orb.shutdown(false)).Start(); }
}

public static class Program
{
    const string DefaultCases = "ping,long,string,octets:64,octets:1024,octets:16384,"
        + "octets:262144,octets:1048576,octets:8388608,put:1048576,get:1048576,samples:100,"
        + "any,oneway,threads:4";
    static double seconds = 2, warmup = 0.3;
    static readonly CultureInfo Inv = CultureInfo.InvariantCulture;

    static string F(double v, string f = "F2") => v.ToString(f, Inv);

    static void Check(bool ok, string what)
    {
        if (!ok) throw new Exception(what + " mismatch");
    }

    static void Timed(string c, double bytes, Action call)
    {
        double tick = 1e6 / Stopwatch.Frequency; // µs per tick
        long warmEnd = Stopwatch.GetTimestamp() + (long)(warmup * Stopwatch.Frequency);
        long warm = 0;
        for (; Stopwatch.GetTimestamp() < warmEnd; ++warm) call();
        var lat = new List<long>(1 << 16);
        long start = Stopwatch.GetTimestamp();
        long end = start + (long)(seconds * Stopwatch.Frequency);
        for (;;)
        {
            long t = Stopwatch.GetTimestamp();
            call();
            long now = Stopwatch.GetTimestamp();
            lat.Add(now - t);
            if (now >= end) break;
        }
        double took = (Stopwatch.GetTimestamp() - start) / (double)Stopwatch.Frequency;
        lat.Sort();
        int n = lat.Count;
        double Pct(double p) => lat[(int)((n - 1) * p)] * tick;
        double rate = n / took;
        Console.WriteLine("{\"case\":\"" + c + "\",\"calls\":" + n + ",\"seconds\":" + F(took, "F3")
            + ",\"rate\":" + F(rate, "F1") + ",\"mean_us\":" + F(lat.Sum() * tick / n)
            + ",\"p50_us\":" + F(Pct(0.5)) + ",\"p90_us\":" + F(Pct(0.9)) + ",\"p99_us\":"
            + F(Pct(0.99)) + ",\"max_us\":" + F(Pct(1.0)) + ",\"mb_per_s\":" + F(rate * bytes / 1e6)
            + ",\"total_calls\":" + (n + warm) + "}");
    }

    static void RunCase(ITarget t, string c)
    {
        string name = c;
        int n = 0;
        int colon = c.IndexOf(':');
        if (colon >= 0)
        {
            name = c.Substring(0, colon);
            n = int.Parse(c.Substring(colon + 1), Inv);
        }
        switch (name)
        {
            case "first":
            {
                // The first call on a fresh reference: the connection is made.
                long t0 = Stopwatch.GetTimestamp();
                t.ping();
                string us = F((Stopwatch.GetTimestamp() - t0) * 1e6 / Stopwatch.Frequency);
                Console.WriteLine("{\"case\":\"first\",\"calls\":1,\"p50_us\":" + us + ",\"mean_us\":" + us + "}");
                break;
            }
            case "ping": Timed(c, 0, t.ping); break;
            case "long":
            {
                int i = 0;
                Timed(c, 0, () => { ++i; Check(t.echo_long(i) == i, "echo_long"); });
                break;
            }
            case "string":
            {
                string s = new string('x', n == 0 ? 100 : n);
                Timed(c, 0, () => Check(t.echo_string(s).Length == s.Length, "echo_string"));
                break;
            }
            case "octets":
            {
                var v = new Sequence<byte>(Enumerable.Repeat((byte)1, n));
                Timed(c, 2.0 * n, () => Check(t.echo_octets(v).Count == n, "echo_octets"));
                break;
            }
            case "put":
            {
                var v = new Sequence<byte>(Enumerable.Repeat((byte)1, n));
                Timed(c, n, () => Check(t.put_octets(v) == n, "put_octets"));
                break;
            }
            case "get": Timed(c, n, () => Check(t.get_octets((uint)n).Count == n, "get_octets")); break;
            case "samples":
            {
                var v = new Sequence<Sample>(Enumerable.Range(0, n)
                    .Select(i => new Sample(i, 1.5, 2.5, 3.5, "sample-" + i)));
                Timed(c, 0, () => Check(t.echo_samples(v).Count == n, "echo_samples"));
                break;
            }
            case "any":
            {
                var a = new Corba.Any();
                SampleHelper.insert(a, new Sample(1, 1.5, 2.5, 3.5, "sample"));
                Timed(c, 0, () => Check(SampleHelper.extract(t.echo_any(a)).id == 1, "echo_any"));
                break;
            }
            case "oneway":
            {
                t.take_posted();
                var sw = Stopwatch.StartNew();
                long sent = 0;
                while (sw.Elapsed.TotalSeconds < seconds)
                {
                    for (int k = 0; k < 64; ++k) t.post((int)sent++);
                }
                double sendTook = sw.Elapsed.TotalSeconds;
                long got = t.take_posted();
                var deadline = Stopwatch.StartNew();
                while (got < sent && deadline.Elapsed.TotalSeconds < 10)
                {
                    Thread.Sleep(1);
                    got += t.take_posted();
                }
                double took = sw.Elapsed.TotalSeconds;
                Console.WriteLine("{\"case\":\"" + c + "\",\"calls\":" + sent + ",\"seconds\":"
                    + F(took, "F3") + ",\"rate\":" + F(sent / took, "F1") + ",\"send_rate\":"
                    + F(sent / sendTook, "F1") + ",\"received\":" + got + ",\"total_calls\":" + sent + "}");
                Check(got == sent, "oneway");
                break;
            }
            case "threads":
            {
                bool stop = false, failed = false;
                long calls = 0;
                var hs = new List<Thread>();
                for (int w = 0; w < Math.Max(n, 1); ++w)
                {
                    int i = w * 1_000_000;
                    var h = new Thread(() =>
                    {
                        try
                        {
                            while (!Volatile.Read(ref stop))
                            {
                                ++i;
                                if (t.echo_long(i) != i) { failed = true; break; }
                                Interlocked.Increment(ref calls);
                            }
                        }
                        catch (Exception) { failed = true; }
                    });
                    h.Start();
                    hs.Add(h);
                }
                Thread.Sleep((int)(warmup * 1000));
                long c0 = Interlocked.Read(ref calls);
                var sw = Stopwatch.StartNew();
                Thread.Sleep((int)(seconds * 1000));
                long c1 = Interlocked.Read(ref calls);
                double took = sw.Elapsed.TotalSeconds;
                Volatile.Write(ref stop, true);
                foreach (var h in hs) h.Join();
                Check(!failed, "threads");
                Console.WriteLine("{\"case\":\"" + c + "\",\"calls\":" + (c1 - c0) + ",\"seconds\":"
                    + F(took, "F3") + ",\"rate\":" + F((c1 - c0) / took, "F1") + ",\"total_calls\":"
                    + Interlocked.Read(ref calls) + "}");
                break;
            }
            default: throw new Exception("unknown case " + c);
        }
    }

    public static int Main(string[] argv)
    {
        string mode = argv[0], ior = "", cases = DefaultCases;
        var orbArgs = new List<string>();
        for (int i = 1; i < argv.Length; ++i)
        {
            string a = argv[i];
            if (i + 1 < argv.Length && (a == "--ior" || a == "--cases" || a == "--seconds" || a == "--warmup"))
            {
                string v = argv[++i];
                if (a == "--ior") ior = v;
                else if (a == "--cases") cases = v;
                else if (a == "--seconds") seconds = double.Parse(v, Inv);
                else warmup = double.Parse(v, Inv);
            }
            else orbArgs.Add(a);
        }
        Corba.ORB orb = Corba.ORB.init(orbArgs.ToArray());
        ITarget target;
        if (mode == "serve" || mode == "inproc")
        {
            var poa = PortableServer.POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
            byte[] id = poa.activate_object(new Impl(orb));
            var reference = poa.id_to_reference(id);
            poa.the_POAManager.activate();
            target = TargetHelper.narrow(reference);
            if (mode == "serve")
            {
                Console.WriteLine(orb.object_to_string(reference));
                Console.Out.Flush();
                orb.run();
                orb.destroy();
                return 0;
            }
        }
        else
        {
            target = TargetHelper.unchecked_narrow(orb.string_to_object(ior));
        }
        int status = 0;
        foreach (string c in cases.Split(',', StringSplitOptions.RemoveEmptyEntries))
        {
            try { RunCase(target, c); }
            catch (Exception e)
            {
                Console.Error.WriteLine("bench: " + c + ": " + e.Message);
                status = 1;
                break;
            }
        }
        orb.destroy();
        return status;
    }
}
