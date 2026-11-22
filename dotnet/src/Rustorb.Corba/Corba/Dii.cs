// The DII and the DSI: Corba.Request, NVList, NamedValue, ExceptionList,
// Environment, ServerRequest and PortableServer.DynamicImplementation, on
// the C ABI's rorb_request and rorb_server_request (docs/dii-dsi.md).
// Values go in and out as anys; contexts are not supported.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Collections.Generic;
using System.Threading;
using Rustorb.Native;

namespace Corba
{
    /// <summary>CORBA::Bounds.</summary>
    public class Bounds : UserException
    {
        public override string _id() => "IDL:omg.org/CORBA/Bounds:1.0";
    }

    /// <summary>A user exception raised through the DII or the DSI, as an
    /// any (CORBA::UnknownUserException).</summary>
    public class UnknownUserException : UserException
    {
        public Any except { get; set; }
        public UnknownUserException(Any except) : base("a user exception: " + except.type.id()) { this.except = except; }
        public override string _id() => "IDL:omg.org/CORBA/UnknownUserException:1.0";
    }

    /// <summary>CORBA::NamedValue: an argument of a request.</summary>
    public sealed class NamedValue
    {
        public string name { get; }
        public Any value { get; }
        /// <summary>Constants.ARG_IN, ARG_OUT or ARG_INOUT.</summary>
        public uint flags { get; }

        public NamedValue(string name, Any value, uint flags)
        {
            this.name = name ?? "";
            this.value = value ?? throw new BAD_PARAM("a null any", 0, CompletionStatus.COMPLETED_NO);
            this.flags = flags;
        }
    }

    /// <summary>CORBA::NVList. item and remove past the end are Bounds.</summary>
    public sealed class NVList
    {
        private readonly List<NamedValue> items = new();

        public uint count
        {
            get
            {
                lock (items)
                {
                    return (uint)items.Count;
                }
            }
        }

        public NamedValue add(uint flags) => add_value("", new Any(), flags);

        public NamedValue add_item(string item_name, uint flags) => add_value(item_name, new Any(), flags);

        public NamedValue add_value(string item_name, Any val, uint flags)
        {
            var nv = new NamedValue(item_name, val, flags);
            lock (items)
            {
                items.Add(nv);
            }
            return nv;
        }

        public NamedValue item(uint index)
        {
            lock (items)
            {
                return index < items.Count ? items[(int)index] : throw new Bounds();
            }
        }

        public void remove(uint index)
        {
            lock (items)
            {
                if (index >= items.Count)
                {
                    throw new Bounds();
                }
                items.RemoveAt((int)index);
            }
        }

        internal NamedValue[] Snapshot()
        {
            lock (items)
            {
                return items.ToArray();
            }
        }
    }

    /// <summary>CORBA::ExceptionList: the TypeCodes of the user exceptions a
    /// request may raise.</summary>
    public sealed class ExceptionList
    {
        private readonly List<TypeCode> items = new();

        public uint count
        {
            get
            {
                lock (items)
                {
                    return (uint)items.Count;
                }
            }
        }

        public void add(TypeCode exc)
        {
            lock (items)
            {
                items.Add(exc ?? throw new BAD_PARAM("a null TypeCode", 0, CompletionStatus.COMPLETED_NO));
            }
        }

        public TypeCode item(uint index)
        {
            lock (items)
            {
                return index < items.Count ? items[(int)index] : throw new Bounds();
            }
        }

        public void remove(uint index)
        {
            lock (items)
            {
                if (index >= items.Count)
                {
                    throw new Bounds();
                }
                items.RemoveAt((int)index);
            }
        }

        internal TypeCode[] Snapshot()
        {
            lock (items)
            {
                return items.ToArray();
            }
        }
    }

    /// <summary>CORBA::Environment: the exception of the last call of a request.</summary>
    public sealed class Environment
    {
        public System.Exception? exception { get; set; }
        public void clear() => exception = null;
    }

    /// <summary>CORBA::Request (the DII). Every send builds the C ABI's
    /// request from the arguments, return type and exception list as they
    /// are then, so a Request can be sent again. invoke and get_response
    /// throw what the call raised (a system exception, or an
    /// UnknownUserException for a user exception of the list) and also keep
    /// it in env.</summary>
    public sealed unsafe class Request
    {
        private readonly Object target_;
        private readonly string operation_;
        private readonly NVList arguments_;
        private readonly NamedValue result_;
        private readonly ExceptionList exceptions_;
        private readonly Environment env_ = new();
        private readonly object sending = new();
        private nint pending;
        private NamedValue[]? pendingArgs;

        internal Request(Object target, string operation, NVList? arguments, NamedValue? result,
            ExceptionList? exceptions)
        {
            target_ = target;
            operation_ = operation;
            arguments_ = arguments ?? new NVList();
            result_ = result ?? new NamedValue("", TypedAny(Rustorb.TypeCodes.Simple(TCKind.tk_void)), 0);
            exceptions_ = exceptions ?? new ExceptionList();
        }

        private static Any TypedAny(TypeCode tc)
        {
            var a = new Any();
            a.type = tc;
            return a;
        }

        public Object target => target_;
        public string operation => operation_;
        public NVList arguments => arguments_;
        public NamedValue result => result_;
        public Environment env => env_;
        public ExceptionList exceptions => exceptions_;

        public Any add_in_arg() => arguments_.add(Constants.ARG_IN).value;
        public Any add_named_in_arg(string name) => arguments_.add_item(name, Constants.ARG_IN).value;
        public Any add_inout_arg() => arguments_.add(Constants.ARG_INOUT).value;
        public Any add_named_inout_arg(string name) => arguments_.add_item(name, Constants.ARG_INOUT).value;
        /// <summary>An out argument: set the type of the any (any.type = tc).</summary>
        public Any add_out_arg() => arguments_.add(Constants.ARG_OUT).value;
        public Any add_named_out_arg(string name) => arguments_.add_item(name, Constants.ARG_OUT).value;

        public void set_return_type(TypeCode tc) => result_.value.type = tc;

        public Any return_value() => result_.value;

        /// <summary>Contexts are not supported: NO_IMPLEMENT.</summary>
        public object ctx() => throw new NO_IMPLEMENT("contexts are not supported", 0, CompletionStatus.COMPLETED_NO);

        /// <summary>Makes the C ABI's request.</summary>
        private nint Build(out NamedValue[] args)
        {
            byte[] op = Strings.ToUtf8(operation_);
            nint r;
            RorbException exc;
            fixed (byte* p = op)
            {
                Errors.Check(Ffi.rorb_request_new(target_.Raw, p, (nuint)op.Length, &r, &exc), ref exc);
            }
            GC.KeepAlive(target_);
            try
            {
                args = arguments_.Snapshot();
                foreach (NamedValue nv in args)
                {
                    uint flags = nv.flags & 3;
                    byte[] n = Strings.ToUtf8(nv.name);
                    fixed (byte* np = n)
                    {
                        Errors.Check(Ffi.rorb_request_add_arg(r, np, (nuint)n.Length, flags, nv.value._handle.Raw,
                            &exc), ref exc);
                    }
                    GC.KeepAlive(nv.value);
                }
                Ffi.rorb_request_set_return_type(r, result_.value.type._handle.Raw);
                foreach (TypeCode tc in exceptions_.Snapshot())
                {
                    Ffi.rorb_request_add_exception(r, tc._handle.Raw);
                    GC.KeepAlive(tc);
                }
                return r;
            }
            catch
            {
                Ffi.rorb_request_release(r);
                throw;
            }
        }

        /// <summary>Takes the outcome of a send: the result and the out and
        /// inout values into the same anys, or the exception.</summary>
        private void Outcome(nint r, NamedValue[] args, int st, ref RorbException exc)
        {
            switch (st)
            {
                case Status.OK:
                    result_.value._handle = new AnyHandle(Ffi.rorb_request_result(r));
                    for (int i = 0; i < args.Length; i++)
                    {
                        if ((args[i].flags & Constants.ARG_OUT) != 0)
                        {
                            nint a = Ffi.rorb_request_argument(r, (nuint)i);
                            if (a != 0)
                            {
                                args[i].value._handle = new AnyHandle(a);
                            }
                        }
                    }
                    return;
                case Status.USER_EXCEPTION:
                    {
                        var e = new UnknownUserException(new Any(new AnyHandle(Ffi.rorb_request_exception(r))));
                        env_.exception = e;
                        throw e;
                    }
                default:
                    {
                        System.Exception e = Errors.Take(st, ref exc);
                        env_.exception = e;
                        throw e;
                    }
            }
        }

        private void Send(delegate*<nint, RorbException*, int> send, bool reply)
        {
            nint r = Build(out NamedValue[] args);
            try
            {
                env_.clear();
                RorbException exc;
                int st = send(r, &exc);
                if (reply)
                {
                    Outcome(r, args, st, ref exc);
                }
                else if (st != Status.OK)
                {
                    System.Exception e = Errors.Take(st, ref exc);
                    env_.exception = e;
                    throw e;
                }
            }
            finally
            {
                Ffi.rorb_request_release(r);
            }
        }

        public void invoke()
        {
            CheckNotPending();
            Send(&Ffi.rorb_request_invoke, true);
        }

        public void send_oneway()
        {
            CheckNotPending();
            Send(&Ffi.rorb_request_send_oneway, false);
        }

        private void CheckNotPending()
        {
            lock (sending)
            {
                if (pending != 0)
                {
                    throw new BAD_INV_ORDER("the request is waiting for a response", 0, CompletionStatus.COMPLETED_NO);
                }
            }
        }

        public void send_deferred()
        {
            lock (sending)
            {
                if (pending != 0)
                {
                    throw new BAD_INV_ORDER("the request is waiting for a response", 0, CompletionStatus.COMPLETED_NO);
                }
                nint r = Build(out NamedValue[] args);
                env_.clear();
                RorbException exc;
                int st = Ffi.rorb_request_send_deferred(r, &exc);
                if (st != Status.OK)
                {
                    Ffi.rorb_request_release(r);
                    System.Exception e = Errors.Take(st, ref exc);
                    env_.exception = e;
                    throw e;
                }
                pending = r;
                pendingArgs = args;
            }
            ORB.Deferred(this, true);
        }

        public bool poll_response()
        {
            lock (sending)
            {
                if (pending == 0)
                {
                    throw new BAD_INV_ORDER("no response is outstanding", 0, CompletionStatus.COMPLETED_NO);
                }
                int ready;
                RorbException exc;
                Errors.Check(Ffi.rorb_request_poll_response(pending, &ready, &exc), ref exc);
                return ready != 0;
            }
        }

        public void get_response()
        {
            nint r;
            NamedValue[] args;
            lock (sending)
            {
                if (pending == 0)
                {
                    throw new BAD_INV_ORDER("no response is outstanding", 0, CompletionStatus.COMPLETED_NO);
                }
                r = pending;
                args = pendingArgs!;
                pending = 0;
                pendingArgs = null;
            }
            ORB.Deferred(this, false);
            try
            {
                RorbException exc;
                int st = Ffi.rorb_request_get_response(r, &exc);
                Outcome(r, args, st, ref exc);
            }
            finally
            {
                Ffi.rorb_request_release(r);
            }
        }

        internal bool Pending
        {
            get
            {
                lock (sending)
                {
                    return pending != 0;
                }
            }
        }

        ~Request()
        {
            if (pending != 0)
            {
                Ffi.rorb_request_release(pending);
            }
        }
    }

    /// <summary>CORBA::ServerRequest (the DSI): given to
    /// DynamicImplementation.invoke for one request.</summary>
    public sealed unsafe class ServerRequest
    {
        private readonly nint handle;
        private readonly string operation_;
        private NVList? parameters;
        private nint[]? initial;

        internal ServerRequest(nint handle, string operation)
        {
            this.handle = handle;
            operation_ = operation;
        }

        public string operation => operation_;

        /// <summary>Reads the in and inout arguments into the anys of nv,
        /// whose TypeCodes are the parameters' types (once, else
        /// BAD_INV_ORDER). The out and inout values set in these anys are the
        /// reply's when invoke returns.</summary>
        public void arguments(NVList nv)
        {
            if (parameters != null)
            {
                throw new BAD_INV_ORDER("arguments was called already", 0, CompletionStatus.COMPLETED_NO);
            }
            NamedValue[] items = nv.Snapshot();
            RorbException exc;
            foreach (NamedValue p in items)
            {
                byte[] n = Strings.ToUtf8(p.name);
                fixed (byte* np = n)
                {
                    Errors.Check(Ffi.rorb_server_request_add_param(handle, np, (nuint)n.Length, p.flags & 3,
                        p.value.type._handle.Raw, &exc), ref exc);
                }
            }
            Errors.Check(Ffi.rorb_server_request_arguments(handle, &exc), ref exc);
            initial = new nint[items.Length];
            for (int i = 0; i < items.Length; i++)
            {
                if ((items[i].flags & Constants.ARG_IN) != 0)
                {
                    nint a = Ffi.rorb_server_request_argument(handle, (nuint)i);
                    if (a != 0)
                    {
                        items[i].value._handle = new AnyHandle(a);
                    }
                }
                initial[i] = items[i].value._handle.Raw;
            }
            parameters = nv;
        }

        public void set_result(Any val)
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_server_request_set_result(handle, val._handle.Raw, &exc), ref exc);
            GC.KeepAlive(val);
        }

        /// <summary>val: an any of a user exception (else BAD_PARAM).</summary>
        public void set_exception(Any val)
        {
            RorbException exc;
            Errors.Check(Ffi.rorb_server_request_set_exception(handle, val._handle.Raw, &exc), ref exc);
            GC.KeepAlive(val);
        }

        /// <summary>Contexts are not supported: NO_IMPLEMENT.</summary>
        public object ctx() => throw new NO_IMPLEMENT("contexts are not supported", 0, CompletionStatus.COMPLETED_NO);

        /// <summary>The out and inout values, then the reply.</summary>
        internal void Reply(nint reply)
        {
            RorbException exc;
            if (parameters != null)
            {
                NamedValue[] items = parameters.Snapshot();
                for (int i = 0; i < items.Length && i < initial!.Length; i++)
                {
                    bool written = (items[i].flags & Constants.ARG_OUT) != 0;
                    // An out value never set stays without a value: the reply says BAD_PARAM.
                    if (written && (items[i].flags != Constants.ARG_OUT || items[i].value._handle.Raw != initial[i]))
                    {
                        Errors.Check(Ffi.rorb_server_request_set_argument(handle, (nuint)i,
                            items[i].value._handle.Raw, &exc), ref exc);
                        GC.KeepAlive(items[i].value);
                    }
                }
            }
            Errors.Check(Ffi.rorb_server_request_reply(handle, reply, &exc), ref exc);
        }
    }

    public static partial class Constants
    {
        public const uint ARG_IN = 1;
        public const uint ARG_OUT = 2;
        public const uint ARG_INOUT = 3;
        public const uint CTX_RESTRICT_SCOPE = 15;
    }
}

namespace PortableServer
{
    /// <summary>The base of DSI servants: invoke gets every request. The
    /// repository id the POA registers is _all_interfaces(null, null)[0],
    /// _primary_interface(null, null) unless _all_interfaces is overridden.</summary>
    public abstract unsafe class DynamicImplementation : Servant
    {
        public abstract void invoke(Corba.ServerRequest request);

        public abstract string _primary_interface(byte[]? oid, POA? poa);

        public override string[] _all_interfaces(POA? poa, byte[]? objectId) =>
            new[] { _primary_interface(objectId, poa) };

        public sealed override void _dispatch(string operation, Rustorb.CdrInput input, Rustorb.ServerReply reply)
        {
            byte[] op = Strings.ToUtf8(operation);
            nint sr;
            RorbException exc;
            fixed (byte* p = op)
            {
                Errors.Check(Ffi.rorb_server_request_new(input.ptr, p, (nuint)op.Length, &sr, &exc), ref exc);
            }
            try
            {
                var request = new Corba.ServerRequest(sr, operation);
                try
                {
                    invoke(request);
                }
                catch (Corba.UnknownUserException u)
                {
                    request.set_exception(u.except);
                }
                request.Reply(reply.Ptr);
            }
            finally
            {
                Ffi.rorb_server_request_release(sr);
            }
        }
    }
}
