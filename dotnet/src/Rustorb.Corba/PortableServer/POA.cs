// PortableServer: the PIDL of the POA, POAManager, Current and the POA
// policies with the IDL naming (the IDL4 to C# mapping leaves them to the
// platform).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using Rustorb.Native;

namespace PortableServer
{
    public enum ThreadPolicyValue { ORB_CTRL_MODEL, SINGLE_THREAD_MODEL, MAIN_THREAD_MODEL }
    public enum LifespanPolicyValue { TRANSIENT, PERSISTENT }
    public enum IdUniquenessPolicyValue { UNIQUE_ID, MULTIPLE_ID }
    public enum IdAssignmentPolicyValue { USER_ID, SYSTEM_ID }
    public enum ImplicitActivationPolicyValue { IMPLICIT_ACTIVATION, NO_IMPLICIT_ACTIVATION }
    public enum ServantRetentionPolicyValue { RETAIN, NON_RETAIN }
    public enum RequestProcessingPolicyValue { USE_ACTIVE_OBJECT_MAP_ONLY, USE_DEFAULT_SERVANT, USE_SERVANT_MANAGER }

    public sealed class ThreadPolicy : Corba.Policy
    {
        internal ThreadPolicy(ThreadPolicyValue v) : base(16, (ulong)v) { value = v; }
        public ThreadPolicyValue value { get; }
    }

    public sealed class LifespanPolicy : Corba.Policy
    {
        internal LifespanPolicy(LifespanPolicyValue v) : base(17, (ulong)v) { value = v; }
        public LifespanPolicyValue value { get; }
    }

    public sealed class IdUniquenessPolicy : Corba.Policy
    {
        internal IdUniquenessPolicy(IdUniquenessPolicyValue v) : base(18, (ulong)v) { value = v; }
        public IdUniquenessPolicyValue value { get; }
    }

    public sealed class IdAssignmentPolicy : Corba.Policy
    {
        internal IdAssignmentPolicy(IdAssignmentPolicyValue v) : base(19, (ulong)v) { value = v; }
        public IdAssignmentPolicyValue value { get; }
    }

    public sealed class ImplicitActivationPolicy : Corba.Policy
    {
        internal ImplicitActivationPolicy(ImplicitActivationPolicyValue v) : base(20, (ulong)v) { value = v; }
        public ImplicitActivationPolicyValue value { get; }
    }

    public sealed class ServantRetentionPolicy : Corba.Policy
    {
        internal ServantRetentionPolicy(ServantRetentionPolicyValue v) : base(21, (ulong)v) { value = v; }
        public ServantRetentionPolicyValue value { get; }
    }

    public sealed class RequestProcessingPolicy : Corba.Policy
    {
        internal RequestProcessingPolicy(RequestProcessingPolicyValue v) : base(22, (ulong)v) { value = v; }
        public RequestProcessingPolicyValue value { get; }
    }

    /// <summary>BiDirPolicy::BidirectionalPolicy (NORMAL 0, BOTH 1;
    /// docs/bidir-giop.md).</summary>
    public sealed class BidirectionalPolicy : Corba.Policy
    {
        public const ushort NORMAL = 0;
        public const ushort BOTH = 1;
        internal BidirectionalPolicy(ushort v) : base(37, v) { value = v; }
        public ushort value { get; }
    }

    /// <summary>PortableServer::POA.</summary>
    public sealed unsafe class POA : Corba.LocalObject
    {
        internal readonly PoaHandle _handle;
        private readonly Corba.ORB? orb;

        internal POA(PoaHandle handle, Corba.ORB? orb)
        {
            _handle = handle;
            this.orb = orb;
        }

        protected override string[] _ids() => new[] { "IDL:omg.org/PortableServer/POA:2.3" };

        public override Corba.ORB _get_orb() =>
            orb ?? throw new Corba.NO_IMPLEMENT("this POA does not know its ORB", 0, Corba.CompletionStatus.COMPLETED_NO);

        public class AdapterAlreadyExists : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableServer/POA/AdapterAlreadyExists:1.0";
        }

        public class AdapterNonExistent : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableServer/POA/AdapterNonExistent:1.0";
        }

        public class InvalidPolicy : Corba.UserException
        {
            public ushort index { get; set; }
            public InvalidPolicy() { }
            public InvalidPolicy(ushort index) { this.index = index; }
            public override string _id() => "IDL:omg.org/PortableServer/POA/InvalidPolicy:1.0";
        }

        public class NoServant : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableServer/POA/NoServant:1.0";
        }

        public class ObjectAlreadyActive : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableServer/POA/ObjectAlreadyActive:1.0";
        }

        public class ObjectNotActive : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableServer/POA/ObjectNotActive:1.0";
        }

        public class ServantAlreadyActive : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableServer/POA/ServantAlreadyActive:1.0";
        }

        public class ServantNotActive : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableServer/POA/ServantNotActive:1.0";
        }

        public class WrongAdapter : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableServer/POA/WrongAdapter:1.0";
        }

        public class WrongPolicy : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableServer/POA/WrongPolicy:1.0";
        }

        private nint Raw => _handle.Raw;

        public string the_name
        {
            get
            {
                nuint len;
                byte* p = Ffi.rorb_poa_name(Raw, &len);
                string s = Strings.FromUtf8(p, len);
                GC.KeepAlive(this);
                return s;
            }
        }

        /// <summary>null for the RootPOA.</summary>
        public POA? the_parent
        {
            get
            {
                nint p = Ffi.rorb_poa_parent(Raw);
                GC.KeepAlive(this);
                return p == 0 ? null : new POA(new PoaHandle(p), orb);
            }
        }

        private static readonly delegate* unmanaged[Cdecl]<nint, nint, void> EachChild = &EachChildCb;

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
        private static void EachChildCb(nint context, nint child)
        {
            var list = (List<nint>)GCHandle.FromIntPtr(context).Target!;
            list.Add(Ffi.rorb_poa_clone(child));
        }

        public POA[] the_children
        {
            get
            {
                var list = new List<nint>();
                GCHandle h = GCHandle.Alloc(list);
                try
                {
                    Ffi.rorb_poa_children(Raw, GCHandle.ToIntPtr(h), (nint)EachChild);
                }
                finally
                {
                    h.Free();
                }
                GC.KeepAlive(this);
                return list.ConvertAll(p => new POA(new PoaHandle(p), orb)).ToArray();
            }
        }

        public POAManager the_POAManager
        {
            get
            {
                var m = new POAManager(new PoaManagerHandle(Ffi.rorb_poa_the_manager(Raw)));
                GC.KeepAlive(this);
                return m;
            }
        }

        public POA create_POA(string adapter_name, POAManager? a_POAManager, Corba.Policy[] policies)
        {
            byte[] n = Strings.ToUtf8(adapter_name);
            policies ??= Array.Empty<Corba.Policy>();
            var ps = new RorbPolicy[policies.Length];
            for (int i = 0; i < policies.Length; i++)
            {
                ps[i] = new RorbPolicy { type = policies[i].policy_type, value = (uint)policies[i].raw };
            }
            nint child;
            RorbException exc;
            fixed (byte* np = n)
            fixed (RorbPolicy* pp = ps)
            {
                Errors.Check(Ffi.rorb_poa_create_poa(Raw, np, (nuint)n.Length,
                    a_POAManager == null ? 0 : a_POAManager._handle.Raw, pp, (nuint)ps.Length, &child, &exc), ref exc);
            }
            GC.KeepAlive(a_POAManager);
            GC.KeepAlive(this);
            return new POA(new PoaHandle(child), orb);
        }

        public POA find_POA(string adapter_name, bool activate_it)
        {
            byte[] n = Strings.ToUtf8(adapter_name);
            nint child;
            RorbException exc;
            fixed (byte* np = n)
            {
                Errors.Check(Ffi.rorb_poa_find_poa(Raw, np, (nuint)n.Length, activate_it ? 1 : 0, &child, &exc), ref exc);
            }
            GC.KeepAlive(this);
            return new POA(new PoaHandle(child), orb);
        }

        public void destroy(bool etherealize_objects, bool wait_for_completion)
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_poa_destroy(Raw, etherealize_objects ? 1 : 0, wait_for_completion ? 1 : 0, &exc), ref exc);
            GC.KeepAlive(this);
        }

        public ThreadPolicy create_thread_policy(ThreadPolicyValue value) => new(value);
        public LifespanPolicy create_lifespan_policy(LifespanPolicyValue value) => new(value);
        public IdUniquenessPolicy create_id_uniqueness_policy(IdUniquenessPolicyValue value) => new(value);
        public IdAssignmentPolicy create_id_assignment_policy(IdAssignmentPolicyValue value) => new(value);
        public ImplicitActivationPolicy create_implicit_activation_policy(ImplicitActivationPolicyValue value) => new(value);
        public ServantRetentionPolicy create_servant_retention_policy(ServantRetentionPolicyValue value) => new(value);
        public RequestProcessingPolicy create_request_processing_policy(RequestProcessingPolicyValue value) => new(value);
        /// <summary>A BiDirPolicy::BidirectionalPolicy (rustorb).</summary>
        public BidirectionalPolicy create_bidirectional_policy(ushort value) => new(value);

        private static Servant Need(Servant? s) =>
            s ?? throw new Corba.BAD_PARAM("a null servant", 0, Corba.CompletionStatus.COMPLETED_NO);


        private void Call(Servant servant, Func<nint, nint, int> f)
        {
            nint s = Need(servant).Native();
            try
            {
                RorbException exc;
                int st = f(s, (nint)(&exc));
                Errors.Check(st, ref exc);
            }
            finally
            {
                Ffi.rorb_servant_release(s);
                GC.KeepAlive(this);
            }
        }

        public byte[] activate_object(Servant p_servant)
        {
            RorbBytes id = default;
            Call(p_servant, (s, e) =>
            {
                RorbBytes b;
                int st = Ffi.rorb_poa_activate_object(Raw, s, &b, (RorbException*)e);
                id = b;
                return st;
            });
            return Strings.Bytes(id);
        }

        public void activate_object_with_id(byte[] id, Servant p_servant)
        {
            Call(p_servant, (s, e) =>
            {
                fixed (byte* ip = id)
                {
                    return Ffi.rorb_poa_activate_object_with_id(Raw, ip, (nuint)id.Length, s, (RorbException*)e);
                }
            });
        }

        public void deactivate_object(byte[] oid)
        {
            RorbException exc;
            fixed (byte* ip = oid)
            {
                Errors.Check(Ffi.rorb_poa_deactivate_object(Raw, ip, (nuint)oid.Length, &exc), ref exc);
            }
            GC.KeepAlive(this);
        }

        public byte[] servant_to_id(Servant p_servant)
        {
            RorbBytes id = default;
            Call(p_servant, (s, e) =>
            {
                RorbBytes b;
                int st = Ffi.rorb_poa_servant_to_id(Raw, s, &b, (RorbException*)e);
                id = b;
                return st;
            });
            return Strings.Bytes(id);
        }

        public Corba.Object servant_to_reference(Servant p_servant)
        {
            nint obj = 0;
            Call(p_servant, (s, e) =>
            {
                nint o;
                int st = Ffi.rorb_poa_servant_to_reference(Raw, s, &o, (RorbException*)e);
                obj = o;
                return st;
            });
            return new Corba.Object(new ObjectHandle(obj));
        }

        private static Servant TakeServant(nint self)
        {
            Servant? s = Servant.FromSelf(self);
            Servant.Unref(self);
            return s ?? throw new Corba.OBJ_ADAPTER("the servant was collected", 0, Corba.CompletionStatus.COMPLETED_NO);
        }

        public Servant id_to_servant(byte[] oid)
        {
            void* self;
            RorbException exc;
            fixed (byte* ip = oid)
            {
                Errors.Check(Ffi.rorb_poa_id_to_servant(Raw, ip, (nuint)oid.Length, &self, &exc), ref exc);
            }
            GC.KeepAlive(this);
            return TakeServant((nint)self);
        }

        public Corba.Object id_to_reference(byte[] oid)
        {
            nint obj;
            RorbException exc;
            fixed (byte* ip = oid)
            {
                Errors.Check(Ffi.rorb_poa_id_to_reference(Raw, ip, (nuint)oid.Length, &obj, &exc), ref exc);
            }
            GC.KeepAlive(this);
            return new Corba.Object(new ObjectHandle(obj));
        }

        private static Corba.Object Ref(Corba.IObject? reference) =>
            reference as Corba.Object ?? throw new Corba.BAD_PARAM("not an object reference of rustorb", 0,
                Corba.CompletionStatus.COMPLETED_NO);

        public byte[] reference_to_id(Corba.IObject reference)
        {
            Corba.Object o = Ref(reference);
            RorbBytes id;
            RorbException exc;
            Errors.Check(Ffi.rorb_poa_reference_to_id(Raw, o.Raw, &id, &exc), ref exc);
            GC.KeepAlive(o);
            GC.KeepAlive(this);
            return Strings.Bytes(id);
        }

        public Servant reference_to_servant(Corba.IObject reference)
        {
            Corba.Object o = Ref(reference);
            void* self;
            RorbException exc;
            Errors.Check(Ffi.rorb_poa_reference_to_servant(Raw, o.Raw, &self, &exc), ref exc);
            GC.KeepAlive(o);
            GC.KeepAlive(this);
            return TakeServant((nint)self);
        }

        public Corba.Object create_reference(string intf)
        {
            byte[] i = Strings.ToUtf8(intf);
            nint obj;
            RorbException exc;
            fixed (byte* ip = i)
            {
                Errors.Check(Ffi.rorb_poa_create_reference(Raw, ip, (nuint)i.Length, &obj, &exc), ref exc);
            }
            GC.KeepAlive(this);
            return new Corba.Object(new ObjectHandle(obj));
        }

        public Corba.Object create_reference_with_id(byte[] oid, string intf)
        {
            byte[] i = Strings.ToUtf8(intf);
            nint obj;
            RorbException exc;
            fixed (byte* op = oid)
            fixed (byte* ip = i)
            {
                Errors.Check(Ffi.rorb_poa_create_reference_with_id(Raw, op, (nuint)oid.Length, ip, (nuint)i.Length, &obj,
                    &exc), ref exc);
            }
            GC.KeepAlive(this);
            return new Corba.Object(new ObjectHandle(obj));
        }

        /// <summary>The default servant (USE_DEFAULT_SERVANT).</summary>
        public Servant get_servant()
        {
            void* self;
            RorbException exc;
            Errors.Check(Ffi.rorb_poa_get_servant(Raw, &self, &exc), ref exc);
            GC.KeepAlive(this);
            return TakeServant((nint)self);
        }

        public void set_servant(Servant p_servant)
        {
            Call(p_servant, (s, e) => Ffi.rorb_poa_set_servant(Raw, s, (RorbException*)e));
        }

        /// <summary>The ids of the POA's seven policies (rustorb), in the order
        /// of the policy types.</summary>
        internal RorbPolicy[] policies()
        {
            var ps = new RorbPolicy[7];
            fixed (RorbPolicy* p = ps)
            {
                Ffi.rorb_poa_policies(Raw, p);
            }
            GC.KeepAlive(this);
            return ps;
        }
    }

    /// <summary>PortableServer::POAManager.</summary>
    public sealed unsafe class POAManager : Corba.LocalObject
    {
        internal readonly PoaManagerHandle _handle;

        internal POAManager(PoaManagerHandle handle)
        {
            _handle = handle;
        }

        protected override string[] _ids() => new[] { "IDL:omg.org/PortableServer/POAManager:2.3" };

        public enum State { HOLDING, ACTIVE, DISCARDING, INACTIVE }

        public class AdapterInactive : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableServer/POAManager/AdapterInactive:1.0";
        }

        private nint Raw => _handle.Raw;

        public void activate()
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_poa_manager_activate(Raw, &exc), ref exc);
            GC.KeepAlive(this);
        }

        public void hold_requests(bool wait_for_completion)
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_poa_manager_hold_requests(Raw, wait_for_completion ? 1 : 0, &exc), ref exc);
            GC.KeepAlive(this);
        }

        public void discard_requests(bool wait_for_completion)
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_poa_manager_discard_requests(Raw, wait_for_completion ? 1 : 0, &exc), ref exc);
            GC.KeepAlive(this);
        }

        public void deactivate(bool etherealize_objects, bool wait_for_completion)
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_poa_manager_deactivate(Raw, etherealize_objects ? 1 : 0,
                wait_for_completion ? 1 : 0, &exc), ref exc);
            GC.KeepAlive(this);
        }

        public State get_state()
        {
            var s = (State)Ffi.rorb_poa_manager_get_state(Raw);
            GC.KeepAlive(this);
            return s;
        }

        public string get_id()
        {
            nuint len;
            byte* p = Ffi.rorb_poa_manager_get_id(Raw, &len);
            string s = Strings.FromUtf8(p, len);
            GC.KeepAlive(this);
            return s;
        }

        public override bool Equals(object? obj)
        {
            if (obj is not POAManager m)
            {
                return false;
            }
            bool r = Ffi.rorb_poa_manager_equal(Raw, m.Raw) != 0;
            GC.KeepAlive(this);
            GC.KeepAlive(m);
            return r;
        }

        public override int GetHashCode() => get_id().GetHashCode();
    }

    /// <summary>PortableServer::POAManagerFactory (its exceptions; the factory
    /// itself comes with the additional features of #186).</summary>
    public static class POAManagerFactory
    {
        public class ManagerAlreadyExists : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableServer/POAManagerFactory/ManagerAlreadyExists:1.0";
        }
    }

    /// <summary>PortableServer::Current: the request the calling thread runs.</summary>
    public sealed unsafe class Current : Corba.LocalObject
    {
        private readonly Corba.ORB? orb;

        internal Current(Corba.ORB? orb)
        {
            this.orb = orb;
        }

        protected override string[] _ids() =>
            new[] { "IDL:omg.org/PortableServer/Current:2.3", "IDL:omg.org/CORBA/Current:1.0" };

        public class NoContext : Corba.UserException
        {
            public override string _id() => "IDL:omg.org/PortableServer/Current/NoContext:1.0";
        }

        public POA get_POA()
        {
            nint poa;
            RorbException exc;
            Errors.Check(Ffi.rorb_current_get_poa(&poa, &exc), ref exc);
            return new POA(new PoaHandle(poa), orb);
        }

        public byte[] get_object_id()
        {
            RorbBytes id;
            RorbException exc;
            Errors.Check(Ffi.rorb_current_get_object_id(&id, &exc), ref exc);
            return Strings.Bytes(id);
        }

        public Corba.Object get_reference()
        {
            nint obj;
            RorbException exc;
            Errors.Check(Ffi.rorb_current_get_reference(&obj, &exc), ref exc);
            return new Corba.Object(new ObjectHandle(obj));
        }

        public Servant get_servant()
        {
            void* self;
            RorbException exc;
            Errors.Check(Ffi.rorb_current_get_servant(&self, &exc), ref exc);
            Servant? s = Servant.FromSelf((nint)self);
            Servant.Unref((nint)self);
            return s ?? throw new Corba.OBJ_ADAPTER("the servant was collected", 0, Corba.CompletionStatus.COMPLETED_NO);
        }
    }

    /// <summary>Narrowing of the local objects ORB.resolve_initial_references
    /// returns.</summary>
    public static class POAHelper
    {
        public static POA? narrow(Corba.IObject? obj) => obj switch
        {
            null => null,
            POA p => p,
            _ => throw new Corba.BAD_PARAM("not a POA", 0, Corba.CompletionStatus.COMPLETED_NO),
        };
    }

    public static class CurrentHelper
    {
        public static Current? narrow(Corba.IObject? obj) => obj switch
        {
            null => null,
            Current c => c,
            _ => throw new Corba.BAD_PARAM("not a PortableServer::Current", 0, Corba.CompletionStatus.COMPLETED_NO),
        };
    }
}
