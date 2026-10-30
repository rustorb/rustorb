// The custom valuetype tests (idl/CustomTest.idl) on the standard IDL to
// Java mapping: the marshaling of Blob, the CustomEcho servant and the
// client cases, those of impls/rustorbpy/customtest.py.

package interop;

import CustomTest.Blob;
import CustomTest.BlobHelper;
import CustomTest.BlobPair;
import CustomTest.CustomEcho;
import CustomTest.CustomEchoPOA;
import ValueTest.Point;
import org.omg.CORBA.LongSeqHolder;
import org.omg.CORBA.ORB;

public final class CustomTests {
    private CustomTests() {}

    /** Blob, marshaled as CustomTest.idl says. */
    static final class BlobImpl extends Blob {
        private static final long serialVersionUID = 1L;

        @Override
        public void marshal(org.omg.CORBA.DataOutputStream os) {
            os.write_long(n);
            os.write_string(label);
            os.write_Value(pt);
            os.write_ulong(values.length);
            os.write_long_array(values, 0, values.length);
        }

        @Override
        public void unmarshal(org.omg.CORBA.DataInputStream is) {
            n = is.read_long();
            label = is.read_string();
            pt = (Point) is.read_Value();
            int len = is.read_ulong();
            LongSeqHolder h = new LongSeqHolder(new int[len]);
            is.read_long_array(h, 0, len);
            values = h.value;
        }
    }

    public static void registerFactories(ORB orb) {
        ValueTests.registerFactories(orb);
        ((org.omg.CORBA_2_3.ORB) orb).register_value_factory(BlobHelper.id(),
                is -> is.read_value(new BlobImpl()));
    }

    public static final class CustomEchoImpl extends CustomEchoPOA {
        private final ORB orb;

        public CustomEchoImpl(ORB orb) {
            this.orb = orb;
        }

        @Override
        public Blob echo_blob(Blob b) {
            return b;
        }

        @Override
        public BlobPair echo_blob_pair(BlobPair p) {
            return p;
        }

        @Override
        public Blob[] echo_blobs(Blob[] s) {
            return s;
        }

        @Override
        public void shutdown() {
            orb.shutdown(false);
        }
    }

    static Blob blob(int n, String label, Point pt, int[] values) {
        Blob b = new BlobImpl();
        b.n = n;
        b.label = label;
        b.pt = pt;
        b.values = values;
        return b;
    }

    static boolean isBlob(Blob b, int n, String label, int[] values) {
        return b instanceof BlobImpl && b.n == n && label.equals(b.label)
                && java.util.Arrays.equals(b.values, values);
    }

    private static void check(boolean cond, String what) {
        Cases.check(cond, what);
    }

    public static void run(Cases.Runner r, CustomEcho echo) {
        r.run("echo_blob[plain]", () -> {
            Blob got = echo.echo_blob(blob(7, "seven", ValueTests.point(1, -2), new int[] {1, 2, 3}));
            check(isBlob(got, 7, "seven", new int[] {1, 2, 3}), "wrong blob");
            check(ValueTests.isPoint(got.pt, 1, -2), "wrong point");
        });
        r.run("echo_blob[null]", () -> check(echo.echo_blob(null) == null, "not null"));
        r.run("echo_blob[empty]", () -> {
            Blob got = echo.echo_blob(blob(0, "", null, new int[0]));
            check(isBlob(got, 0, "", new int[0]), "wrong blob");
            check(got.pt == null, "not null");
        });
        r.run("echo_blob[derived]", () -> {
            Blob got = echo.echo_blob(blob(3, "three", ValueTests.point3(), new int[] {-1}));
            check(isBlob(got, 3, "three", new int[] {-1}), "wrong blob");
            check(ValueTests.isPoint3(got.pt), "wrong point3");
        });
        r.run("echo_blob_pair[shared]", () -> {
            Point p = ValueTests.point(5, 6);
            BlobPair got = echo.echo_blob_pair(new BlobPair(blob(1, "one", p, new int[] {4, 5}), p));
            check(isBlob(got.data, 1, "one", new int[] {4, 5}), "wrong blob");
            check(got.data.pt == got.pt, "not one instance");
            check(ValueTests.isPoint(got.pt, 5, 6), "wrong point");
        });
        r.run("echo_blob_pair[distinct]", () -> {
            BlobPair got = echo.echo_blob_pair(new BlobPair(
                    blob(1, "one", ValueTests.point(5, 6), new int[0]), ValueTests.point(5, 6)));
            check(isBlob(got.data, 1, "one", new int[0]), "wrong blob");
            check(got.data.pt != got.pt, "one instance");
        });
        r.run("echo_blobs", () -> {
            int[] many = new int[100];
            for (int i = 0; i < many.length; i++) {
                many[i] = i;
            }
            Blob b = blob(2, "two", null, many);
            Blob c = blob(9, "nine", ValueTests.point(9, 9), new int[0]);
            Blob[] got = echo.echo_blobs(new Blob[] {b, c, b, null});
            check(got.length == 4, "wrong length");
            check(isBlob(got[0], 2, "two", many) && isBlob(got[1], 9, "nine", new int[0]), "wrong blobs");
            check(got[0] == got[2] && got[0] != got[1], "sharing lost");
            check(got[3] == null, "not null");
        });
    }
}
