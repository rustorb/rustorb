// Portable Interceptors and IOP::Codec: the local interfaces of
// PortableInterceptor and IOP as the IDL4 to C# mapping writes them (I
// prefixed interfaces, attributes as properties), on the rorb_pi, rorb_cri,
// rorb_sri, rorb_ior_info, rorb_init_info and rorb_codec functions of the C
// ABI. See docs/portable-interceptors.md and docs/csharp-binding.md.
//
//   class Init : PortableInterceptor.ORBInitializer
//   {
//       public override void pre_init(IORBInitInfo info) =>
//           info.add_client_request_interceptor(new MyInterceptor());
//   }
//   var orb = Corba.ORB.init(args, "my-orb", new Init());
//
// The info objects a point gets are only usable during the point: kept past
// it they raise BAD_INV_ORDER (minor 14), and an ORBInitInfo kept past
// ORB.init raises OBJECT_NOT_EXIST (minor 10).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Omg.Types;
using Rustorb.Native;

namespace Corba
{
    /// <summary>CORBA::ParameterMode.</summary>
    public enum ParameterMode
    {
        PARAM_IN,
        PARAM_OUT,
        PARAM_INOUT,
    }
}

namespace Dynamic
{
    /// <summary>Dynamic::Parameter.</summary>
    public class Parameter
    {
        public Corba.Any argument { get; set; }
        public Corba.ParameterMode mode { get; set; }

        public Parameter()
        {
            argument = new Corba.Any();
        }

        public Parameter(Corba.Any argument, Corba.ParameterMode mode)
        {
            this.argument = argument;
            this.mode = mode;
        }
    }
}

namespace Messaging
{
    /// <summary>The constants of Messaging (Messaging::SyncScope is a short).</summary>
    public static class Constants
    {
        public const short SYNC_NONE = 0;
        public const short SYNC_WITH_TRANSPORT = 1;
        public const short SYNC_WITH_SERVER = 2;
        public const short SYNC_WITH_TARGET = 3;
    }
}

namespace IOP
{
    /// <summary>The constants of IOP.</summary>
    public static class Constants
    {
        public const uint TAG_INTERNET_IOP = 0;
        public const uint TAG_MULTIPLE_COMPONENTS = 1;
        public const uint TAG_ORB_TYPE = 0;
        public const uint TAG_CODE_SETS = 1;
        public const uint TAG_POLICIES = 2;
        public const uint TAG_ALTERNATE_IIOP_ADDRESS = 3;
        public const uint TAG_SSL_SEC_TRANS = 20;
        public const uint TransactionService = 0;
        public const uint CodeSets = 1;
        public const uint BI_DIR_IIOP = 5;
        public const uint SendingContextRunTime = 6;
        public const short ENCODING_CDR_ENCAPS = 0;
    }

    /// <summary>IOP::ServiceContext.</summary>
    public class ServiceContext
    {
        public uint context_id { get; set; }
        public byte[] context_data { get; set; } = Array.Empty<byte>();

        public ServiceContext() { }

        public ServiceContext(uint context_id, byte[] context_data)
        {
            this.context_id = context_id;
            this.context_data = context_data;
        }
    }

    /// <summary>IOP::TaggedComponent.</summary>
    public class TaggedComponent
    {
        public uint tag { get; set; }
        public byte[] component_data { get; set; } = Array.Empty<byte>();

        public TaggedComponent() { }

        public TaggedComponent(uint tag, byte[] component_data)
        {
            this.tag = tag;
            this.component_data = component_data;
        }
    }

    /// <summary>IOP::TaggedProfile.</summary>
    public class TaggedProfile
    {
        public uint tag { get; set; }
        public byte[] profile_data { get; set; } = Array.Empty<byte>();

        public TaggedProfile() { }

        public TaggedProfile(uint tag, byte[] profile_data)
        {
            this.tag = tag;
            this.profile_data = profile_data;
        }
    }

    /// <summary>IOP::Encoding.</summary>
    public class Encoding
    {
        public short format { get; set; }
        public byte major_version { get; set; }
        public byte minor_version { get; set; }

        public Encoding() { }

        public Encoding(short format, byte major_version, byte minor_version)
        {
            this.format = format;
            this.major_version = major_version;
            this.minor_version = minor_version;
        }
    }

    /// <summary>IOP::Codec: CDR encapsulations of anys (encode / decode) and
    /// of values alone (encode_value / decode_value).</summary>
    public interface ICodec : Corba.IObject
    {
        byte[] encode(Corba.Any data);
        Corba.Any decode(byte[] data);
        byte[] encode_value(Corba.Any data);
        Corba.Any decode_value(byte[] data, Corba.TypeCode tc);
    }

    /// <summary>IOP::CodecFactory.</summary>
    public interface ICodecFactory : Corba.IObject
    {
        ICodec create_codec(Encoding enc);
    }

    public sealed unsafe class Codec : Corba.LocalObject, ICodec
    {
        private readonly CodecHandle handle;

        internal Codec(CodecHandle handle)
        {
            this.handle = handle;
        }

        protected override string[] _ids() => new[] { "IDL:omg.org/IOP/Codec:1.0", "IDL:omg.org/CORBA/LocalObject:1.0" };

        public class InvalidTypeForEncoding : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/IOP/Codec/InvalidTypeForEncoding:1.0";
        }

        public class FormatMismatch : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/IOP/Codec/FormatMismatch:1.0";
        }

        public class TypeMismatch : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/IOP/Codec/TypeMismatch:1.0";
        }

        public byte[] encode(Corba.Any data) => Encode(data, 0);
        public byte[] encode_value(Corba.Any data) => Encode(data, 1);
        public Corba.Any decode(byte[] data) => Decode(data, null);

        public Corba.Any decode_value(byte[] data, Corba.TypeCode tc) =>
            Decode(data, tc ?? throw new Corba.BAD_PARAM("a null TypeCode", 0, Corba.CompletionStatus.COMPLETED_NO));

        private byte[] Encode(Corba.Any data, int valueOnly)
        {
            Rustorb.Seq.NotNull(data);
            RorbBytes b;
            RorbException exc;
            Errors.Check(Ffi.rorb_codec_encode(handle.Raw, data._handle.Raw, valueOnly, &b, &exc), ref exc);
            GC.KeepAlive(data);
            GC.KeepAlive(this);
            return Strings.Bytes(b);
        }

        private Corba.Any Decode(byte[] data, Corba.TypeCode? tc)
        {
            Rustorb.Seq.NotNull(data);
            nint any;
            RorbException exc;
            fixed (byte* p = data)
            {
                Errors.Check(Ffi.rorb_codec_decode(handle.Raw, p, (nuint)data.Length, tc == null ? 0 : tc._handle.Raw,
                    &any, &exc), ref exc);
            }
            GC.KeepAlive(tc);
            GC.KeepAlive(this);
            return new Corba.Any(new AnyHandle(any));
        }
    }

    /// <summary>The initial reference "CodecFactory" (and
    /// ORBInitInfo.codec_factory): ENCODING_CDR_ENCAPS 1.0 to 1.2.</summary>
    public sealed unsafe class CodecFactory : Corba.LocalObject, ICodecFactory
    {
        private readonly OrbHandle orb;

        internal CodecFactory(OrbHandle orb)
        {
            this.orb = orb;
        }

        protected override string[] _ids() =>
            new[] { "IDL:omg.org/IOP/CodecFactory:1.0", "IDL:omg.org/CORBA/LocalObject:1.0" };

        public class UnknownEncoding : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/IOP/CodecFactory/UnknownEncoding:1.0";
        }

        public ICodec create_codec(Encoding enc)
        {
            Rustorb.Seq.NotNull(enc);
            nint codec;
            RorbException exc;
            Errors.Check(Ffi.rorb_orb_create_codec(orb.Raw, enc.format, enc.major_version, enc.minor_version, &codec,
                &exc), ref exc);
            GC.KeepAlive(this);
            return new Codec(new CodecHandle(codec));
        }
    }

    public static class CodecFactoryHelper
    {
        public static ICodecFactory? narrow(Corba.IObject? obj) => obj switch
        {
            null => null,
            ICodecFactory f => f,
            _ => throw new Corba.BAD_PARAM("not an IOP::CodecFactory", 0, Corba.CompletionStatus.COMPLETED_NO),
        };
    }
}

namespace PortableInterceptor
{
    /// <summary>The constants of PortableInterceptor (ReplyStatus is a short).</summary>
    public static class Constants
    {
        public const short SUCCESSFUL = 0;
        public const short SYSTEM_EXCEPTION = 1;
        public const short USER_EXCEPTION = 2;
        public const short LOCATION_FORWARD = 3;
        public const short TRANSPORT_RETRY = 4;
        public const short UNKNOWN = 5;
    }

    /// <summary>PortableInterceptor::ForwardRequest: raised by the points
    /// that may send the request to another object.</summary>
    public class ForwardRequest : Corba.UserException
    {
        public Corba.IObject? forward { get; set; }
        public ForwardRequest() { }
        public ForwardRequest(Corba.IObject? forward) { this.forward = forward; }
        public override string _id() => "IDL:omg.org/PortableInterceptor/ForwardRequest:1.0";
    }

    /// <summary>PortableInterceptor::InvalidSlot.</summary>
    public class InvalidSlot : Corba.UserException
    {
        public override string _id() => "IDL:omg.org/PortableInterceptor/InvalidSlot:1.0";
    }

    public interface IInterceptor : Corba.IObject
    {
        /// <summary>The name, unique among the interceptors of a kind but for "".</summary>
        string name { get; }
        void destroy();
    }

    /// <summary>What ClientRequestInfo and ServerRequestInfo share. Arguments,
    /// exceptions and the result are NO_RESOURCES (minor 1) but for requests
    /// of the DII or the DSI; contexts and operation_context always are.</summary>
    public interface IRequestInfo : Corba.IObject
    {
        uint request_id { get; }
        string operation { get; }
        ISequence<Dynamic.Parameter> arguments { get; }
        ISequence<Corba.TypeCode> exceptions { get; }
        ISequence<string> contexts { get; }
        ISequence<string> operation_context { get; }
        Corba.Any result { get; }
        bool response_expected { get; }
        short sync_scope { get; }
        short reply_status { get; }
        Corba.IObject? forward_reference { get; }
        Corba.Any get_slot(uint id);
        IOP.ServiceContext get_request_service_context(uint id);
        IOP.ServiceContext get_reply_service_context(uint id);
    }

    public interface IClientRequestInfo : IRequestInfo
    {
        Corba.IObject? target { get; }
        Corba.IObject? effective_target { get; }
        IOP.TaggedProfile effective_profile { get; }
        Corba.Any received_exception { get; }
        string received_exception_id { get; }
        /// <summary>BAD_PARAM (minor 28) when the effective profile has no such component.</summary>
        IOP.TaggedComponent get_effective_component(uint id);
        ISequence<IOP.TaggedComponent> get_effective_components(uint id);
        /// <summary>The policy of the type in effect, or null; INV_POLICY
        /// (minor 2) for a type that is not a client policy.</summary>
        Corba.Policy? get_request_policy(uint type);
        /// <summary>Only in send_request.</summary>
        void add_request_service_context(IOP.ServiceContext service_context, bool replace);
    }

    public interface IServerRequestInfo : IRequestInfo
    {
        Corba.Any sending_exception { get; }
        /// <summary>The repository id of sending_exception (rustorb).</summary>
        string sending_exception_id { get; }
        byte[] object_id { get; }
        byte[] adapter_id { get; }
        string server_id { get; }
        string orb_id { get; }
        ISequence<string> adapter_name { get; }
        string target_most_derived_interface { get; }
        Corba.Policy get_server_policy(uint type);
        /// <summary>The request scope slot, which the servant's thread then sees.</summary>
        void set_slot(uint id, Corba.Any data);
        bool target_is_a(string id);
        void add_reply_service_context(IOP.ServiceContext service_context, bool replace);
    }

    public interface IIORInfo : Corba.IObject
    {
        Corba.Policy get_effective_policy(uint type);
        void add_ior_component(IOP.TaggedComponent a_component);
        /// <summary>BAD_PARAM (minor 29) but for TAG_INTERNET_IOP.</summary>
        void add_ior_component_to_profile(IOP.TaggedComponent a_component, uint profile_id);
        ISequence<string> adapter_name { get; }
    }

    public interface IClientRequestInterceptor : IInterceptor
    {
        void send_request(IClientRequestInfo ri);
        /// <summary>Never called: rustorb has no AMI polling.</summary>
        void send_poll(IClientRequestInfo ri);
        void receive_reply(IClientRequestInfo ri);
        void receive_exception(IClientRequestInfo ri);
        void receive_other(IClientRequestInfo ri);
    }

    public interface IServerRequestInterceptor : IInterceptor
    {
        void receive_request_service_contexts(IServerRequestInfo ri);
        void receive_request(IServerRequestInfo ri);
        void send_reply(IServerRequestInfo ri);
        void send_exception(IServerRequestInfo ri);
        void send_other(IServerRequestInfo ri);
    }

    public interface IIORInterceptor : IInterceptor
    {
        void establish_components(IIORInfo info);
    }

    /// <summary>PortableInterceptor::Current (the initial reference
    /// "PICurrent").</summary>
    public interface ICurrent : Corba.IObject
    {
        Corba.Any get_slot(uint id);
        void set_slot(uint id, Corba.Any data);
    }

    /// <summary>What an ORBInitializer gets: usable only during pre_init or
    /// post_init.</summary>
    public interface IORBInitInfo : Corba.IObject
    {
        ISequence<string> arguments { get; }
        string orb_id { get; }
        IOP.ICodecFactory codec_factory { get; }
        void register_initial_reference(string id, Corba.IObject obj);
        /// <summary>Only in post_init (BAD_INV_ORDER in pre_init).</summary>
        Corba.IObject? resolve_initial_references(string id);
        void add_client_request_interceptor(IClientRequestInterceptor interceptor);
        void add_server_request_interceptor(IServerRequestInterceptor interceptor);
        void add_ior_interceptor(IIORInterceptor interceptor);
        uint allocate_slot_id();
        /// <summary>Not supported: NO_IMPLEMENT.</summary>
        void register_policy_factory(uint type, Corba.IObject policy_factory);
    }

    public interface IORBInitializer : Corba.IObject
    {
        void pre_init(IORBInitInfo info);
        void post_init(IORBInitInfo info);
    }

    // --- bases whose operations do nothing -------------------------------------

    public abstract class ClientRequestInterceptor : Corba.LocalObject, IClientRequestInterceptor
    {
        protected override string[] _ids() => new[]
        {
            "IDL:omg.org/PortableInterceptor/ClientRequestInterceptor:1.0",
            "IDL:omg.org/PortableInterceptor/Interceptor:1.0", "IDL:omg.org/CORBA/LocalObject:1.0",
        };

        public virtual string name => "";
        public virtual void destroy() { }
        public virtual void send_request(IClientRequestInfo ri) { }
        public virtual void send_poll(IClientRequestInfo ri) { }
        public virtual void receive_reply(IClientRequestInfo ri) { }
        public virtual void receive_exception(IClientRequestInfo ri) { }
        public virtual void receive_other(IClientRequestInfo ri) { }
    }

    public abstract class ServerRequestInterceptor : Corba.LocalObject, IServerRequestInterceptor
    {
        protected override string[] _ids() => new[]
        {
            "IDL:omg.org/PortableInterceptor/ServerRequestInterceptor:1.0",
            "IDL:omg.org/PortableInterceptor/Interceptor:1.0", "IDL:omg.org/CORBA/LocalObject:1.0",
        };

        public virtual string name => "";
        public virtual void destroy() { }
        public virtual void receive_request_service_contexts(IServerRequestInfo ri) { }
        public virtual void receive_request(IServerRequestInfo ri) { }
        public virtual void send_reply(IServerRequestInfo ri) { }
        public virtual void send_exception(IServerRequestInfo ri) { }
        public virtual void send_other(IServerRequestInfo ri) { }
    }

    public abstract class IORInterceptor : Corba.LocalObject, IIORInterceptor
    {
        protected override string[] _ids() => new[]
        {
            "IDL:omg.org/PortableInterceptor/IORInterceptor:1.0",
            "IDL:omg.org/PortableInterceptor/Interceptor:1.0", "IDL:omg.org/CORBA/LocalObject:1.0",
        };

        public virtual string name => "";
        public virtual void destroy() { }
        public virtual void establish_components(IIORInfo info) { }
    }

    public abstract class ORBInitializer : Corba.LocalObject, IORBInitializer
    {
        protected override string[] _ids() => new[]
        {
            "IDL:omg.org/PortableInterceptor/ORBInitializer:1.0", "IDL:omg.org/CORBA/LocalObject:1.0",
        };

        public virtual void pre_init(IORBInitInfo info) { }
        public virtual void post_init(IORBInitInfo info) { }
    }

    /// <summary>PortableInterceptor::register_orb_initializer: for every ORB
    /// that ORB.init creates after it in the process.</summary>
    public static unsafe class ORBInitializerRegistry
    {
        public static void register_orb_initializer(IORBInitializer init)
        {
            Library.Check();
            Rustorb.Seq.NotNull(init);
            ManagerPeer.Set(init, null, self =>
            {
                Ffi.rorb_register_orb_initializer(self, PiCallbacks.InitializerVtable);
                return 0;
            });
        }
    }

    public static class CurrentHelper
    {
        public static ICurrent? narrow(Corba.IObject? obj) => obj switch
        {
            null => null,
            ICurrent c => c,
            _ => throw new Corba.BAD_PARAM("not a PortableInterceptor::Current", 0, Corba.CompletionStatus.COMPLETED_NO),
        };
    }

    // --- the implementations ---------------------------------------------------

    /// <summary>The slots as the calling thread sees them.</summary>
    public sealed unsafe class Current : Corba.LocalObject, ICurrent
    {
        private readonly PiCurrentHandle handle;

        internal Current(PiCurrentHandle handle)
        {
            this.handle = handle;
        }

        protected override string[] _ids() => new[]
        {
            "IDL:omg.org/PortableInterceptor/Current:1.0", "IDL:omg.org/CORBA/Current:1.0",
            "IDL:omg.org/CORBA/LocalObject:1.0",
        };

        public Corba.Any get_slot(uint id)
        {
            nint a;
            RorbException exc;
            Errors.Check(Ffi.rorb_pi_current_get_slot(handle.Raw, id, &a, &exc), ref exc);
            GC.KeepAlive(this);
            return new Corba.Any(new AnyHandle(a));
        }

        public void set_slot(uint id, Corba.Any data)
        {
            Rustorb.Seq.NotNull(data);
            RorbException exc;
            Errors.Check(Ffi.rorb_pi_current_set_slot(handle.Raw, id, data._handle.Raw, &exc), ref exc);
            GC.KeepAlive(data);
            GC.KeepAlive(this);
        }
    }

    /// <summary>An info handle lent for one point: Close ends the loan, after
    /// which uses raise BAD_INV_ORDER (minor 14), or OBJECT_NOT_EXIST (minor
    /// 10) for an ORBInitInfo. Uses from other threads during the point wait
    /// for each other.</summary>
    public abstract class LoanedInfo : Corba.LocalObject
    {
        private readonly object gate = new();
        private nint h;

        private protected LoanedInfo(nint h)
        {
            this.h = h;
        }

        internal void Close()
        {
            lock (gate)
            {
                h = 0;
            }
        }

        private protected virtual System.Exception Gone() =>
            new Corba.BAD_INV_ORDER("the info of a point used after it", 0x4f4d0000 | 14, Corba.CompletionStatus.COMPLETED_NO);

        private protected T With<T>(Func<nint, T> f)
        {
            lock (gate)
            {
                if (h == 0)
                {
                    throw Gone();
                }
                return f(h);
            }
        }

        private protected void Do(Action<nint> f) => With(x => { f(x); return 0; });
    }

    internal static unsafe class InfoCalls
    {
        internal static string Borrowed(byte* s, nuint len) => s == null ? "" : Strings.FromUtf8(s, len);

        internal static Corba.Any AnyOf(Func<nint> f)
        {
            nint a = f();
            return new Corba.Any(new AnyHandle(a));
        }

        internal static Corba.IObject? ObjectOf(nint obj) => obj == 0 ? null : new Corba.Object(new ObjectHandle(obj));

        internal static nint ObjectRaw(Corba.IObject? obj) => obj switch
        {
            null => 0,
            Corba.Object o => o.Raw,
            _ => throw new Corba.BAD_PARAM("a local object where an object reference is needed", 0,
                Corba.CompletionStatus.COMPLETED_NO),
        };

        internal static ISequence<T> Collect<T>(Action<nint> f)
        {
            var list = new Sequence<T>();
            GCHandle g = GCHandle.Alloc(list);
            try
            {
                f(GCHandle.ToIntPtr(g));
            }
            finally
            {
                g.Free();
            }
            return list;
        }

        internal static readonly nint EachString = (nint)(delegate* unmanaged[Cdecl]<nint, byte*, nuint, void>)&EachStringCb;
        internal static readonly nint EachComponent =
            (nint)(delegate* unmanaged[Cdecl]<nint, uint, byte*, nuint, void>)&EachComponentCb;
        internal static readonly nint EachParameter = (nint)(delegate* unmanaged[Cdecl]<nint, nint, uint, void>)&EachParameterCb;
        internal static readonly nint EachTypeCode = (nint)(delegate* unmanaged[Cdecl]<nint, nint, void>)&EachTypeCodeCb;

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static void EachStringCb(nint context, byte* s, nuint len) =>
            ((Sequence<string>)GCHandle.FromIntPtr(context).Target!).Add(Strings.FromUtf8(s, len));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static void EachComponentCb(nint context, uint tag, byte* data, nuint len) =>
            ((Sequence<IOP.TaggedComponent>)GCHandle.FromIntPtr(context).Target!).Add(
                new IOP.TaggedComponent(tag, new ReadOnlySpan<byte>(data, checked((int)len)).ToArray()));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static void EachParameterCb(nint context, nint any, uint mode) =>
            ((Sequence<Dynamic.Parameter>)GCHandle.FromIntPtr(context).Target!).Add(
                new Dynamic.Parameter(new Corba.Any(new AnyHandle(any)), (Corba.ParameterMode)mode));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static void EachTypeCodeCb(nint context, nint tc) =>
            ((Sequence<Corba.TypeCode>)GCHandle.FromIntPtr(context).Target!).Add(new Corba.TypeCode(new TypeCodeHandle(tc)));

        internal static Corba.Policy Policy(uint type, int status, ulong raw, ref RorbException exc)
        {
            Errors.Check(status, ref exc);
            return new Corba.Policy(type, raw);
        }

        internal static Corba.NO_RESOURCES NoResources() =>
            new("not available in rustorb", 0x4f4d0000 | 1, Corba.CompletionStatus.COMPLETED_NO);
    }

    internal sealed unsafe class ClientRequestInfo : LoanedInfo, IClientRequestInfo
    {
        internal ClientRequestInfo(nint h) : base(h) { }

        protected override string[] _ids() => new[]
        {
            "IDL:omg.org/PortableInterceptor/ClientRequestInfo:1.0", "IDL:omg.org/PortableInterceptor/RequestInfo:1.0",
            "IDL:omg.org/CORBA/LocalObject:1.0",
        };

        public uint request_id => With(h => Ffi.rorb_cri_request_id(h));

        public string operation => With(h =>
        {
            nuint len;
            byte* s = Ffi.rorb_cri_operation(h, &len);
            return InfoCalls.Borrowed(s, len);
        });

        public ISequence<Dynamic.Parameter> arguments => With(h => InfoCalls.Collect<Dynamic.Parameter>(c =>
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_cri_arguments(h, c, InfoCalls.EachParameter, &exc), ref exc);
        }));

        public ISequence<Corba.TypeCode> exceptions => With(h => InfoCalls.Collect<Corba.TypeCode>(c =>
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_cri_exceptions(h, c, InfoCalls.EachTypeCode, &exc), ref exc);
        }));

        public ISequence<string> contexts => With<ISequence<string>>(h => throw InfoCalls.NoResources());
        public ISequence<string> operation_context => With<ISequence<string>>(h => throw InfoCalls.NoResources());

        public Corba.Any result => With(h => InfoCalls.AnyOf(() =>
        {
            nint a;
            RorbException exc;
            Errors.Check(Ffi.rorb_cri_result(h, &a, &exc), ref exc);
            return a;
        }));

        public bool response_expected => With(h => Ffi.rorb_cri_response_expected(h) != 0);
        public short sync_scope => With(h => Ffi.rorb_cri_sync_scope(h));

        public short reply_status => With(h =>
        {
            short s;
            RorbException exc;
            Errors.Check(Ffi.rorb_cri_reply_status(h, &s, &exc), ref exc);
            return s;
        });

        public Corba.IObject? forward_reference => With(h =>
        {
            nint o;
            RorbException exc;
            Errors.Check(Ffi.rorb_cri_forward_reference(h, &o, &exc), ref exc);
            return InfoCalls.ObjectOf(o);
        });

        public Corba.Any get_slot(uint id) => With(h => InfoCalls.AnyOf(() =>
        {
            nint a;
            RorbException exc;
            Errors.Check(Ffi.rorb_cri_get_slot(h, id, &a, &exc), ref exc);
            return a;
        }));

        public IOP.ServiceContext get_request_service_context(uint id) => With(h =>
        {
            RorbBytes b;
            RorbException exc;
            Errors.Check(Ffi.rorb_cri_get_request_service_context(h, id, &b, &exc), ref exc);
            return new IOP.ServiceContext(id, Strings.Bytes(b));
        });

        public IOP.ServiceContext get_reply_service_context(uint id) => With(h =>
        {
            RorbBytes b;
            RorbException exc;
            Errors.Check(Ffi.rorb_cri_get_reply_service_context(h, id, &b, &exc), ref exc);
            return new IOP.ServiceContext(id, Strings.Bytes(b));
        });

        public Corba.IObject? target => With(h => InfoCalls.ObjectOf(Ffi.rorb_cri_target(h)));
        public Corba.IObject? effective_target => With(h => InfoCalls.ObjectOf(Ffi.rorb_cri_effective_target(h)));

        public IOP.TaggedProfile effective_profile => With(h =>
        {
            RorbBytes b;
            uint tag = Ffi.rorb_cri_effective_profile(h, &b);
            return new IOP.TaggedProfile(tag, Strings.Bytes(b));
        });

        public Corba.Any received_exception => With(h => InfoCalls.AnyOf(() =>
        {
            nint a;
            RorbException exc;
            Errors.Check(Ffi.rorb_cri_received_exception(h, &a, &exc), ref exc);
            return a;
        }));

        public string received_exception_id => With(h =>
        {
            RorbBytes b;
            RorbException exc;
            Errors.Check(Ffi.rorb_cri_received_exception_id(h, &b, &exc), ref exc);
            return Strings.Take(b);
        });

        public IOP.TaggedComponent get_effective_component(uint id) => With(h =>
        {
            RorbBytes b;
            RorbException exc;
            Errors.Check(Ffi.rorb_cri_get_effective_component(h, id, &b, &exc), ref exc);
            return new IOP.TaggedComponent(id, Strings.Bytes(b));
        });

        public ISequence<IOP.TaggedComponent> get_effective_components(uint id) => With(h =>
            InfoCalls.Collect<IOP.TaggedComponent>(c =>
            {
                RorbException exc;
                Errors.Check(Ffi.rorb_cri_get_effective_components(h, id, c, InfoCalls.EachComponent, &exc), ref exc);
            }));

        public Corba.Policy? get_request_policy(uint type) => With(h =>
        {
            int found;
            ulong raw;
            RorbException exc;
            Errors.Check(Ffi.rorb_cri_get_request_policy(h, type, &found, &raw, &exc), ref exc);
            return found != 0 ? new Corba.Policy(type, raw) : null;
        });

        public void add_request_service_context(IOP.ServiceContext service_context, bool replace)
        {
            Rustorb.Seq.NotNull(service_context);
            byte[] data = service_context.context_data ?? Array.Empty<byte>();
            Do(h =>
            {
                RorbException exc;
                fixed (byte* p = data)
                {
                    Errors.Check(Ffi.rorb_cri_add_request_service_context(h, service_context.context_id, p,
                        (nuint)data.Length, replace ? 1 : 0, &exc), ref exc);
                }
            });
        }
    }

    internal sealed unsafe class ServerRequestInfo : LoanedInfo, IServerRequestInfo
    {
        internal ServerRequestInfo(nint h) : base(h) { }

        protected override string[] _ids() => new[]
        {
            "IDL:omg.org/PortableInterceptor/ServerRequestInfo:1.0", "IDL:omg.org/PortableInterceptor/RequestInfo:1.0",
            "IDL:omg.org/CORBA/LocalObject:1.0",
        };

        public uint request_id => With(h => Ffi.rorb_sri_request_id(h));

        public string operation => With(h =>
        {
            nuint len;
            byte* s = Ffi.rorb_sri_operation(h, &len);
            return InfoCalls.Borrowed(s, len);
        });

        public ISequence<Dynamic.Parameter> arguments => With(h => InfoCalls.Collect<Dynamic.Parameter>(c =>
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_arguments(h, c, InfoCalls.EachParameter, &exc), ref exc);
        }));

        public ISequence<Corba.TypeCode> exceptions => With(h => InfoCalls.Collect<Corba.TypeCode>(c =>
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_exceptions(h, c, InfoCalls.EachTypeCode, &exc), ref exc);
        }));

        public ISequence<string> contexts => With<ISequence<string>>(h => throw InfoCalls.NoResources());
        public ISequence<string> operation_context => With<ISequence<string>>(h => throw InfoCalls.NoResources());

        public Corba.Any result => With(h => InfoCalls.AnyOf(() =>
        {
            nint a;
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_result(h, &a, &exc), ref exc);
            return a;
        }));

        public bool response_expected => With(h => Ffi.rorb_sri_response_expected(h) != 0);
        public short sync_scope => With(h => Ffi.rorb_sri_sync_scope(h));

        public short reply_status => With(h =>
        {
            short s;
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_reply_status(h, &s, &exc), ref exc);
            return s;
        });

        public Corba.IObject? forward_reference => With(h =>
        {
            nint o;
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_forward_reference(h, &o, &exc), ref exc);
            return InfoCalls.ObjectOf(o);
        });

        public Corba.Any get_slot(uint id) => With(h => InfoCalls.AnyOf(() =>
        {
            nint a;
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_get_slot(h, id, &a, &exc), ref exc);
            return a;
        }));

        public IOP.ServiceContext get_request_service_context(uint id) => With(h =>
        {
            RorbBytes b;
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_get_request_service_context(h, id, &b, &exc), ref exc);
            return new IOP.ServiceContext(id, Strings.Bytes(b));
        });

        public IOP.ServiceContext get_reply_service_context(uint id) => With(h =>
        {
            RorbBytes b;
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_get_reply_service_context(h, id, &b, &exc), ref exc);
            return new IOP.ServiceContext(id, Strings.Bytes(b));
        });

        public Corba.Any sending_exception => With(h => InfoCalls.AnyOf(() =>
        {
            nint a;
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_sending_exception(h, &a, &exc), ref exc);
            return a;
        }));

        public string sending_exception_id => With(h =>
        {
            RorbBytes b;
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_sending_exception_id(h, &b, &exc), ref exc);
            return Strings.Take(b);
        });

        public byte[] object_id => With(h =>
        {
            RorbBytes b;
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_object_id(h, &b, &exc), ref exc);
            return Strings.Bytes(b);
        });

        public byte[] adapter_id => With(h =>
        {
            RorbBytes b;
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_adapter_id(h, &b, &exc), ref exc);
            return Strings.Bytes(b);
        });

        public string server_id => With(h =>
        {
            nuint len;
            byte* s = Ffi.rorb_sri_server_id(h, &len);
            return InfoCalls.Borrowed(s, len);
        });

        public string orb_id => With(h =>
        {
            nuint len;
            byte* s = Ffi.rorb_sri_orb_id(h, &len);
            return InfoCalls.Borrowed(s, len);
        });

        public ISequence<string> adapter_name => With(h => InfoCalls.Collect<string>(c =>
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_adapter_name(h, c, InfoCalls.EachString, &exc), ref exc);
        }));

        public string target_most_derived_interface => With(h =>
        {
            RorbBytes b;
            RorbException exc;
            Errors.Check(Ffi.rorb_sri_target_most_derived_interface(h, &b, &exc), ref exc);
            return Strings.Take(b);
        });

        public Corba.Policy get_server_policy(uint type) => With(h =>
        {
            ulong raw;
            RorbException exc;
            int st = Ffi.rorb_sri_get_server_policy(h, type, &raw, &exc);
            return InfoCalls.Policy(type, st, raw, ref exc);
        });

        public void set_slot(uint id, Corba.Any data)
        {
            Rustorb.Seq.NotNull(data);
            Do(h =>
            {
                RorbException exc;
                Errors.Check(Ffi.rorb_sri_set_slot(h, id, data._handle.Raw, &exc), ref exc);
                GC.KeepAlive(data);
            });
        }

        public bool target_is_a(string id)
        {
            byte[] b = Strings.ToUtf8(id ?? "");
            return With(h =>
            {
                int r;
                RorbException exc;
                fixed (byte* p = b)
                {
                    Errors.Check(Ffi.rorb_sri_target_is_a(h, p, (nuint)b.Length, &r, &exc), ref exc);
                }
                return r != 0;
            });
        }

        public void add_reply_service_context(IOP.ServiceContext service_context, bool replace)
        {
            Rustorb.Seq.NotNull(service_context);
            byte[] data = service_context.context_data ?? Array.Empty<byte>();
            Do(h =>
            {
                RorbException exc;
                fixed (byte* p = data)
                {
                    Errors.Check(Ffi.rorb_sri_add_reply_service_context(h, service_context.context_id, p,
                        (nuint)data.Length, replace ? 1 : 0, &exc), ref exc);
                }
            });
        }
    }

    internal sealed unsafe class IORInfo : LoanedInfo, IIORInfo
    {
        internal IORInfo(nint h) : base(h) { }

        protected override string[] _ids() => new[]
        {
            "IDL:omg.org/PortableInterceptor/IORInfo:1.0", "IDL:omg.org/CORBA/LocalObject:1.0",
        };

        public Corba.Policy get_effective_policy(uint type) => With(h =>
        {
            ulong raw;
            RorbException exc;
            int st = Ffi.rorb_ior_info_get_effective_policy(h, type, &raw, &exc);
            return InfoCalls.Policy(type, st, raw, ref exc);
        });

        public void add_ior_component(IOP.TaggedComponent a_component)
        {
            Rustorb.Seq.NotNull(a_component);
            byte[] data = a_component.component_data ?? Array.Empty<byte>();
            Do(h =>
            {
                fixed (byte* p = data)
                {
                    Ffi.rorb_ior_info_add_ior_component(h, a_component.tag, p, (nuint)data.Length);
                }
            });
        }

        public void add_ior_component_to_profile(IOP.TaggedComponent a_component, uint profile_id)
        {
            Rustorb.Seq.NotNull(a_component);
            byte[] data = a_component.component_data ?? Array.Empty<byte>();
            Do(h =>
            {
                RorbException exc;
                fixed (byte* p = data)
                {
                    Errors.Check(Ffi.rorb_ior_info_add_ior_component_to_profile(h, a_component.tag, p,
                        (nuint)data.Length, profile_id, &exc), ref exc);
                }
            });
        }

        public ISequence<string> adapter_name => With(h =>
            InfoCalls.Collect<string>(c => Ffi.rorb_ior_info_adapter_name(h, c, InfoCalls.EachString)));
    }

    public sealed unsafe class ORBInitInfo : LoanedInfo, IORBInitInfo
    {
        internal ORBInitInfo(nint h) : base(h) { }

        protected override string[] _ids() => new[]
        {
            "IDL:omg.org/PortableInterceptor/ORBInitInfo:1.0", "IDL:omg.org/CORBA/LocalObject:1.0",
        };

        /// <summary>ORBInitInfo::DuplicateName.</summary>
        public class DuplicateName : Corba.UserException
        {
            public string name { get; set; } = "";
            public DuplicateName() { }
            public DuplicateName(string name) { this.name = name; }
            public override string _id() => "IDL:omg.org/PortableInterceptor/ORBInitInfo/DuplicateName:1.0";
        }

        /// <summary>ORBInitInfo::InvalidName.</summary>
        public class InvalidName : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableInterceptor/ORBInitInfo/InvalidName:1.0";
        }

        private protected override System.Exception Gone() =>
            new Corba.OBJECT_NOT_EXIST("an ORBInitInfo used after ORB.init", 0x4f4d0000 | 10,
                Corba.CompletionStatus.COMPLETED_NO);

        public ISequence<string> arguments => With(h =>
            InfoCalls.Collect<string>(c => Ffi.rorb_init_info_arguments(h, c, InfoCalls.EachString)));

        public string orb_id => With(h =>
        {
            nuint len;
            byte* s = Ffi.rorb_init_info_orb_id(h, &len);
            return InfoCalls.Borrowed(s, len);
        });

        public IOP.ICodecFactory codec_factory => With(h => new IOP.CodecFactory(new OrbHandle(Ffi.rorb_init_info_orb(h))));

        /// <summary>InvalidName for "" or a name already registered; local
        /// objects are BAD_PARAM, as in ORB.register_initial_reference.</summary>
        public void register_initial_reference(string id, Corba.IObject obj)
        {
            byte[] b = Strings.ToUtf8(id ?? "");
            nint raw = InfoCalls.ObjectRaw(obj);
            Do(h =>
            {
                RorbException exc;
                int st;
                fixed (byte* p = b)
                {
                    st = Ffi.rorb_init_info_register_initial_reference(h, p, (nuint)b.Length, raw, &exc);
                }
                GC.KeepAlive(obj);
                Translate(st, ref exc);
            });
        }

        private static void Translate(int st, ref RorbException exc)
        {
            try
            {
                Errors.Check(st, ref exc);
            }
            catch (Corba.ORB.InvalidName)
            {
                throw new InvalidName();
            }
        }

        private static readonly string[] Own = { "RootPOA", "POACurrent", "DynAnyFactory", "PICurrent", "CodecFactory" };

        public Corba.IObject? resolve_initial_references(string id)
        {
            id ??= "";
            bool own = Array.IndexOf(Own, id) >= 0;
            // For the ORB's own, the core only checks the stage: BAD_INV_ORDER
            // in pre_init, InvalidName for "" in post_init.
            byte[] b = Strings.ToUtf8(own ? "" : id);
            return With(h =>
            {
                nint obj = 0;
                RorbException exc;
                int st;
                fixed (byte* p = b)
                {
                    st = Ffi.rorb_init_info_resolve_initial_references(h, p, (nuint)b.Length, &obj, &exc);
                }
                if (st == Status.OK)
                {
                    return InfoCalls.ObjectOf(obj);
                }
                if (!(own && st == Status.LOCAL_EXCEPTION && exc.local == Local.INVALID_NAME))
                {
                    Translate(st, ref exc);
                }
                Errors.Take(st, ref exc);
                var orb = new Corba.ORB(new OrbHandle(Ffi.rorb_init_info_orb(h)));
                return orb.resolve_initial_references(id);
            });
        }

        public void add_client_request_interceptor(IClientRequestInterceptor interceptor) =>
            Add(interceptor, PiCallbacks.ClientVtable, (h, n, l, s, v, e) =>
                Ffi.rorb_init_info_add_client_request_interceptor(h, n, l, s, v, e));

        public void add_server_request_interceptor(IServerRequestInterceptor interceptor) =>
            Add(interceptor, PiCallbacks.ServerVtable, (h, n, l, s, v, e) =>
                Ffi.rorb_init_info_add_server_request_interceptor(h, n, l, s, v, e));

        public void add_ior_interceptor(IIORInterceptor interceptor) =>
            Add(interceptor, PiCallbacks.IorVtable, (h, n, l, s, v, e) =>
                Ffi.rorb_init_info_add_ior_interceptor(h, n, l, s, v, e));

        private delegate int Adder(nint h, byte* name, nuint len, nint self, void* vtable, RorbException* exc);

        private void Add(IInterceptor interceptor, nint* vtable, Adder add)
        {
            if (interceptor == null)
            {
                throw new Corba.BAD_PARAM("a null interceptor", 0, Corba.CompletionStatus.COMPLETED_NO);
            }
            string name = interceptor.name ?? "";
            byte[] b = Strings.ToUtf8(name);
            Do(h => ManagerPeer.Set(interceptor, null, self =>
            {
                RorbException exc;
                int st;
                fixed (byte* p = b)
                {
                    st = add(h, p, (nuint)b.Length, self, vtable, &exc);
                }
                try
                {
                    Errors.Check(st, ref exc);
                }
                catch (DuplicateName d)
                {
                    d.name = name;
                    throw;
                }
                return 0;
            }));
        }

        public uint allocate_slot_id() => With(h => Ffi.rorb_init_info_allocate_slot_id(h));

        public void register_policy_factory(uint type, Corba.IObject policy_factory)
        {
            Do(h => { });
            throw new Corba.NO_IMPLEMENT("policy factories are not supported", 0, Corba.CompletionStatus.COMPLETED_NO);
        }
    }

    /// <summary>The vtables of interceptors and initializers, on the self of
    /// a ManagerPeer.</summary>
    internal static unsafe class PiCallbacks
    {
        internal static readonly nint* ClientVtable = Make(5,
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, nint, int>)&SendRequest,
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, nint, int>)&ReceiveReply,
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, nint, int>)&ReceiveException,
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, nint, int>)&ReceiveOther,
            (nint)(delegate* unmanaged[Cdecl]<nint, void>)&Destroy);

        internal static readonly nint* ServerVtable = Make(6,
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, nint, int>)&ReceiveRequestServiceContexts,
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, nint, int>)&ReceiveRequest,
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, nint, int>)&SendReply,
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, nint, int>)&SendException,
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, nint, int>)&SendOther,
            (nint)(delegate* unmanaged[Cdecl]<nint, void>)&Destroy);

        internal static readonly nint* IorVtable = Make(2,
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, nint, int>)&EstablishComponents,
            (nint)(delegate* unmanaged[Cdecl]<nint, void>)&Destroy);

        internal static readonly nint* InitializerVtable = Make(2,
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, nint, int>)&PreInit,
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, nint, int>)&PostInit);

        private static nint* Make(int n, params nint[] first)
        {
            var v = (nint*)NativeMemory.Alloc((nuint)(sizeof(nint) * (n + 2)));
            for (int i = 0; i < n; i++)
            {
                v[i] = first[i];
            }
            v[n] = ManagerPeer.AddRefFn;
            v[n + 1] = ManagerPeer.RemoveRefFn;
            return v;
        }

        private static readonly byte[] UnknownId = Strings.ToUtf8("IDL:omg.org/CORBA/UNKNOWN:1.0");

        private static void SetSystem(nint error, Corba.SystemException e)
        {
            byte[] id = Strings.ToUtf8(e._id());
            fixed (byte* p = id)
            {
                Ffi.rorb_pi_error_system(error, p, (nuint)id.Length, e.minor, (uint)e.completed);
            }
        }

        /// <summary>Runs a point: lends the info for the call, and gives the
        /// core what it raised.</summary>
        private static int Run<T>(nint self, LoanedInfo info, nint error, Action<T, LoanedInfo> body)
        {
            try
            {
                body((T)ManagerPeer.Of(self).target, info);
                return Status.OK;
            }
            catch (ForwardRequest f)
            {
                try
                {
                    if (f.forward is not Corba.Object o)
                    {
                        throw new Corba.BAD_PARAM("ForwardRequest without an object reference", 0,
                            Corba.CompletionStatus.COMPLETED_NO);
                    }
                    Ffi.rorb_pi_error_forward(error, o.Raw);
                    GC.KeepAlive(o);
                }
                catch (Corba.SystemException e)
                {
                    SetSystem(error, e);
                }
                return Status.SYSTEM_EXCEPTION;
            }
            catch (Corba.SystemException e)
            {
                SetSystem(error, e);
                return Status.SYSTEM_EXCEPTION;
            }
            catch (System.Exception e)
            {
                // Only ForwardRequest may be raised: the others are UNKNOWN.
                if (e is not Corba.UserException)
                {
                    Console.Error.WriteLine("rustorb: exception in an interceptor: " + e);
                }
                fixed (byte* p = UnknownId)
                {
                    Ffi.rorb_pi_error_system(error, p, (nuint)UnknownId.Length, 0,
                        (uint)Corba.CompletionStatus.COMPLETED_MAYBE);
                }
                return Status.SYSTEM_EXCEPTION;
            }
            finally
            {
                info.Close();
            }
        }

        private static int Client(nint self, nint ri, nint error, Action<IClientRequestInterceptor, IClientRequestInfo> f) =>
            Run<IClientRequestInterceptor>(self, new ClientRequestInfo(ri), error, (t, i) => f(t, (IClientRequestInfo)i));

        private static int Server(nint self, nint ri, nint error, Action<IServerRequestInterceptor, IServerRequestInfo> f) =>
            Run<IServerRequestInterceptor>(self, new ServerRequestInfo(ri), error, (t, i) => f(t, (IServerRequestInfo)i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int SendRequest(nint self, nint ri, nint error) => Client(self, ri, error, (t, i) => t.send_request(i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int ReceiveReply(nint self, nint ri, nint error) => Client(self, ri, error, (t, i) => t.receive_reply(i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int ReceiveException(nint self, nint ri, nint error) =>
            Client(self, ri, error, (t, i) => t.receive_exception(i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int ReceiveOther(nint self, nint ri, nint error) => Client(self, ri, error, (t, i) => t.receive_other(i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int ReceiveRequestServiceContexts(nint self, nint ri, nint error) =>
            Server(self, ri, error, (t, i) => t.receive_request_service_contexts(i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int ReceiveRequest(nint self, nint ri, nint error) => Server(self, ri, error, (t, i) => t.receive_request(i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int SendReply(nint self, nint ri, nint error) => Server(self, ri, error, (t, i) => t.send_reply(i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int SendException(nint self, nint ri, nint error) => Server(self, ri, error, (t, i) => t.send_exception(i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int SendOther(nint self, nint ri, nint error) => Server(self, ri, error, (t, i) => t.send_other(i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int EstablishComponents(nint self, nint info, nint error) =>
            Run<IIORInterceptor>(self, new IORInfo(info), error, (t, i) => t.establish_components((IIORInfo)i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int PreInit(nint self, nint info, nint error) =>
            Run<IORBInitializer>(self, new ORBInitInfo(info), error, (t, i) => t.pre_init((IORBInitInfo)i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int PostInit(nint self, nint info, nint error) =>
            Run<IORBInitializer>(self, new ORBInitInfo(info), error, (t, i) => t.post_init((IORBInitInfo)i));

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static void Destroy(nint self)
        {
            try
            {
                ((IInterceptor)ManagerPeer.Of(self).target).destroy();
            }
            catch (System.Exception e)
            {
                Console.Error.WriteLine("rustorb: Interceptor.destroy raised " + e);
            }
        }
    }
}
