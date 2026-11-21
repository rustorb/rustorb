// The .NET InteropTest client and server (rustorbcs): `rustorbcs client
// ARGS` resolves the Echo object from the naming service, runs every case
// and exits non-zero if one fails; `rustorbcs server ARGS` serves Echo;
// `rustorbcs naming-check ARGS` runs the CosNaming sequence of
// run-naming-check.sh; `rustorbcs dynany-probe` the cases of
// run-dynany-probe.sh.
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
                default:
                    Console.Error.WriteLine("usage: rustorbcs client|server|naming-check|dynany-probe ARGS");
                    return 2;
            }
        }

        private static int Server(string[] argv)
        {
            Options opt = Common.ParseArgs("server", argv);
            Corba.ORB orb = Common.OrbInit(opt);
            PortableServer.POA poa = Common.PersistentPoa(orb, "InteropTest");
            byte[] oid = Common.Oid("Echo");
            poa.activate_object_with_id(oid, new EchoImpl(orb));
            Common.BindName(Common.RootContext(orb, opt.Timeout), opt.Name, poa.id_to_reference(oid));
            Console.WriteLine("server ready: " + opt.Name);
            Console.Out.Flush();
            orb.run();
            orb.destroy();
            Console.WriteLine("server stopped");
            return 0;
        }

        private static int Client(string[] argv)
        {
            Options opt = Common.ParseArgs("client", argv);
            Corba.ORB orb = Common.OrbInit(opt);
            int status;
            try
            {
                PortableServer.POA poa = Common.PersistentPoa(orb, "InteropTestClient");
                var root = Common.RootContext(orb, opt.Timeout);
                Corba.IObject obj = Common.Retry("the Echo server", () => Common.Resolve(root, opt.Name), opt.Timeout);
                IEcho echo = EchoHelper.narrow(obj);
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
