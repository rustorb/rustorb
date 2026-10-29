// The InteropTest::Echo servant of the Java servers, on the standard IDL to
// Java mapping. Behaves as the comments in idl/InteropTest.idl say.

package interop;

import java.util.ArrayList;
import java.util.List;

import InteropTest.Callback;
import InteropTest.Color;
import InteropTest.DivideByZero;
import InteropTest.Echo;
import InteropTest.EchoPOA;
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
import org.omg.CORBA.ORB;

public class EchoImpl extends EchoPOA {
    private final ORB orb;
    private int counter;
    private List<String> posted = new ArrayList<>();
    private Callback callback;

    public EchoImpl(ORB orb) {
        this.orb = orb;
    }

    // --- basic types ---
    public boolean echo_boolean(boolean v) { return v; }
    public byte echo_octet(byte v) { return v; }
    public char echo_char(char v) { return v; }
    public char echo_wchar(char v) { return v; }
    public short echo_short(short v) { return v; }
    public short echo_ushort(short v) { return v; }
    public int echo_long(int v) { return v; }
    public int echo_ulong(int v) { return v; }
    public long echo_longlong(long v) { return v; }
    public long echo_ulonglong(long v) { return v; }
    public float echo_float(float v) { return v; }
    public double echo_double(double v) { return v; }
    public String echo_string(String v) { return v; }
    public String echo_wstring(String v) { return v; }

    // --- constructed types ---
    public Color echo_color(Color v) { return v; }
    public Point echo_point(Point v) { return v; }
    public Record echo_record(Record v) { return v; }
    public Record[] echo_records(Record[] v) { return v; }
    public Value echo_value(Value v) { return v; }
    public Shade echo_shade(Shade v) { return v; }
    public int[] echo_long_array(int[] v) { return v; }
    public double[][] echo_matrix(double[][] v) { return v; }
    public int[] echo_long_seq(int[] v) { return v; }
    public String[] echo_string_seq(String[] v) { return v; }
    public byte[] echo_octets(byte[] v) { return v; }

    // --- parameter modes ---
    public int swap(IntHolder a, IntHolder b) {
        int sum = a.value + b.value;
        int t = a.value;
        a.value = b.value;
        b.value = t;
        return sum;
    }

    public int divide(int a, int b, IntHolder remainder) throws DivideByZero {
        if (b == 0) {
            throw new DivideByZero();
        }
        remainder.value = a % b;
        return a / b;
    }

    public void make(int x, PointHolder p, RecordHolder r) {
        p.value = new Point(x, -x);
        r.value = new Record(x, "r" + x, Color.BLUE, new Point(x, -x), new int[] {x, x + 1}, true);
    }

    // --- exceptions ---
    public void fail(int code, String reason) throws Oops {
        throw new Oops(code, reason);
    }

    public void fail_system() {
        throw new BAD_PARAM(42, CompletionStatus.COMPLETED_NO);
    }

    // --- attributes ---
    public synchronized int counter() { return counter; }
    public synchronized void counter(int v) { counter = v; }
    public String name() { return "rustorb-interop-echo"; }

    // --- oneway ---
    public synchronized void post(String message) {
        posted.add(message);
    }

    public synchronized String[] posted() {
        String[] result = posted.toArray(new String[0]);
        posted = new ArrayList<>();
        return result;
    }

    // --- object references ---
    public Echo self_ref() { return _this(); }

    public Echo echo_ref(Echo other) {
        return other == null ? _this() : other;
    }

    public boolean is_nil(Echo other) { return other == null; }

    // --- callbacks ---
    // notify is _notify in Java (it is a method of java.lang.Object).
    public String call_back(Callback cb, String message) {
        return cb._notify(message);
    }

    public synchronized void register_callback(Callback cb) {
        callback = cb;
    }

    public String trigger(String message) throws Oops {
        Callback cb;
        synchronized (this) {
            cb = callback;
        }
        if (cb == null) {
            throw new Oops(1, "no callback");
        }
        return cb._notify(message);
    }

    // --- lifecycle ---
    public void shutdown() {
        orb.shutdown(false);
    }
}
