// The comparison on the FFM prototype (org.rustorb.ffm): hand-written
// stubs and a servant in the shape a generator would produce.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

import java.lang.foreign.MemorySegment;

import org.rustorb.ffm.Call;
import org.rustorb.ffm.In;
import org.rustorb.ffm.Operation;
import org.rustorb.ffm.Orb;
import org.rustorb.ffm.Out;
import org.rustorb.ffm.Reply;
import org.rustorb.ffm.Servant;
import org.rustorb.ffm.Stubs;

public final class BenchFfm {
    /** What -bjava would generate for Bench::Echo, on the prototype. */
    static final class EchoStub implements Harness.Target {
        private static final Operation PING = new Operation("ping", false);
        private static final Operation ECHO_LONG = new Operation("echo_long", false);
        private static final Operation ECHO_RECORD = new Operation("echo_record", false);
        private static final Operation ECHO_OCTETS = new Operation("echo_octets", false);

        private final MemorySegment obj;

        EchoStub(MemorySegment obj) {
            this.obj = obj;
        }

        @Override
        public void ping() {
            Stubs.invoke(obj, PING, new Call() {});
        }

        @Override
        public int echoLong(int v) {
            final class C extends Call {
                int result;

                @Override
                public void write(Out out) {
                    out.writeLong(v);
                }

                @Override
                public void read(In in) {
                    result = in.readLong();
                }
            }
            C c = new C();
            Stubs.invoke(obj, ECHO_LONG, c);
            return c.result;
        }

        @Override
        public int echoRecord(int v) {
            Record r = new Record();
            r.f = new int[16];
            java.util.Arrays.fill(r.f, v);
            final class C extends Call {
                Record result;

                @Override
                public void write(Out out) {
                    r.write(out);
                }

                @Override
                public void read(In in) {
                    result = Record.read(in);
                }
            }
            C c = new C();
            Stubs.invoke(obj, ECHO_RECORD, c);
            return c.result.f[15];
        }

        @Override
        public byte[] echoOctets(byte[] v) {
            final class C extends Call {
                byte[] result;

                @Override
                public void write(Out out) {
                    out.writeOctets(v);
                }

                @Override
                public void read(In in) {
                    result = in.readOctets();
                }
            }
            C c = new C();
            Stubs.invoke(obj, ECHO_OCTETS, c);
            return c.result;
        }
    }

    /** Bench::Record: sixteen longs, written and read one by one as a
     * generated Helper does (the fields are an array only to keep this
     * short). */
    static final class Record {
        int[] f;

        void write(Out out) {
            for (int v : f) {
                out.writeLong(v);
            }
        }

        static Record read(In in) {
            Record r = new Record();
            r.f = new int[16];
            for (int i = 0; i < 16; i++) {
                r.f[i] = in.readLong();
            }
            return r;
        }
    }

    /** What -bjava would generate as EchoPOA, with the implementation. */
    static final class EchoImpl extends Servant {
        @Override
        protected String repositoryId() {
            return "IDL:Bench/Echo:1.0";
        }

        @Override
        protected void invoke(String op, In in, Reply reply) {
            switch (op) {
                case "ping":
                    reply.output();
                    break;
                case "echo_long": {
                    int v = in.readLong();
                    reply.output().writeLong(v);
                    break;
                }
                case "echo_record": {
                    Record v = Record.read(in);
                    v.write(reply.output());
                    break;
                }
                case "echo_octets": {
                    byte[] v = in.readOctets();
                    reply.output().writeOctets(v);
                    break;
                }
                default:
                    throw new IllegalArgumentException(op);
            }
        }
    }

    public static void main(String[] args) throws Exception {
        Orb server = new Orb("-ORBendPoint", "giop:tcp:127.0.0.1:");
        Thread thread = new Thread(server::run, "server ORB");
        thread.setDaemon(true);
        thread.start();
        Orb client = new Orb();
        MemorySegment local = client.activate(new EchoImpl());
        String ior = server.objectToString(server.activate(new EchoImpl()));
        MemorySegment remote = client.stringToObject(ior);
        Harness.run("ffm", new Harness.Side() {
            @Override
            public Harness.Target colocated() {
                return new EchoStub(local);
            }

            @Override
            public Harness.Target remote() {
                return new EchoStub(remote);
            }

            @Override
            public void close() throws Exception {
                server.shutdown(true);
                thread.join();
                server.close();
                client.close();
            }
        });
    }
}
