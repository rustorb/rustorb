// Corba.IObject and Corba.Object (IDL4 to C# A.1.3): object references.
// The mapping gives them no members; rustorb gives them those of
// CORBA::Object (CORBA 3.4 part 1, 8.3) with the IDL naming. The stub of an
// interface Foo is the class Foo : Corba.Object (A.1.5).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using Rustorb.Native;

namespace Corba
{
    /// <summary>CORBA::Object: what every object reference provides. A nil
    /// reference is null.</summary>
    public interface IObject
    {
        bool _is_a(string logical_type_id);
        bool _non_existent();
        bool _is_equivalent(IObject? other_object);
        uint _hash(uint maximum);
        ORB _get_orb();
    }

    /// <summary>An object reference of rustorb, and the base of the stubs.</summary>
    public unsafe class Object : IObject
    {
        internal readonly ObjectHandle _handle;

        internal Object(ObjectHandle handle)
        {
            _handle = handle;
        }

        /// <summary>The same reference (for the constructors of the stubs).</summary>
        public Object(Object other)
        {
            _handle = Rustorb.Seq.NotNull(other)._handle;
        }

        internal nint Raw => _handle.Raw;

        public virtual bool _is_a(string logical_type_id)
        {
            if (logical_type_id == "IDL:omg.org/CORBA/Object:1.0")
            {
                return true;
            }
            byte[] id = Strings.ToUtf8(logical_type_id);
            int r;
            RorbException exc;
            fixed (byte* p = id)
            {
                Errors.Check(Ffi.rorb_object_is_a(Raw, p, (nuint)id.Length, &r, &exc), ref exc);
            }
            GC.KeepAlive(this);
            return r != 0;
        }

        public bool _non_existent()
        {
            int r;
            RorbException exc;
            Errors.Check(Ffi.rorb_object_non_existent(Raw, &r, &exc), ref exc);
            GC.KeepAlive(this);
            return r != 0;
        }

        public bool _is_equivalent(IObject? other_object)
        {
            if (other_object is not Object o)
            {
                return false;
            }
            bool r = Ffi.rorb_object_is_equivalent(Raw, o.Raw) != 0;
            GC.KeepAlive(this);
            GC.KeepAlive(o);
            return r;
        }

        public uint _hash(uint maximum)
        {
            uint h = Ffi.rorb_object_hash(Raw, maximum);
            GC.KeepAlive(this);
            return h;
        }

        /// <summary>Two references are equal when _is_equivalent says so
        /// (the same object key and address, without a call), so structs
        /// with object members compare by the objects they refer to.</summary>
        public override bool Equals(object? obj) => obj is Object o && _is_equivalent(o);

        public override int GetHashCode() => (int)_hash(int.MaxValue);

        public ORB _get_orb()
        {
            var orb = new ORB(new OrbHandle(Ffi.rorb_object_orb(Raw)));
            GC.KeepAlive(this);
            return orb;
        }

        /// <summary>The repository id in the IOR (may be empty).</summary>
        public string _repository_id()
        {
            nuint len;
            byte* p = Ffi.rorb_object_type_id(Raw, &len);
            string s = Strings.FromUtf8(p, len);
            GC.KeepAlive(this);
            return s;
        }

        /// <summary>Object::validate_connection.</summary>
        public bool _validate_connection()
        {
            int r;
            RorbException exc;
            Errors.Check(Ffi.rorb_object_validate_connection(Raw, &r, &exc), ref exc);
            GC.KeepAlive(this);
            return r != 0;
        }

        /// <summary>The reply timeout of calls on this reference, in
        /// microseconds (rustorb): negative unsets it, 0 is no limit.</summary>
        public long _call_timeout_us
        {
            get
            {
                long v = Ffi.rorb_object_call_timeout(Raw);
                GC.KeepAlive(this);
                return v;
            }
            set
            {
                Ffi.rorb_object_set_call_timeout(Raw, value);
                GC.KeepAlive(this);
            }
        }

        public override string ToString()
        {
            try
            {
                return _get_orb().object_to_string(this);
            }
            catch (SystemException)
            {
                return "Object(" + _repository_id() + ")";
            }
        }
    }

    /// <summary>The base of local objects (POA, POAManager, Current...).
    /// They are not references of the ORB: _non_existent is false and they
    /// cannot be sent.</summary>
    public abstract class LocalObject : IObject
    {
        /// <summary>The repository ids of the local interface, most derived first.</summary>
        protected abstract string[] _ids();

        public virtual bool _is_a(string logical_type_id) =>
            logical_type_id == "IDL:omg.org/CORBA/Object:1.0" || Array.IndexOf(_ids(), logical_type_id) >= 0;

        public bool _non_existent() => false;
        public virtual bool _is_equivalent(IObject? other_object) => ReferenceEquals(this, other_object);
        public uint _hash(uint maximum) => maximum == 0 ? 0 : (uint)GetHashCode() % maximum;
        public virtual ORB _get_orb() => throw new NO_IMPLEMENT("_get_orb of a local object", 0, CompletionStatus.COMPLETED_NO);
    }

    /// <summary>CORBA::Policy, as its type and value.</summary>
    public class Policy
    {
        /// <summary>CORBA::PolicyType.</summary>
        public uint policy_type { get; }
        internal readonly ulong raw;

        internal Policy(uint type, ulong raw)
        {
            policy_type = type;
            this.raw = raw;
        }

        public Policy copy() => this;
        public void destroy() { }
    }

    /// <summary>CORBA::PolicyError.</summary>
    public class PolicyError : UserException
    {
        public short reason { get; set; }
        public PolicyError() { }
        public PolicyError(short reason) { this.reason = reason; }
        public override string _id() => "IDL:omg.org/CORBA/PolicyError:1.0";
    }
}

namespace Rustorb
{
    /// <summary>The calls of the generated stubs.</summary>
    public static unsafe class Stubs
    {
        /// <summary>Invokes op on target: write writes the in and inout
        /// arguments, read reads the result and the inout and out ones, and
        /// readUserException reads a user exception the operation declares (its
        /// repository id given), or returns null for one it does not.</summary>
        public static void Invoke(Corba.Object target, string op, bool oneway, Action<CdrOutput>? write,
            Action<CdrInput>? read, Func<string, CdrInput, Exception?>? readUserException)
        {
            byte[] name = Strings.ToUtf8(op);
            var c = new Call { write = write, read = read, readUserException = readUserException };
            Callbacks.With(c, ctx =>
            {
                RorbException exc;
                int st;
                fixed (byte* p = name)
                {
                    var inv = new RorbInvocation
                    {
                        operation = p,
                        operation_len = (nuint)name.Length,
                        oneway = oneway ? 1 : 0,
                        timeout_us = -1,
                        context = ctx,
                        write_args = write == null ? 0 : (nint)Callbacks.Write,
                        read_reply = read == null ? 0 : (nint)Callbacks.Read,
                        read_user_exception = (nint)Callbacks.ReadUserException,
                    };
                    st = Ffi.rorb_object_invoke(target.Raw, &inv, &exc);
                }
                GC.KeepAlive(target);
                switch (st)
                {
                    case Status.OK:
                        return 0;
                    case Status.USER_EXCEPTION:
                        Errors.Rethrow(c.userException!);
                        return 0;
                    case Status.CALLBACK_FAILED:
                        Ffi.rorb_exception_clear(&exc);
                        c.ThrowError();
                        throw new Corba.INTERNAL("a callback failed without an exception", 0,
                            Corba.CompletionStatus.COMPLETED_MAYBE);
                    default:
                        throw Errors.Take(st, ref exc);
                }
            });
        }

        /// <summary>The reference as the stub type T (made by make) if the
        /// object is a T: obj itself when it already is one, else after a
        /// remote _is_a. null for null; BAD_PARAM when it is not a T.</summary>
        public static T? Narrow<T>(Corba.IObject? obj, string id, Func<Corba.Object, T> make) where T : class
        {
            if (obj == null)
            {
                return null;
            }
            if (obj is T t)
            {
                return t;
            }
            if (obj is Corba.Object o && o._is_a(id))
            {
                return make(o);
            }
            throw new Corba.BAD_PARAM($"the object is not a {id}", 0, Corba.CompletionStatus.COMPLETED_NO);
        }

        /// <summary>Like Narrow, without asking the object.</summary>
        public static T? UncheckedNarrow<T>(Corba.IObject? obj, Func<Corba.Object, T> make) where T : class
        {
            if (obj == null)
            {
                return null;
            }
            if (obj is T t)
            {
                return t;
            }
            if (obj is Corba.Object o)
            {
                return make(o);
            }
            throw new Corba.BAD_PARAM("not an object reference of rustorb", 0, Corba.CompletionStatus.COMPLETED_NO);
        }
    }
}
