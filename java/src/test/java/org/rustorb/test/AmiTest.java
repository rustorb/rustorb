// Callback AMI in the Java binding (docs/ami.md): the sendc_ operations of
// the stub classes generated with rustorb-idl -bjava -Wbami, the
// AMI_<I>Handler servants and org.omg.Messaging.ExceptionHolder.
//
// A server ORB, a client ORB and an ORB for handlers talk over TCP on the
// loopback interface, in this process.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import AmiJava.AMI_CalcHandler;
import AmiJava.AMI_CalcHandlerHelper;
import AmiJava.AMI_CalcHandlerPOA;
import AmiJava.Calc;
import AmiJava.CalcHelper;
import AmiJava.CalcPOA;
import AmiJava.Oops;
import AmiJava.OopsHelper;
import AmiJava.Pair;
import AmiJava.PairsHolder;
import AmiJava._CalcStub;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.COMM_FAILURE;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.IntHolder;
import org.omg.CORBA.NO_PERMISSION;
import org.omg.CORBA.ORB;
import org.omg.CORBA.SystemException;
import org.omg.CORBA.TIMEOUT;
import org.omg.CORBA.TRANSIENT;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.UNKNOWN;
import org.omg.Messaging.ExceptionHolder;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.POAHelper;
import org.rustorb.Timeouts;

class AmiTest {
    static final long WAIT = 10;

    static ORB server;
    static ORB client;
    static ORB handlers;
    static _CalcStub calc;

    static final class CalcImpl extends CalcPOA {
        private volatile String label = "start";

        @Override
        public int add(int a, int b, IntHolder carry) throws Oops {
            if (a < 0) {
                throw new Oops(a, "negative");
            }
            int s = a + b;
            carry.value = s / 100;
            return s % 100;
        }

        @Override
        public Pair[] swap(PairsHolder ps, String tag) {
            Pair[] in = ps.value;
            Pair[] ret = new Pair[in.length];
            for (int i = 0; i < in.length; i++) {
                ret[i] = in[in.length - 1 - i];
            }
            ps.value = new Pair[] {new Pair(in.length, tag)};
            return ret;
        }

        @Override
        public void fail(int minor) {
            throw new NO_PERMISSION("", minor, CompletionStatus.COMPLETED_YES);
        }

        @Override
        public void sleep(int ms) {
            try {
                Thread.sleep(ms);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            }
        }

        @Override
        public String label() {
            return label;
        }

        @Override
        public void label(String value) {
            label = value;
        }

        @Override
        public int count() {
            return 7;
        }

        @Override
        public void fire(int n) {}
    }

    /** One call of a handler. */
    record Call(String name, List<Object> args, Thread thread) {}

    /** An AMI_CalcHandler servant that queues its calls. */
    static class Handler extends AMI_CalcHandlerPOA {
        final BlockingQueue<Call> calls = new LinkedBlockingQueue<>();

        void record(String name, Object... args) {
            calls.add(new Call(name, Arrays.asList(args), Thread.currentThread()));
        }

        Call next() throws InterruptedException {
            Call c = calls.poll(WAIT, TimeUnit.SECONDS);
            assertTrue(c != null, "no call of the handler");
            return c;
        }

        @Override
        public void add(int ami_return_val, int carry) {
            record("add", ami_return_val, carry);
        }

        @Override
        public void add_excep(ExceptionHolder excep_holder) {
            record("add_excep", excep_holder);
        }

        @Override
        public void swap(Pair[] ami_return_val, Pair[] ps) {
            record("swap", ami_return_val, ps);
        }

        @Override
        public void swap_excep(ExceptionHolder excep_holder) {
            record("swap_excep", excep_holder);
        }

        @Override
        public void fail() {
            record("fail");
        }

        @Override
        public void fail_excep(ExceptionHolder excep_holder) {
            record("fail_excep", excep_holder);
        }

        @Override
        public void sleep() {
            record("sleep");
        }

        @Override
        public void sleep_excep(ExceptionHolder excep_holder) {
            record("sleep_excep", excep_holder);
        }

        @Override
        public void get_label(String ami_return_val) {
            record("get_label", ami_return_val);
        }

        @Override
        public void get_label_excep(ExceptionHolder excep_holder) {
            record("get_label_excep", excep_holder);
        }

        @Override
        public void set_label() {
            record("set_label");
        }

        @Override
        public void set_label_excep(ExceptionHolder excep_holder) {
            record("set_label_excep", excep_holder);
        }

        @Override
        public void get_count(int ami_return_val) {
            record("get_count", ami_return_val);
        }

        @Override
        public void get_count_excep(ExceptionHolder excep_holder) {
            record("get_count_excep", excep_holder);
        }
    }

    static ORB orb(String... extra) throws Exception {
        List<String> args = new ArrayList<>(List.of("-ORBendPoint", "giop:tcp:127.0.0.1:"));
        args.addAll(List.of(extra));
        ORB orb = ORB.init(args.toArray(new String[0]), null);
        POAHelper.narrow(orb.resolve_initial_references("RootPOA")).the_POAManager().activate();
        return orb;
    }

    static org.omg.CORBA.Object activate(ORB orb, org.omg.PortableServer.Servant servant)
            throws Exception {
        POA root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        return root.servant_to_reference(servant);
    }

    // TEMPORARY (#139): which test a crash on Windows is in.
    @org.junit.jupiter.api.BeforeEach
    void name(org.junit.jupiter.api.TestInfo info) {
        System.out.println("AmiTest: " + info.getDisplayName());
        System.out.flush();
    }

    @BeforeAll
    static void init() throws Exception {
        server = orb();
        client = orb();
        handlers = orb();
        String ior = server.object_to_string(activate(server, new CalcImpl()));
        calc = (_CalcStub) CalcHelper.narrow(client.string_to_object(ior));
    }

    @AfterAll
    static void destroy() {
        client.destroy();
        handlers.destroy();
        server.destroy();
    }

    /** A handler served by the client ORB. */
    static AMI_CalcHandler local(Handler h) throws Exception {
        return AMI_CalcHandlerHelper.narrow(activate(client, h));
    }

    /** A handler served by another ORB, as the client sees it. */
    static AMI_CalcHandler remote(Handler h) throws Exception {
        String ior = handlers.object_to_string(activate(handlers, h));
        return AMI_CalcHandlerHelper.narrow(client.string_to_object(ior));
    }

    static ExceptionHolder holder(Call c) {
        return (ExceptionHolder) c.args().get(0);
    }

    @Test
    void results() throws Exception {
        Handler h = new Handler();
        calc.sendc_add(local(h), 70, 45);
        Call c = h.next();
        assertEquals("add", c.name());
        assertEquals(List.of(15, 1), c.args());
        assertNotEquals(Thread.currentThread(), c.thread());
    }

    @Test
    void inoutAndOut() throws Exception {
        Handler h = new Handler();
        Pair[] ps = {new Pair(1, "one"), new Pair(2, "two")};
        calc.sendc_swap(local(h), ps, "tag");
        // The arguments are copied: changing them later changes nothing.
        ps[0].a = 100;
        ps[1] = new Pair(3, "three");
        Call c = h.next();
        assertEquals("swap", c.name());
        Pair[] ret = (Pair[]) c.args().get(0);
        Pair[] out = (Pair[]) c.args().get(1);
        assertEquals(2, ret.length);
        assertEquals(2, ret[0].a);
        assertEquals("two", ret[0].b);
        assertEquals(1, ret[1].a);
        assertEquals("one", ret[1].b);
        assertEquals(1, out.length);
        assertEquals(2, out[0].a);
        assertEquals("tag", out[0].b);
    }

    @Test
    void attributes() throws Exception {
        Handler h = new Handler();
        AMI_CalcHandler ref = local(h);
        calc.sendc_set_label(ref, "new");
        assertEquals("set_label", h.next().name());
        calc.sendc_get_label(ref);
        Call c = h.next();
        assertEquals("get_label", c.name());
        assertEquals(List.of("new"), c.args());
        calc.sendc_get_count(ref);
        c = h.next();
        assertEquals("get_count", c.name());
        assertEquals(List.of(7), c.args());
    }

    static void checkUser(ExceptionHolder holder) {
        Oops e = assertThrows(Oops.class, holder::raise_exception);
        assertEquals(-1, e.code);
        assertEquals("negative", e.why);
        assertThrows(Oops.class, () -> holder.raise_exception_with_list(
                new TypeCode[] {OopsHelper.type()}));
        UNKNOWN u = assertThrows(UNKNOWN.class,
                () -> holder.raise_exception_with_list(new TypeCode[0]));
        assertEquals(CompletionStatus.COMPLETED_YES, u.completed);
    }

    static void checkSystem(ExceptionHolder holder) {
        NO_PERMISSION e = assertThrows(NO_PERMISSION.class, holder::raise_exception);
        assertEquals(5, e.minor);
        assertEquals(CompletionStatus.COMPLETED_YES, e.completed);
        assertThrows(NO_PERMISSION.class,
                () -> holder.raise_exception_with_list(new TypeCode[0]));
    }

    @Test
    void userException() throws Exception {
        Handler h = new Handler();
        calc.sendc_add(local(h), -1, 0);
        Call c = h.next();
        assertEquals("add_excep", c.name());
        checkUser(holder(c));
    }

    @Test
    void systemException() throws Exception {
        Handler h = new Handler();
        calc.sendc_fail(local(h), 5);
        Call c = h.next();
        assertEquals("fail_excep", c.name());
        checkSystem(holder(c));
    }

    // The holder is sent as a valuetype: the other side has only its state,
    // marshaled_exception, and reads it with the generated Helpers.
    @Test
    void handlerOfAnotherOrb() throws Exception {
        Handler h = new Handler();
        AMI_CalcHandler ref = remote(h);
        calc.sendc_add(ref, 1, 2);
        Call c = h.next();
        assertEquals("add", c.name());
        assertEquals(List.of(3, 0), c.args());
        calc.sendc_add(ref, -1, 0);
        c = h.next();
        assertEquals("add_excep", c.name());
        checkUser(holder(c));
        calc.sendc_fail(ref, 5);
        c = h.next();
        assertEquals("fail_excep", c.name());
        checkSystem(holder(c));
    }

    @Test
    void timeout() throws Exception {
        Handler h = new Handler();
        Timeouts.setClientCallTimeout(calc, 100);
        try {
            calc.sendc_sleep(local(h), 2000);
        } finally {
            Timeouts.setClientCallTimeout(calc, 0);
        }
        Call c = h.next();
        assertEquals("sleep_excep", c.name());
        TIMEOUT e = assertThrows(TIMEOUT.class, holder(c)::raise_exception);
        assertEquals(CompletionStatus.COMPLETED_MAYBE, e.completed);
    }

    // The request is sent; the reply is dropped.
    @Test
    void nilHandler() throws Exception {
        calc.sendc_set_label(null, "from nil");
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(WAIT);
        while (!calc.label().equals("from nil")) {
            assertTrue(System.nanoTime() < deadline);
            Thread.sleep(10);
        }
    }

    @Test
    void many() throws Exception {
        Handler h = new Handler();
        AMI_CalcHandler ref = remote(h);
        int n = 100;
        for (int i = 0; i < n; i++) {
            calc.sendc_add(ref, i, 1000);
        }
        boolean[] seen = new boolean[n];
        for (int i = 0; i < n; i++) {
            Call c = h.next();
            assertEquals("add", c.name());
            int a = (int) c.args().get(1) * 100 + (int) c.args().get(0) - 1000;
            assertFalse(seen[a]);
            seen[a] = true;
        }
    }

    // A handler that throws: the exception is dropped, and later replies
    // still come.
    @Test
    void handlerThrows() throws Exception {
        Handler h = new Handler() {
            @Override
            public void get_count(int ami_return_val) {
                super.get_count(ami_return_val);
                throw new IllegalStateException("dropped");
            }
        };
        AMI_CalcHandler ref = local(h);
        calc.sendc_get_count(ref);
        calc.sendc_get_count(ref);
        assertEquals("get_count", h.next().name());
        assertEquals("get_count", h.next().name());
    }

    // A target whose ORB is gone: the handler gets the failure.
    @Test
    void unreachableTarget() throws Exception {
        ORB gone = orb();
        String ior = gone.object_to_string(activate(gone, new CalcImpl()));
        gone.destroy();
        _CalcStub target = (_CalcStub) CalcHelper.unchecked_narrow(client.string_to_object(ior));
        Handler h = new Handler();
        target.sendc_add(local(h), 1, 2);
        Call c = h.next();
        assertEquals("add_excep", c.name());
        SystemException e = assertThrows(SystemException.class, holder(c)::raise_exception);
        assertTrue(e instanceof TRANSIENT || e instanceof COMM_FAILURE, e.toString());
    }

    // A client ORB destroyed while waiting: the handler, called through a
    // reference of an ORB still running, gets the failure, once. (A handler
    // reference of the destroyed ORB could not be called.) Sending once shut
    // down fails at once.
    @Test
    void shutDownClient() throws Exception {
        Handler h = new Handler();
        AMI_CalcHandler ref = remote(h);
        ORB orb = orb();
        _CalcStub shortCalc = (_CalcStub) CalcHelper.narrow(
                orb.string_to_object(client.object_to_string(calc)));
        shortCalc.sendc_sleep(ref, 1000);
        Thread.sleep(100);
        orb.destroy();
        Call c = h.next();
        assertEquals("sleep_excep", c.name());
        SystemException e = assertThrows(SystemException.class, holder(c)::raise_exception);
        assertTrue(e instanceof TRANSIENT || e instanceof COMM_FAILURE, e.toString());
        assertNull(h.calls.poll(200, TimeUnit.MILLISECONDS));
        assertThrows(BAD_INV_ORDER.class, () -> shortCalc.sendc_add(ref, 1, 2));
    }

    @Test
    void stubsOnly() throws Exception {
        // sendc_ is on the stub class, not the interface that servants
        // implement; oneway operations have none.
        for (java.lang.reflect.Method m : Calc.class.getMethods()) {
            assertFalse(m.getName().startsWith("sendc_"), m.getName());
        }
        for (java.lang.reflect.Method m : _CalcStub.class.getMethods()) {
            assertNotEquals("sendc_fire", m.getName());
        }
        assertArrayEquals(new String[] {"IDL:AmiJava/AMI_CalcHandler:1.0",
            "IDL:omg.org/Messaging/ReplyHandler:1.0"}, new Handler()._all_interfaces(null, null));
    }
}
