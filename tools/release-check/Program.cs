// The release workflow builds this against the Rustorb package it made and
// runs it: the native library comes from the package's runtimes/<rid>/native.
using System;

static class Check
{
    static void Main(string[] args)
    {
        Corba.ORB orb = Corba.ORB.init(args, out string[] _);
        orb.resolve_initial_references("RootPOA");
        orb.destroy();
        Console.WriteLine("Rustorb: ok");
    }
}
