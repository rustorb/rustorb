// Servant activators, servant locators and adapter activators, on the
// Target of interop/idl/PoaProbe.idl (interop/run-poa-probe.sh covers the
// behaviour against omniORB).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System.Collections.Generic;
using System.Text;
using PoaProbe;
using PortableServer;
using Xunit;

namespace Rustorb.Tests
{
    public sealed class ProbeTarget : TargetPOA
    {
        public override CallInfo work(int ms) => new CallInfo("", "", "", 0, 0);
        public override CallInfo relay(ITarget other, int ms, out string outer_thread)
        {
            outer_thread = "";
            return other.work(ms);
        }
        public override void post(int seq, int ms) { }
    }

    public sealed class ManagerFixture : System.IDisposable
    {
        public Corba.ORB Orb { get; } = Corba.ORB.init(new string[0]);
        public POA Root { get; }

        public ManagerFixture()
        {
            Root = POAHelper.narrow(Orb.resolve_initial_references("RootPOA"));
            Root.the_POAManager.activate();
        }

        public void Dispose()
        {
            Orb.shutdown(true);
            Orb.destroy();
        }
    }

    public class ServantManagerTests : IClassFixture<ManagerFixture>
    {
        private const string TargetId = "IDL:rustorb.org/PoaProbe/Target:1.0";
        private readonly ManagerFixture f;

        public ServantManagerTests(ManagerFixture fixture)
        {
            f = fixture;
        }

        private static byte[] Oid(string s) => Encoding.ASCII.GetBytes(s);

        private POA Child(string name, params Corba.Policy[] extra)
        {
            var p = new List<Corba.Policy>
            {
                f.Root.create_lifespan_policy(LifespanPolicyValue.PERSISTENT),
                f.Root.create_id_assignment_policy(IdAssignmentPolicyValue.USER_ID),
            };
            p.AddRange(extra);
            return f.Root.create_POA(name, f.Root.the_POAManager, p.ToArray());
        }

        private ITarget Ref(POA poa, string oid) =>
            TargetHelper.unchecked_narrow(f.Orb.string_to_object(f.Orb.object_to_string(
                poa.create_reference_with_id(Oid(oid), TargetId))));

        private sealed class Activator : ServantActivator
        {
            public readonly List<string> Calls = new();
            public Corba.IObject Forward;
            private readonly Servant servant = new ProbeTarget();

            public override Servant incarnate(byte[] oid, POA adapter)
            {
                string id = Encoding.ASCII.GetString(oid);
                lock (Calls) Calls.Add("incarnate " + id + " " + adapter.the_name);
                if (id == "bad")
                {
                    throw new Corba.OBJECT_NOT_EXIST("", 7, Corba.CompletionStatus.COMPLETED_NO);
                }
                if (id == "fwd")
                {
                    throw new ForwardRequest(Forward);
                }
                return servant;
            }

            public override void etherealize(byte[] oid, POA adapter, Servant serv, bool cleanup_in_progress,
                bool remaining_activations)
            {
                lock (Calls) Calls.Add($"etherealize {Encoding.ASCII.GetString(oid)} {cleanup_in_progress} {remaining_activations} {ReferenceEquals(serv, servant)}");
            }
        }

        [Fact]
        public void ServantActivator()
        {
            POA poa = Child("sm_activator",
                f.Root.create_request_processing_policy(RequestProcessingPolicyValue.USE_SERVANT_MANAGER));
            var a = new Activator();
            Assert.Null(poa.get_servant_manager());
            poa.set_servant_manager(a);
            Assert.Same(a, poa.get_servant_manager());
            Assert.Throws<Corba.BAD_INV_ORDER>(() => poa.set_servant_manager(new Activator()));

            ITarget t = Ref(poa, "x");
            t.work(0);
            t.work(0);
            Assert.Equal(new[] { "incarnate x sm_activator" }, a.Calls);
            Assert.IsType<ProbeTarget>(poa.id_to_servant(Oid("x")));

            var e = Assert.Throws<Corba.OBJECT_NOT_EXIST>(() => Ref(poa, "bad").work(0));
            Assert.Equal(7u, e.minor);

            POA plain = Child("sm_plain");
            plain.activate_object_with_id(Oid("obj"), new ProbeTarget());
            a.Forward = plain.id_to_reference(Oid("obj"));
            Ref(poa, "fwd").work(0);

            poa.deactivate_object(Oid("x"));
            // etherealize may come after deactivate_object returns.
            for (int i = 0; i < 200 && !a.Calls.Contains("etherealize x False False True"); i++)
            {
                System.Threading.Thread.Sleep(10);
            }
            Assert.Contains("etherealize x False False True", a.Calls);
            poa.destroy(true, true);
            plain.destroy(false, true);
        }

        private sealed class Locator : ServantLocator
        {
            public readonly List<string> Calls = new();
            private readonly Servant servant = new ProbeTarget();

            public override Servant preinvoke(byte[] oid, POA adapter, string operation, out object the_cookie)
            {
                string id = Encoding.ASCII.GetString(oid);
                the_cookie = new[] { id + "-cookie" };
                lock (Calls) Calls.Add($"preinvoke {id} {operation}");
                return servant;
            }

            public override void postinvoke(byte[] oid, POA adapter, string operation, object the_cookie,
                Servant the_servant)
            {
                lock (Calls) Calls.Add($"postinvoke {((string[])the_cookie)[0]} {operation} {ReferenceEquals(the_servant, servant)}");
            }
        }

        [Fact]
        public void ServantLocator()
        {
            POA poa = Child("sm_locator",
                f.Root.create_request_processing_policy(RequestProcessingPolicyValue.USE_SERVANT_MANAGER),
                f.Root.create_servant_retention_policy(ServantRetentionPolicyValue.NON_RETAIN));
            var l = new Locator();
            Assert.Throws<Corba.OBJ_ADAPTER>(() => poa.set_servant_manager(new Activator()));
            poa.set_servant_manager(l);
            Ref(poa, "y").work(0);
            Ref(poa, "y").post(1, 0);
            Assert.Equal(new[]
            {
                "preinvoke y work", "postinvoke y-cookie work True",
            }, l.Calls.GetRange(0, 2));
            poa.destroy(false, true);
        }

        private sealed class Lazy : AdapterActivator
        {
            public readonly List<string> Calls = new();

            public override bool unknown_adapter(POA parent, string name)
            {
                Calls.Add(name);
                if (!name.StartsWith("lazy"))
                {
                    return false;
                }
                POA p = parent.create_POA(name, parent.the_POAManager, new Corba.Policy[0]);
                return true;
            }
        }

        [Fact]
        public void AdapterActivator()
        {
            POA parent = Child("sm_parent");
            var a = new Lazy();
            Assert.Null(parent.the_activator);
            parent.the_activator = a;
            Assert.Same(a, parent.the_activator);
            POA lazy = parent.find_POA("lazy1", true);
            Assert.Equal("lazy1", lazy.the_name);
            Assert.Throws<POA.AdapterNonExistent>(() => parent.find_POA("other", true));
            Assert.Throws<POA.AdapterNonExistent>(() => parent.find_POA("lazy2", false));
            Assert.Equal(new[] { "lazy1", "other" }, a.Calls);
            parent.the_activator = null;
            Assert.Null(parent.the_activator);
            parent.destroy(false, true);
        }
    }
}
