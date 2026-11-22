// rustorb .NET server for the POA and thread model probes (PoaProbe.idl).
//
//   rustorbcs poa-probe-server --host HOST --port N --ior-file FILE [ORB options]
//
// The same objects and POAs as impls/omniorbpy/poa_probe_server.py, on the
// IDL4 to C# mapping; see run-poa-probe.sh.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

#nullable enable

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Threading;
using PoaProbe;
using PortableServer;

namespace Interop
{
    public static class PoaProbeServer
    {
        private static readonly Stopwatch clock = Stopwatch.StartNew();
        private const string TARGET_ID = "IDL:rustorb.org/PoaProbe/Target:1.0";
        private static Thread mainThread = null!;
        private static readonly Dictionary<Thread, string> labels = new();
        private static readonly List<string> events = new();

        private static double Now() => clock.Elapsed.TotalSeconds;

        private static string ThreadLabel()
        {
            Thread t = Thread.CurrentThread;
            if (t == mainThread)
            {
                return "main";
            }
            lock (labels)
            {
                if (!labels.TryGetValue(t, out string? l))
                {
                    l = "T" + (labels.Count + 1);
                    labels[t] = l;
                }
                return l;
            }
        }

        private static void Event(string text)
        {
            lock (events)
            {
                events.Add(text);
            }
        }

        private static string Str(byte[] oid) => Encoding.Latin1.GetString(oid);
        private static byte[] Oid(string s) => Encoding.Latin1.GetBytes(s);

        private static void Sleep(int ms)
        {
            if (ms > 0)
            {
                Thread.Sleep(ms);
            }
        }

        private static string PoaPath(POA? poa)
        {
            var names = new List<string>();
            while (poa != null)
            {
                POA? parent = poa.the_parent;
                if (parent == null)
                {
                    break;
                }
                names.Insert(0, poa.the_name);
                poa = parent;
            }
            return names.Count == 0 ? "RootPOA" : string.Join("/", names);
        }

        private sealed class TargetImpl : TargetPOA
        {
            private readonly Current current;

            internal TargetImpl(Current current)
            {
                this.current = current;
            }

            public override CallInfo work(int ms)
            {
                double start = Now();
                string thread = ThreadLabel();
                string id;
                string poa;
                try
                {
                    id = Str(current.get_object_id());
                    poa = PoaPath(current.get_POA());
                }
                catch (Current.NoContext)
                {
                    throw new Corba.INTERNAL("no context", 0, Corba.CompletionStatus.COMPLETED_NO);
                }
                Sleep(ms);
                return new CallInfo(thread, id, poa, start, Now());
            }

            public override void post(int seq, int ms)
            {
                Sleep(ms);
                Event("post " + seq);
            }

            public override CallInfo relay(ITarget? other, int ms, out string outer_thread)
            {
                outer_thread = ThreadLabel();
                return other!.work(ms);
            }
        }

        private sealed class Activator : ServantActivator
        {
            private readonly Servant servant;
            private readonly Corba.IObject forward;

            internal Activator(Servant servant, Corba.IObject forward)
            {
                this.servant = servant;
                this.forward = forward;
            }

            public override Servant incarnate(byte[] oid, POA adapter)
            {
                string name = Str(oid);
                Event("incarnate " + name);
                if (name == "bad")
                {
                    throw new Corba.OBJECT_NOT_EXIST("", 0, Corba.CompletionStatus.COMPLETED_NO);
                }
                if (name == "fwd")
                {
                    throw new ForwardRequest(forward);
                }
                return servant;
            }

            public override void etherealize(byte[] oid, POA adapter, Servant serv, bool cleanup_in_progress,
                bool remaining_activations)
            {
                Event("etherealize " + Str(oid));
            }
        }

        private sealed class Locator : ServantLocator
        {
            private readonly Servant servant;
            private readonly Corba.IObject forward;

            internal Locator(Servant servant, Corba.IObject forward)
            {
                this.servant = servant;
                this.forward = forward;
            }

            public override Servant preinvoke(byte[] oid, POA adapter, string operation, out object? the_cookie)
            {
                the_cookie = null;
                string name = Str(oid);
                Event("preinvoke " + name + " " + operation);
                if (name == "bad")
                {
                    throw new Corba.OBJECT_NOT_EXIST("", 0, Corba.CompletionStatus.COMPLETED_NO);
                }
                if (name == "fwd")
                {
                    throw new ForwardRequest(forward);
                }
                return servant;
            }

            public override void postinvoke(byte[] oid, POA adapter, string operation, object? the_cookie,
                Servant the_servant)
            {
                Event("postinvoke " + Str(oid) + " " + operation);
            }
        }

        /// <summary>Creates POAs named lazy* on demand, with a default servant.</summary>
        private sealed class LazyActivator : AdapterActivator
        {
            private readonly Servant servant;

            internal LazyActivator(Servant servant)
            {
                this.servant = servant;
            }

            public override bool unknown_adapter(POA parent, string name)
            {
                Event("unknown_adapter " + name);
                if (!name.StartsWith("lazy", StringComparison.Ordinal))
                {
                    return false;
                }
                try
                {
                    POA poa = parent.create_POA(name, parent.the_POAManager, new Corba.Policy[]
                    {
                        parent.create_lifespan_policy(LifespanPolicyValue.PERSISTENT),
                        parent.create_id_assignment_policy(IdAssignmentPolicyValue.USER_ID),
                        parent.create_servant_retention_policy(ServantRetentionPolicyValue.NON_RETAIN),
                        parent.create_request_processing_policy(RequestProcessingPolicyValue.USE_DEFAULT_SERVANT),
                        parent.create_id_uniqueness_policy(IdUniquenessPolicyValue.MULTIPLE_ID),
                    });
                    poa.set_servant(servant);
                    return true;
                }
                catch (Exception e)
                {
                    Console.Error.WriteLine("unknown_adapter: " + e);
                    return false;
                }
            }
        }

        private sealed class ControlImpl : ControlPOA
        {
            private readonly Corba.ORB orb;
            private readonly POA root;
            private readonly POAManager managed;
            private readonly Current current;
            private readonly Dictionary<string, Corba.Object> systemRefs;

            internal ControlImpl(Corba.ORB orb, POA root, POAManager managed, Current current,
                Dictionary<string, Corba.Object> systemRefs)
            {
                this.orb = orb;
                this.root = root;
                this.managed = managed;
                this.current = current;
                this.systemRefs = systemRefs;
            }

            private POA Find(string path)
            {
                POA poa = root;
                foreach (string name in path.Split('/'))
                {
                    if (name.Length == 0 || name == "RootPOA")
                    {
                        continue;
                    }
                    try
                    {
                        poa = poa.find_POA(name, true);
                    }
                    catch (POA.AdapterNonExistent)
                    {
                        throw new Failed("no POA " + path);
                    }
                }
                return poa;
            }

            public override string reference(string poa, string oid)
            {
                if (!systemRefs.TryGetValue(poa, out Corba.Object? r))
                {
                    r = Find(poa).create_reference_with_id(Oid(oid), TARGET_ID);
                }
                return orb.object_to_string(r);
            }

            public override void set_state(string state)
            {
                try
                {
                    switch (state)
                    {
                        case "active": managed.activate(); break;
                        case "holding": managed.hold_requests(false); break;
                        case "discarding": managed.discard_requests(false); break;
                        case "inactive": managed.deactivate(false, false); break;
                        default: throw new Failed("unknown state " + state);
                    }
                }
                catch (POAManager.AdapterInactive)
                {
                    throw new Failed("AdapterInactive");
                }
            }

            public override void deactivate_object(string poa, string oid)
            {
                try
                {
                    Find(poa).deactivate_object(Oid(oid));
                }
                catch (POA.ObjectNotActive)
                {
                    throw new Failed("ObjectNotActive");
                }
                catch (POA.WrongPolicy)
                {
                    throw new Failed("WrongPolicy");
                }
            }

            public override void activate_object(string poa, string oid)
            {
                try
                {
                    Find(poa).activate_object_with_id(Oid(oid), new TargetImpl(current));
                }
                catch (POA.ObjectAlreadyActive)
                {
                    throw new Failed("ObjectAlreadyActive");
                }
                catch (POA.ServantAlreadyActive)
                {
                    throw new Failed("ServantAlreadyActive");
                }
                catch (POA.WrongPolicy)
                {
                    throw new Failed("WrongPolicy");
                }
            }

            public override void destroy_poa(string poa) => Find(poa).destroy(true, false);

            public override Omg.Types.ISequence<string> events()
            {
                lock (PoaProbeServer.events)
                {
                    var r = new Omg.Types.Sequence<string>(PoaProbeServer.events);
                    PoaProbeServer.events.Clear();
                    return r;
                }
            }

            public override string shutdown_inside(bool wait_for_completion)
            {
                try
                {
                    orb.shutdown(wait_for_completion);
                }
                catch (Corba.SystemException e)
                {
                    return $"{e._id()} minor 0x{e.minor:x8}";
                }
                return "";
            }

            public override void shutdown_later(int delay_ms)
            {
                var t = new Thread(() =>
                {
                    Sleep(delay_ms);
                    orb.shutdown(false);
                })
                { IsBackground = true };
                t.Start();
            }
        }

        private static Corba.Policy[] Policies(POA root, params Corba.Policy[] extra)
        {
            var p = new Corba.Policy[2 + extra.Length];
            p[0] = root.create_lifespan_policy(LifespanPolicyValue.PERSISTENT);
            p[1] = root.create_id_assignment_policy(IdAssignmentPolicyValue.USER_ID);
            extra.CopyTo(p, 2);
            return p;
        }

        public static int Run(string[] argv)
        {
            mainThread = Thread.CurrentThread;
            string host = "";
            string? iorFile = null;
            int port = 0;
            var orbArgs = new List<string>();
            for (int i = 0; i < argv.Length; i++)
            {
                string a = argv[i];
                if (a == "--host" && i + 1 < argv.Length)
                {
                    host = argv[++i];
                }
                else if (a == "--port" && i + 1 < argv.Length)
                {
                    port = int.Parse(argv[++i], System.Globalization.CultureInfo.InvariantCulture);
                }
                else if (a == "--ior-file" && i + 1 < argv.Length)
                {
                    iorFile = argv[++i];
                }
                else
                {
                    orbArgs.Add(a);
                }
            }
            if (iorFile == null)
            {
                Console.Error.WriteLine("usage: poa-probe-server --ior-file FILE [--host H] [--port N]");
                return 2;
            }
            if (port != 0)
            {
                orbArgs.Add("-ORBendPoint");
                orbArgs.Add($"giop:tcp:{host}:{port}");
            }

            Corba.ORB orb = Corba.ORB.init(orbArgs.ToArray());
            POA root = POAHelper.narrow(orb.resolve_initial_references("RootPOA"))!;
            Current current = CurrentHelper.narrow(orb.resolve_initial_references("POACurrent"))!;
            POAManager rootManager = root.the_POAManager;
            rootManager.activate();

            var systemRefs = new Dictionary<string, Corba.Object>();
            var servant = new TargetImpl(current);
            // RootPOA: TRANSIENT, SYSTEM_ID, IMPLICIT_ACTIVATION.
            systemRefs["RootPOA"] = root.servant_to_reference(servant);

            POA persistentUser = root.create_POA("persistent_user", rootManager, Policies(root));
            persistentUser.activate_object_with_id(Oid("obj"), servant);
            persistentUser.activate_object_with_id(Oid("victim"), new TargetImpl(current));
            POA nested = persistentUser.create_POA("nested", rootManager, Policies(root));
            nested.activate_object_with_id(Oid("obj"), servant);

            POA persistentSystem = root.create_POA("persistent_system", rootManager, new Corba.Policy[]
            {
                root.create_lifespan_policy(LifespanPolicyValue.PERSISTENT),
                root.create_id_assignment_policy(IdAssignmentPolicyValue.SYSTEM_ID),
            });
            byte[] id = persistentSystem.activate_object(new TargetImpl(current));
            systemRefs["persistent_system"] = persistentSystem.id_to_reference(id);

            POA transientUser = root.create_POA("transient_user", rootManager, new Corba.Policy[]
            {
                root.create_lifespan_policy(LifespanPolicyValue.TRANSIENT),
                root.create_id_assignment_policy(IdAssignmentPolicyValue.USER_ID),
            });
            transientUser.activate_object_with_id(Oid("obj"), servant);

            POA transientSystem = root.create_POA("transient_system", rootManager, new Corba.Policy[]
            {
                root.create_lifespan_policy(LifespanPolicyValue.TRANSIENT),
                root.create_id_assignment_policy(IdAssignmentPolicyValue.SYSTEM_ID),
            });
            id = transientSystem.activate_object(new TargetImpl(current));
            systemRefs["transient_system"] = transientSystem.id_to_reference(id);

            foreach (string name in new[] { "single", "single2" })
            {
                POA poa = root.create_POA(name, rootManager, Policies(root,
                    root.create_thread_policy(ThreadPolicyValue.SINGLE_THREAD_MODEL)));
                poa.activate_object_with_id(Oid("a"), servant);
                poa.activate_object_with_id(Oid("b"), new TargetImpl(current));
            }

            POA mainPoa = root.create_POA("main", rootManager, Policies(root,
                root.create_thread_policy(ThreadPolicyValue.MAIN_THREAD_MODEL)));
            mainPoa.activate_object_with_id(Oid("a"), servant);
            mainPoa.activate_object_with_id(Oid("b"), new TargetImpl(current));

            POA managed = root.create_POA("managed", null, Policies(root));
            POAManager managedManager = managed.the_POAManager;
            managed.activate_object_with_id(Oid("obj"), servant);

            POA doomed = root.create_POA("doomed", rootManager, Policies(root));
            doomed.activate_object_with_id(Oid("obj"), servant);

            Corba.Object forward = persistentUser.id_to_reference(Oid("obj"));
            POA activator = root.create_POA("activator", rootManager, Policies(root,
                root.create_request_processing_policy(RequestProcessingPolicyValue.USE_SERVANT_MANAGER)));
            activator.set_servant_manager(new Activator(servant, forward));
            POA locator = root.create_POA("locator", rootManager, Policies(root,
                root.create_request_processing_policy(RequestProcessingPolicyValue.USE_SERVANT_MANAGER),
                root.create_servant_retention_policy(ServantRetentionPolicyValue.NON_RETAIN)));
            locator.set_servant_manager(new Locator(servant, forward));

            POA dflt = root.create_POA("default", rootManager, Policies(root,
                root.create_request_processing_policy(RequestProcessingPolicyValue.USE_DEFAULT_SERVANT),
                root.create_servant_retention_policy(ServantRetentionPolicyValue.NON_RETAIN),
                root.create_id_uniqueness_policy(IdUniquenessPolicyValue.MULTIPLE_ID)));
            dflt.set_servant(servant);

            root.the_activator = new LazyActivator(servant);

            // The Control object has its own POAManager, so that it keeps
            // working whatever the probe does to the others.
            POA controlPoa = root.create_POA("control", null, Policies(root));
            controlPoa.activate_object_with_id(Oid("Control"),
                new ControlImpl(orb, root, managedManager, current, systemRefs));
            controlPoa.the_POAManager.activate();
            managedManager.activate();

            string ior = orb.object_to_string(controlPoa.id_to_reference(Oid("Control")));
            string tmp = iorFile + ".tmp";
            File.WriteAllText(tmp, ior + "\n", Encoding.ASCII);
            File.Move(tmp, iorFile, true);
            Console.WriteLine("server ready");
            Console.Out.Flush();

            orb.run();
            Console.WriteLine("server stopped");
            Console.Out.Flush();
            orb.destroy();
            return 0;
        }
    }
}
