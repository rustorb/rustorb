// PortableServer.Servant and how the core calls it: each servant has a peer
// whose GCHandle is the self of its rorb_servant. The peer holds the servant
// strongly while the core holds it (add_ref / remove_ref), weakly otherwise,
// so that a servant the POA has let go can be collected.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Threading;
using Rustorb.Native;

namespace PortableServer
{
    /// <summary>The base of servants. The generated skeleton &lt;Name&gt;POA derives
    /// from it and implements I&lt;Name&gt;Operations; applications derive from the
    /// skeleton.</summary>
    public abstract unsafe class Servant
    {
        private Peer? peer;

        /// <summary>The repository ids of the servant's interfaces, most derived
        /// first.</summary>
        public abstract string[] _all_interfaces(POA? poa, byte[]? objectId);

        /// <summary>Whether the servant supports the interface.</summary>
        public virtual bool _is_a(string repositoryId) =>
            repositoryId == "IDL:omg.org/CORBA/Object:1.0" || Array.IndexOf(_all_interfaces(null, null), repositoryId) >= 0;

        /// <summary>The POA _this activates the servant in: the RootPOA of the
        /// first ORB initialised.</summary>
        public virtual POA _default_POA()
        {
            nint poa;
            RorbException exc;
            Errors.Check(Ffi.rorb_default_root_poa(&poa, &exc), ref exc);
            return new POA(new PoaHandle(poa), null);
        }

        /// <summary>Whether the servant does not exist (answered for _non_existent).</summary>
        public virtual bool _non_existent() => false;

        /// <summary>Reads the arguments of operation, calls it and writes the
        /// reply. Written by the generated skeletons.</summary>
        [EditorBrowsable(EditorBrowsableState.Never)]
        public abstract void _dispatch(string operation, Rustorb.CdrInput input, Rustorb.ServerReply reply);

        /// <summary>The reference of the request being run on this servant, else
        /// the servant activated (implicitly) in _default_POA().</summary>
        protected Corba.Object _this_object()
        {
            nint s = Native();
            try
            {
                nint obj;
                if (Ffi.rorb_servant_current_reference(s, &obj) != 0)
                {
                    return new Corba.Object(new ObjectHandle(obj));
                }
            }
            finally
            {
                Ffi.rorb_servant_release(s);
            }
            return _default_POA().servant_to_reference(this);
        }

        /// <summary>A new handle of the core's servant for this one.</summary>
        internal nint Native()
        {
            Peer p = LazyInitializer.EnsureInitialized(ref peer, () => new Peer(this));
            string[] ids = _all_interfaces(null, null);
            byte[] id = Strings.ToUtf8(ids.Length > 0 ? ids[0] : "IDL:omg.org/CORBA/Object:1.0");
            fixed (byte* ip = id)
            {
                nint s = Ffi.rorb_servant_get(GCHandle.ToIntPtr(p.self), ip, (nuint)id.Length, Peer.Vtable);
                GC.KeepAlive(this);
                return s;
            }
        }

        ~Servant()
        {
            peer?.Finalized();
        }

        /// <summary>The servant of a self pointer given back by the core.</summary>
        internal static Servant? FromSelf(nint self)
        {
            if (self == 0)
            {
                return null;
            }
            var p = (Peer)GCHandle.FromIntPtr(self).Target!;
            return p.Get();
        }

        /// <summary>Gives back the reference the core added for the caller
        /// (id_to_servant and friends).</summary>
        internal static void Unref(nint self)
        {
            if (self != 0)
            {
                ((Peer)GCHandle.FromIntPtr(self).Target!).RemoveRef();
            }
        }

        internal sealed class Peer
        {
            internal static readonly RorbServantVtable* Vtable = MakeVtable();

            internal readonly GCHandle self;
            private readonly WeakReference<Servant> weak;
            private Servant? strong;
            private int refs;
            private bool finalized;

            internal Peer(Servant s)
            {
                weak = new WeakReference<Servant>(s);
                self = GCHandle.Alloc(this);
            }

            internal Servant? Get()
            {
                lock (this)
                {
                    if (strong != null)
                    {
                        return strong;
                    }
                }
                return weak.TryGetTarget(out Servant? s) ? s : null;
            }

            internal void AddRef()
            {
                lock (this)
                {
                    if (refs++ == 0)
                    {
                        weak.TryGetTarget(out strong);
                    }
                }
            }

            internal void RemoveRef()
            {
                bool free;
                lock (this)
                {
                    if (--refs == 0)
                    {
                        strong = null;
                    }
                    free = refs == 0 && finalized;
                }
                if (free)
                {
                    self.Free();
                }
            }

            internal void Finalized()
            {
                bool free;
                lock (this)
                {
                    finalized = true;
                    free = refs == 0;
                }
                if (free)
                {
                    self.Free();
                }
            }

            private static RorbServantVtable* MakeVtable()
            {
                var v = (RorbServantVtable*)NativeMemory.Alloc((nuint)sizeof(RorbServantVtable));
                v->dispatch = (nint)(delegate* unmanaged[Cdecl]<nint, byte*, nuint, nint, nint, int>)&Dispatch;
                v->is_a = (nint)(delegate* unmanaged[Cdecl]<nint, byte*, nuint, int>)&IsA;
                v->add_ref = (nint)(delegate* unmanaged[Cdecl]<nint, void>)&AddRefCb;
                v->remove_ref = (nint)(delegate* unmanaged[Cdecl]<nint, void>)&RemoveRefCb;
                return v;
            }

            private static Peer Of(nint self) => (Peer)GCHandle.FromIntPtr(self).Target!;

            [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
            private static void AddRefCb(nint self) => Of(self).AddRef();

            [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
            private static void RemoveRefCb(nint self) => Of(self).RemoveRef();

            [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
            private static int IsA(nint self, byte* id, nuint len)
            {
                try
                {
                    Servant? s = Of(self).Get();
                    return s != null && s._is_a(Strings.FromUtf8(id, len)) ? 1 : 0;
                }
                catch (Exception)
                {
                    return 0;
                }
            }

            [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
            private static int Dispatch(nint self, byte* op, nuint opLen, nint input, nint reply)
            {
                var r = new Rustorb.ServerReply(reply);
                try
                {
                    Servant? s = Of(self).Get();
                    if (s == null)
                    {
                        throw new Corba.OBJECT_NOT_EXIST("the servant was collected", 0,
                            Corba.CompletionStatus.COMPLETED_NO);
                    }
                    s._dispatch(Strings.FromUtf8(op, opLen), new Rustorb.CdrInput(input), r);
                    return Status.OK;
                }
                catch (Corba.SystemException e)
                {
                    r.system_exception(e);
                    return Status.SYSTEM_EXCEPTION;
                }
                catch (Exception e)
                {
                    Console.Error.WriteLine("rustorb: a servant raised " + e);
                    r.system_exception(new Corba.UNKNOWN("", 0, Corba.CompletionStatus.COMPLETED_MAYBE));
                    return Status.SYSTEM_EXCEPTION;
                }
            }
        }
    }
}

namespace Rustorb
{
    /// <summary>The reply of a request a servant runs. Used by the generated
    /// skeletons.</summary>
    public readonly unsafe struct ServerReply
    {
        private readonly nint ptr;

        internal ServerReply(nint ptr)
        {
            this.ptr = ptr;
        }

        /// <summary>The stream of a normal reply.</summary>
        public CdrOutput output() => new CdrOutput(Ffi.rorb_server_reply_output(ptr));

        /// <summary>Replaces the reply with the user exception id; its members
        /// are written to the stream returned.</summary>
        public CdrOutput user_exception(string id)
        {
            byte[] b = Native.Strings.ToUtf8(id);
            fixed (byte* p = b)
            {
                return new CdrOutput(Ffi.rorb_server_reply_user_exception(ptr, p, (nuint)b.Length));
            }
        }

        internal void system_exception(Corba.SystemException e)
        {
            byte[] b = Native.Strings.ToUtf8(e._id());
            fixed (byte* p = b)
            {
                Ffi.rorb_server_reply_system_exception(ptr, p, (nuint)b.Length, e.minor, (uint)e.completed);
            }
        }

        /// <summary>Writes the results of an operation that has run: a system
        /// exception while writing them is COMPLETED_YES.</summary>
        public void results(Action<CdrOutput> write)
        {
            try
            {
                write(output());
            }
            catch (Corba.SystemException e)
            {
                e.completed = Corba.CompletionStatus.COMPLETED_YES;
                throw;
            }
        }
    }

    /// <summary>The exceptions of the generated skeletons.</summary>
    public static class Servants
    {
        /// <summary>The exception for an operation the servant does not have.</summary>
        public static Corba.BAD_OPERATION BadOperation() =>
            new Corba.BAD_OPERATION("", 0x4f4d0002, Corba.CompletionStatus.COMPLETED_NO);

        /// <summary>Throws MARSHAL (COMPLETED_NO) if the arguments of a request
        /// could not be read.</summary>
        public static void CheckArguments(CdrInput input)
        {
            if (input.failed)
            {
                throw new Corba.MARSHAL("bad arguments", 0, Corba.CompletionStatus.COMPLETED_NO);
            }
        }
    }
}
