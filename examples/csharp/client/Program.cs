// Looks "Echo" up in the naming service and calls it.
//
//   dotnet run --project client -- -ORBInitRef NameService=corbaloc::localhost:2809/NameService [--shutdown]

using System;
using System.Linq;
using CosNaming;
using Example;

static class Client
{
    static void Main(string[] args)
    {
        Corba.ORB orb = Corba.ORB.init(args, out string[] rest);
        bool shutdown = rest.Contains("--shutdown");

        INamingContextExt root = NamingContextExtHelper.narrow(orb.resolve_initial_references("NameService"));
        IEcho echo = EchoHelper.narrow(root.resolve_str("Echo"));

        Console.WriteLine("client: " + echo.echo("hello from C#"));
        if (shutdown)
        {
            echo.shutdown();
        }
        orb.destroy();
    }
}
