// Helpers shared by the .NET InteropTest server and client (rustorbcs): the
// same options and naming service handling as the other implementations,
// on the IDL4 to C# mapping.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Text;
using System.Threading;
using CosNaming;

namespace Interop
{
    public sealed class Options
    {
        public string Naming;
        public string Name;
        public string Host = "";
        public int Port;
        public double Timeout = 30.0;
        public bool ShutdownServer;
        public string Suite = "basic";
        public readonly List<string> OrbArgs = new List<string>();
    }

    public static class Common
    {
        /// <summary>Parses the options shared with the other
        /// implementations; unknown arguments are passed to ORB.init.</summary>
        public static Options ParseArgs(string prog, string[] argv)
        {
            var opt = new Options();
            for (int i = 0; i < argv.Length; ++i)
            {
                string a = argv[i];
                switch (a)
                {
                    case "--naming": opt.Naming = Value(argv, ++i, a); break;
                    case "--name": opt.Name = Value(argv, ++i, a); break;
                    case "--host": opt.Host = Value(argv, ++i, a); break;
                    case "--port": opt.Port = int.Parse(Value(argv, ++i, a), CultureInfo.InvariantCulture); break;
                    case "--timeout": opt.Timeout = double.Parse(Value(argv, ++i, a), CultureInfo.InvariantCulture); break;
                    case "--shutdown-server": opt.ShutdownServer = true; break;
                    case "--suite":
                        opt.Suite = Value(argv, ++i, a);
                        if (Array.IndexOf(new[] { "basic", "any", "value", "custom", "abstract" }, opt.Suite) < 0)
                        {
                            Console.Error.WriteLine("unknown suite " + opt.Suite);
                            Environment.Exit(2);
                        }
                        break;
                    default: opt.OrbArgs.Add(a); break;
                }
            }
            if (opt.Naming == null || opt.Name == null)
            {
                Console.Error.WriteLine("usage: " + prog
                    + " --naming CORBALOC --name PATH [--host H] [--port N] [--timeout S]"
                    + " [--shutdown-server] [--suite basic|any|value|custom|abstract] [ORB options]");
                Environment.Exit(2);
            }
            return opt;
        }

        private static string Value(string[] argv, int i, string option)
        {
            if (i >= argv.Length)
            {
                Console.Error.WriteLine("missing value for " + option);
                Environment.Exit(2);
            }
            return argv[i];
        }

        /// <summary>The ORB, with the naming service, the endpoint and the
        /// GIOP limit, as the other rustorb implementations make it.</summary>
        public static Corba.ORB OrbInit(Options opt)
        {
            var args = new List<string>(opt.OrbArgs) { "-ORBInitRef", "NameService=" + opt.Naming };
            // INTEROP_SSL=DIR (run-matrix.sh --ssl): TLS only, certificates
            // required both ways, as omniORB requires by default.
            string ssl = Environment.GetEnvironmentVariable("INTEROP_SSL");
            bool tls = !string.IsNullOrEmpty(ssl);
            if (tls)
            {
                args.AddRange(new[]
                {
                    "-ORBsslCAFile", ssl + "/ca.pem", "-ORBsslKeyFile", ssl + "/server-bundle.pem",
                    "-ORBsslVerifyClient", "required",
                });
            }
            // INTEROP_UNIX=DIR (run-matrix.sh --unix): a Unix socket in DIR.
            string unix = Environment.GetEnvironmentVariable("INTEROP_UNIX");
            if (opt.Port != 0 && !string.IsNullOrEmpty(unix))
            {
                args.Add("-ORBendPoint");
                args.Add("giop:unix:" + unix + "/" + opt.Host + "-" + opt.Port + ".sock");
            }
            else if (opt.Port != 0)
            {
                args.Add("-ORBendPoint");
                args.Add((tls ? "giop:ssl:" : "giop:tcp:") + opt.Host + ":" + opt.Port);
            }
            string giop = Environment.GetEnvironmentVariable("ORBmaxGIOPVersion");
            if (!string.IsNullOrEmpty(giop))
            {
                args.Add("-ORBmaxGIOPVersion");
                args.Add(giop);
            }
            // run-matrix.sh --fragment N: rustorb splits GIOP 1.2 messages.
            string frag = Environment.GetEnvironmentVariable("INTEROP_FRAGMENT_SIZE");
            if (!string.IsNullOrEmpty(frag))
            {
                args.Add("-ORBgiopFragmentSize");
                args.Add(frag);
            }
            return Corba.ORB.init(args.ToArray());
        }

        public static NameComponent[] ToName(string path)
        {
            var name = new List<NameComponent>();
            foreach (string part in path.Split('/'))
            {
                if (part.Length > 0)
                {
                    name.Add(new NameComponent(part, ""));
                }
            }
            return name.ToArray();
        }

        private static Omg.Types.ISequence<NameComponent> Seq(params NameComponent[] n) =>
            new Omg.Types.Sequence<NameComponent>(n);

        /// <summary>Calls fn until it stops throwing "not there yet"
        /// exceptions or the timeout passes (then exits with 1).</summary>
        public static T Retry<T>(string what, Func<T> fn, double timeout)
        {
            var clock = Stopwatch.StartNew();
            for (;;)
            {
                try
                {
                    return fn();
                }
                catch (Corba.TRANSIENT)
                {
                }
                catch (Corba.COMM_FAILURE)
                {
                }
                catch (Corba.OBJECT_NOT_EXIST)
                {
                }
                catch (NamingContext.NotFound)
                {
                }
                if (clock.Elapsed.TotalSeconds >= timeout)
                {
                    Console.Error.WriteLine("timed out waiting for " + what);
                    Environment.Exit(1);
                }
                Thread.Sleep(200);
            }
        }

        public static INamingContext RootContext(Corba.ORB orb, double timeout) =>
            Retry("the naming service", () =>
                NamingContextHelper.narrow(orb.resolve_initial_references("NameService"))
                ?? throw new Corba.TRANSIENT(), timeout);

        public static Corba.IObject Resolve(INamingContext root, string path) =>
            root.resolve(Seq(ToName(path)));

        /// <summary>Binds obj at path, creating intermediate contexts as needed.</summary>
        public static void BindName(INamingContext root, string path, Corba.IObject obj)
        {
            NameComponent[] name = ToName(path);
            INamingContext ctx = root;
            for (int i = 0; i + 1 < name.Length; ++i)
            {
                try
                {
                    ctx = ctx.bind_new_context(Seq(name[i]));
                }
                catch (NamingContext.AlreadyBound)
                {
                    ctx = NamingContextHelper.narrow(ctx.resolve(Seq(name[i])));
                }
            }
            ctx.rebind(Seq(name[name.Length - 1]), obj);
        }

        /// <summary>A PERSISTENT / USER_ID child POA, so that object keys are
        /// fixed and GIOP captures are reproducible between runs.</summary>
        public static PortableServer.POA PersistentPoa(Corba.ORB orb, string name)
        {
            var root = PortableServer.POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
            PortableServer.POAManager mgr = root.the_POAManager;
            var poa = root.create_POA(name, mgr, new Corba.Policy[]
            {
                root.create_lifespan_policy(PortableServer.LifespanPolicyValue.PERSISTENT),
                root.create_id_assignment_policy(PortableServer.IdAssignmentPolicyValue.USER_ID),
            });
            mgr.activate();
            return poa;
        }

        public static byte[] Oid(string s) => Encoding.ASCII.GetBytes(s);
    }
}
