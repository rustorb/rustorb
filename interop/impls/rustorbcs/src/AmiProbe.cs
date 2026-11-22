// The rustorb .NET side of run-ami-probe.sh: callback AMI (docs/ami.md)
// against the Echo server of any implementation, printing the same lines
// as impls/rustorbpy/ami_probe.py. On the stubs of InteropTest.idl
// generated with -Wbami (build.sh).
//
//   rustorbcs ami-probe REFERENCE [ORB options]
//
// REFERENCE names the Echo object, such as corbaname::127.0.0.1:2809#ami/Echo.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Concurrent;
using System.Linq;
using System.Reflection;
using System.Threading;
using InteropTest;

namespace Interop
{
    public static class AmiProbe
    {
        private static readonly TimeSpan Wait = TimeSpan.FromSeconds(10);
        private static readonly BlockingCollection<string> lines = new();
        private static int status;

        /// <summary>What raise_exception() throws, as a probe line.</summary>
        private static string Describe(Messaging.ExceptionHolder holder)
        {
            if (holder == null)
            {
                return "nil holder";
            }
            try
            {
                holder.raise_exception();
                return "nothing raised";
            }
            catch (Oops e)
            {
                return "Oops " + e.code + " " + e.reason;
            }
            catch (DivideByZero)
            {
                return "DivideByZero";
            }
            catch (Corba.SystemException e)
            {
                return e.GetType().Name + " " + e.minor + " " + e.completed;
            }
            catch (Exception e)
            {
                return "unexpected " + e;
            }
        }

        /// <summary>The handler: the operations the probe expects put their
        /// line into the queue; any other one puts "unexpected OP". A proxy of
        /// the operations interface, behind the generated tie class.</summary>
        public class Handler : DispatchProxy
        {
            protected override object Invoke(MethodInfo method, object[] args)
            {
                string op = method.Name;
                object[] a = args ?? new object[0];
                switch (op)
                {
                    case "echo_long":
                    case "echo_string":
                    case "get_counter":
                    case "get_name":
                        lines.Add(op + ": " + a[0]);
                        break;
                    case "swap":
                        lines.Add(op + ": " + a[0] + " " + a[1] + " " + a[2]);
                        break;
                    case "divide":
                        lines.Add(op + ": " + a[0] + " " + a[1]);
                        break;
                    case "set_counter":
                        lines.Add(op);
                        break;
                    case "divide_excep":
                    case "fail_excep":
                    case "fail_system_excep":
                    case "trigger_excep":
                        lines.Add(op + ": " + Describe((Messaging.ExceptionHolder)a[0]));
                        break;
                    default:
                        lines.Add("unexpected " + op);
                        break;
                }
                return null;
            }
        }

        /// <summary>Answers after 2 s, longer than the call timeout of the last case.</summary>
        private sealed class SlowCallback : CallbackPOA
        {
            public override string notify(string message)
            {
                Thread.Sleep(2000);
                return "ack:" + message;
            }
        }

        private static void Show(string what)
        {
            if (!lines.TryTake(out string line, Wait))
            {
                line = what + ": no reply";
                status = 1;
            }
            Console.WriteLine(line);
        }

        public static int Run(string[] argv)
        {
            if (argv.Length < 1)
            {
                Console.Error.WriteLine("usage: rustorbcs ami-probe REFERENCE [ORB options]");
                return 2;
            }
            Corba.ORB orb = Corba.ORB.init(new[] { "-ORBendPoint", "giop:tcp:127.0.0.1:" }.Concat(argv.Skip(1)).ToArray());
            var poa = PortableServer.POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
            poa.the_POAManager.activate();
            // sendc_ is on the stub class.
            var echo = (Echo)EchoHelper.narrow(orb.string_to_object(argv[0]));
            var impl = DispatchProxy.Create<IAMI_EchoHandlerOperations, Handler>();
            IAMI_EchoHandler h = AMI_EchoHandlerHelper.narrow(poa.servant_to_reference(new AMI_EchoHandlerPOATie(impl)));

            echo.sendc_echo_long(h, 42);
            Show("echo_long");
            echo.sendc_echo_string(h, "hello AMI");
            Show("echo_string");
            echo.sendc_swap(h, 1, 2);
            Show("swap");
            echo.sendc_divide(h, 7, 2);
            Show("divide");
            echo.sendc_divide(h, 1, 0);
            Show("divide_excep");
            echo.sendc_fail(h, 7, "bad");
            Show("fail_excep");
            echo.sendc_fail_system(h);
            Show("fail_system_excep");
            echo.sendc_set_counter(h, 5);
            Show("set_counter");
            echo.sendc_get_counter(h);
            Show("get_counter");
            echo.sendc_get_name(h);
            Show("get_name");
            // A nil handler: the request is sent, the reply dropped.
            echo.sendc_set_counter(null, 9);
            DateTime deadline = DateTime.UtcNow + Wait;
            while (echo.counter != 9 && DateTime.UtcNow < deadline)
            {
                Thread.Sleep(10);
            }
            Console.WriteLine("nil handler: counter " + echo.counter);
            // The server calls the slow callback; the call times out first.
            ICallback cb = CallbackHelper.narrow(poa.servant_to_reference(new SlowCallback()));
            echo.register_callback(cb);
            echo._call_timeout_us = 500_000;
            echo.sendc_trigger(h, "late");
            echo._call_timeout_us = 0;
            Show("trigger_excep");
            // Let the server's call of the callback finish before going away.
            Thread.Sleep(2000);
            orb.destroy();
            return status;
        }
    }
}
