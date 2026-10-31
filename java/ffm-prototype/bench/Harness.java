// The measurements of the JNI / FFM comparison, the same for both: each
// case runs for a while after a warm-up, in rounds, and the median round
// is reported.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.LongAdder;

final class Harness {
    private Harness() {}

    /** The Echo interface of Bench.idl, called through one binding. */
    interface Target {
        void ping();

        int echoLong(int v);

        /** echo_record with all the fields set to v; returns field 15. */
        int echoRecord(int v);

        byte[] echoOctets(byte[] v);
    }

    /** One binding: an object of the same ORB and one of another ORB in
     * this process (over TCP on the loopback interface). */
    interface Side extends AutoCloseable {
        Target colocated();

        Target remote();
    }

    private static final long WARMUP_MS = Long.getLong("bench.warmup", 1000);
    private static final long ROUND_MS = Long.getLong("bench.round", 1000);
    private static final int ROUNDS = Integer.getInteger("bench.rounds", 5);

    interface Op {
        void run(int i);
    }

    /** The median time of op per call over ROUNDS rounds, on threads threads, in ns. */
    static double measure(int threads, Op op) throws InterruptedException {
        loop(threads, op, WARMUP_MS);
        double[] rounds = new double[ROUNDS];
        for (int r = 0; r < ROUNDS; r++) {
            long start = System.nanoTime();
            long n = loop(threads, op, ROUND_MS);
            rounds[r] = (double) (System.nanoTime() - start) / n;
        }
        Arrays.sort(rounds);
        return rounds[ROUNDS / 2];
    }

    private static long loop(int threads, Op op, long ms) throws InterruptedException {
        AtomicBoolean stop = new AtomicBoolean();
        LongAdder count = new LongAdder();
        List<Thread> list = new ArrayList<>();
        Throwable[] failed = new Throwable[1];
        for (int t = 0; t < threads; t++) {
            Thread th = new Thread(() -> {
                int i = 0;
                try {
                    while (!stop.get()) {
                        op.run(i++);
                        count.increment();
                    }
                } catch (Throwable e) {
                    failed[0] = e;
                    stop.set(true);
                }
            });
            th.start();
            list.add(th);
        }
        Thread.sleep(ms);
        stop.set(true);
        for (Thread th : list) {
            th.join();
        }
        if (failed[0] != null) {
            throw new IllegalStateException(failed[0]);
        }
        return count.sum();
    }

    static void check(boolean ok) {
        if (!ok) {
            throw new IllegalStateException("wrong reply");
        }
    }

    /** Runs the cases on side and prints one line each: case, ns per call. */
    static void run(String name, Side side) throws Exception {
        Target local = side.colocated();
        Target remote = side.remote();
        byte[] small = new byte[64 * 1024];
        byte[] large = new byte[1024 * 1024];
        Arrays.fill(small, (byte) 7);
        Arrays.fill(large, (byte) 9);
        report(name, "colocated ping", measure(1, i -> local.ping()));
        report(name, "colocated echo_long", measure(1, i -> check(local.echoLong(i) == i)));
        report(name, "colocated echo_record", measure(1, i -> check(local.echoRecord(i) == i)));
        report(name, "colocated echo_octets 64KiB", measure(1, i -> check(local.echoOctets(small).length == small.length)));
        report(name, "remote ping", measure(1, i -> remote.ping()));
        report(name, "remote echo_long", measure(1, i -> check(remote.echoLong(i) == i)));
        report(name, "remote echo_record", measure(1, i -> check(remote.echoRecord(i) == i)));
        report(name, "remote echo_long 8 threads", measure(8, i -> check(remote.echoLong(i) == i)));
        report(name, "remote echo_octets 64KiB", measure(1, i -> check(remote.echoOctets(small).length == small.length)));
        report(name, "remote echo_octets 1MiB", measure(1, i -> check(remote.echoOctets(large).length == large.length)));
        side.close();
    }

    private static void report(String name, String what, double ns) {
        System.out.println(String.format(Locale.ROOT, "%s\t%s\t%.0f", name, what, ns));
    }
}
