// The comparison on the JNI binding: the runtime of java/ with stubs and
// skeletons of rustorb-idl -bjava.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

import Bench.Echo;
import Bench.EchoHelper;
import Bench.EchoPOA;
import Bench.Record;
import org.omg.CORBA.ORB;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;

public final class BenchJni {
    static final class EchoImpl extends EchoPOA {
        @Override
        public void ping() {}

        @Override
        public int echo_long(int v) {
            return v;
        }

        @Override
        public Record echo_record(Record v) {
            return v;
        }

        @Override
        public byte[] echo_octets(byte[] v) {
            return v;
        }
    }

    static Harness.Target target(Echo e) {
        return new Harness.Target() {
            @Override
            public void ping() {
                e.ping();
            }

            @Override
            public int echoLong(int v) {
                return e.echo_long(v);
            }

            @Override
            public int echoRecord(int v) {
                Record r = new Record(v, v, v, v, v, v, v, v, v, v, v, v, v, v, v, v);
                return e.echo_record(r).f15;
            }

            @Override
            public byte[] echoOctets(byte[] v) {
                return e.echo_octets(v);
            }
        };
    }

    public static void main(String[] args) throws Exception {
        ORB server = ORB.init(new String[] {"-ORBendPoint", "giop:tcp:127.0.0.1:"}, null);
        POA serverPoa = POAHelper.narrow(server.resolve_initial_references("RootPOA"));
        serverPoa.the_POAManager().activate();
        Thread thread = new Thread(server::run, "server ORB");
        thread.setDaemon(true);
        thread.start();
        ORB client = ORB.init(new String[0], null);
        POA clientPoa = POAHelper.narrow(client.resolve_initial_references("RootPOA"));
        clientPoa.the_POAManager().activate();

        Echo local = EchoHelper.narrow(clientPoa.servant_to_reference(new EchoImpl()));
        String ior = server.object_to_string(serverPoa.servant_to_reference(new EchoImpl()));
        Echo remote = EchoHelper.narrow(client.string_to_object(ior));
        Harness.run("jni", new Harness.Side() {
            @Override
            public Harness.Target colocated() {
                return target(local);
            }

            @Override
            public Harness.Target remote() {
                return target(remote);
            }

            @Override
            public void close() throws Exception {
                server.shutdown(true);
                thread.join();
                server.destroy();
                client.destroy();
            }
        });
    }
}
