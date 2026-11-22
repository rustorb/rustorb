// Servant managers (ServantActivator, ServantLocator) and adapter
// activators: the local interfaces of PortableServer as the IDL4 to C#
// mapping writes them, and how the core calls them. Each manager given to a
// POA gets a peer whose GCHandle is the self of the C ABI; the peer holds the
// manager while the POA holds it (add_ref / remove_ref).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Rustorb.Native;

namespace PortableServer
{
    /// <summary>PortableServer::ForwardRequest: raised by incarnate or
    /// preinvoke to send the client to another object.</summary>
    public class ForwardRequest : Corba.UserException
    {
        public Corba.IObject? forward_reference { get; set; }
        public ForwardRequest() { }
        public ForwardRequest(Corba.IObject? forward_reference) { this.forward_reference = forward_reference; }
        public override string _id() => "IDL:omg.org/PortableServer/ForwardRequest:1.0";
    }

    /// <summary>PortableServer::ServantManager.</summary>
    public interface IServantManager : Corba.IObject
    {
    }

    /// <summary>PortableServer::ServantActivator (POAs with RETAIN).</summary>
    public interface IServantActivator : IServantManager
    {
        /// <exception cref="ForwardRequest">ForwardRequest</exception>
        Servant incarnate(byte[] oid, POA adapter);
        void etherealize(byte[] oid, POA adapter, Servant serv, bool cleanup_in_progress, bool remaining_activations);
    }

    /// <summary>PortableServer::ServantLocator (POAs with NON_RETAIN). The
    /// native Cookie is an object, given back to postinvoke as it is.</summary>
    public interface IServantLocator : IServantManager
    {
        /// <exception cref="ForwardRequest">ForwardRequest</exception>
        Servant preinvoke(byte[] oid, POA adapter, string operation, out object? the_cookie);
        void postinvoke(byte[] oid, POA adapter, string operation, object? the_cookie, Servant the_servant);
    }

    /// <summary>PortableServer::AdapterActivator.</summary>
    public interface IAdapterActivator : Corba.IObject
    {
        bool unknown_adapter(POA parent, string name);
    }

    /// <summary>A base for servant activators: a local object.</summary>
    public abstract class ServantActivator : Corba.LocalObject, IServantActivator
    {
        protected override string[] _ids() => new[]
        {
            "IDL:omg.org/PortableServer/ServantActivator:2.3", "IDL:omg.org/PortableServer/ServantManager:1.0",
            "IDL:omg.org/CORBA/LocalObject:1.0",
        };

        public abstract Servant incarnate(byte[] oid, POA adapter);

        public virtual void etherealize(byte[] oid, POA adapter, Servant serv, bool cleanup_in_progress,
            bool remaining_activations)
        {
        }
    }

    /// <summary>A base for servant locators: a local object.</summary>
    public abstract class ServantLocator : Corba.LocalObject, IServantLocator
    {
        protected override string[] _ids() => new[]
        {
            "IDL:omg.org/PortableServer/ServantLocator:2.3", "IDL:omg.org/PortableServer/ServantManager:1.0",
            "IDL:omg.org/CORBA/LocalObject:1.0",
        };

        public abstract Servant preinvoke(byte[] oid, POA adapter, string operation, out object? the_cookie);

        public virtual void postinvoke(byte[] oid, POA adapter, string operation, object? the_cookie,
            Servant the_servant)
        {
        }
    }

    /// <summary>A base for adapter activators: a local object.</summary>
    public abstract class AdapterActivator : Corba.LocalObject, IAdapterActivator
    {
        protected override string[] _ids() => new[]
        {
            "IDL:omg.org/PortableServer/AdapterActivator:1.0", "IDL:omg.org/CORBA/LocalObject:1.0",
        };

        public abstract bool unknown_adapter(POA parent, string name);
    }
}

namespace Rustorb.Native
{
    /// <summary>The self of a servant manager or adapter activator given to a
    /// POA: holds it while the POA does.</summary>
    internal sealed unsafe class ManagerPeer
    {
        internal readonly object target;
        internal readonly Corba.ORB? orb;
        internal readonly GCHandle self;
        private int refs;
        private bool setting = true;

        private ManagerPeer(object target, Corba.ORB? orb)
        {
            this.target = target;
            this.orb = orb;
            self = GCHandle.Alloc(this);
        }

        /// <summary>Runs set with a new peer's self; the peer stays while the
        /// POA holds it.</summary>
        internal static void Set(object target, Corba.ORB? orb, Func<nint, int> set)
        {
            var p = new ManagerPeer(target, orb);
            try
            {
                set(GCHandle.ToIntPtr(p.self));
            }
            finally
            {
                p.Done();
            }
        }

        /// <summary>New peers for the targets: their selfs stay valid until
        /// Release, and after it while the core holds them.</summary>
        internal static ManagerPeer[] Hold(object[] targets)
        {
            var ps = new ManagerPeer[targets.Length];
            for (int i = 0; i < targets.Length; i++)
            {
                ps[i] = new ManagerPeer(Rustorb.Seq.NotNull(targets[i]), null);
            }
            return ps;
        }

        internal static void Release(ManagerPeer[] peers)
        {
            foreach (ManagerPeer p in peers)
            {
                p.Done();
            }
        }

        internal nint Self => GCHandle.ToIntPtr(self);

        private void Done()
        {
            bool free;
            lock (this)
            {
                setting = false;
                free = refs == 0;
            }
            if (free)
            {
                self.Free();
            }
        }

        internal static ManagerPeer Of(nint self) => (ManagerPeer)GCHandle.FromIntPtr(self).Target!;

        internal void AddRef()
        {
            lock (this)
            {
                refs++;
            }
        }

        internal void RemoveRef()
        {
            bool free;
            lock (this)
            {
                free = --refs == 0 && !setting;
            }
            if (free)
            {
                self.Free();
            }
        }

        /// <summary>The manager of a self the core gave back with a reference
        /// added for the caller.</summary>
        internal static object? Take(nint self)
        {
            if (self == 0)
            {
                return null;
            }
            ManagerPeer p = Of(self);
            object t = p.target;
            p.RemoveRef();
            return t;
        }

        // --- the vtables ---------------------------------------------------------

        internal static readonly nint* ActivatorVtable = Make(
            (nint)(delegate* unmanaged[Cdecl]<nint, byte*, nuint, nint, nint*, nint, int>)&Incarnate,
            (nint)(delegate* unmanaged[Cdecl]<nint, byte*, nuint, nint, nint, int, int, void>)&Etherealize);

        internal static readonly nint* LocatorVtable = Make(
            (nint)(delegate* unmanaged[Cdecl]<nint, byte*, nuint, nint, byte*, nuint, nint*, nint*, nint, int>)&Preinvoke,
            (nint)(delegate* unmanaged[Cdecl]<nint, byte*, nuint, nint, byte*, nuint, nint, nint, void>)&Postinvoke);

        internal static readonly nint* AdapterActivatorVtable = Make(
            (nint)(delegate* unmanaged[Cdecl]<nint, nint, byte*, nuint, int>)&UnknownAdapter);

        private static nint* Make(params nint[] first)
        {
            var v = (nint*)NativeMemory.Alloc((nuint)(sizeof(nint) * (first.Length + 2)));
            for (int i = 0; i < first.Length; i++)
            {
                v[i] = first[i];
            }
            v[first.Length] = (nint)(delegate* unmanaged[Cdecl]<nint, void>)&AddRefCb;
            v[first.Length + 1] = (nint)(delegate* unmanaged[Cdecl]<nint, void>)&RemoveRefCb;
            return v;
        }

        internal static readonly nint AddRefFn = (nint)(delegate* unmanaged[Cdecl]<nint, void>)&AddRefCb;
        internal static readonly nint RemoveRefFn = (nint)(delegate* unmanaged[Cdecl]<nint, void>)&RemoveRefCb;

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static void AddRefCb(nint self) => Of(self).AddRef();

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static void RemoveRefCb(nint self) => Of(self).RemoveRef();

        private static byte[] Id(byte* id, nuint len) => new ReadOnlySpan<byte>(id, checked((int)len)).ToArray();

        /// <summary>A POA of its own for a handle the core lends to a callback.</summary>
        private PortableServer.POA Poa(nint adapter) =>
            new PortableServer.POA(new PoaHandle(Ffi.rorb_poa_clone(adapter)), orb);

        /// <summary>What incarnate or preinvoke raised, given to the core.</summary>
        private static int Error(Exception e, nint error)
        {
            switch (e)
            {
                case PortableServer.ForwardRequest f:
                    if (f.forward_reference is Corba.Object o)
                    {
                        Ffi.rorb_manager_error_forward(error, o.Raw);
                        GC.KeepAlive(o);
                    }
                    else
                    {
                        Ffi.rorb_manager_error_forward(error, 0);
                    }
                    return Status.USER_EXCEPTION;
                case Corba.SystemException s:
                    byte[] id = Strings.ToUtf8(s._id());
                    fixed (byte* p = id)
                    {
                        Ffi.rorb_manager_error_system(error, p, (nuint)id.Length, s.minor, (uint)s.completed);
                    }
                    return Status.SYSTEM_EXCEPTION;
                default:
                    Console.Error.WriteLine("rustorb: a servant manager raised " + e);
                    return Status.SYSTEM_EXCEPTION;
            }
        }

        private static nint Provide(PortableServer.Servant? s) =>
            s == null ? 0 : s.Native();

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int Incarnate(nint self, byte* id, nuint len, nint adapter, nint* servant, nint error)
        {
            try
            {
                ManagerPeer p = Of(self);
                var a = (PortableServer.IServantActivator)p.target;
                *servant = Provide(a.incarnate(Id(id, len), p.Poa(adapter)));
                return Status.OK;
            }
            catch (Exception e)
            {
                return Error(e, error);
            }
        }

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static void Etherealize(nint self, byte* id, nuint len, nint adapter, nint servant, int cleanup,
            int remaining)
        {
            try
            {
                ManagerPeer p = Of(self);
                PortableServer.Servant? s = PortableServer.Servant.FromSelf(servant);
                if (s != null)
                {
                    ((PortableServer.IServantActivator)p.target).etherealize(Id(id, len), p.Poa(adapter), s,
                        cleanup != 0, remaining != 0);
                }
            }
            catch (Exception e)
            {
                Console.Error.WriteLine("rustorb: etherealize raised " + e);
            }
        }

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int Preinvoke(nint self, byte* id, nuint len, nint adapter, byte* op, nuint opLen,
            nint* servant, nint* cookie, nint error)
        {
            try
            {
                ManagerPeer p = Of(self);
                var l = (PortableServer.IServantLocator)p.target;
                PortableServer.Servant s = l.preinvoke(Id(id, len), p.Poa(adapter), Strings.FromUtf8(op, opLen),
                    out object? c);
                *cookie = c == null ? 0 : GCHandle.ToIntPtr(GCHandle.Alloc(c));
                *servant = Provide(s);
                return Status.OK;
            }
            catch (Exception e)
            {
                return Error(e, error);
            }
        }

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static void Postinvoke(nint self, byte* id, nuint len, nint adapter, byte* op, nuint opLen,
            nint cookie, nint servant)
        {
            object? c = null;
            if (cookie != 0)
            {
                GCHandle h = GCHandle.FromIntPtr(cookie);
                c = h.Target;
                h.Free();
            }
            try
            {
                ManagerPeer p = Of(self);
                PortableServer.Servant? s = PortableServer.Servant.FromSelf(servant);
                if (s != null)
                {
                    ((PortableServer.IServantLocator)p.target).postinvoke(Id(id, len), p.Poa(adapter),
                        Strings.FromUtf8(op, opLen), c, s);
                }
            }
            catch (Exception e)
            {
                Console.Error.WriteLine("rustorb: postinvoke raised " + e);
            }
        }

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static int UnknownAdapter(nint self, nint parent, byte* name, nuint len)
        {
            try
            {
                ManagerPeer p = Of(self);
                return ((PortableServer.IAdapterActivator)p.target).unknown_adapter(p.Poa(parent),
                    Strings.FromUtf8(name, len)) ? 1 : 0;
            }
            catch (Exception e)
            {
                Console.Error.WriteLine("rustorb: unknown_adapter raised " + e);
                return 0;
            }
        }
    }
}
