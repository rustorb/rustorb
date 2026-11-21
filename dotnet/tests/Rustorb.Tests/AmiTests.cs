// Callback AMI (docs/ami.md) through the ORB in one process, with the
// sendc_ operations and the AMI_EchoHandler that rustorb-idl -bcsharp -Wbami
// generates from interop/idl/InteropTest.idl (interop/run-ami-probe.sh covers
// the other ORBs).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Concurrent;
using System.Reflection;
using InteropTest;
using Xunit;

namespace Rustorb.Tests
{
    /// <summary>The handler: a proxy of the operations interface, behind the
    /// generated tie class, that puts the operation and what its reply brings
    /// into a queue (an exception holder as the exception it raises).</summary>
    public class EchoHandlerProxy : DispatchProxy
    {
        public static readonly BlockingCollection<(string, object[])> Got = new();

        protected override object Invoke(MethodInfo method, object[] args)
        {
            object[] a = args ?? new object[0];
            if (a.Length == 1 && a[0] is Messaging.ExceptionHolder h)
            {
                try
                {
                    h.raise_exception();
                    a = new object[] { null };
                }
                catch (Exception e)
                {
                    a = new object[] { e };
                }
            }
            Got.Add((method.Name, a));
            return null;
        }

        public static object[] Next(string op)
        {
            Assert.True(Got.TryTake(out var got, TimeSpan.FromSeconds(10)), "no reply");
            Assert.Equal(op, got.Item1);
            return got.Item2;
        }
    }

    public class AmiTests : IClassFixture<EchoFixture>
    {
        private readonly EchoFixture f;

        public AmiTests(EchoFixture fixture)
        {
            f = fixture;
        }

        [Fact]
        public void Callbacks()
        {
            var impl = DispatchProxy.Create<IAMI_EchoHandlerOperations, EchoHandlerProxy>();
            IAMI_EchoHandler h = AMI_EchoHandlerHelper.narrow(f.Poa.servant_to_reference(new AMI_EchoHandlerPOATie(impl)));
            // sendc_ is on the stub class.
            var echo = (Echo)f.Echo;

            echo.sendc_echo_long(h, 42);
            Assert.Equal(new object[] { 42 }, EchoHandlerProxy.Next("echo_long"));
            echo.sendc_echo_string(h, "hello AMI");
            Assert.Equal(new object[] { "hello AMI" }, EchoHandlerProxy.Next("echo_string"));
            echo.sendc_swap(h, 1, 2);
            Assert.Equal(new object[] { 3, 2, 1 }, EchoHandlerProxy.Next("swap"));
            echo.sendc_divide(h, 7, 2);
            Assert.Equal(new object[] { 3, 1 }, EchoHandlerProxy.Next("divide"));

            echo.sendc_divide(h, 1, 0);
            Assert.IsType<DivideByZero>(EchoHandlerProxy.Next("divide_excep")[0]);
            echo.sendc_fail(h, 7, "bad");
            var oops = Assert.IsType<Oops>(EchoHandlerProxy.Next("fail_excep")[0]);
            Assert.Equal((7, "bad"), (oops.code, oops.reason));
            echo.sendc_fail_system(h);
            Assert.IsType<Corba.BAD_PARAM>(EchoHandlerProxy.Next("fail_system_excep")[0]);

            echo.sendc_set_counter(h, 5);
            EchoHandlerProxy.Next("set_counter");
            echo.sendc_get_counter(h);
            Assert.Equal(new object[] { 5 }, EchoHandlerProxy.Next("get_counter"));

            // A nil handler: the request is sent, the reply dropped.
            echo.sendc_set_counter(null, 9);
            DateTime deadline = DateTime.UtcNow + TimeSpan.FromSeconds(10);
            while (echo.counter != 9 && DateTime.UtcNow < deadline)
            {
                System.Threading.Thread.Sleep(10);
            }
            Assert.Equal(9, echo.counter);
            Assert.Empty(EchoHandlerProxy.Got);
        }
    }
}
