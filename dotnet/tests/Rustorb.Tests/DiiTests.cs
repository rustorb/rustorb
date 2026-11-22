// The DII (Corba.Request) against the static Echo servant and a DSI
// servant (PortableServer.DynamicImplementation), and the static stub
// against the DSI servant (the interop rustorbcs-dii runs the whole basic
// suite this way).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using System.Threading;
using Corba;
using InteropTest;
using PortableServer;
using Xunit;
using TypeCode = Corba.TypeCode;

namespace Rustorb.Tests
{
    /// <summary>divide, fail, echo_string and swap of Echo behind the DSI.</summary>
    public sealed class DsiEcho : DynamicImplementation
    {
        public readonly List<string> Operations = new();

        public override string _primary_interface(byte[] oid, POA poa) => EchoHelper.id();

        public override void invoke(ServerRequest request)
        {
            lock (Operations) Operations.Add(request.operation);
            var p = new NVList();
            Any In(TypeCode tc, uint flags)
            {
                var a = new Any();
                a.type = tc;
                return p.add_value("", a, flags).value;
            }
            TypeCode tLong = TypeCodes.Simple(TCKind.tk_long), tString = TypeCodes.Simple(TCKind.tk_string);
            switch (request.operation)
            {
                case "divide":
                    {
                        In(tLong, Corba.Constants.ARG_IN);
                        In(tLong, Corba.Constants.ARG_IN);
                        Any rem = In(tLong, Corba.Constants.ARG_OUT);
                        request.arguments(p);
                        Assert.Throws<BAD_INV_ORDER>(() => request.arguments(p));
                        int a = p.item(0).value.extract_long(), b = p.item(1).value.extract_long();
                        if (b == 0)
                        {
                            var e = new Any();
                            DivideByZeroHelper.insert(e, new DivideByZero());
                            request.set_exception(e);
                            return;
                        }
                        rem.insert_long(a % b);
                        var r = new Any();
                        r.insert_long(a / b);
                        request.set_result(r);
                        return;
                    }
                case "fail":
                    {
                        In(tLong, Corba.Constants.ARG_IN);
                        In(tString, Corba.Constants.ARG_IN);
                        request.arguments(p);
                        var e = new Any();
                        OopsHelper.insert(e, new Oops(p.item(0).value.extract_long(), p.item(1).value.extract_string()));
                        throw new UnknownUserException(e);
                    }
                case "echo_string":
                    {
                        In(tString, Corba.Constants.ARG_IN);
                        request.arguments(p);
                        request.set_result(p.item(0).value);
                        return;
                    }
                case "swap":
                    {
                        Any a = In(tLong, Corba.Constants.ARG_INOUT), b = In(tLong, Corba.Constants.ARG_INOUT);
                        request.arguments(p);
                        int x = a.extract_long(), y = b.extract_long();
                        a.insert_long(y);
                        b.insert_long(x);
                        var r = new Any();
                        r.insert_long(x + y);
                        request.set_result(r);
                        return;
                    }
                case "make":
                    // An out argument never set: BAD_PARAM.
                    In(tLong, Corba.Constants.ARG_IN);
                    In(PointHelper.type(), Corba.Constants.ARG_OUT);
                    In(RecordHelper.type(), Corba.Constants.ARG_OUT);
                    request.arguments(p);
                    return;
                default:
                    throw new BAD_OPERATION("", 0, CompletionStatus.COMPLETED_NO);
            }
        }
    }

    public sealed class DiiFixture : IDisposable
    {
        public ORB Orb { get; } = ORB.init(new string[0]);
        public Corba.Object Static { get; }
        public Corba.Object Dynamic { get; }
        public DsiEcho Dsi { get; } = new DsiEcho();

        public DiiFixture()
        {
            var poa = POAHelper.narrow(Orb.resolve_initial_references("RootPOA"));
            poa.the_POAManager.activate();
            Static = Orb.string_to_object(Orb.object_to_string(poa.servant_to_reference(new EchoImpl())));
            Dynamic = Orb.string_to_object(Orb.object_to_string(poa.servant_to_reference(Dsi)));
        }

        public void Dispose()
        {
            Orb.shutdown(true);
            Orb.destroy();
        }
    }

    public class DiiTests : IClassFixture<DiiFixture>
    {
        private readonly DiiFixture f;

        public DiiTests(DiiFixture fixture)
        {
            f = fixture;
        }

        private static Request Divide(Corba.Object target, int a, int b)
        {
            Request r = target._request("divide");
            r.add_in_arg().insert_long(a);
            r.add_in_arg().insert_long(b);
            r.add_out_arg().type = TypeCodes.Simple(TCKind.tk_long);
            r.set_return_type(TypeCodes.Simple(TCKind.tk_long));
            r.exceptions.add(DivideByZeroHelper.type());
            return r;
        }

        [Theory]
        [InlineData(false)]
        [InlineData(true)]
        public void OutArgumentsAndUserExceptions(bool dsi)
        {
            Corba.Object target = dsi ? f.Dynamic : f.Static;
            Request r = Divide(target, 17, 5);
            Any rem = r.arguments.item(2).value;
            r.invoke();
            Assert.Equal(3, r.return_value().extract_long());
            Assert.Equal(2, rem.extract_long());
            Assert.Null(r.env.exception);
            Assert.Equal("divide", r.operation);
            Assert.Same(target, r.target);

            // Sent again with other arguments.
            r.arguments.item(0).value.insert_long(9);
            r.invoke();
            Assert.Equal(1, r.return_value().extract_long());
            Assert.Equal(4, rem.extract_long());

            Request z = Divide(target, 1, 0);
            var u = Assert.Throws<UnknownUserException>(() => z.invoke());
            Assert.Same(u, z.env.exception);
            Assert.IsType<DivideByZero>(DivideByZeroHelper.extract(u.except));

            // Not in the exception list: UNKNOWN.
            Request fail = target._request("fail");
            fail.add_in_arg().insert_long(1);
            fail.add_in_arg().insert_string("x");
            var e = Assert.Throws<UNKNOWN>(() => fail.invoke());
            Assert.Equal(0x4f4d0001u, e.minor);
            Assert.Same(e, fail.env.exception);
        }

        [Fact]
        public void StaticStubToDsi()
        {
            IEcho echo = EchoHelper.unchecked_narrow(f.Dynamic);
            Assert.Equal("hi", echo.echo_string("hi"));
            Assert.Equal(3, echo.divide(7, 2, out int rem));
            Assert.Equal(1, rem);
            Assert.Throws<DivideByZero>(() => echo.divide(1, 0, out _));
            var oops = Assert.Throws<Oops>(() => echo.fail(4, "four"));
            Assert.Equal(4, oops.code);
            int a = 1, b = 2;
            Assert.Equal(3, echo.swap(ref a, ref b));
            Assert.Equal((2, 1), (a, b));
            Assert.Throws<BAD_PARAM>(() => echo.make(1, out _, out _));
            Assert.Throws<BAD_OPERATION>(() => echo.echo_long(1));
        }

        [Fact]
        public void Deferred()
        {
            Request r = Divide(f.Static, 10, 3);
            Assert.Throws<BAD_INV_ORDER>(() => r.poll_response());
            r.send_deferred();
            Assert.Throws<BAD_INV_ORDER>(() => r.send_deferred());
            r.get_response();
            Assert.Equal(3, r.return_value().extract_long());

            var all = new[] { Divide(f.Static, 8, 2), Divide(f.Dynamic, 9, 2) };
            f.Orb.send_multiple_requests_deferred(all);
            var got = new HashSet<Request>();
            for (int i = 0; i < 2; i++)
            {
                got.Add(f.Orb.get_next_response());
            }
            Assert.Equal(new HashSet<Request>(all), got);
            Assert.Equal(4, all[0].return_value().extract_long());
            Assert.Equal(4, all[1].return_value().extract_long());
            Assert.Throws<BAD_INV_ORDER>(() => f.Orb.get_next_response());
        }

        [Fact]
        public void Lists()
        {
            NVList l = f.Orb.create_list(0);
            l.add_item("a", Corba.Constants.ARG_IN);
            var any = new Any();
            any.insert_long(5);
            NamedValue nv = l.add_value("b", any, Corba.Constants.ARG_INOUT);
            Assert.Equal(2u, l.count);
            Assert.Equal("b", l.item(1).name);
            Assert.Same(any, nv.value);
            Assert.Equal(Corba.Constants.ARG_INOUT, nv.flags);
            Assert.Throws<Bounds>(() => l.item(2));
            l.remove(0);
            Assert.Equal("b", l.item(0).name);
            Assert.Throws<Bounds>(() => l.remove(1));

            ExceptionList x = f.Orb.create_exception_list();
            x.add(OopsHelper.type());
            Assert.Equal(1u, x.count);
            Assert.True(x.item(0).equal(OopsHelper.type()));
            Assert.Throws<Bounds>(() => x.item(1));

            Assert.Throws<NO_IMPLEMENT>(() => f.Static._create_request(new object(), "x", null, null));
            Request r = f.Static._create_request(null, "echo_string", null,
                f.Orb.create_named_value("", new Any { type = TypeCodes.Simple(TCKind.tk_string) }, 0));
            r.add_in_arg().insert_string("y");
            r.invoke();
            Assert.Equal("y", r.result.value.extract_string());
        }

        [Fact]
        public void Oneway()
        {
            Request post = f.Static._request("post");
            post.add_in_arg().insert_string("dii");
            post.send_oneway();
            IEcho echo = EchoHelper.unchecked_narrow(f.Static);
            // posted() drains what it returns.
            var seen = new List<string>();
            for (int i = 0; i < 200 && !seen.Contains("dii"); i++)
            {
                seen.AddRange(echo.posted());
                Thread.Sleep(10);
            }
            Assert.Contains("dii", seen);
        }
    }
}
