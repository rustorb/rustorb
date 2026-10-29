// Client call timeouts of the Java binding (org.rustorb.Timeouts), against
// a server of the test's own that reads requests and never answers them.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.InputStream;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.util.concurrent.atomic.AtomicInteger;

import org.junit.jupiter.api.Test;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.ORB;
import org.omg.CORBA.TIMEOUT;
import org.rustorb.Timeouts;

class TimeoutTest {
    /** Accepts connections and counts the CancelRequest messages read. */
    static final class Sink implements AutoCloseable {
        final ServerSocket socket;
        final AtomicInteger cancels = new AtomicInteger();

        Sink() throws Exception {
            socket = new ServerSocket(0, 16, InetAddress.getLoopbackAddress());
            Thread t = new Thread(() -> {
                try {
                    for (;;) {
                        Socket c = socket.accept();
                        Thread r = new Thread(() -> read(c));
                        r.setDaemon(true);
                        r.start();
                    }
                } catch (Exception e) {
                    // closed
                }
            });
            t.setDaemon(true);
            t.start();
        }

        private void read(Socket c) {
            try (InputStream in = c.getInputStream()) {
                byte[] header = new byte[12];
                for (;;) {
                    in.readNBytes(header, 0, 12);
                    boolean little = (header[6] & 1) != 0;
                    int size = 0;
                    for (int i = 0; i < 4; i++) {
                        int b = header[8 + (little ? 3 - i : i)] & 0xff;
                        size = size << 8 | b;
                    }
                    if (in.readNBytes(size).length < size) {
                        return;
                    }
                    if (header[7] == 2) {
                        cancels.incrementAndGet();
                    }
                }
            } catch (Exception e) {
                // closed
            }
        }

        String url() {
            return "corbaloc::127.0.0.1:" + socket.getLocalPort() + "/Sink";
        }

        @Override
        public void close() throws Exception {
            socket.close();
        }
    }

    /** Milliseconds until _non_existent on obj throws TIMEOUT. */
    static long timeOut(org.omg.CORBA.Object obj) {
        long start = System.nanoTime();
        TIMEOUT e = assertThrows(TIMEOUT.class, obj::_non_existent);
        assertEquals(CompletionStatus.COMPLETED_MAYBE, e.completed);
        return (System.nanoTime() - start) / 1_000_000;
    }

    static void assertAbout(long expected, long ms) {
        assertTrue(ms >= expected && ms < expected + 400, ms + " ms, expected about " + expected);
    }

    @Test
    void timeoutsOfTheOrbTheThreadAndTheReference() throws Exception {
        ORB orb = ORB.init(new String[0], null);
        try (Sink sink = new Sink()) {
            org.omg.CORBA.Object obj = orb.string_to_object(sink.url());

            Timeouts.setClientCallTimeout(200);
            assertEquals(200, Timeouts.clientCallTimeout(orb));
            assertAbout(200, timeOut(obj));
            Timeouts.setClientCallTimeout(orb, 30000);

            // The thread's overrides the ORB's, on that thread only.
            Timeouts.setClientThreadCallTimeout(300);
            assertAbout(300, timeOut(obj));
            long[] other = new long[1];
            Thread t = new Thread(() -> {
                Timeouts.setClientThreadCallTimeout(100);
                other[0] = timeOut(obj);
            });
            t.start();
            t.join();
            assertAbout(100, other[0]);

            // The reference's overrides the thread's; 0 unsets it.
            Timeouts.setClientCallTimeout(obj, 100);
            assertAbout(100, timeOut(obj));
            assertAbout(300, timeOut(orb.string_to_object(sink.url())));
            Timeouts.setClientCallTimeout(obj, 0);
            assertAbout(300, timeOut(obj));
            Timeouts.setClientThreadCallTimeout(0);
            Timeouts.setClientCallTimeout(orb, 200);
            assertAbout(200, timeOut(obj));

            Thread.sleep(200);
            assertEquals(7, sink.cancels.get());

            Timeouts.setClientConnectTimeout(orb, 1000);
            assertEquals(1000, Timeouts.clientConnectTimeout(orb));
            Timeouts.setClientConnectTimeout(0);
            assertEquals(0, Timeouts.clientConnectTimeout(orb));
            assertThrows(BAD_PARAM.class, () -> Timeouts.setClientCallTimeout((org.omg.CORBA.Object) null, 1));
        } finally {
            orb.destroy();
        }
    }

    @Test
    void theOrbPropertySetsTheCallTimeout() {
        java.util.Properties props = new java.util.Properties();
        props.setProperty("ORBclientCallTimeOutPeriod", "1500");
        ORB orb = ORB.init(new String[0], props);
        try {
            assertEquals(1500, Timeouts.clientCallTimeout(orb));
        } finally {
            orb.destroy();
        }
    }
}
