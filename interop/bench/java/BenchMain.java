// Java program of the rustorb / omniORB comparison benchmark (interop/bench,
// docs/omniorb-comparison.md) on the rustorb Java binding: the same cases,
// options and output as the Rust, C++ and Python programs.
//
//   java BenchMain serve  [ORB options]
//   java BenchMain client --ior IOR [options] [ORB options]
//   java BenchMain inproc [options] [ORB options]

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;

import Bench.Sample;
import Bench.SampleHelper;
import Bench.Target;
import Bench.TargetHelper;
import Bench.TargetPOA;
import org.omg.CORBA.Any;
import org.omg.CORBA.ORB;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;

public final class BenchMain {
    static final String DEFAULT_CASES = "ping,long,string,octets:64,octets:1024,octets:16384,"
            + "octets:262144,octets:1048576,octets:8388608,put:1048576,get:1048576,samples:100,"
            + "any,oneway,threads:4";

    static double seconds = 2, warmup = 0.3;

    static final class Impl extends TargetPOA {
        private final ORB orb;
        private final AtomicInteger posted = new AtomicInteger();

        Impl(ORB orb) {
            this.orb = orb;
        }

        public void ping() {}
        public int echo_long(int v) { return v; }
        public String echo_string(String v) { return v; }
        public byte[] echo_octets(byte[] v) { return v; }
        public int put_octets(byte[] v) { return v.length; }
        public byte[] get_octets(int n) {
            byte[] r = new byte[n];
            Arrays.fill(r, (byte) 7);
            return r;
        }
        public Sample[] echo_samples(Sample[] v) { return v; }
        public Any echo_any(Any v) { return v; }
        public void post(int v) { posted.incrementAndGet(); }
        public int take_posted() { return posted.getAndSet(0); }
        public void shutdown() { new Thread(() -> orb.shutdown(false)).start(); }
    }

    interface Call {
        void run() throws Exception;
    }

    static void check(boolean ok, String what) {
        if (!ok) throw new RuntimeException(what + " mismatch");
    }

    static String f(double v) {
        return String.format(Locale.ROOT, "%.2f", v);
    }

    static void timed(String c, double bytes, Call call) throws Exception {
        long warmEnd = System.nanoTime() + (long) (warmup * 1e9);
        long warm = 0;
        for (; System.nanoTime() < warmEnd; ++warm) call.run();
        long[] lat = new long[1 << 16];
        int n = 0;
        long start = System.nanoTime();
        long end = start + (long) (seconds * 1e9);
        for (;;) {
            long t = System.nanoTime();
            call.run();
            long now = System.nanoTime();
            if (n == lat.length) lat = Arrays.copyOf(lat, n * 2);
            lat[n++] = now - t;
            if (now >= end) break;
        }
        double took = (System.nanoTime() - start) / 1e9;
        long[] l = Arrays.copyOf(lat, n);
        Arrays.sort(l);
        double sum = 0;
        for (long v : l) sum += v;
        final int count = n;
        java.util.function.DoubleUnaryOperator pct = p -> l[(int) ((count - 1) * p)] / 1000.0;
        double rate = n / took;
        System.out.println("{\"case\":\"" + c + "\",\"calls\":" + n + ",\"seconds\":"
                + String.format(Locale.ROOT, "%.3f", took) + ",\"rate\":"
                + String.format(Locale.ROOT, "%.1f", rate) + ",\"mean_us\":" + f(sum / n / 1000)
                + ",\"p50_us\":" + f(pct.applyAsDouble(0.5)) + ",\"p90_us\":"
                + f(pct.applyAsDouble(0.9)) + ",\"p99_us\":" + f(pct.applyAsDouble(0.99))
                + ",\"max_us\":" + f(pct.applyAsDouble(1.0)) + ",\"mb_per_s\":"
                + f(rate * bytes / 1e6) + ",\"total_calls\":" + (n + warm) + "}");
        System.out.flush();
    }

    static void runCase(ORB orb, Target t, String c) throws Exception {
        String name = c;
        int n = 0;
        int colon = c.indexOf(':');
        if (colon >= 0) {
            name = c.substring(0, colon);
            n = Integer.parseInt(c.substring(colon + 1));
        }
        final int size = n;
        switch (name) {
            case "first" -> {
                // The first call on a fresh reference: the connection is made.
                long t0 = System.nanoTime();
                t.ping();
                String us = f((System.nanoTime() - t0) / 1000.0);
                System.out.println("{\"case\":\"first\",\"calls\":1,\"p50_us\":" + us
                        + ",\"mean_us\":" + us + "}");
                System.out.flush();
            }
            case "ping" -> timed(c, 0, t::ping);
            case "long" -> {
                int[] i = {0};
                timed(c, 0, () -> {
                    ++i[0];
                    check(t.echo_long(i[0]) == i[0], "echo_long");
                });
            }
            case "string" -> {
                String s = "x".repeat(size == 0 ? 100 : size);
                timed(c, 0, () -> check(t.echo_string(s).length() == s.length(), "echo_string"));
            }
            case "octets" -> {
                byte[] v = new byte[size];
                Arrays.fill(v, (byte) 1);
                timed(c, 2.0 * size, () -> check(t.echo_octets(v).length == size, "echo_octets"));
            }
            case "put" -> {
                byte[] v = new byte[size];
                timed(c, size, () -> check(t.put_octets(v) == size, "put_octets"));
            }
            case "get" -> timed(c, size, () -> check(t.get_octets(size).length == size, "get_octets"));
            case "samples" -> {
                Sample[] v = new Sample[size];
                for (int i = 0; i < size; ++i) v[i] = new Sample(i, 1.5, 2.5, 3.5, "sample-" + i);
                timed(c, 0, () -> check(t.echo_samples(v).length == size, "echo_samples"));
            }
            case "any" -> {
                Any a = orb.create_any();
                SampleHelper.insert(a, new Sample(1, 1.5, 2.5, 3.5, "sample"));
                timed(c, 0, () -> check(SampleHelper.extract(t.echo_any(a)).id == 1, "echo_any"));
            }
            case "oneway" -> {
                t.take_posted();
                long start = System.nanoTime();
                long end = start + (long) (seconds * 1e9);
                long sent = 0;
                while (System.nanoTime() < end) {
                    for (int k = 0; k < 64; ++k) t.post((int) sent++);
                }
                double sendTook = (System.nanoTime() - start) / 1e9;
                long got = t.take_posted();
                long deadline = System.nanoTime() + 10_000_000_000L;
                while (got < sent && System.nanoTime() < deadline) {
                    Thread.sleep(1);
                    got += t.take_posted();
                }
                double took = (System.nanoTime() - start) / 1e9;
                System.out.println("{\"case\":\"" + c + "\",\"calls\":" + sent + ",\"seconds\":"
                        + String.format(Locale.ROOT, "%.3f", took) + ",\"rate\":"
                        + String.format(Locale.ROOT, "%.1f", sent / took) + ",\"send_rate\":"
                        + String.format(Locale.ROOT, "%.1f", sent / sendTook) + ",\"received\":"
                        + got + ",\"total_calls\":" + sent + "}");
                System.out.flush();
                check(got == sent, "oneway");
            }
            case "threads" -> {
                AtomicBoolean stop = new AtomicBoolean(), failed = new AtomicBoolean();
                AtomicLong calls = new AtomicLong();
                List<Thread> hs = new ArrayList<>();
                for (int w = 0; w < Math.max(size, 1); ++w) {
                    final int base = w * 1_000_000;
                    Thread h = new Thread(() -> {
                        int i = base;
                        try {
                            while (!stop.get()) {
                                ++i;
                                if (t.echo_long(i) != i) {
                                    failed.set(true);
                                    break;
                                }
                                calls.incrementAndGet();
                            }
                        } catch (RuntimeException e) {
                            failed.set(true);
                        }
                    });
                    h.start();
                    hs.add(h);
                }
                Thread.sleep((long) (warmup * 1000));
                long c0 = calls.get();
                long start = System.nanoTime();
                Thread.sleep((long) (seconds * 1000));
                long c1 = calls.get();
                double took = (System.nanoTime() - start) / 1e9;
                stop.set(true);
                for (Thread h : hs) h.join();
                check(!failed.get(), "threads");
                System.out.println("{\"case\":\"" + c + "\",\"calls\":" + (c1 - c0)
                        + ",\"seconds\":" + String.format(Locale.ROOT, "%.3f", took)
                        + ",\"rate\":" + String.format(Locale.ROOT, "%.1f", (c1 - c0) / took)
                        + ",\"total_calls\":" + calls.get() + "}");
                System.out.flush();
            }
            default -> throw new RuntimeException("unknown case " + c);
        }
    }

    public static void main(String[] argv) throws Exception {
        String mode = argv[0];
        String ior = "", cases = DEFAULT_CASES;
        List<String> orbArgs = new ArrayList<>();
        for (int i = 1; i < argv.length; ++i) {
            String a = argv[i];
            if (i + 1 < argv.length && (a.equals("--ior") || a.equals("--cases")
                    || a.equals("--seconds") || a.equals("--warmup"))) {
                String v = argv[++i];
                switch (a) {
                    case "--ior" -> ior = v;
                    case "--cases" -> cases = v;
                    case "--seconds" -> seconds = Double.parseDouble(v);
                    default -> warmup = Double.parseDouble(v);
                }
            } else {
                orbArgs.add(a);
            }
        }
        ORB orb = ORB.init(orbArgs.toArray(new String[0]), null);
        Target target;
        if (mode.equals("serve") || mode.equals("inproc")) {
            POA poa = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
            byte[] id = poa.activate_object(new Impl(orb));
            org.omg.CORBA.Object ref = poa.id_to_reference(id);
            poa.the_POAManager().activate();
            target = TargetHelper.narrow(ref);
            if (mode.equals("serve")) {
                System.out.println(orb.object_to_string(ref));
                System.out.flush();
                orb.run();
                orb.destroy();
                System.exit(0);
            }
        } else {
            target = TargetHelper.unchecked_narrow(orb.string_to_object(ior));
        }
        int status = 0;
        for (String c : cases.split(",")) {
            if (c.isEmpty()) continue;
            try {
                runCase(orb, target, c);
            } catch (Exception e) {
                System.err.println("bench: " + c + ": " + e);
                status = 1;
                break;
            }
        }
        orb.destroy();
        System.exit(status);
    }
}
