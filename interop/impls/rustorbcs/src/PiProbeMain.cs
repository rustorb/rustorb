// The rustorb .NET side of run-pi-probe.sh: the probe of
// impls/rustorb/src/pi_probe.rs on PortableInterceptor and IOP of the .NET
// binding, printing the same lines.
//
//   rustorbcs pi-probe server [ORB options]       prints the IOR of an Echo
//                                                 object, then serves
//   rustorbcs pi-probe client IOR [ORB options]   calls echo
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

#nullable enable

using System;
using System.Collections.Generic;
using System.Linq;
using Corba;
using IOP;
using PortableInterceptor;
using SystemException = Corba.SystemException;

namespace Interop
{
    public static class PiProbeMain
    {
        private const uint CONTEXT = 0x50495052;
        private const uint TAG = 0x50495052;
        private const string NAME = "rustorbcs";

        private static Probe? probe;

        private static byte[] Encode(ICodec codec, string s)
        {
            var any = new Any();
            any.insert_string(s);
            return codec.encode_value(any);
        }

        private static string Decode(ICodec codec, byte[] data)
        {
            try
            {
                return codec.decode_value(data, Rustorb.TypeCodes.Simple(TCKind.tk_string)).extract_string();
            }
            catch (System.Exception e)
            {
                return "<" + e.GetType().Name + ">";
            }
        }

        private static string Id(System.Exception e) => e is SystemException s ? s._id() : e.ToString();

        private sealed class Probe : LocalObject, IClientRequestInterceptor, IServerRequestInterceptor, IIORInterceptor
        {
            private readonly ICodec codec;
            private readonly uint slot;
            public readonly List<string> Seen = new();

            public Probe(ICodec codec, uint slot)
            {
                this.codec = codec;
                this.slot = slot;
            }

            protected override string[] _ids() => new[] { "IDL:omg.org/PortableInterceptor/Interceptor:1.0" };

            public string name => "pi-probe";

            public void destroy() { }

            public void send_request(IClientRequestInfo ri) =>
                ri.add_request_service_context(new ServiceContext(CONTEXT, Encode(codec, "from " + NAME)), false);

            public void send_poll(IClientRequestInfo ri) { }

            public void receive_reply(IClientRequestInfo ri)
            {
                if (ri.operation != "echo")
                {
                    return;
                }
                string context, component;
                try
                {
                    context = Decode(codec, ri.get_reply_service_context(CONTEXT).context_data);
                }
                catch (SystemException e)
                {
                    context = Id(e);
                }
                try
                {
                    component = Decode(codec, ri.get_effective_component(TAG).component_data);
                }
                catch (SystemException e)
                {
                    component = Id(e);
                }
                lock (Seen)
                {
                    Seen.Add("reply context: " + context);
                    Seen.Add("component: " + component);
                }
            }

            public void receive_exception(IClientRequestInfo ri) { }

            public void receive_other(IClientRequestInfo ri) { }

            public void receive_request_service_contexts(IServerRequestInfo ri)
            {
                string got;
                try
                {
                    got = Decode(codec, ri.get_request_service_context(CONTEXT).context_data);
                }
                catch (SystemException e)
                {
                    got = Id(e);
                }
                // For clients that cannot read the reply context (run-pi-probe.sh).
                Console.WriteLine("request context: " + got);
                Console.Out.Flush();
                var any = new Any();
                any.insert_string(got);
                ri.set_slot(slot, any);
            }

            public void receive_request(IServerRequestInfo ri) { }

            public void send_reply(IServerRequestInfo ri)
            {
                string got;
                try
                {
                    got = ri.get_slot(slot).extract_string();
                }
                catch (System.Exception e)
                {
                    got = "<" + e.GetType().Name + ">";
                }
                ri.add_reply_service_context(new ServiceContext(CONTEXT, Encode(codec, NAME + " got: " + got)), false);
            }

            public void send_exception(IServerRequestInfo ri) { }

            public void send_other(IServerRequestInfo ri) { }

            public void establish_components(IIORInfo info) =>
                info.add_ior_component(new TaggedComponent(TAG, Encode(codec, NAME + " component")));
        }

        private sealed class Init : PortableInterceptor.ORBInitializer
        {
            public override void pre_init(IORBInitInfo info)
            {
                ICodec codec = info.codec_factory.create_codec(new IOP.Encoding(IOP.Constants.ENCODING_CDR_ENCAPS, 1, 2));
                probe = new Probe(codec, info.allocate_slot_id());
                info.add_client_request_interceptor(probe);
                info.add_server_request_interceptor(probe);
                info.add_ior_interceptor(probe);
            }
        }

        private sealed class Echo : PiProbe.EchoPOA
        {
            public override string echo(string s) => s;
        }

        public static int Run(string[] args)
        {
            PortableInterceptor.ORBInitializerRegistry.register_orb_initializer(new Init());
            string mode = args.Length > 0 ? args[0] : "";
            if (mode == "server")
            {
                ORB orb = ORB.init(args.Skip(1).ToArray());
                var root = PortableServer.POAHelper.narrow(orb.resolve_initial_references("RootPOA"))!;
                root.the_POAManager.activate();
                Console.WriteLine(orb.object_to_string(root.servant_to_reference(new Echo())));
                Console.Out.Flush();
                orb.run();
                return 0;
            }
            if (mode == "client" && args.Length >= 2)
            {
                ORB orb = ORB.init(args.Skip(2).ToArray());
                string result;
                try
                {
                    result = PiProbe.EchoHelper.narrow(orb.string_to_object(args[1]))!.echo("hello");
                }
                catch (System.Exception e)
                {
                    result = Id(e);
                }
                lock (probe!.Seen)
                {
                    foreach (string line in probe.Seen)
                    {
                        Console.WriteLine(line);
                    }
                }
                Console.WriteLine("result: " + result);
                orb.destroy();
                return 0;
            }
            Console.Error.WriteLine("usage: rustorbcs pi-probe server|client IOR [ORB options]");
            return 2;
        }
    }
}
