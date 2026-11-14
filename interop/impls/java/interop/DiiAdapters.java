// The rustorbjava-dii implementation: the Java client and server of this
// directory with every call of the basic, any and value suites made through
// the DII and their servants run through the DSI (#156).
//
// With RUSTORB_DII set, Client wraps the reference it tests in a stub below
// (DiiEcho, DiiAnyEcho, DiiValueEcho), whose operations build an
// org.omg.CORBA.Request from the TypeCodes of their parameters, and Server
// activates its servants behind a PortableServer.DynamicImplementation
// (DsiEcho, DsiAnyEcho, DsiValueEcho) that reads the arguments with
// ServerRequest.arguments and calls the static servant's operation. The
// messages are those of the static stubs and skeletons. (The callback
// objects of the client, and the calls the servants make, stay static.)
// Only the standard API is used.

package interop;

import java.util.function.Consumer;

import AnyTest.AnyEcho;
import AnyTest.AnyEchoHelper;
import AnyTest.NVListHelper;
import AnyTest._AnyEchoStub;
import InteropTest.Callback;
import InteropTest.CallbackHelper;
import InteropTest.Color;
import InteropTest.ColorHelper;
import InteropTest.DivideByZero;
import InteropTest.DivideByZeroHelper;
import InteropTest.Echo;
import InteropTest.EchoHelper;
import InteropTest.LongArrayHelper;
import InteropTest.LongSeqHelper;
import InteropTest.MatrixHelper;
import InteropTest.OctetSeqHelper;
import InteropTest.Oops;
import InteropTest.OopsHelper;
import InteropTest.Point;
import InteropTest.PointHelper;
import InteropTest.PointHolder;
import InteropTest.Record;
import InteropTest.RecordHelper;
import InteropTest.RecordHolder;
import InteropTest.RecordSeqHelper;
import InteropTest.Shade;
import InteropTest.ShadeHelper;
import InteropTest.StringSeqHelper;
import InteropTest.Value;
import InteropTest.ValueHelper;
import InteropTest._EchoStub;
import ValueTest.Account;
import ValueTest.AccountHelper;
import ValueTest.Counter;
import ValueTest.CounterHelper;
import ValueTest.LongBox;
import ValueTest.LongBoxHelper;
import ValueTest.Node;
import ValueTest.NodeHelper;
import ValueTest.Pair;
import ValueTest.PairBoxHelper;
import ValueTest.PairHelper;
import ValueTest.PointSeqHelper;
import ValueTest.Shape;
import ValueTest.ShapeHelper;
import ValueTest.StringBoxHelper;
import ValueTest.Tally;
import ValueTest.TallyHelper;
import ValueTest.ValueEcho;
import ValueTest.ValueEchoHelper;
import ValueTest._ValueEchoStub;
import org.omg.CORBA.ARG_IN;
import org.omg.CORBA.ARG_INOUT;
import org.omg.CORBA.ARG_OUT;
import org.omg.CORBA.Any;
import org.omg.CORBA.AnyHolder;
import org.omg.CORBA.BAD_OPERATION;
import org.omg.CORBA.Bounds;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.IntHolder;
import org.omg.CORBA.NVList;
import org.omg.CORBA.ORB;
import org.omg.CORBA.Request;
import org.omg.CORBA.ServerRequest;
import org.omg.CORBA.SystemException;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.UNKNOWN;
import org.omg.CORBA.UnknownUserException;
import org.omg.CORBA.portable.ObjectImpl;
import org.omg.PortableServer.DynamicImplementation;
import org.omg.PortableServer.POA;
import org.omg.PortableServer.Servant;

public final class DiiAdapters {
    private DiiAdapters() {}

    /** Whether RUSTORB_DII is set (and not "0"). */
    public static boolean enabled() {
        String v = System.getenv("RUSTORB_DII");
        return v != null && !v.isEmpty() && !v.equals("0");
    }

    static TypeCode tc(TCKind kind) {
        return ORB.init().get_primitive_tc(kind);
    }

    static final TypeCode BOOLEAN = tc(TCKind.tk_boolean);
    static final TypeCode OCTET = tc(TCKind.tk_octet);
    static final TypeCode CHAR = tc(TCKind.tk_char);
    static final TypeCode WCHAR = tc(TCKind.tk_wchar);
    static final TypeCode SHORT = tc(TCKind.tk_short);
    static final TypeCode USHORT = tc(TCKind.tk_ushort);
    static final TypeCode LONG = tc(TCKind.tk_long);
    static final TypeCode ULONG = tc(TCKind.tk_ulong);
    static final TypeCode LONGLONG = tc(TCKind.tk_longlong);
    static final TypeCode ULONGLONG = tc(TCKind.tk_ulonglong);
    static final TypeCode FLOAT = tc(TCKind.tk_float);
    static final TypeCode DOUBLE = tc(TCKind.tk_double);
    static final TypeCode STRING = tc(TCKind.tk_string);
    static final TypeCode WSTRING = tc(TCKind.tk_wstring);
    static final TypeCode ANY = tc(TCKind.tk_any);
    static final TypeCode TYPECODE = tc(TCKind.tk_TypeCode);

    // --- the client side ------------------------------------------------------------

    /** A call of an operation through an org.omg.CORBA.Request. */
    static final class Call {
        private final Request req;

        Call(org.omg.CORBA.Object target, String op) {
            req = target._request(op);
        }

        Call in(Consumer<Any> value) {
            value.accept(req.add_in_arg());
            return this;
        }

        Call inout(Consumer<Any> value) {
            value.accept(req.add_inout_arg());
            return this;
        }

        Call out(TypeCode tc) {
            req.add_out_arg().type(tc);
            return this;
        }

        Call returns(TypeCode tc) {
            req.set_return_type(tc);
            return this;
        }

        Call raises(TypeCode tc) {
            req.exceptions().add(tc);
            return this;
        }

        /** Invokes; the user exception the operation raised, or null. A
         * system exception is thrown. */
        UnknownUserException invoke() {
            req.invoke();
            Exception e = req.env().exception();
            if (e == null) {
                return null;
            }
            if (e instanceof UnknownUserException) {
                return (UnknownUserException) e;
            }
            if (e instanceof SystemException) {
                throw (SystemException) e;
            }
            throw new UNKNOWN(e.toString(), 0, CompletionStatus.COMPLETED_MAYBE);
        }

        /** Invokes an operation without user exceptions. */
        Call call() {
            UnknownUserException u = invoke();
            if (u != null) {
                throw unexpected(u);
            }
            return this;
        }

        void oneway() {
            req.send_oneway();
        }

        Any result() {
            return req.return_value();
        }

        Any arg(int index) {
            try {
                return req.arguments().item(index).value();
            } catch (Bounds e) {
                throw new BAD_OPERATION("no argument " + index, 0, CompletionStatus.COMPLETED_NO);
            }
        }
    }

    static UNKNOWN unexpected(UnknownUserException u) {
        return new UNKNOWN("unexpected user exception " + u.except.type(), 0,
                CompletionStatus.COMPLETED_YES);
    }

    /** A stub of the DII class on the delegate of ref. */
    private static <T extends ObjectImpl> T wrap(org.omg.CORBA.Object ref, T stub) {
        stub._set_delegate(((ObjectImpl) ref)._get_delegate());
        return stub;
    }

    public static Echo echo(Echo ref) {
        return ref == null ? null : wrap(ref, new DiiEcho());
    }

    public static AnyEcho anyEcho(AnyEcho ref) {
        return ref == null ? null : wrap(ref, new DiiAnyEcho());
    }

    public static ValueEcho valueEcho(ValueEcho ref) {
        return ref == null ? null : wrap(ref, new DiiValueEcho());
    }

    /** InteropTest::Echo through the DII. */
    static final class DiiEcho extends _EchoStub {
        private static final long serialVersionUID = 1L;

        private Call call(String op) {
            return new Call(this, op);
        }

        @Override
        public boolean echo_boolean(boolean v) {
            return call("echo_boolean").in(a -> a.insert_boolean(v)).returns(BOOLEAN).call()
                    .result().extract_boolean();
        }

        @Override
        public byte echo_octet(byte v) {
            return call("echo_octet").in(a -> a.insert_octet(v)).returns(OCTET).call()
                    .result().extract_octet();
        }

        @Override
        public char echo_char(char v) {
            return call("echo_char").in(a -> a.insert_char(v)).returns(CHAR).call()
                    .result().extract_char();
        }

        @Override
        public char echo_wchar(char v) {
            return call("echo_wchar").in(a -> a.insert_wchar(v)).returns(WCHAR).call()
                    .result().extract_wchar();
        }

        @Override
        public short echo_short(short v) {
            return call("echo_short").in(a -> a.insert_short(v)).returns(SHORT).call()
                    .result().extract_short();
        }

        @Override
        public short echo_ushort(short v) {
            return call("echo_ushort").in(a -> a.insert_ushort(v)).returns(USHORT).call()
                    .result().extract_ushort();
        }

        @Override
        public int echo_long(int v) {
            return call("echo_long").in(a -> a.insert_long(v)).returns(LONG).call()
                    .result().extract_long();
        }

        @Override
        public int echo_ulong(int v) {
            return call("echo_ulong").in(a -> a.insert_ulong(v)).returns(ULONG).call()
                    .result().extract_ulong();
        }

        @Override
        public long echo_longlong(long v) {
            return call("echo_longlong").in(a -> a.insert_longlong(v)).returns(LONGLONG).call()
                    .result().extract_longlong();
        }

        @Override
        public long echo_ulonglong(long v) {
            return call("echo_ulonglong").in(a -> a.insert_ulonglong(v)).returns(ULONGLONG)
                    .call().result().extract_ulonglong();
        }

        @Override
        public float echo_float(float v) {
            return call("echo_float").in(a -> a.insert_float(v)).returns(FLOAT).call()
                    .result().extract_float();
        }

        @Override
        public double echo_double(double v) {
            return call("echo_double").in(a -> a.insert_double(v)).returns(DOUBLE).call()
                    .result().extract_double();
        }

        @Override
        public String echo_string(String v) {
            return call("echo_string").in(a -> a.insert_string(v)).returns(STRING).call()
                    .result().extract_string();
        }

        @Override
        public String echo_wstring(String v) {
            return call("echo_wstring").in(a -> a.insert_wstring(v)).returns(WSTRING).call()
                    .result().extract_wstring();
        }

        @Override
        public Color echo_color(Color v) {
            return ColorHelper.extract(call("echo_color").in(a -> ColorHelper.insert(a, v))
                    .returns(ColorHelper.type()).call().result());
        }

        @Override
        public Point echo_point(Point v) {
            return PointHelper.extract(call("echo_point").in(a -> PointHelper.insert(a, v))
                    .returns(PointHelper.type()).call().result());
        }

        @Override
        public Record echo_record(Record v) {
            return RecordHelper.extract(call("echo_record").in(a -> RecordHelper.insert(a, v))
                    .returns(RecordHelper.type()).call().result());
        }

        @Override
        public Record[] echo_records(Record[] v) {
            return RecordSeqHelper.extract(call("echo_records")
                    .in(a -> RecordSeqHelper.insert(a, v)).returns(RecordSeqHelper.type()).call()
                    .result());
        }

        @Override
        public Value echo_value(Value v) {
            return ValueHelper.extract(call("echo_value").in(a -> ValueHelper.insert(a, v))
                    .returns(ValueHelper.type()).call().result());
        }

        @Override
        public Shade echo_shade(Shade v) {
            return ShadeHelper.extract(call("echo_shade").in(a -> ShadeHelper.insert(a, v))
                    .returns(ShadeHelper.type()).call().result());
        }

        @Override
        public int[] echo_long_array(int[] v) {
            return LongArrayHelper.extract(call("echo_long_array")
                    .in(a -> LongArrayHelper.insert(a, v)).returns(LongArrayHelper.type()).call()
                    .result());
        }

        @Override
        public double[][] echo_matrix(double[][] v) {
            return MatrixHelper.extract(call("echo_matrix").in(a -> MatrixHelper.insert(a, v))
                    .returns(MatrixHelper.type()).call().result());
        }

        @Override
        public int[] echo_long_seq(int[] v) {
            return LongSeqHelper.extract(call("echo_long_seq").in(a -> LongSeqHelper.insert(a, v))
                    .returns(LongSeqHelper.type()).call().result());
        }

        @Override
        public String[] echo_string_seq(String[] v) {
            return StringSeqHelper.extract(call("echo_string_seq")
                    .in(a -> StringSeqHelper.insert(a, v)).returns(StringSeqHelper.type()).call()
                    .result());
        }

        @Override
        public byte[] echo_octets(byte[] v) {
            return OctetSeqHelper.extract(call("echo_octets").in(a -> OctetSeqHelper.insert(a, v))
                    .returns(OctetSeqHelper.type()).call().result());
        }

        @Override
        public int swap(IntHolder a, IntHolder b) {
            Call c = call("swap").inout(x -> x.insert_long(a.value))
                    .inout(x -> x.insert_long(b.value)).returns(LONG).call();
            a.value = c.arg(0).extract_long();
            b.value = c.arg(1).extract_long();
            return c.result().extract_long();
        }

        @Override
        public int divide(int a, int b, IntHolder remainder) throws DivideByZero {
            Call c = call("divide").in(x -> x.insert_long(a)).in(x -> x.insert_long(b)).out(LONG)
                    .returns(LONG).raises(DivideByZeroHelper.type());
            UnknownUserException u = c.invoke();
            if (u != null) {
                if (u.except.type().equivalent(DivideByZeroHelper.type())) {
                    throw DivideByZeroHelper.extract(u.except);
                }
                throw unexpected(u);
            }
            remainder.value = c.arg(2).extract_long();
            return c.result().extract_long();
        }

        @Override
        public void make(int x, PointHolder p, RecordHolder r) {
            Call c = call("make").in(a -> a.insert_long(x)).out(PointHelper.type())
                    .out(RecordHelper.type()).call();
            p.value = PointHelper.extract(c.arg(1));
            r.value = RecordHelper.extract(c.arg(2));
        }

        private static void raiseOops(UnknownUserException u) throws Oops {
            if (u != null) {
                if (u.except.type().equivalent(OopsHelper.type())) {
                    throw OopsHelper.extract(u.except);
                }
                throw unexpected(u);
            }
        }

        @Override
        public void fail(int code, String reason) throws Oops {
            raiseOops(call("fail").in(a -> a.insert_long(code)).in(a -> a.insert_string(reason))
                    .raises(OopsHelper.type()).invoke());
        }

        @Override
        public void fail_system() {
            call("fail_system").call();
        }

        @Override
        public int counter() {
            return call("_get_counter").returns(LONG).call().result().extract_long();
        }

        @Override
        public void counter(int value) {
            call("_set_counter").in(a -> a.insert_long(value)).call();
        }

        @Override
        public String name() {
            return call("_get_name").returns(STRING).call().result().extract_string();
        }

        @Override
        public void post(String message) {
            call("post").in(a -> a.insert_string(message)).oneway();
        }

        @Override
        public String[] posted() {
            return StringSeqHelper.extract(call("posted").returns(StringSeqHelper.type()).call()
                    .result());
        }

        @Override
        public Echo self_ref() {
            return EchoHelper.extract(call("self_ref").returns(EchoHelper.type()).call().result());
        }

        @Override
        public Echo echo_ref(Echo other) {
            return EchoHelper.extract(call("echo_ref").in(a -> EchoHelper.insert(a, other))
                    .returns(EchoHelper.type()).call().result());
        }

        @Override
        public boolean is_nil(Echo other) {
            return call("is_nil").in(a -> EchoHelper.insert(a, other)).returns(BOOLEAN).call()
                    .result().extract_boolean();
        }

        @Override
        public String call_back(Callback cb, String message) {
            return call("call_back").in(a -> CallbackHelper.insert(a, cb))
                    .in(a -> a.insert_string(message)).returns(STRING).call().result()
                    .extract_string();
        }

        @Override
        public void register_callback(Callback cb) {
            call("register_callback").in(a -> CallbackHelper.insert(a, cb)).call();
        }

        @Override
        public String trigger(String message) throws Oops {
            Call c = call("trigger").in(a -> a.insert_string(message)).returns(STRING)
                    .raises(OopsHelper.type());
            raiseOops(c.invoke());
            return c.result().extract_string();
        }

        @Override
        public void shutdown() {
            call("shutdown").oneway();
        }
    }

    /** AnyTest::AnyEcho through the DII. */
    static final class DiiAnyEcho extends _AnyEchoStub {
        private static final long serialVersionUID = 1L;

        private Call call(String op) {
            return new Call(this, op);
        }

        @Override
        public Any echo(Any a) {
            return call("echo").in(x -> x.insert_any(a)).returns(ANY).call().result()
                    .extract_any();
        }

        @Override
        public Any make(String name) {
            return call("make").in(x -> x.insert_string(name)).returns(ANY).call().result()
                    .extract_any();
        }

        @Override
        public boolean check(String name, Any a) {
            return call("check").in(x -> x.insert_string(name)).in(x -> x.insert_any(a))
                    .returns(BOOLEAN).call().result().extract_boolean();
        }

        @Override
        public void out_inout(String name, AnyHolder b, AnyHolder c) {
            Call r = call("out_inout").in(x -> x.insert_string(name)).out(ANY)
                    .inout(x -> x.insert_any(c.value)).call();
            b.value = r.arg(1).extract_any();
            c.value = r.arg(2).extract_any();
        }

        @Override
        public TypeCode echo_tc(TypeCode tc) {
            return call("echo_tc").in(x -> x.insert_TypeCode(tc)).returns(TYPECODE).call()
                    .result().extract_TypeCode();
        }

        @Override
        public TypeCode make_tc(String name) {
            return call("make_tc").in(x -> x.insert_string(name)).returns(TYPECODE).call()
                    .result().extract_TypeCode();
        }

        @Override
        public boolean check_tc(String name, TypeCode tc) {
            return call("check_tc").in(x -> x.insert_string(name)).in(x -> x.insert_TypeCode(tc))
                    .returns(BOOLEAN).call().result().extract_boolean();
        }

        @Override
        public AnyTest.NameValue[] echo_nvlist(AnyTest.NameValue[] l) {
            return NVListHelper.extract(call("echo_nvlist").in(x -> NVListHelper.insert(x, l))
                    .returns(NVListHelper.type()).call().result());
        }

        @Override
        public String describe(Any a) {
            return call("describe").in(x -> x.insert_any(a)).returns(STRING).call().result()
                    .extract_string();
        }
    }

    /** ValueTest::ValueEcho through the DII. */
    static final class DiiValueEcho extends _ValueEchoStub {
        private static final long serialVersionUID = 1L;

        private Call call(String op) {
            return new Call(this, op);
        }

        @Override
        public ValueTest.Point echo_point(ValueTest.Point p) {
            return ValueTest.PointHelper.extract(call("echo_point")
                    .in(a -> ValueTest.PointHelper.insert(a, p))
                    .returns(ValueTest.PointHelper.type()).call().result());
        }

        @Override
        public Pair echo_pair(Pair p) {
            return PairHelper.extract(call("echo_pair").in(a -> PairHelper.insert(a, p))
                    .returns(PairHelper.type()).call().result());
        }

        @Override
        public Node echo_node(Node n) {
            return NodeHelper.extract(call("echo_node").in(a -> NodeHelper.insert(a, n))
                    .returns(NodeHelper.type()).call().result());
        }

        @Override
        public Shape echo_shape(Shape s) {
            return ShapeHelper.extract(call("echo_shape").in(a -> ShapeHelper.insert(a, s))
                    .returns(ShapeHelper.type()).call().result());
        }

        @Override
        public ValueTest.Point[] echo_points(ValueTest.Point[] s) {
            return PointSeqHelper.extract(call("echo_points").in(a -> PointSeqHelper.insert(a, s))
                    .returns(PointSeqHelper.type()).call().result());
        }

        @Override
        public boolean same(ValueTest.Point a, ValueTest.Point b) {
            return call("same").in(x -> ValueTest.PointHelper.insert(x, a))
                    .in(x -> ValueTest.PointHelper.insert(x, b)).returns(BOOLEAN).call().result()
                    .extract_boolean();
        }

        @Override
        public int count_nodes(Node n) {
            return call("count_nodes").in(a -> NodeHelper.insert(a, n)).returns(LONG).call()
                    .result().extract_long();
        }

        @Override
        public LongBox echo_long_box(LongBox b) {
            return LongBoxHelper.extract(call("echo_long_box").in(a -> LongBoxHelper.insert(a, b))
                    .returns(LongBoxHelper.type()).call().result());
        }

        @Override
        public String echo_string_box(String b) {
            return StringBoxHelper.extract(call("echo_string_box")
                    .in(a -> StringBoxHelper.insert(a, b)).returns(StringBoxHelper.type()).call()
                    .result());
        }

        @Override
        public Pair echo_pair_box(Pair b) {
            return PairBoxHelper.extract(call("echo_pair_box").in(a -> PairBoxHelper.insert(a, b))
                    .returns(PairBoxHelper.type()).call().result());
        }

        @Override
        public Any echo_any(Any a) {
            return call("echo_any").in(x -> x.insert_any(a)).returns(ANY).call().result()
                    .extract_any();
        }

        @Override
        public Tally echo_tally(Tally t) {
            return TallyHelper.extract(call("echo_tally").in(a -> TallyHelper.insert(a, t))
                    .returns(TallyHelper.type()).call().result());
        }

        @Override
        public int call_count(Counter c) {
            return call("call_count").in(a -> CounterHelper.insert(a, c)).returns(LONG).call()
                    .result().extract_long();
        }

        @Override
        public int account_doubled(Account a) {
            return call("account_doubled").in(x -> AccountHelper.insert(x, a)).returns(LONG)
                    .call().result().extract_long();
        }

        @Override
        public Account make_account(int balance) {
            return AccountHelper.extract(call("make_account").in(a -> a.insert_long(balance))
                    .returns(AccountHelper.type()).call().result());
        }

        @Override
        public void shutdown() {
            call("shutdown").oneway();
        }
    }

    // --- the server side ------------------------------------------------------------------

    /** The parameters of a ServerRequest. */
    static final class Params {
        private final ORB orb;
        private final ServerRequest req;
        private final NVList list;

        Params(ORB orb, ServerRequest req) {
            this.orb = orb;
            this.req = req;
            list = orb.create_list(0);
        }

        private Any add(int flags, TypeCode tc) {
            Any a = list.add(flags).value();
            a.type(tc);
            return a;
        }

        Any in(TypeCode tc) {
            return add(ARG_IN.value, tc);
        }

        Any inout(TypeCode tc) {
            return add(ARG_INOUT.value, tc);
        }

        Any out(TypeCode tc) {
            return add(ARG_OUT.value, tc);
        }

        /** Reads the arguments into the anys in, inout and out returned. */
        Params read() {
            req.arguments(list);
            return this;
        }

        void result(Consumer<Any> value) {
            Any a = orb.create_any();
            value.accept(a);
            req.set_result(a);
        }

        void raise(Consumer<Any> exception) {
            Any a = orb.create_any();
            exception.accept(a);
            req.set_exception(a);
        }
    }

    /** The servant to activate for impl: itself, or a DSI servant calling
     * it with RUSTORB_DII. */
    public static Servant servant(ORB orb, EchoImpl impl) {
        return enabled() ? dsi(orb, impl) : impl;
    }

    public static Servant servant(ORB orb, AnyTests.AnyEchoImpl impl) {
        return enabled() ? dsi(orb, impl) : impl;
    }

    public static Servant servant(ORB orb, ValueTests.ValueEchoImpl impl) {
        return enabled() ? dsi(orb, impl) : impl;
    }

    /** A DSI servant calling impl. */
    public static DynamicImplementation dsi(ORB orb, EchoImpl impl) {
        return new DsiEcho(orb, impl);
    }

    public static DynamicImplementation dsi(ORB orb, AnyTests.AnyEchoImpl impl) {
        return new DsiAnyEcho(orb, impl);
    }

    public static DynamicImplementation dsi(ORB orb, ValueTests.ValueEchoImpl impl) {
        return new DsiValueEcho(orb, impl);
    }

    /** The base of the DSI servants: the repository id of impl. */
    abstract static class Dsi extends DynamicImplementation {
        final ORB orb;
        private final String[] ids;

        Dsi(ORB orb, Servant impl) {
            this.orb = orb;
            this.ids = impl._all_interfaces(null, null);
        }

        @Override
        public String[] _all_interfaces(POA poa, byte[] objectId) {
            return ids.clone();
        }

        static BAD_OPERATION noSuchOperation(String op) {
            return new BAD_OPERATION(op, 0, CompletionStatus.COMPLETED_NO);
        }
    }

    /** InteropTest::Echo behind the DSI. */
    static final class DsiEcho extends Dsi {
        private final EchoImpl impl;

        DsiEcho(ORB orb, EchoImpl impl) {
            super(orb, impl);
            this.impl = impl;
            impl.self = () -> EchoHelper.narrow(_this_object());
        }

        @Override
        public void invoke(ServerRequest req) {
            Params p = new Params(orb, req);
            String op = req.operation();
            switch (op) {
                case "echo_boolean": {
                    Any v = p.in(BOOLEAN);
                    p.read().result(r -> r.insert_boolean(impl.echo_boolean(v.extract_boolean())));
                    break;
                }
                case "echo_octet": {
                    Any v = p.in(OCTET);
                    p.read().result(r -> r.insert_octet(impl.echo_octet(v.extract_octet())));
                    break;
                }
                case "echo_char": {
                    Any v = p.in(CHAR);
                    p.read().result(r -> r.insert_char(impl.echo_char(v.extract_char())));
                    break;
                }
                case "echo_wchar": {
                    Any v = p.in(WCHAR);
                    p.read().result(r -> r.insert_wchar(impl.echo_wchar(v.extract_wchar())));
                    break;
                }
                case "echo_short": {
                    Any v = p.in(SHORT);
                    p.read().result(r -> r.insert_short(impl.echo_short(v.extract_short())));
                    break;
                }
                case "echo_ushort": {
                    Any v = p.in(USHORT);
                    p.read().result(r -> r.insert_ushort(impl.echo_ushort(v.extract_ushort())));
                    break;
                }
                case "echo_long": {
                    Any v = p.in(LONG);
                    p.read().result(r -> r.insert_long(impl.echo_long(v.extract_long())));
                    break;
                }
                case "echo_ulong": {
                    Any v = p.in(ULONG);
                    p.read().result(r -> r.insert_ulong(impl.echo_ulong(v.extract_ulong())));
                    break;
                }
                case "echo_longlong": {
                    Any v = p.in(LONGLONG);
                    p.read().result(
                            r -> r.insert_longlong(impl.echo_longlong(v.extract_longlong())));
                    break;
                }
                case "echo_ulonglong": {
                    Any v = p.in(ULONGLONG);
                    p.read().result(
                            r -> r.insert_ulonglong(impl.echo_ulonglong(v.extract_ulonglong())));
                    break;
                }
                case "echo_float": {
                    Any v = p.in(FLOAT);
                    p.read().result(r -> r.insert_float(impl.echo_float(v.extract_float())));
                    break;
                }
                case "echo_double": {
                    Any v = p.in(DOUBLE);
                    p.read().result(r -> r.insert_double(impl.echo_double(v.extract_double())));
                    break;
                }
                case "echo_string": {
                    Any v = p.in(STRING);
                    p.read().result(r -> r.insert_string(impl.echo_string(v.extract_string())));
                    break;
                }
                case "echo_wstring": {
                    Any v = p.in(WSTRING);
                    p.read().result(r -> r.insert_wstring(impl.echo_wstring(v.extract_wstring())));
                    break;
                }
                case "echo_color": {
                    Any v = p.in(ColorHelper.type());
                    p.read().result(r -> ColorHelper.insert(r,
                            impl.echo_color(ColorHelper.extract(v))));
                    break;
                }
                case "echo_point": {
                    Any v = p.in(PointHelper.type());
                    p.read().result(r -> PointHelper.insert(r,
                            impl.echo_point(PointHelper.extract(v))));
                    break;
                }
                case "echo_record": {
                    Any v = p.in(RecordHelper.type());
                    p.read().result(r -> RecordHelper.insert(r,
                            impl.echo_record(RecordHelper.extract(v))));
                    break;
                }
                case "echo_records": {
                    Any v = p.in(RecordSeqHelper.type());
                    p.read().result(r -> RecordSeqHelper.insert(r,
                            impl.echo_records(RecordSeqHelper.extract(v))));
                    break;
                }
                case "echo_value": {
                    Any v = p.in(ValueHelper.type());
                    p.read().result(r -> ValueHelper.insert(r,
                            impl.echo_value(ValueHelper.extract(v))));
                    break;
                }
                case "echo_shade": {
                    Any v = p.in(ShadeHelper.type());
                    p.read().result(r -> ShadeHelper.insert(r,
                            impl.echo_shade(ShadeHelper.extract(v))));
                    break;
                }
                case "echo_long_array": {
                    Any v = p.in(LongArrayHelper.type());
                    p.read().result(r -> LongArrayHelper.insert(r,
                            impl.echo_long_array(LongArrayHelper.extract(v))));
                    break;
                }
                case "echo_matrix": {
                    Any v = p.in(MatrixHelper.type());
                    p.read().result(r -> MatrixHelper.insert(r,
                            impl.echo_matrix(MatrixHelper.extract(v))));
                    break;
                }
                case "echo_long_seq": {
                    Any v = p.in(LongSeqHelper.type());
                    p.read().result(r -> LongSeqHelper.insert(r,
                            impl.echo_long_seq(LongSeqHelper.extract(v))));
                    break;
                }
                case "echo_string_seq": {
                    Any v = p.in(StringSeqHelper.type());
                    p.read().result(r -> StringSeqHelper.insert(r,
                            impl.echo_string_seq(StringSeqHelper.extract(v))));
                    break;
                }
                case "echo_octets": {
                    Any v = p.in(OctetSeqHelper.type());
                    p.read().result(r -> OctetSeqHelper.insert(r,
                            impl.echo_octets(OctetSeqHelper.extract(v))));
                    break;
                }
                case "swap": {
                    Any a = p.inout(LONG);
                    Any b = p.inout(LONG);
                    p.read();
                    IntHolder ha = new IntHolder(a.extract_long());
                    IntHolder hb = new IntHolder(b.extract_long());
                    int sum = impl.swap(ha, hb);
                    a.insert_long(ha.value);
                    b.insert_long(hb.value);
                    p.result(r -> r.insert_long(sum));
                    break;
                }
                case "divide": {
                    Any a = p.in(LONG);
                    Any b = p.in(LONG);
                    Any rem = p.out(LONG);
                    p.read();
                    IntHolder h = new IntHolder();
                    try {
                        int q = impl.divide(a.extract_long(), b.extract_long(), h);
                        rem.insert_long(h.value);
                        p.result(r -> r.insert_long(q));
                    } catch (DivideByZero e) {
                        p.raise(x -> DivideByZeroHelper.insert(x, e));
                    }
                    break;
                }
                case "make": {
                    Any x = p.in(LONG);
                    Any pt = p.out(PointHelper.type());
                    Any rec = p.out(RecordHelper.type());
                    p.read();
                    PointHolder hp = new PointHolder();
                    RecordHolder hr = new RecordHolder();
                    impl.make(x.extract_long(), hp, hr);
                    PointHelper.insert(pt, hp.value);
                    RecordHelper.insert(rec, hr.value);
                    break;
                }
                case "fail": {
                    Any code = p.in(LONG);
                    Any reason = p.in(STRING);
                    p.read();
                    try {
                        impl.fail(code.extract_long(), reason.extract_string());
                    } catch (Oops e) {
                        p.raise(x -> OopsHelper.insert(x, e));
                    }
                    break;
                }
                case "fail_system":
                    p.read();
                    impl.fail_system();
                    break;
                case "_get_counter":
                    p.read().result(r -> r.insert_long(impl.counter()));
                    break;
                case "_set_counter": {
                    Any v = p.in(LONG);
                    p.read();
                    impl.counter(v.extract_long());
                    break;
                }
                case "_get_name":
                    p.read().result(r -> r.insert_string(impl.name()));
                    break;
                case "post": {
                    Any v = p.in(STRING);
                    p.read();
                    impl.post(v.extract_string());
                    break;
                }
                case "posted":
                    p.read().result(r -> StringSeqHelper.insert(r, impl.posted()));
                    break;
                case "self_ref":
                    p.read().result(r -> EchoHelper.insert(r, impl.self_ref()));
                    break;
                case "echo_ref": {
                    Any v = p.in(EchoHelper.type());
                    p.read().result(r -> EchoHelper.insert(r,
                            impl.echo_ref(EchoHelper.extract(v))));
                    break;
                }
                case "is_nil": {
                    Any v = p.in(EchoHelper.type());
                    p.read().result(r -> r.insert_boolean(impl.is_nil(EchoHelper.extract(v))));
                    break;
                }
                case "call_back": {
                    Any cb = p.in(CallbackHelper.type());
                    Any m = p.in(STRING);
                    p.read().result(r -> r.insert_string(
                            impl.call_back(CallbackHelper.extract(cb), m.extract_string())));
                    break;
                }
                case "register_callback": {
                    Any cb = p.in(CallbackHelper.type());
                    p.read();
                    impl.register_callback(CallbackHelper.extract(cb));
                    break;
                }
                case "trigger": {
                    Any m = p.in(STRING);
                    p.read();
                    try {
                        String s = impl.trigger(m.extract_string());
                        p.result(r -> r.insert_string(s));
                    } catch (Oops e) {
                        p.raise(x -> OopsHelper.insert(x, e));
                    }
                    break;
                }
                case "shutdown":
                    p.read();
                    impl.shutdown();
                    break;
                default:
                    throw noSuchOperation(op);
            }
        }
    }

    /** AnyTest::AnyEcho behind the DSI. */
    static final class DsiAnyEcho extends Dsi {
        private final AnyTests.AnyEchoImpl impl;

        DsiAnyEcho(ORB orb, AnyTests.AnyEchoImpl impl) {
            super(orb, impl);
            this.impl = impl;
            impl.self = () -> AnyEchoHelper.narrow(_this_object());
        }

        @Override
        public void invoke(ServerRequest req) {
            Params p = new Params(orb, req);
            String op = req.operation();
            switch (op) {
                case "echo": {
                    Any a = p.in(ANY);
                    p.read().result(r -> r.insert_any(impl.echo(a.extract_any())));
                    break;
                }
                case "make": {
                    Any n = p.in(STRING);
                    p.read().result(r -> r.insert_any(impl.make(n.extract_string())));
                    break;
                }
                case "check": {
                    Any n = p.in(STRING);
                    Any a = p.in(ANY);
                    p.read().result(r -> r.insert_boolean(
                            impl.check(n.extract_string(), a.extract_any())));
                    break;
                }
                case "out_inout": {
                    Any n = p.in(STRING);
                    Any b = p.out(ANY);
                    Any c = p.inout(ANY);
                    p.read();
                    AnyHolder hb = new AnyHolder();
                    AnyHolder hc = new AnyHolder(c.extract_any());
                    impl.out_inout(n.extract_string(), hb, hc);
                    b.insert_any(hb.value);
                    c.insert_any(hc.value);
                    break;
                }
                case "echo_tc": {
                    Any t = p.in(TYPECODE);
                    p.read().result(r -> r.insert_TypeCode(impl.echo_tc(t.extract_TypeCode())));
                    break;
                }
                case "make_tc": {
                    Any n = p.in(STRING);
                    p.read().result(r -> r.insert_TypeCode(impl.make_tc(n.extract_string())));
                    break;
                }
                case "check_tc": {
                    Any n = p.in(STRING);
                    Any t = p.in(TYPECODE);
                    p.read().result(r -> r.insert_boolean(
                            impl.check_tc(n.extract_string(), t.extract_TypeCode())));
                    break;
                }
                case "echo_nvlist": {
                    Any l = p.in(NVListHelper.type());
                    p.read().result(r -> NVListHelper.insert(r,
                            impl.echo_nvlist(NVListHelper.extract(l))));
                    break;
                }
                case "describe": {
                    Any a = p.in(ANY);
                    p.read().result(r -> r.insert_string(impl.describe(a.extract_any())));
                    break;
                }
                default:
                    throw noSuchOperation(op);
            }
        }
    }

    /** ValueTest::ValueEcho behind the DSI. */
    static final class DsiValueEcho extends Dsi {
        private final ValueTests.ValueEchoImpl impl;

        DsiValueEcho(ORB orb, ValueTests.ValueEchoImpl impl) {
            super(orb, impl);
            this.impl = impl;
        }

        @Override
        public void invoke(ServerRequest req) {
            Params p = new Params(orb, req);
            String op = req.operation();
            switch (op) {
                case "echo_point": {
                    Any v = p.in(ValueTest.PointHelper.type());
                    p.read().result(r -> ValueTest.PointHelper.insert(r,
                            impl.echo_point(ValueTest.PointHelper.extract(v))));
                    break;
                }
                case "echo_pair": {
                    Any v = p.in(PairHelper.type());
                    p.read().result(r -> PairHelper.insert(r,
                            impl.echo_pair(PairHelper.extract(v))));
                    break;
                }
                case "echo_node": {
                    Any v = p.in(NodeHelper.type());
                    p.read().result(r -> NodeHelper.insert(r,
                            impl.echo_node(NodeHelper.extract(v))));
                    break;
                }
                case "echo_shape": {
                    Any v = p.in(ShapeHelper.type());
                    p.read().result(r -> ShapeHelper.insert(r,
                            impl.echo_shape(ShapeHelper.extract(v))));
                    break;
                }
                case "echo_points": {
                    Any v = p.in(PointSeqHelper.type());
                    p.read().result(r -> PointSeqHelper.insert(r,
                            impl.echo_points(PointSeqHelper.extract(v))));
                    break;
                }
                case "same": {
                    Any a = p.in(ValueTest.PointHelper.type());
                    Any b = p.in(ValueTest.PointHelper.type());
                    p.read().result(r -> r.insert_boolean(impl.same(
                            ValueTest.PointHelper.extract(a), ValueTest.PointHelper.extract(b))));
                    break;
                }
                case "count_nodes": {
                    Any v = p.in(NodeHelper.type());
                    p.read().result(r -> r.insert_long(impl.count_nodes(NodeHelper.extract(v))));
                    break;
                }
                case "echo_long_box": {
                    Any v = p.in(LongBoxHelper.type());
                    p.read().result(r -> LongBoxHelper.insert(r,
                            impl.echo_long_box(LongBoxHelper.extract(v))));
                    break;
                }
                case "echo_string_box": {
                    Any v = p.in(StringBoxHelper.type());
                    p.read().result(r -> StringBoxHelper.insert(r,
                            impl.echo_string_box(StringBoxHelper.extract(v))));
                    break;
                }
                case "echo_pair_box": {
                    Any v = p.in(PairBoxHelper.type());
                    p.read().result(r -> PairBoxHelper.insert(r,
                            impl.echo_pair_box(PairBoxHelper.extract(v))));
                    break;
                }
                case "echo_any": {
                    Any v = p.in(ANY);
                    p.read().result(r -> r.insert_any(impl.echo_any(v.extract_any())));
                    break;
                }
                case "echo_tally": {
                    Any v = p.in(TallyHelper.type());
                    p.read().result(r -> TallyHelper.insert(r,
                            impl.echo_tally(TallyHelper.extract(v))));
                    break;
                }
                case "call_count": {
                    Any v = p.in(CounterHelper.type());
                    p.read().result(r -> r.insert_long(impl.call_count(CounterHelper.extract(v))));
                    break;
                }
                case "account_doubled": {
                    Any v = p.in(AccountHelper.type());
                    p.read().result(r -> r.insert_long(
                            impl.account_doubled(AccountHelper.extract(v))));
                    break;
                }
                case "make_account": {
                    Any v = p.in(LONG);
                    p.read().result(r -> AccountHelper.insert(r,
                            impl.make_account(v.extract_long())));
                    break;
                }
                case "shutdown":
                    p.read();
                    impl.shutdown();
                    break;
                default:
                    throw noSuchOperation(op);
            }
        }
    }
}
