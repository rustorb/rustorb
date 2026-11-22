// Serves Example::Echo and binds it in the naming service as "Echo".
//
//   rustorb-idl -bcsharp -C gen ../idl/Echo.idl
//   dotnet run --project server -- -ORBInitRef NameService=corbaloc::localhost:2809/NameService

using System;
using CosNaming;
using Example;

sealed class EchoImpl : EchoPOA
{
    private readonly Corba.ORB orb;

    public EchoImpl(Corba.ORB orb)
    {
        this.orb = orb;
    }

    public override string echo(string message)
    {
        Console.WriteLine("server: " + message);
        return message;
    }

    public override void shutdown() => orb.shutdown(false);
}

static class Server
{
    static void Main(string[] args)
    {
        Corba.ORB orb = Corba.ORB.init(args);
        var poa = PortableServer.POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
        poa.the_POAManager.activate();
        IEcho echo = EchoHelper.narrow(poa.servant_to_reference(new EchoImpl(orb)));

        INamingContext root = NamingContextHelper.narrow(orb.resolve_initial_references("NameService"));
        root.rebind(new Omg.Types.Sequence<NameComponent>(new[] { new NameComponent("Echo", "") }), echo);
        Console.WriteLine("server: ready");

        orb.run();
        orb.destroy();
    }
}
