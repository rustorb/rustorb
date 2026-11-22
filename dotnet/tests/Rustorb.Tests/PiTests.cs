// Portable Interceptors and IOP::Codec: an ORB with request and IOR
// interceptors calling the Echo servant and the DSI servant of DiiTests
// (interop/run-pi-probe.sh covers the behaviour against JacORB and omniORB).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using Corba;
using InteropTest;
using IOP;
using PortableInterceptor;
using PortableServer;
using Xunit;
using TypeCode = Corba.TypeCode;

namespace Rustorb.Tests
{
    public sealed class PiProbe : LocalObject, IClientRequestInterceptor, IServerRequestInterceptor, IIORInterceptor
    {
        public const uint Context = 0x50495052;
        public const uint Tag = 0x50495052;

        public ICodec Codec;
        public uint Slot;
        public Corba.IObject ForwardTo;
        public bool Forwarded;
        public readonly List<string> Seen = new();
        public readonly List<string> Established = new();
        public IClientRequestInfo Kept;
        public int Destroyed;

        protected override string[] _ids() => new[] { "IDL:omg.org/PortableInterceptor/Interceptor:1.0" };

        public string name => "probe";

        public void destroy() => Destroyed++;

        private void Note(string s)
        {
            lock (Seen) Seen.Add(s);
        }

        public byte[] Encode(string s)
        {
            var a = new Any();
            a.insert_string(s);
            return Codec.encode_value(a);
        }

        public string Decode(byte[] d) => Codec.decode_value(d, TypeCodes.Simple(TCKind.tk_string)).extract_string();

        public void send_request(IClientRequestInfo ri)
        {
            Kept = ri;
            if (ri.operation == "_is_a" || ri.operation == "_non_existent")
            {
                return;
            }
            ri.add_request_service_context(new ServiceContext(Context, Encode("from client")), false);
            Note($"send_request {ri.operation} slot={ri.get_slot(Slot).type.kind()}");
            if (ri.operation == "echo_string")
            {
                try
                {
                    Note("arguments " + ri.arguments.Count + " " + ri.arguments[0].argument.extract_string());
                }
                catch (NO_RESOURCES e)
                {
                    Note("arguments NO_RESOURCES " + (e.minor & 0xfff));
                }
            }
            if (ri.operation == "echo_long" && !Forwarded && ForwardTo != null)
            {
                Forwarded = true;
                throw new PortableInterceptor.ForwardRequest(ForwardTo);
            }
        }

        public void send_poll(IClientRequestInfo ri) { }

        public void receive_reply(IClientRequestInfo ri)
        {
            if (ri.operation != "echo_string")
            {
                return;
            }
            Note("reply context " + Decode(ri.get_reply_service_context(Context).context_data));
            Note("component " + Decode(ri.get_effective_component(Tag).component_data));
            Note("status " + ri.reply_status);
        }

        public void receive_exception(IClientRequestInfo ri) =>
            Note($"receive_exception {ri.operation} {ri.received_exception_id} {ri.reply_status}");

        public void receive_other(IClientRequestInfo ri) => Note($"receive_other {ri.operation} {ri.reply_status}");

        public void receive_request_service_contexts(IServerRequestInfo ri)
        {
            if (ri.operation != "echo_string" && ri.operation != "echo_short")
            {
                return;
            }
            string got = Decode(ri.get_request_service_context(Context).context_data);
            var a = new Any();
            a.insert_string(got);
            ri.set_slot(Slot, a);
        }

        public void receive_request(IServerRequestInfo ri)
        {
            if (ri.operation == "echo_short")
            {
                throw new NO_PERMISSION("", 5, CompletionStatus.COMPLETED_NO);
            }
            if (ri.operation == "echo_string")
            {
                Note($"receive_request {ri.target_most_derived_interface} {ri.target_is_a(EchoHelper.id())} {ri.adapter_name.Count}");
            }
        }

        public void send_reply(IServerRequestInfo ri)
        {
            if (ri.operation == "echo_string")
            {
                string got = ri.get_slot(Slot).extract_string();
                ri.add_reply_service_context(new ServiceContext(Context, Encode("server got: " + got)), false);
            }
        }

        public void send_exception(IServerRequestInfo ri) =>
            Note($"send_exception {ri.operation} {ri.sending_exception_id}");

        public void send_other(IServerRequestInfo ri) { }

        public void establish_components(IIORInfo info)
        {
            lock (Established) Established.Add("establish_components " + info.adapter_name.Count + " " + info.get_effective_policy(17).policy_type);
            info.add_ior_component(new TaggedComponent(Tag, Encode("component")));
        }
    }

    public sealed class PiInit : PortableInterceptor.ORBInitializer
    {
        public readonly PiProbe Probe = new();
        public readonly List<string> Calls = new();
        public IORBInitInfo Kept;

        public override void pre_init(IORBInitInfo info)
        {
            Kept = info;
            Calls.Add("pre_init " + info.orb_id);
            Probe.Codec = info.codec_factory.create_codec(new IOP.Encoding(IOP.Constants.ENCODING_CDR_ENCAPS, 1, 2));
            Probe.Slot = info.allocate_slot_id();
            info.add_client_request_interceptor(Probe);
            info.add_server_request_interceptor(Probe);
            info.add_ior_interceptor(Probe);
            var e = Assert.Throws<ORBInitInfo.DuplicateName>(() => info.add_client_request_interceptor(Probe));
            Assert.Equal("probe", e.name);
            Assert.Throws<BAD_INV_ORDER>(() => info.resolve_initial_references("RootPOA"));
            Assert.Throws<NO_IMPLEMENT>(() => info.register_policy_factory(1000, null));
        }

        public override void post_init(IORBInitInfo info)
        {
            Calls.Add("post_init");
            Assert.NotNull(PortableInterceptor.CurrentHelper.narrow(info.resolve_initial_references("PICurrent")));
            // Not RootPOA: made here, it would come before the IOR interceptors.
            Assert.NotNull(info.resolve_initial_references("CodecFactory"));
            Assert.Throws<ORBInitInfo.InvalidName>(() => info.resolve_initial_references("NoSuchService"));
        }
    }

    public sealed class PiFixture : IDisposable
    {
        public PiInit Init { get; } = new();
        public ORB Orb { get; }
        public IEcho Echo { get; }
        public Corba.Object Dynamic { get; }
        public DsiEcho Dsi { get; } = new();

        public PiFixture()
        {
            Orb = ORB.init(new string[0], "pi-test", Init);
            var poa = POAHelper.narrow(Orb.resolve_initial_references("RootPOA"));
            poa.the_POAManager.activate();
            Echo = EchoHelper.unchecked_narrow(
                Orb.string_to_object(Orb.object_to_string(poa.servant_to_reference(new EchoImpl()))));
            Dynamic = Orb.string_to_object(Orb.object_to_string(poa.servant_to_reference(Dsi)));
        }

        public void Dispose()
        {
            Orb.shutdown(true);
            Orb.destroy();
        }
    }

    public class PiTests : IClassFixture<PiFixture>
    {
        private readonly PiFixture f;

        public PiTests(PiFixture fixture)
        {
            f = fixture;
        }

        private List<string> Seen(Action call)
        {
            lock (f.Init.Probe.Seen) f.Init.Probe.Seen.Clear();
            call();
            lock (f.Init.Probe.Seen) return new List<string>(f.Init.Probe.Seen);
        }

        [Fact]
        public void Initializer()
        {
            Assert.Equal(new[] { "pre_init pi-test", "post_init" }, f.Init.Calls);
            Assert.Throws<OBJECT_NOT_EXIST>(() => f.Init.Kept.orb_id);
        }

        [Fact]
        public void ContextsComponentsAndSlots()
        {
            List<string> seen = Seen(() => Assert.Equal("hi", f.Echo.echo_string("hi")));
            Assert.Contains("send_request echo_string slot=tk_null", seen);
            Assert.Contains("arguments NO_RESOURCES 1", seen);
            Assert.Contains($"receive_request {EchoHelper.id()} True 1", seen);
            Assert.Contains(f.Init.Probe.Established, s => s == "establish_components 1 17");
            Assert.Contains("reply context server got: from client", seen);
            Assert.Contains("component component", seen);
            Assert.Contains("status 0", seen);
            // The info kept past the point.
            var e = Assert.Throws<BAD_INV_ORDER>(() => f.Init.Probe.Kept.operation);
            Assert.Equal(0x4f4d000eu, e.minor);
        }

        [Fact]
        public void PiCurrent()
        {
            ICurrent current = PortableInterceptor.CurrentHelper.narrow(f.Orb.resolve_initial_references("PICurrent"));
            var a = new Any();
            a.insert_long(7);
            current.set_slot(f.Init.Probe.Slot, a);
            try
            {
                Assert.Equal(7, current.get_slot(f.Init.Probe.Slot).extract_long());
                List<string> seen = Seen(() => f.Echo.echo_string("x"));
                Assert.Contains("send_request echo_string slot=tk_long", seen);
                Assert.Throws<InvalidSlot>(() => current.get_slot(99));
            }
            finally
            {
                current.set_slot(f.Init.Probe.Slot, new Any());
            }
        }

        [Fact]
        public void Exceptions()
        {
            List<string> seen = Seen(() =>
            {
                var e = Assert.Throws<NO_PERMISSION>(() => f.Echo.echo_short(1));
                Assert.Equal(5u, e.minor);
            });
            Assert.Contains("send_exception echo_short IDL:omg.org/CORBA/NO_PERMISSION:1.0", seen);
            Assert.Contains("receive_exception echo_short IDL:omg.org/CORBA/NO_PERMISSION:1.0 1", seen);
        }

        [Fact]
        public void Forward()
        {
            f.Init.Probe.ForwardTo = f.Dynamic;
            f.Init.Probe.Forwarded = false;
            try
            {
                // A reference of its own: the forward stays with it.
                IEcho echo = EchoHelper.unchecked_narrow(f.Orb.string_to_object(f.Orb.object_to_string(f.Echo)));
                List<string> seen = Seen(() => Assert.Throws<BAD_OPERATION>(() => echo.echo_long(3)));
                lock (f.Dsi.Operations) Assert.Contains("echo_long", f.Dsi.Operations);
            }
            finally
            {
                f.Init.Probe.ForwardTo = null;
            }
        }

        [Fact]
        public void DiiArguments()
        {
            Request r = f.Dynamic._request("echo_string");
            r.add_in_arg().insert_string("dii");
            r.set_return_type(TypeCodes.Simple(TCKind.tk_string));
            List<string> seen = Seen(() => r.invoke());
            Assert.Equal("dii", r.return_value().extract_string());
            Assert.Contains("arguments 1 dii", seen);
        }

        [Fact]
        public void Codec()
        {
            var factory = CodecFactoryHelper.narrow(f.Orb.resolve_initial_references("CodecFactory"));
            ICodec codec = factory.create_codec(new IOP.Encoding(IOP.Constants.ENCODING_CDR_ENCAPS, 1, 2));
            var a = new Any();
            a.insert_long(42);
            Any back = codec.decode(codec.encode(a));
            Assert.Equal(42, back.extract_long());
            Assert.Equal(42, codec.decode_value(codec.encode_value(a), TypeCodes.Simple(TCKind.tk_long)).extract_long());
            Assert.Throws<IOP.Codec.FormatMismatch>(() => codec.decode(new byte[] { 0 }));
            Assert.Throws<CodecFactory.UnknownEncoding>(() => factory.create_codec(new IOP.Encoding(1, 1, 2)));
            Assert.Contains("PICurrent", f.Orb.list_initial_services());
            Assert.Contains("CodecFactory", f.Orb.list_initial_services());
        }
    }
}
