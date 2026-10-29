// The test cases of the Java clients, the same as those of the other clients
// (case names included), on the standard IDL to Java mapping.

package interop;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;

import InteropTest.Callback;
import InteropTest.CallbackHelper;
import InteropTest.CallbackPOA;
import InteropTest.Color;
import InteropTest.DivideByZero;
import InteropTest.Echo;
import InteropTest.MAGIC;
import InteropTest.Oops;
import InteropTest.Point;
import InteropTest.PointHolder;
import InteropTest.Record;
import InteropTest.RecordHolder;
import InteropTest.Shade;
import InteropTest.Value;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.IntHolder;
import org.omg.CORBA.SystemException;
import org.omg.PortableServer.POA;

public final class Cases {
    private Cases() {}

    static final class CheckFailed extends RuntimeException {
        private static final long serialVersionUID = 1L;

        CheckFailed(String message) {
            super(message);
        }
    }

    public interface Case {
        void run() throws Exception;
    }

    public static final class Runner {
        private int passed;
        private final List<String> failed = new ArrayList<>();

        public void run(String name, Case fn) {
            try {
                fn.run();
                ++passed;
                System.out.println("ok   " + name);
            } catch (CheckFailed e) {
                fail(name, e.getMessage());
            } catch (SystemException e) {
                fail(name, "unexpected " + e + " (minor " + e.minor + ", " + e.completed.value() + ")");
            } catch (Exception | LinkageError e) {
                // LinkageError: a class of the ORB that cannot be loaded
                // (JacORB's javax.rmi.CORBA on Java 17, see known-failures.txt).
                fail(name, "unexpected " + e);
            }
        }

        private void fail(String name, String why) {
            failed.add(name);
            System.out.println("FAIL " + name + ": " + why);
        }

        public List<String> failed() {
            return failed;
        }

        public void summary() {
            System.out.println();
            System.out.println(passed + " passed, " + failed.size() + " failed");
            if (!failed.isEmpty()) {
                System.out.println("failed: " + String.join(", ", failed));
            }
            System.out.flush();
        }
    }

    static void check(boolean cond, String what) {
        if (!cond) {
            throw new CheckFailed(what);
        }
    }

    static CheckFailed notRaised() {
        return new CheckFailed("expected exception was not raised");
    }

    static final class CallbackImpl extends CallbackPOA {
        final List<String> received = Collections.synchronizedList(new ArrayList<>());

        public String _notify(String message) {
            received.add(message);
            return "ack:" + message;
        }
    }

    // --- comparisons (the mapping has no equals for IDL types) ---

    static boolean eq(Point a, Point b) {
        return a.x == b.x && a.y == b.y;
    }

    static boolean eq(Record a, Record b) {
        return a.id == b.id && a.name.equals(b.name) && a.hue.value() == b.hue.value()
                && eq(a.pos, b.pos) && Arrays.equals(a.values, b.values) && a.active == b.active;
    }

    static boolean eq(Record[] a, Record[] b) {
        if (a.length != b.length) {
            return false;
        }
        for (int i = 0; i < a.length; ++i) {
            if (!eq(a[i], b[i])) {
                return false;
            }
        }
        return true;
    }

    static boolean eq(Value a, Value b) {
        if (a.discriminator() != b.discriminator()) {
            return false;
        }
        switch (a.discriminator()) {
            case 1:
                return a.l() == b.l();
            case 2:
                return a.s().equals(b.s());
            case 3:
                return eq(a.p(), b.p());
            case 4:
            case 5:
                return Arrays.equals(a.seq(), b.seq());
            default:
                return a.flag() == b.flag();
        }
    }

    static boolean eq(Shade a, Shade b) {
        if (a.discriminator().value() != b.discriminator().value()) {
            return false;
        }
        switch (a.discriminator().value()) {
            case Color._RED:
                return a.level() == b.level();
            case Color._GREEN:
                return a.label().equals(b.label());
            default:
                return true;
        }
    }

    static Record record(int id, String name, Color c, Point pos, int[] values, boolean active) {
        return new Record(id, name, c, pos, values, active);
    }

    interface Echoer<T> {
        T call(T v) throws Exception;
    }

    @SafeVarargs
    static <T> void echoValues(Runner r, String op, Echoer<T> call, T... values) {
        for (int i = 0; i < values.length; ++i) {
            T v = values[i];
            r.run(op + "[" + i + "]", () -> {
                T got = call.call(v);
                check(v.equals(got), op + " returned " + got);
            });
        }
    }

    /** Runs every case against echo; poa is where the callback object goes. */
    public static void run(Runner r, Echo echo, POA poa) throws Exception {
        // --- basic types ---
        echoValues(r, "echo_boolean", echo::echo_boolean, true, false);
        echoValues(r, "echo_octet", echo::echo_octet, (byte) 0, (byte) 0x7f, (byte) 0xff);
        echoValues(r, "echo_char", echo::echo_char, 'a', '\0', '~');
        echoValues(r, "echo_wchar", echo::echo_wchar, 'a', '\u3042', '\u00e9');
        echoValues(r, "echo_short", echo::echo_short, (short) 0, (short) -32768, (short) 32767);
        // unsigned values are carried in the signed Java types.
        echoValues(r, "echo_ushort", echo::echo_ushort, (short) 0, (short) 65535);
        echoValues(r, "echo_long", echo::echo_long, 0, Integer.MIN_VALUE, Integer.MAX_VALUE, MAGIC.value);
        echoValues(r, "echo_ulong", echo::echo_ulong, 0, (int) 4294967295L);
        echoValues(r, "echo_longlong", echo::echo_longlong, 0L, Long.MIN_VALUE, Long.MAX_VALUE);
        echoValues(r, "echo_ulonglong", echo::echo_ulonglong, 0L, -1L);
        echoValues(r, "echo_double", echo::echo_double, 0.0, -1.5, 1e300, Double.MIN_VALUE);
        echoValues(r, "echo_string", echo::echo_string, "", "hello", "x".repeat(10000), "caf\u00e9");
        // U+1F980 is sent as a surrogate pair, as the other clients do.
        echoValues(r, "echo_wstring", echo::echo_wstring,
                "", "hello", "\u3053\u3093\u306b\u3061\u306f", "\ud83e\udd80");
        echoValues(r, "echo_float", echo::echo_float, 0.0f, -1.5f, Float.MAX_VALUE, Float.MIN_NORMAL);

        // --- constructed types ---
        Point p = new Point(1.25, -2.5);
        Record rec = record(7, "seven", Color.GREEN, p, new int[] {1, -2, 3}, true);
        Record rec2 = record(-1, "", Color.RED, new Point(0.0, 0.0), new int[0], false);

        r.run("echo_color", () -> {
            for (Color c : new Color[] {Color.RED, Color.GREEN, Color.BLUE}) {
                check(echo.echo_color(c).value() == c.value(), "echo_color " + c.value());
            }
        });
        r.run("echo_point", () -> check(eq(echo.echo_point(p), p), "echo_point"));
        r.run("echo_record", () -> check(eq(echo.echo_record(rec), rec), "echo_record"));
        r.run("echo_records", () -> {
            Record[] seq = {rec, rec2, rec};
            check(eq(echo.echo_records(seq), seq), "echo_records");
        });
        r.run("echo_records[empty]", () -> check(echo.echo_records(new Record[0]).length == 0, "empty"));
        // Large enough to be sent in several fragments, with doubles after
        // each fragment boundary; record i has a name of varying length so
        // that the doubles land at every alignment.
        r.run("echo_records[large]", () -> {
            Color[] colors = {Color.RED, Color.GREEN, Color.BLUE};
            Record[] seq = new Record[400];
            for (int i = 0; i < seq.length; ++i) {
                seq[i] = record(i, "r" + i, colors[i % 3], new Point(i + 0.5, -0.25 * i),
                        new int[] {i, i + 1}, i % 2 == 0);
            }
            check(eq(echo.echo_records(seq), seq), "echo_records[large]");
        });

        // Each value is made inside its case, so that a mapping that refuses
        // one (see known-failures.txt) fails only that case.
        List<java.util.function.Consumer<Value>> values = Arrays.asList(
            v -> v.l(-5),
            v -> v.s("union"),
            v -> v.p(p),
            v -> v.seq(4, new int[] {1, 2}),
            v -> v.seq(5, new int[0]),
            v -> v.flag(99, true),
            v -> v.flag(0, false));
        for (int i = 0; i < values.size(); ++i) {
            java.util.function.Consumer<Value> set = values.get(i);
            r.run("echo_value[" + i + "]", () -> {
                Value v = new Value();
                set.accept(v);
                check(eq(echo.echo_value(v), v), "echo_value");
            });
        }

        Shade[] shades = new Shade[3];
        for (int i = 0; i < shades.length; ++i) {
            shades[i] = new Shade();
        }
        shades[0].level((byte) 200);
        shades[1].label("leaf");
        shades[2].__default();
        for (int i = 0; i < shades.length; ++i) {
            Shade s = shades[i];
            r.run("echo_shade[" + i + "]", () -> {
                if (s == shades[2]) {
                    check(s.discriminator().value() == Color._BLUE, "implicit default");
                }
                check(eq(echo.echo_shade(s), s), "echo_shade");
            });
        }

        r.run("echo_long_array", () -> {
            int[] a = {1, -1, 2147483647, 0};
            check(Arrays.equals(echo.echo_long_array(a), a), "echo_long_array");
        });
        r.run("echo_matrix", () -> {
            double[][] m = {{1.0, 2.0, 3.0}, {-4.0, 0.5, 1e-9}};
            check(Arrays.deepEquals(echo.echo_matrix(m), m), "echo_matrix");
        });
        r.run("echo_long_seq", () -> {
            int[] seq = new int[1000];
            for (int i = 0; i < seq.length; ++i) {
                seq[i] = i - 500;
            }
            check(Arrays.equals(echo.echo_long_seq(seq), seq), "echo_long_seq");
        });
        r.run("echo_long_seq[empty]", () -> check(echo.echo_long_seq(new int[0]).length == 0, "empty"));
        r.run("echo_string_seq", () -> {
            String[] seq = {"a", "", "ccc"};
            check(Arrays.equals(echo.echo_string_seq(seq), seq), "echo_string_seq");
        });
        r.run("echo_octets", () -> {
            byte[] seq = new byte[256];
            for (int i = 0; i < seq.length; ++i) {
                seq[i] = (byte) i;
            }
            check(Arrays.equals(echo.echo_octets(seq), seq), "echo_octets");
        });

        // --- parameter modes ---
        r.run("swap", () -> {
            IntHolder a = new IntHolder(3);
            IntHolder b = new IntHolder(-10);
            check(echo.swap(a, b) == -7, "swap result");
            check(a.value == -10 && b.value == 3, "swap inout");
        });
        r.run("divide", () -> {
            IntHolder rem = new IntHolder();
            check(echo.divide(17, 5, rem) == 3 && rem.value == 2, "17 / 5");
            check(echo.divide(-17, 5, rem) == -3 && rem.value == -2, "-17 / 5");
            try {
                echo.divide(1, 0, rem);
                throw notRaised();
            } catch (DivideByZero e) {
                // expected
            }
        });
        r.run("make", () -> {
            PointHolder pt = new PointHolder();
            RecordHolder rc = new RecordHolder();
            echo.make(12, pt, rc);
            check(eq(pt.value, new Point(12.0, -12.0)), "make p");
            check(eq(rc.value, record(12, "r12", Color.BLUE, new Point(12.0, -12.0),
                    new int[] {12, 13}, true)), "make r");
        });

        // --- exceptions ---
        r.run("fail", () -> {
            try {
                echo.fail(7, "boom");
                throw notRaised();
            } catch (Oops e) {
                check(e.code == 7 && e.reason.equals("boom"), "Oops members");
            }
        });
        r.run("fail_system", () -> {
            try {
                echo.fail_system();
                throw notRaised();
            } catch (BAD_PARAM e) {
                check(e.minor == 42 && e.completed.value() == CompletionStatus._COMPLETED_NO,
                        "BAD_PARAM minor " + e.minor);
            }
        });

        // --- attributes ---
        r.run("attribute counter", () -> {
            echo.counter(41);
            check(echo.counter() == 41, "41");
            echo.counter(-1);
            check(echo.counter() == -1, "-1");
        });
        r.run("attribute name", () -> check(echo.name().equals("rustorb-interop-echo"), "name"));

        // --- object references ---
        r.run("self_ref", () -> {
            Echo ref = echo.self_ref();
            check(ref._is_equivalent(echo), "equivalent");
            check(ref.echo_long(5) == 5, "callable");
        });
        r.run("echo_ref", () -> {
            Echo a = echo.echo_ref(echo);
            check(a._is_equivalent(echo), "echo_ref(echo)");
            Echo b = echo.echo_ref(null);
            check(b._is_equivalent(echo), "echo_ref(nil)");
        });
        r.run("is_nil", () -> {
            check(echo.is_nil(null), "nil");
            check(!echo.is_nil(echo), "non-nil");
        });

        // --- callbacks ---
        CallbackImpl cbServant = new CallbackImpl();
        byte[] oid = "Callback".getBytes(java.nio.charset.StandardCharsets.US_ASCII);
        poa.activate_object_with_id(oid, cbServant);
        Callback cb = CallbackHelper.narrow(poa.id_to_reference(oid));

        r.run("call_back", () -> check(echo.call_back(cb, "hi").equals("ack:hi"), "call_back"));
        r.run("register_callback/trigger", () -> {
            echo.register_callback(cb);
            check(echo.trigger("later").equals("ack:later"), "trigger");
            List<String> received = cbServant.received;
            check(!received.isEmpty() && received.get(received.size() - 1).equals("later"), "received");
        });

        // --- oneway ---
        // Last, as in the other clients (the number of posted() polls
        // depends on timing).
        r.run("oneway post", () -> {
            echo.posted();
            List<String> sent = new ArrayList<>(Arrays.asList("one", "two", "three"));
            for (String m : sent) {
                echo.post(m);
            }
            List<String> got = new ArrayList<>();
            long deadline = System.nanoTime() + 5_000_000_000L;
            while (got.size() < sent.size() && System.nanoTime() < deadline) {
                got.addAll(Arrays.asList(echo.posted()));
                Thread.sleep(50);
            }
            // Oneway requests may be dispatched concurrently, so order is
            // not checked.
            Collections.sort(got);
            Collections.sort(sent);
            check(got.equals(sent), "posted " + got);
        });
    }

}
