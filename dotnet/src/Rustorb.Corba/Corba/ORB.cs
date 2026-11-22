// Corba.ORB: the PIDL of CORBA::ORB with the IDL naming (the IDL4 to C#
// mapping leaves the ORB to the platform).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using Rustorb.Native;

namespace Corba
{
    /// <summary>CORBA::ORB.</summary>
    public sealed unsafe class ORB
    {
        internal readonly OrbHandle _handle;

        internal ORB(OrbHandle handle)
        {
            _handle = handle;
        }

        /// <summary>ORB::InvalidName.</summary>
        public class InvalidName : UserException
        {
            public override string _id() => "IDL:omg.org/CORBA/ORB/InvalidName:1.0";
        }

        internal nint Raw => _handle.Raw;

        /// <summary>CORBA::ORB_init: the ORB options of args (-ORBInitRef,
        /// -ORBendPoint... as in the other bindings of rustorb) configure it; the
        /// other arguments are ignored.</summary>
        public static ORB init(string[] args) => init(args, out _);

        /// <summary>ORB_init that also gives the arguments that are not ORB
        /// options.</summary>
        public static ORB init(string[] args, out string[] rest) => Init(args, null, null, out rest);

        /// <summary>ORB_init with the ORB id (which -ORBid overrides) and the
        /// initializers of this ORB only, run after the ones of
        /// PortableInterceptor.ORBInitializerRegistry. A failing initializer
        /// fails the ORB with its exception.</summary>
        public static ORB init(string[] args, string orb_id, params PortableInterceptor.IORBInitializer[] initializers) =>
            Init(args, orb_id ?? "", initializers ?? System.Array.Empty<PortableInterceptor.IORBInitializer>(), out _);

        private struct InitializerEntry
        {
            public nint self;
            public void* vtable;
        }

        private static ORB Init(string[] args, string? orbId, PortableInterceptor.IORBInitializer[]? inits,
            out string[] rest)
        {
            Library.Check();
            args ??= System.Array.Empty<string>();
            var all = new string[args.Length + 1];
            all[0] = "dotnet";
            System.Array.Copy(args, 0, all, 1, args.Length);
            var pins = new GCHandle[all.Length];
            var argv = new byte*[all.Length + 1];
            try
            {
                for (int i = 0; i < all.Length; i++)
                {
                    byte[] b = Strings.ToUtf8(all[i]);
                    var z = new byte[b.Length + 1];
                    b.CopyTo(z, 0);
                    pins[i] = GCHandle.Alloc(z, GCHandleType.Pinned);
                    argv[i] = (byte*)pins[i].AddrOfPinnedObject();
                }
                int argc = all.Length;
                nint orb = 0;
                RorbException exc = default;
                fixed (byte** pv = argv)
                {
                    if (orbId == null)
                    {
                        Errors.Check(Ffi.rorb_orb_init(&argc, pv, &orb, &exc), ref exc);
                    }
                    else
                    {
                        var entries = new InitializerEntry[inits!.Length];
                        byte[] id = Strings.ToUtf8(orbId);
                        ManagerPeer[] held = ManagerPeer.Hold(inits);
                        try
                        {
                            for (int i = 0; i < held.Length; i++)
                            {
                                entries[i] = new InitializerEntry
                                {
                                    self = held[i].Self, vtable = PortableInterceptor.PiCallbacks.InitializerVtable,
                                };
                            }
                            int st;
                            fixed (byte* ip = id)
                            fixed (InitializerEntry* ep = entries)
                            {
                                st = Ffi.rorb_orb_init_pi(&argc, pv, ip, (nuint)id.Length, ep, (nuint)entries.Length,
                                    &orb, &exc);
                            }
                            Errors.Check(st, ref exc);
                        }
                        finally
                        {
                            ManagerPeer.Release(held);
                        }
                    }
                    rest = new string[Math.Max(argc - 1, 0)];
                    for (int i = 1; i < argc; i++)
                    {
                        rest[i - 1] = Marshal.PtrToStringUTF8((nint)pv[i]) ?? "";
                    }
                }
                return new ORB(new OrbHandle(orb));
            }
            finally
            {
                foreach (GCHandle p in pins)
                {
                    if (p.IsAllocated)
                    {
                        p.Free();
                    }
                }
            }
        }

        public string object_to_string(IObject? obj)
        {
            if (obj != null && obj is not Object)
            {
                throw new MARSHAL("a local object has no IOR", 0, CompletionStatus.COMPLETED_NO);
            }
            var o = obj as Object;
            RorbBytes s;
            RorbException exc;
            Errors.Check(Ffi.rorb_orb_object_to_string(Raw, o == null ? 0 : o.Raw, &s, &exc), ref exc);
            GC.KeepAlive(o);
            GC.KeepAlive(this);
            return Strings.Take(s);
        }

        /// <summary>An IOR:, corbaloc: or corbaname: string to a reference; null
        /// for a nil one.</summary>
        public Object? string_to_object(string str)
        {
            byte[] b = Strings.ToUtf8(str);
            nint obj;
            RorbException exc;
            fixed (byte* p = b)
            {
                Errors.Check(Ffi.rorb_orb_string_to_object(Raw, p, (nuint)b.Length, &obj, &exc), ref exc);
            }
            GC.KeepAlive(this);
            return obj == 0 ? null : new Object(new ObjectHandle(obj));
        }

        /// <summary>"RootPOA" gives a PortableServer.POA and "POACurrent" a
        /// PortableServer.Current; the others object references.</summary>
        public IObject? resolve_initial_references(string identifier)
        {
            if (identifier == "RootPOA")
            {
                nint poa;
                RorbException e;
                Errors.Check(Ffi.rorb_orb_root_poa(Raw, &poa, &e), ref e);
                GC.KeepAlive(this);
                return new PortableServer.POA(new PoaHandle(poa), this);
            }
            if (identifier == "POACurrent")
            {
                return new PortableServer.Current(this);
            }
            if (identifier == "DynAnyFactory")
            {
                return DynamicAny.DynAnyFactory.Instance;
            }
            if (identifier == "PICurrent")
            {
                var c = new PortableInterceptor.Current(new PiCurrentHandle(Ffi.rorb_orb_pi_current(Raw)));
                GC.KeepAlive(this);
                return c;
            }
            if (identifier == "CodecFactory")
            {
                var f = new IOP.CodecFactory(new OrbHandle(Ffi.rorb_orb_clone(Raw)));
                GC.KeepAlive(this);
                return f;
            }
            byte[] b = Strings.ToUtf8(identifier);
            nint obj;
            RorbException exc;
            fixed (byte* p = b)
            {
                Errors.Check(Ffi.rorb_orb_resolve_initial_references(Raw, p, (nuint)b.Length, &obj, &exc), ref exc);
            }
            GC.KeepAlive(this);
            return obj == 0 ? null : new Object(new ObjectHandle(obj));
        }

        public void register_initial_reference(string identifier, IObject obj)
        {
            if (obj is not Object o)
            {
                throw new BAD_PARAM("register_initial_reference takes an object reference", 27,
                    CompletionStatus.COMPLETED_NO);
            }
            byte[] b = Strings.ToUtf8(identifier);
            RorbException exc;
            fixed (byte* p = b)
            {
                Errors.Check(Ffi.rorb_orb_register_initial_reference(Raw, p, (nuint)b.Length, o.Raw, &exc), ref exc);
            }
            GC.KeepAlive(o);
            GC.KeepAlive(this);
        }

        private static readonly delegate* unmanaged[Cdecl]<nint, byte*, nuint, void> EachName = &EachNameCb;

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        private static void EachNameCb(nint context, byte* name, nuint len)
        {
            var list = (List<string>)GCHandle.FromIntPtr(context).Target!;
            list.Add(Strings.FromUtf8(name, len));
        }

        public string[] list_initial_services()
        {
            var list = new List<string>();
            GCHandle h = GCHandle.Alloc(list);
            try
            {
                Ffi.rorb_orb_list_initial_services(Raw, GCHandle.ToIntPtr(h), (nint)EachName);
            }
            finally
            {
                h.Free();
            }
            GC.KeepAlive(this);
            if (!list.Contains("RootPOA"))
            {
                list.Add("RootPOA");
            }
            foreach (string local in new[] { "POACurrent", "DynAnyFactory", "PICurrent", "CodecFactory" })
            {
                if (!list.Contains(local))
                {
                    list.Add(local);
                }
            }
            return list.ToArray();
        }

        /// <summary>Blocks until the ORB is shut down.</summary>
        public void run()
        {
            Ffi.rorb_orb_run(Raw);
            GC.KeepAlive(this);
        }

        public bool work_pending()
        {
            int r;
            RorbException exc;
            Errors.Check(Ffi.rorb_orb_work_pending(Raw, &r, &exc), ref exc);
            GC.KeepAlive(this);
            return r != 0;
        }

        public void perform_work()
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_orb_perform_work(Raw, &exc), ref exc);
            GC.KeepAlive(this);
        }

        public void shutdown(bool wait_for_completion)
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_orb_shutdown(Raw, wait_for_completion ? 1 : 0, &exc), ref exc);
            GC.KeepAlive(this);
        }

        public void destroy()
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_orb_destroy(Raw, &exc), ref exc);
            GC.KeepAlive(this);
        }

        /// <summary>The reply timeout of the ORB's calls, in microseconds
        /// (rustorb): 0 is no limit.</summary>
        public long call_timeout_us
        {
            get { long v = Ffi.rorb_orb_call_timeout(Raw); GC.KeepAlive(this); return v; }
            set { Ffi.rorb_orb_set_call_timeout(Raw, value); GC.KeepAlive(this); }
        }

        /// <summary>The reply timeout of the calling thread's calls (every ORB),
        /// in microseconds (rustorb): negative unsets it, 0 is no limit.</summary>
        public static long thread_call_timeout_us
        {
            get => Ffi.rorb_thread_call_timeout();
            set => Ffi.rorb_set_thread_call_timeout(value);
        }

        // --- the DII ---------------------------------------------------------------

        public NVList create_list(int count) => new NVList();

        public NamedValue create_named_value(string name, Any value, uint flags) => new NamedValue(name, value, flags);

        public ExceptionList create_exception_list() => new ExceptionList();

        public Environment create_environment() => new Environment();

        /// <summary>Contexts are not supported: NO_IMPLEMENT.</summary>
        public object get_default_context() =>
            throw new NO_IMPLEMENT("contexts are not supported", 0, CompletionStatus.COMPLETED_NO);

        private static readonly List<Request> deferred = new();

        internal static void Deferred(Request r, bool add)
        {
            lock (deferred)
            {
                if (add)
                {
                    deferred.Add(r);
                }
                else
                {
                    deferred.Remove(r);
                }
            }
        }

        public void send_multiple_requests_oneway(Request[] req)
        {
            foreach (Request r in req)
            {
                r.send_oneway();
            }
        }

        public void send_multiple_requests_deferred(Request[] req)
        {
            foreach (Request r in req)
            {
                r.send_deferred();
            }
        }

        /// <summary>Whether a request sent deferred has its response
        /// (BAD_INV_ORDER when none is outstanding).</summary>
        public bool poll_next_response()
        {
            Request[] all;
            lock (deferred)
            {
                all = deferred.ToArray();
            }
            if (all.Length == 0)
            {
                throw new BAD_INV_ORDER("no deferred request is outstanding", 11, CompletionStatus.COMPLETED_NO);
            }
            foreach (Request r in all)
            {
                if (r.Pending && r.poll_response())
                {
                    return true;
                }
            }
            return false;
        }

        /// <summary>The next request sent deferred whose response has come,
        /// after get_response (it looks every millisecond; BAD_INV_ORDER when
        /// none is outstanding).</summary>
        public Request get_next_response()
        {
            while (true)
            {
                Request[] all;
                lock (deferred)
                {
                    all = deferred.ToArray();
                }
                if (all.Length == 0)
                {
                    throw new BAD_INV_ORDER("no deferred request is outstanding", 11, CompletionStatus.COMPLETED_NO);
                }
                foreach (Request r in all)
                {
                    bool ready;
                    try
                    {
                        ready = r.Pending && r.poll_response();
                    }
                    catch (BAD_INV_ORDER)
                    {
                        continue;
                    }
                    if (ready)
                    {
                        try
                        {
                            r.get_response();
                        }
                        catch (UnknownUserException) { }
                        catch (SystemException) { }
                        return r;
                    }
                }
                System.Threading.Thread.Sleep(1);
            }
        }

        public Any create_any() => new Any();

        public TypeCode get_primitive_tc(TCKind kind) => Rustorb.TypeCodes.Simple(kind);
        public TypeCode create_string_tc(uint bound) => Rustorb.TypeCodes.String(bound);
        public TypeCode create_wstring_tc(uint bound) => Rustorb.TypeCodes.WString(bound);
        public TypeCode create_fixed_tc(ushort digits, short scale) => Rustorb.TypeCodes.Fixed(digits, scale);
        public TypeCode create_sequence_tc(uint bound, TypeCode element_type) => Rustorb.TypeCodes.Sequence(element_type, bound);
        public TypeCode create_array_tc(uint length, TypeCode element_type) => Rustorb.TypeCodes.Array(element_type, length);
        public TypeCode create_alias_tc(string id, string name, TypeCode original_type) => Rustorb.TypeCodes.Alias(id, name, original_type);
        public TypeCode create_interface_tc(string id, string name) => Rustorb.TypeCodes.Interface(TCKind.tk_objref, id, name);
        public TypeCode create_enum_tc(string id, string name, string[] members) => Rustorb.TypeCodes.Enum(id, name, members);
        public TypeCode create_recursive_tc(string id) => Rustorb.TypeCodes.Recursive(id);

        public TypeCode create_struct_tc(string id, string name, StructMember[] members) =>
            Rustorb.TypeCodes.Struct(id, name, System.Array.ConvertAll(members, m => m.name),
                System.Array.ConvertAll(members, m => m.type));

        public TypeCode create_exception_tc(string id, string name, StructMember[] members) =>
            Rustorb.TypeCodes.Exception(id, name, System.Array.ConvertAll(members, m => m.name),
                System.Array.ConvertAll(members, m => m.type));

        public TypeCode create_value_tc(string id, string name, short type_modifier, TypeCode? concrete_base,
            ValueMember[] members) =>
            Rustorb.TypeCodes.Value(TCKind.tk_value, id, name, type_modifier, concrete_base,
                System.Array.ConvertAll(members, m => m.name), System.Array.ConvertAll(members, m => m.type),
                System.Array.ConvertAll(members, m => m.access));

        /// <summary>Registers the factory of the valuetype id for the process
        /// (rustorb keeps one registry, shared by its ORBs); returns the
        /// factory it replaces, or null.</summary>
        public IValueFactory? register_value_factory(string id, IValueFactory factory) =>
            Rustorb.Values.Register(id, factory);

        public void unregister_value_factory(string id) => Rustorb.Values.Unregister(id);

        /// <summary>The factory registered for id, else the default factory of
        /// the generated code (valuetypes without operations, value boxes);
        /// null if there is none.</summary>
        public IValueFactory? lookup_value_factory(string id) => Rustorb.Values.Lookup(id);

        public TypeCode create_value_box_tc(string id, string name, TypeCode boxed_type) =>
            Rustorb.TypeCodes.ValueBox(id, name, boxed_type);

        public TypeCode create_native_tc(string id, string name) => Rustorb.TypeCodes.Interface(TCKind.tk_native, id, name);

        public TypeCode create_abstract_interface_tc(string id, string name) =>
            Rustorb.TypeCodes.Interface(TCKind.tk_abstract_interface, id, name);

        public TypeCode create_local_interface_tc(string id, string name) =>
            Rustorb.TypeCodes.Interface(TCKind.tk_local_interface, id, name);

        /// <summary>The default member, if any, has the octet 0 as its label.</summary>
        public TypeCode create_union_tc(string id, string name, TypeCode discriminator_type, UnionMember[] members)
        {
            var labels = new ulong[members.Length];
            int def = -1;
            for (int i = 0; i < members.Length; i++)
            {
                Any l = members[i].label;
                if (l.type.kind() == TCKind.tk_octet)
                {
                    def = i;
                    continue;
                }
                ulong v;
                if (Ffi.rorb_any_label(l._handle.Raw, &v) == 0)
                {
                    throw new BAD_PARAM("a union label that is not of a discriminator type", 19,
                        CompletionStatus.COMPLETED_NO);
                }
                labels[i] = v;
                GC.KeepAlive(l);
            }
            return Rustorb.TypeCodes.Union(id, name, discriminator_type, labels, def,
                System.Array.ConvertAll(members, m => m.name), System.Array.ConvertAll(members, m => m.type));
        }
    }

    /// <summary>CORBA::StructMember (type_def is not kept: there is no
    /// interface repository).</summary>
    public class StructMember
    {
        public string name { get; set; } = "";
        public TypeCode type { get; set; } = Rustorb.TypeCodes.Simple(TCKind.tk_null);
        public StructMember() { }
        public StructMember(string name, TypeCode type) { this.name = name; this.type = type; }
    }

    /// <summary>CORBA::ValueMember (id, defined_in, version and type_def are
    /// not kept: there is no interface repository).</summary>
    public class ValueMember
    {
        public string name { get; set; } = "";
        public string id { get; set; } = "";
        public string defined_in { get; set; } = "";
        public string version { get; set; } = "";
        public TypeCode type { get; set; } = Rustorb.TypeCodes.Simple(TCKind.tk_null);
        /// <summary>Constants.PRIVATE_MEMBER or Constants.PUBLIC_MEMBER.</summary>
        public short access { get; set; }
        public ValueMember() { }
        public ValueMember(string name, TypeCode type, short access) { this.name = name; this.type = type; this.access = access; }
    }

    /// <summary>The constants of the CORBA module (IDL4 to C#: a Constants
    /// class per namespace) that the runtime uses.</summary>
    public static partial class Constants
    {
        public const short VM_NONE = 0;
        public const short VM_CUSTOM = 1;
        public const short VM_ABSTRACT = 2;
        public const short VM_TRUNCATABLE = 3;
        public const short PRIVATE_MEMBER = 0;
        public const short PUBLIC_MEMBER = 1;
    }

    /// <summary>CORBA::UnionMember.</summary>
    public class UnionMember
    {
        public string name { get; set; } = "";
        public Any label { get; set; } = new Any();
        public TypeCode type { get; set; } = Rustorb.TypeCodes.Simple(TCKind.tk_null);
        public UnionMember() { }
        public UnionMember(string name, Any label, TypeCode type) { this.name = name; this.label = label; this.type = type; }
    }
}
