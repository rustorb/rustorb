// The DII and the DSI in the Java binding (#156, docs/dii-dsi.md): the
// basic, any and value interop cases through a DII client and a DSI
// servant (interop/impls/java DiiAdapters), in the three pairs DII -> DSI,
// DII -> static servant and static stub -> DSI, and the API: Request,
// NVList, ExceptionList, deferred requests, ORB.get_next_response,
// ServerRequest and its order of calls.

package org.rustorb.test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import AnyTest.AnyEcho;
import AnyTest.AnyEchoHelper;
import InteropTest.Echo;
import InteropTest.EchoHelper;
import InteropTest.Oops;
import InteropTest.OopsHelper;
import InteropTest.Point;
import InteropTest.PointHelper;
import ValueTest.ValueEcho;
import ValueTest.ValueEchoHelper;
import interop.AnyTests;
import interop.Cases;
import interop.DiiAdapters;
import interop.EchoImpl;
import interop.ValueTests;
import org.junit.jupiter.api.Test;
import org.omg.CORBA.ARG_IN;
import org.omg.CORBA.ARG_OUT;
import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.BAD_OPERATION;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.Bounds;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.ContextList;
import org.omg.CORBA.ExceptionList;
import org.omg.CORBA.LocalObject;
import org.omg.CORBA.NO_IMPLEMENT;
import org.omg.CORBA.NVList;
import org.omg.CORBA.NamedValue;
import org.omg.CORBA.ORB;
import org.omg.CORBA.Request;
import org.omg.CORBA.ServerRequest;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.UNKNOWN;
import org.omg.CORBA.UnknownUserException;
import org.omg.PortableServer.DynamicImplementation;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.Servant;

class DiiTest {
    static final String ECHO_ID = "IDL:rustorb.org/InteropTest/Echo:1.0";

    static TypeCode tc(TCKind k) {
        return ORB.init().get_primitive_tc(k);
    }

    /** The reference of servant on the server ORB, for the client ORB. */
    static org.omg.CORBA.Object ref(Orbs orbs, Servant servant) throws Exception {
        String ior = orbs.server.object_to_string(orbs.serverPoa.servant_to_reference(servant));
        return orbs.client.string_to_object(ior);
    }

    // --- the interop cases ---------------------------------------------------------

    /** The pairs: whether the client uses the DII, whether the servant
     * the DSI. */
    static final boolean[][] PAIRS = {{true, true}, {true, false}, {false, true}};

    @Test
    void basic() throws Exception {
        for (boolean[] pair : PAIRS) {
            try (Orbs orbs = new Orbs()) {
                EchoImpl impl = new EchoImpl(orbs.server);
                Echo echo = EchoHelper.narrow(ref(orbs, pair[1] ? DiiAdapters.dsi(orbs.server, impl)
                        : impl));
                if (pair[0]) {
                    echo = DiiAdapters.echo(echo);
                }
                Cases.Runner r = new Cases.Runner();
                Cases.run(r, echo, orbs.clientPoa);
                r.summary();
                assertEquals(Collections.emptyList(), r.failed(), "dii " + pair[0] + " dsi " + pair[1]);
            }
        }
    }

    @Test
    void any() throws Exception {
        for (boolean[] pair : PAIRS) {
            try (Orbs orbs = new Orbs()) {
                AnyTests.AnyEchoImpl impl = new AnyTests.AnyEchoImpl(orbs.server);
                AnyEcho echo = AnyEchoHelper.narrow(ref(orbs, pair[1]
                        ? DiiAdapters.dsi(orbs.server, impl) : impl));
                if (pair[0]) {
                    echo = DiiAdapters.anyEcho(echo);
                }
                Cases.Runner r = new Cases.Runner();
                AnyTests.run(r, orbs.client, echo);
                r.summary();
                assertEquals(Collections.emptyList(), r.failed(), "dii " + pair[0] + " dsi " + pair[1]);
            }
        }
    }

    @Test
    void value() throws Exception {
        for (boolean[] pair : PAIRS) {
            try (Orbs orbs = new Orbs()) {
                ValueTests.registerFactories(orbs.server);
                ValueTests.registerFactories(orbs.client);
                ValueTests.ValueEchoImpl impl = new ValueTests.ValueEchoImpl(orbs.server);
                ValueEcho echo = ValueEchoHelper.narrow(ref(orbs, pair[1]
                        ? DiiAdapters.dsi(orbs.server, impl) : impl));
                if (pair[0]) {
                    echo = DiiAdapters.valueEcho(echo);
                }
                Cases.Runner r = new Cases.Runner();
                ValueTests.run(r, orbs.client, echo);
                r.summary();
                // The anys of the arguments copy their valuetypes: two
                // arguments that are one instance arrive as two (see
                // docs/dii-dsi.md).
                assertEquals(Collections.singletonList("same[shared]"), r.failed(),
                        "dii " + pair[0] + " dsi " + pair[1]);
            }
        }
    }

    // --- the DII ---------------------------------------------------------------------------

    @Test
    void requestAccessors() throws Exception {
        try (Orbs orbs = new Orbs()) {
            org.omg.CORBA.Object obj = ref(orbs, new EchoImpl(orbs.server));
            Request req = obj._request("echo_long");
            assertSame(obj, req.target());
            assertEquals("echo_long", req.operation());
            assertEquals(0, req.arguments().count());
            assertEquals(TCKind.tk_void, req.return_value().type().kind());
            assertEquals("", req.result().name());
            assertEquals(0, req.exceptions().count());
            assertEquals(0, req.contexts().count());
            assertNull(req.env().exception());
            assertThrows(NO_IMPLEMENT.class, req::ctx);
            assertThrows(NO_IMPLEMENT.class, () -> req.ctx(null));

            req.add_named_in_arg("v").insert_long(7);
            req.set_return_type(tc(TCKind.tk_long));
            NamedValue nv = req.arguments().item(0);
            assertEquals("v", nv.name());
            assertEquals(ARG_IN.value, nv.flags());
            req.invoke();
            assertEquals(7, req.return_value().extract_long());
            assertNull(req.env().exception());
            // The same request again, with another value.
            req.arguments().item(0).value().insert_long(8);
            req.invoke();
            assertEquals(8, req.return_value().extract_long());
        }
    }

    @Test
    void createRequest() throws Exception {
        try (Orbs orbs = new Orbs()) {
            ORB orb = orbs.client;
            org.omg.CORBA.Object obj = ref(orbs, new EchoImpl(orbs.server));
            NVList args = orb.create_list(3);
            args.add_value("a", any(orb, 17), ARG_IN.value);
            args.add_value("b", any(orb, 5), ARG_IN.value);
            args.add_item("remainder", ARG_OUT.value).value().type(tc(TCKind.tk_long));
            Any ret = orb.create_any();
            ret.type(tc(TCKind.tk_long));
            NamedValue result = orb.create_named_value("result", ret, 0);
            ExceptionList exc = orb.create_exception_list();
            exc.add(InteropTest.DivideByZeroHelper.type());
            Request req = obj._create_request(null, "divide", args, result, exc,
                    orb.create_context_list());
            req.invoke();
            assertEquals(3, ret.extract_long());
            assertEquals(2, args.item(2).value().extract_long());
            assertSame(result, req.result());

            // A user exception of the list: in env().
            args.item(1).value().insert_long(0);
            req.invoke();
            UnknownUserException u = assertInstanceOf(UnknownUserException.class,
                    req.env().exception());
            assertTrue(u.except.type().equivalent(InteropTest.DivideByZeroHelper.type()));
            InteropTest.DivideByZeroHelper.extract(u.except);

            // Contexts are not supported.
            ContextList ctxs = orb.create_context_list();
            ctxs.add("x");
            assertThrows(NO_IMPLEMENT.class,
                    () -> obj._create_request(null, "divide", args, result, exc, ctxs));
            assertThrows(NO_IMPLEMENT.class, orb::get_default_context);
            assertThrows(NO_IMPLEMENT.class, () -> orb.create_operation_list(obj));
        }
    }

    static Any any(ORB orb, int v) {
        Any a = orb.create_any();
        a.insert_long(v);
        return a;
    }

    @Test
    void exceptions() throws Exception {
        try (Orbs orbs = new Orbs()) {
            org.omg.CORBA.Object obj = ref(orbs, new EchoImpl(orbs.server));
            // A system exception: in env(), not thrown.
            Request sys = obj._request("fail_system");
            sys.invoke();
            BAD_PARAM bp = assertInstanceOf(BAD_PARAM.class, sys.env().exception());
            assertEquals(42, bp.minor);
            assertEquals(CompletionStatus.COMPLETED_NO, bp.completed);

            // A user exception not in the list: UNKNOWN.
            Request fail = obj._request("fail");
            fail.add_in_arg().insert_long(3);
            fail.add_in_arg().insert_string("no");
            fail.invoke();
            UNKNOWN unk = assertInstanceOf(UNKNOWN.class, fail.env().exception());
            assertEquals(0x4f4d0001, unk.minor);
            assertEquals(CompletionStatus.COMPLETED_YES, unk.completed);

            // A successful call clears env().
            Request ok = echoLong(obj, 1);
            ok.env().exception(new BAD_PARAM());
            ok.invoke();
            assertNull(ok.env().exception());

            // In the list: UnknownUserException in env(), not thrown.
            fail.exceptions().add(OopsHelper.type());
            fail.invoke();
            UnknownUserException u = (UnknownUserException) fail.env().exception();
            Oops oops = OopsHelper.extract(u.except);
            assertEquals(3, oops.code);
            assertEquals("no", oops.reason);

            // An unknown operation.
            Request none = obj._request("no_such_operation");
            none.invoke();
            assertInstanceOf(BAD_OPERATION.class, none.env().exception());

            // An argument without a value.
            Request bad = obj._request("echo_long");
            bad.add_in_arg();
            bad.invoke();
            assertInstanceOf(org.omg.CORBA.SystemException.class, bad.env().exception());

            // A deferred request reports in env() too.
            Request post = obj._request("no_such_operation");
            post.send_deferred();
            post.get_response();
            assertInstanceOf(BAD_OPERATION.class, post.env().exception());

            // Local objects have no requests.
            LocalObject local = new LocalObject();
            assertThrows(NO_IMPLEMENT.class, () -> local._request("x"));
            assertThrows(NO_IMPLEMENT.class,
                    () -> local._create_request(null, "x", null, null));
        }
    }

    @Test
    void lists() throws Exception {
        ORB orb = ORB.init();
        NVList l = orb.create_list(0);
        NamedValue a = l.add(ARG_IN.value);
        assertEquals("", a.name());
        l.add_item("b", ARG_OUT.value);
        Any v = orb.create_any();
        NamedValue c = l.add_value("c", v, ARG_IN.value);
        assertSame(v, c.value());
        assertEquals(3, l.count());
        assertThrows(Bounds.class, () -> l.item(3));
        assertThrows(Bounds.class, () -> l.item(-1));
        assertThrows(Bounds.class, () -> l.remove(5));
        l.remove(0);
        assertEquals(2, l.count());
        assertThrows(Bounds.class, () -> l.item(2));

        ExceptionList e = orb.create_exception_list();
        e.add(OopsHelper.type());
        assertEquals(1, e.count());
        assertThrows(Bounds.class, () -> e.item(1));
        assertThrows(Bounds.class, () -> e.remove(1));

        ContextList cl = orb.create_context_list();
        cl.add("x");
        assertThrows(Bounds.class, () -> cl.item(1));

        org.omg.CORBA.Environment env = orb.create_environment();
        assertNull(env.exception());
        env.exception(new BAD_PARAM());
        assertInstanceOf(BAD_PARAM.class, env.exception());
        env.clear();
        assertNull(env.exception());
    }

    @Test
    void deferred() throws Exception {
        try (Orbs orbs = new Orbs()) {
            ORB orb = orbs.client;
            org.omg.CORBA.Object obj = ref(orbs, new EchoImpl(orbs.server));
            assertThrows(BAD_INV_ORDER.class, orb::get_next_response);
            assertFalse(orb.poll_next_response());

            Request one = echoLong(obj, 1);
            assertThrows(BAD_INV_ORDER.class, one::poll_response);
            assertThrows(BAD_INV_ORDER.class, one::get_response);
            one.send_deferred();
            assertThrows(BAD_INV_ORDER.class, one::send_deferred);
            one.get_response();
            assertEquals(1, one.return_value().extract_long());
            assertThrows(BAD_INV_ORDER.class, one::get_response);

            // poll_response until the reply is there.
            one.arguments().item(0).value().insert_long(2);
            one.send_deferred();
            long end = System.nanoTime() + 10_000_000_000L;
            while (!one.poll_response() && System.nanoTime() < end) {
                Thread.sleep(1);
            }
            assertTrue(one.poll_response());
            one.get_response();
            assertEquals(2, one.return_value().extract_long());

            // Several, taken with get_next_response.
            Request[] reqs = new Request[5];
            for (int i = 0; i < reqs.length; i++) {
                reqs[i] = echoLong(obj, 10 + i);
            }
            orb.send_multiple_requests_deferred(reqs);
            Set<Integer> got = new HashSet<>();
            for (int i = 0; i < reqs.length; i++) {
                Request r = orb.get_next_response();
                got.add(r.return_value().extract_long());
            }
            assertEquals(Set.of(10, 11, 12, 13, 14), got);
            assertFalse(orb.poll_next_response());
            assertThrows(BAD_INV_ORDER.class, orb::get_next_response);

            // A system exception taken by get_next_response is in env().
            Request sys = obj._request("fail_system");
            sys.send_deferred();
            assertSame(sys, orb.get_next_response());
            assertInstanceOf(BAD_PARAM.class, sys.env().exception());

            // Oneway requests.
            Request[] posts = new Request[3];
            for (int i = 0; i < posts.length; i++) {
                posts[i] = obj._request("post");
                posts[i].add_in_arg().insert_string("m" + i);
            }
            orb.send_multiple_requests_oneway(posts);
            Echo echo = EchoHelper.narrow(obj);
            List<String> all = new ArrayList<>();
            end = System.nanoTime() + 10_000_000_000L;
            while (all.size() < 3 && System.nanoTime() < end) {
                Collections.addAll(all, echo.posted());
                Thread.sleep(1);
            }
            // The servant threads may run the oneway requests in any order.
            assertEquals(Set.of("m0", "m1", "m2"), new HashSet<>(all));
        }
    }

    static Request echoLong(org.omg.CORBA.Object obj, int v) {
        Request r = obj._request("echo_long");
        r.add_in_arg().insert_long(v);
        r.set_return_type(tc(TCKind.tk_long));
        return r;
    }

    // --- the DSI ------------------------------------------------------------------------

    /** A DSI servant of Echo for echo_long, divide and fail, which checks
     * the order of the ServerRequest calls. */
    static final class Probe extends DynamicImplementation {
        final List<String> seen = Collections.synchronizedList(new ArrayList<>());

        @Override
        public String[] _all_interfaces(POA poa, byte[] objectId) {
            return new String[] {ECHO_ID};
        }

        private void expect(Class<? extends Exception> c, Runnable r, String what) {
            try {
                r.run();
                seen.add(what + ": no exception");
            } catch (Exception e) {
                seen.add(what + ": " + (c.isInstance(e) ? "ok" : e.toString()));
            }
        }

        @Override
        public void invoke(ServerRequest req) {
            ORB orb = ORB.init();
            NVList args = orb.create_list(0);
            switch (req.operation()) {
                case "echo_long": {
                    Any v = args.add_item("v", ARG_IN.value).value();
                    v.type(tc(TCKind.tk_long));
                    Any early = orb.create_any();
                    early.insert_long(0);
                    expect(BAD_INV_ORDER.class, () -> req.set_result(early), "result first");
                    req.arguments(args);
                    expect(BAD_INV_ORDER.class, () -> req.arguments(args), "arguments again");
                    expect(BAD_PARAM.class, () -> req.set_exception(v), "not an exception");
                    expect(NO_IMPLEMENT.class, req::ctx, "ctx");
                    int n = v.extract_long();
                    if (n == 99) {
                        throw new IllegalStateException("not a CORBA exception");
                    }
                    Any r = orb.create_any();
                    r.insert_long(n);
                    req.set_result(r);
                    break;
                }
                case "divide": {
                    // The out argument is not set: the reply is BAD_PARAM.
                    args.add_item("a", ARG_IN.value).value().type(tc(TCKind.tk_long));
                    args.add_item("b", ARG_IN.value).value().type(tc(TCKind.tk_long));
                    args.add_item("remainder", ARG_OUT.value).value().type(tc(TCKind.tk_long));
                    req.arguments(args);
                    Any r = orb.create_any();
                    r.insert_long(0);
                    req.set_result(r);
                    break;
                }
                case "fail": {
                    args.add_item("code", ARG_IN.value).value().type(tc(TCKind.tk_long));
                    args.add_item("reason", ARG_IN.value).value().type(tc(TCKind.tk_string));
                    req.arguments(args);
                    try {
                        Any e = orb.create_any();
                        OopsHelper.insert(e, new Oops(args.item(0).value().extract_long(),
                                args.item(1).value().extract_string()));
                        req.set_exception(e);
                    } catch (Bounds b) {
                        throw new AssertionError(b);
                    }
                    break;
                }
                default:
                    throw new BAD_OPERATION(req.operation(), 0, CompletionStatus.COMPLETED_NO);
            }
        }
    }

    @Test
    void serverRequest() throws Exception {
        try (Orbs orbs = new Orbs()) {
            Probe probe = new Probe();
            Echo echo = EchoHelper.narrow(ref(orbs, probe));
            assertEquals(5, echo.echo_long(5));
            assertEquals(List.of("result first: ok", "arguments again: ok", "not an exception: ok",
                    "ctx: ok"), probe.seen);
            assertTrue(echo._is_a(ECHO_ID));

            UNKNOWN u = assertThrows(UNKNOWN.class, () -> echo.echo_long(99));
            assertEquals(CompletionStatus.COMPLETED_MAYBE, u.completed);
            assertThrows(BAD_PARAM.class, () -> echo.divide(1, 2, new org.omg.CORBA.IntHolder()));
            Oops oops = assertThrows(Oops.class, () -> echo.fail(4, "four"));
            assertEquals(4, oops.code);
            assertThrows(BAD_OPERATION.class, () -> echo.echo_string("x"));
        }
    }

    @Test
    void sameOrb() throws Exception {
        try (Orbs orbs = new Orbs()) {
            // DII -> DSI in one ORB.
            EchoImpl impl = new EchoImpl(orbs.client);
            Echo echo = DiiAdapters.echo(EchoHelper.narrow(
                    orbs.clientPoa.servant_to_reference(DiiAdapters.dsi(orbs.client, impl))));
            assertEquals(6, echo.echo_long(6));
            Point p = echo.echo_point(new Point(1.5, -2));
            assertEquals(1.5, p.x);
            assertTrue(echo.self_ref()._is_equivalent(echo));
            assertThrows(Oops.class, () -> echo.fail(1, "x"));
        }
    }
}
