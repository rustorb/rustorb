// The .NET InteropTest client and server (rustorbcs): `rustorbcs client
// ARGS` resolves the Echo object from the naming service, runs every case
// and exits non-zero if one fails; `rustorbcs server ARGS` serves Echo;
// `rustorbcs naming-check ARGS` runs the CosNaming sequence of
// run-naming-check.sh; `rustorbcs dynany-probe` the cases of
// run-dynany-probe.sh; `rustorbcs poa-probe-server` the server of
// run-poa-probe.sh; `rustorbcs pi-probe` the probe of run-pi-probe.sh.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Linq;
using InteropTest;

namespace Interop
{
    public static class Program
    {
        public static int Main(string[] argv)
        {
            string[] rest = argv.Skip(1).ToArray();
            switch (argv.Length > 0 ? argv[0] : "")
            {
                case "client": return Client(rest);
                case "server": return Server(rest);
                case "naming-check": return NamingCheck.Run(rest);
                case "dynany-probe": return DynAnyProbe.Run(rest);
                case "poa-probe-server": return PoaProbeServer.Run(rest);
                case "pi-probe": return PiProbeMain.Run(rest);
                default:
                    Console.Error.WriteLine("usage: rustorbcs client|server|naming-check|dynany-probe|poa-probe-server|pi-probe ARGS");
                    return 2;
            }
        }

        private static int Server(string[] argv)
        {
            Options opt = Common.ParseArgs("server", argv);
            Corba.ORB orb = Common.OrbInit(opt);
            if (opt.Suite == "any")
            {
                return AnyServer(orb, opt);
            }
            if (opt.Suite == "value" || opt.Suite == "custom" || opt.Suite == "abstract")
            {
                return ValueServer(orb, opt);
            }
            PortableServer.POA poa = Common.PersistentPoa(orb, "InteropTest");
            byte[] oid = Common.Oid("Echo");
            var impl = new EchoImpl(orb);
            if (Dii.Enabled)
            {
                poa.activate_object_with_id(oid, new DsiEcho(impl,
                    PortableServer.CurrentHelper.narrow(orb.resolve_initial_references("POACurrent"))));
            }
            else
            {
                poa.activate_object_with_id(oid, impl);
            }
            Common.BindName(Common.RootContext(orb, opt.Timeout), opt.Name, poa.id_to_reference(oid));
            Console.WriteLine("server ready: " + opt.Name);
            Console.Out.Flush();
            orb.run();
            orb.destroy();
            Console.WriteLine("server stopped");
            return 0;
        }

        /// <summary>The stage 2 suite: AnyTest::AnyEcho, with an Echo object
        /// next to it for the client to shut the server down.</summary>
        private static int AnyServer(Corba.ORB orb, Options opt)
        {
            AnyTests.Factory = DynamicAny.DynAnyFactoryHelper.narrow(orb.resolve_initial_references("DynAnyFactory"));
            PortableServer.POA poa = Common.PersistentPoa(orb, "AnyTest");
            byte[] anyOid = Common.Oid("AnyEcho");
            poa.activate_object_with_id(anyOid, new AnyTests.AnyEchoImpl());
            byte[] oid = Common.Oid("Echo");
            poa.activate_object_with_id(oid, new EchoImpl(orb));
            var root = Common.RootContext(orb, opt.Timeout);
            Common.BindName(root, opt.Name, poa.id_to_reference(anyOid));
            Common.BindName(root, opt.Name + "-Echo", poa.id_to_reference(oid));
            Console.WriteLine("server ready: " + opt.Name);
            Console.Out.Flush();
            orb.run();
            orb.destroy();
            Console.WriteLine("server stopped");
            return 0;
        }

        /// <summary>The value, custom and abstract suites: ValueTest::ValueEcho,
        /// CustomTest::CustomEcho or AbstractTest::AbstractEcho, which also
        /// stops the server.</summary>
        private static int ValueServer(Corba.ORB orb, Options opt)
        {
            string module;
            string oidName;
            PortableServer.Servant servant;
            switch (opt.Suite)
            {
                case "value":
                    ValueTests.RegisterFactories(orb);
                    (module, oidName, servant) = ("ValueTest", "ValueEcho", new ValueTests.ValueEchoImpl(orb));
                    break;
                case "custom":
                    CustomTests.RegisterFactories(orb);
                    (module, oidName, servant) = ("CustomTest", "CustomEcho", new CustomTests.CustomEchoImpl(orb));
                    break;
                default:
                    AbstractTests.RegisterFactories(orb);
                    (module, oidName, servant) = ("AbstractTest", "AbstractEcho", new AbstractTests.AbstractEchoImpl(orb));
                    break;
            }
            PortableServer.POA poa = Common.PersistentPoa(orb, module);
            byte[] oid = Common.Oid(oidName);
            poa.activate_object_with_id(oid, servant);
            Common.BindName(Common.RootContext(orb, opt.Timeout), opt.Name, poa.id_to_reference(oid));
            Console.WriteLine("server ready: " + opt.Name);
            Console.Out.Flush();
            orb.run();
            orb.destroy();
            Console.WriteLine("server stopped");
            return 0;
        }

        /// <summary>The value, custom and abstract suites against the object at
        /// opt.Name, which the client shuts down itself.</summary>
        private static int ValueClient(Corba.ORB orb, Options opt)
        {
            // For the abstract suite: the Person the client sends is
            // activated in the RootPOA (and the Counter of the value suite).
            Common.PersistentPoa(orb, "InteropTestClient");
            var root = Common.RootContext(orb, opt.Timeout);
            Corba.IObject obj = Common.Retry("the " + opt.Suite + " server", () => Common.Resolve(root, opt.Name), opt.Timeout);
            var r = new Runner();
            switch (opt.Suite)
            {
                case "value":
                {
                    ValueTests.RegisterFactories(orb);
                    ValueTest.IValueEcho echo = ValueTest.ValueEchoHelper.narrow(obj);
                    Common.Retry("the ValueEcho server", echo._non_existent, opt.Timeout);
                    ValueTests.Run(r, echo);
                    if (opt.ShutdownServer) echo.shutdown();
                    break;
                }
                case "custom":
                {
                    CustomTests.RegisterFactories(orb);
                    CustomTest.ICustomEcho echo = CustomTest.CustomEchoHelper.narrow(obj);
                    Common.Retry("the CustomEcho server", echo._non_existent, opt.Timeout);
                    CustomTests.Run(r, echo);
                    if (opt.ShutdownServer) echo.shutdown();
                    break;
                }
                default:
                {
                    AbstractTests.RegisterFactories(orb);
                    AbstractTest.IAbstractEcho echo = AbstractTest.AbstractEchoHelper.narrow(obj);
                    Common.Retry("the AbstractEcho server", echo._non_existent, opt.Timeout);
                    AbstractTests.Run(r, echo);
                    if (opt.ShutdownServer) echo.shutdown();
                    break;
                }
            }
            r.Summary();
            return r.Failed.Count == 0 ? 0 : 1;
        }

        /// <summary>The stage 2 suite against the AnyEcho object at
        /// opt.Name; the server's Echo object is at opt.Name + "-Echo".</summary>
        private static int AnyClient(Corba.ORB orb, Options opt)
        {
            AnyTests.Factory = DynamicAny.DynAnyFactoryHelper.narrow(orb.resolve_initial_references("DynAnyFactory"));
            var root = Common.RootContext(orb, opt.Timeout);
            Corba.IObject obj = Common.Retry("the AnyEcho server", () => Common.Resolve(root, opt.Name), opt.Timeout);
            AnyTest.IAnyEcho echo = AnyTest.AnyEchoHelper.narrow(obj);
            Common.Retry("the AnyEcho server", echo._non_existent, opt.Timeout);
            var r = new Runner();
            AnyTests.Run(r, echo);
            if (opt.ShutdownServer)
            {
                EchoHelper.narrow(Common.Resolve(root, opt.Name + "-Echo")).shutdown();
            }
            r.Summary();
            return r.Failed.Count == 0 ? 0 : 1;
        }

        private static int Client(string[] argv)
        {
            Options opt = Common.ParseArgs("client", argv);
            Corba.ORB orb = Common.OrbInit(opt);
            int status;
            try
            {
                if (opt.Suite != "basic")
                {
                    status = opt.Suite == "any" ? AnyClient(orb, opt) : ValueClient(orb, opt);
                    orb.destroy();
                    return status;
                }
                PortableServer.POA poa = Common.PersistentPoa(orb, "InteropTestClient");
                var root = Common.RootContext(orb, opt.Timeout);
                Corba.IObject obj = Common.Retry("the Echo server", () => Common.Resolve(root, opt.Name), opt.Timeout);
                IEcho echo = EchoHelper.narrow(obj);
                if (Dii.Enabled)
                {
                    echo = DiiEcho.Wrap(echo);
                }
                Common.Retry("the Echo server", echo._non_existent, opt.Timeout);

                var r = new Runner();
                Cases.Run(r, echo, poa);
                if (opt.ShutdownServer)
                {
                    echo.shutdown();
                }
                r.Summary();
                status = r.Failed.Count == 0 ? 0 : 1;
            }
            catch (Corba.SystemException e)
            {
                Console.Error.WriteLine("CORBA exception: " + e);
                status = 1;
            }
            orb.destroy();
            return status;
        }
    }
}
