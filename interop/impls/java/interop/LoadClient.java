// Java load client for interop/run-load-local.py (rustorbjava; on the
// standard IDL to Java mapping, so jacorb too): calls an Echo server,
// resolved from the naming service, as fast as it can for a while and
// prints the rate of each phase.
//
//   LoadClient --naming CORBALOC --name PATH [--seconds S] [--threads N]
//              [--timeout S] [ORB options]
//
// The phases, each --seconds long, are those of the Rust load client
// (impls/rustorb/src/load_client.rs) but for `connections`: `small`
// (echo_long from N threads on one connection), `string` (echo_string of
// 100 characters) and `large` (echo_octets of 1 MiB from 4 threads). Each
// line reads `PHASE calls=N errors=N rate=N/s`; a failed call makes the exit
// status 1. When the Echo object is not found within --timeout seconds (30
// by default) the exit status is 2.

package interop;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;

import InteropTest.Echo;
import InteropTest.EchoHelper;
import org.omg.CORBA.ORB;
import org.omg.CORBA.TRANSIENT;
import org.omg.CosNaming.NamingContext;

public final class LoadClient {
    private LoadClient() {}

    private interface Call {
        boolean run(long i) throws Exception;
    }

    /** Runs call from `workers` threads until `seconds` pass and prints the
     * totals. Returns whether every call succeeded. */
    private static boolean phase(String label, double seconds, int workers, Call call)
            throws InterruptedException {
        AtomicBoolean stop = new AtomicBoolean();
        AtomicLong calls = new AtomicLong();
        AtomicLong errors = new AtomicLong();
        long start = System.nanoTime();
        List<Thread> threads = new ArrayList<>();
        for (int w = 0; w < workers; ++w) {
            Thread t = new Thread(() -> {
                long i = 0;
                while (!stop.get()) {
                    boolean ok;
                    try {
                        ok = call.run(i);
                    } catch (Exception e) {
                        System.err.println(label + ": " + e);
                        ok = false;
                    }
                    (ok ? calls : errors).incrementAndGet();
                    ++i;
                }
            });
            t.start();
            threads.add(t);
        }
        Thread.sleep((long) (seconds * 1000));
        stop.set(true);
        for (Thread t : threads) {
            t.join();
        }
        double took = (System.nanoTime() - start) / 1e9;
        System.out.printf("%s calls=%d errors=%d rate=%.0f/s%n", label, calls.get(), errors.get(),
                calls.get() / took);
        System.out.flush();
        return errors.get() == 0;
    }

    public static void main(String[] argv) throws Exception {
        // --seconds and --threads are this client's; the rest go to
        // Common.parseArgs.
        double seconds = 5;
        int threads = 8;
        List<String> rest = new ArrayList<>();
        for (int i = 0; i < argv.length; ++i) {
            if ((argv[i].equals("--seconds") || argv[i].equals("--threads")) && i + 1 < argv.length) {
                if (argv[i].equals("--seconds")) {
                    seconds = Double.parseDouble(argv[++i]);
                } else {
                    threads = Integer.parseInt(argv[++i]);
                }
            } else {
                rest.add(argv[i]);
            }
        }
        Common.Options opt = Common.parseArgs("LoadClient", rest.toArray(new String[0]));
        ORB orb = Common.orbInit(opt, "InteropTestLoadClient");
        NamingContext root = Common.rootContext(orb, opt.timeout);
        Echo echo = Common.retry("the Echo server", () -> {
            Echo e = EchoHelper.narrow(root.resolve(Common.toName(opt.name)));
            if (e == null) {
                throw new TRANSIENT();
            }
            e.echo_long(1);
            return e;
        }, opt.timeout, 2);

        boolean ok = phase("small", seconds, threads, i -> echo.echo_long((int) i) == (int) i);
        String text = "x".repeat(100);
        ok &= phase("string", seconds, threads, i -> echo.echo_string(text).equals(text));
        byte[] data = new byte[1 << 20];
        for (int i = 0; i < data.length; ++i) {
            data[i] = (byte) i;
        }
        ok &= phase("large", seconds, 4, i -> Arrays.equals(echo.echo_octets(data), data));
        orb.destroy();
        System.exit(ok ? 0 : 1);
    }
}
