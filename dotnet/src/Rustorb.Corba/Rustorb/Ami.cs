// Callback AMI (docs/ami.md) in the .NET binding: the sendc_ operations of
// stubs generated with -Wbami on rorb_object_sendc, Messaging.ExceptionHolder
// and the registry of user exceptions it raises.
//
// marshaled_exception holds the exception as in a reply body (the
// repository id, then the members of a user exception or the minor code and
// completion status of a system exception), plain CDR from offset 0 in the
// byte order byte_order says (TRUE: little endian), with the code sets
// UTF-8 and UTF-16: what rorb_cdr_encode writes and rorb_in_from_bytes
// reads, the same in every language of rustorb.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

#nullable enable

using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Rustorb.Native;

namespace Messaging
{
    /// <summary>Messaging::ExceptionHolder: what the &lt;op&gt;_excep operations
    /// of an AMI_&lt;I&gt;Handler receive. Its state is the private members of
    /// the specification; raise_exception throws the exception it holds.</summary>
    public class ExceptionHolder : Corba.ValueBase
    {
        private static readonly string[] ids = { ExceptionHolderHelper.id() };

        protected bool is_system_exception { get; set; }
        protected bool byte_order { get; set; }
        protected Omg.Types.ISequence<byte> marshaled_exception { get; set; } = new Omg.Types.Sequence<byte>();

        // The exception itself, for a holder made in this process.
        private readonly Exception? exception;

        public ExceptionHolder()
        {
        }

        internal ExceptionHolder(bool system, byte[] data, Exception? exception)
        {
            is_system_exception = system;
            byte_order = BitConverter.IsLittleEndian;
            marshaled_exception = new Omg.Types.Sequence<byte>(data);
            this.exception = exception;
        }

        /// <summary>Throws the exception: a system exception, or the user
        /// exception, read with the code generated for it (a user exception
        /// unknown here is UNKNOWN).</summary>
        public void raise_exception() => Raise(null);

        /// <summary>Like raise_exception, but a user exception whose TypeCode
        /// is not in exc_list is UNKNOWN.</summary>
        public void raise_exception_with_list(IList<Corba.TypeCode> exc_list)
        {
            if (exc_list == null)
            {
                throw new Corba.BAD_PARAM("a null exc_list", 0, Corba.CompletionStatus.COMPLETED_NO);
            }
            Raise(exc_list);
        }

        private void Raise(IList<Corba.TypeCode>? list)
        {
            if (exception is Corba.SystemException s)
            {
                Errors.Rethrow(s);
            }
            if (exception is Corba.UserException u)
            {
                if (list != null && !Rustorb.Ami.Listed(list, u._id()))
                {
                    throw Rustorb.Ami.Unlisted(u._id());
                }
                Errors.Rethrow(u);
            }
            Rustorb.Ami.Raise(is_system_exception, byte_order, Rustorb.Seq.Span(marshaled_exception).ToArray(), list);
        }

        public override string[] _truncatable_ids() => ids;

        public override void _write(Rustorb.CdrOutput __out)
        {
            __out.write_boolean(is_system_exception);
            __out.write_boolean(byte_order);
            var m = marshaled_exception ?? new Omg.Types.Sequence<byte>();
            __out.write_ulong((uint)m.Count);
            __out.write_octet_array(Rustorb.Seq.Span(m));
        }

        public override void _read(Rustorb.CdrInput __in)
        {
            is_system_exception = __in.read_boolean();
            byte_order = __in.read_boolean();
            int n = __in.read_length(1);
            var m = Rustorb.Seq.Sized<byte>(n);
            __in.read_octet_array(Rustorb.Seq.Writable(m));
            marshaled_exception = m;
        }
    }

    /// <summary>The TypeCode, any and stream operations of ExceptionHolder.</summary>
    public static class ExceptionHolderHelper
    {
        private const string __id = "IDL:omg.org/Messaging/ExceptionHolder:1.0";
        private static Corba.TypeCode? __tc;

        static ExceptionHolderHelper()
        {
            Rustorb.Values.Default(__id, () => new ExceptionHolder());
            Rustorb.Values.Type(__id, type);
        }

        public static string id() => __id;

        public static Corba.TypeCode type()
        {
            lock (typeof(Corba.TypeCode))
            {
                return __tc ??= Rustorb.TypeCodes.Value(Corba.TCKind.tk_value, __id, "ExceptionHolder", 0, null,
                    new[] { "is_system_exception", "byte_order", "marshaled_exception" },
                    new[]
                    {
                        Rustorb.TypeCodes.Simple(Corba.TCKind.tk_boolean),
                        Rustorb.TypeCodes.Simple(Corba.TCKind.tk_boolean),
                        Rustorb.TypeCodes.Sequence(Rustorb.TypeCodes.Simple(Corba.TCKind.tk_octet), 0),
                    },
                    new short[] { 0, 0, 0 });
            }
        }

        public static void insert(Corba.Any a, ExceptionHolder? v) => Rustorb.Anys.Insert(a, type(), o => write(o, v));

        public static ExceptionHolder? extract(Corba.Any a) => Rustorb.Anys.Extract(a, type(), read);

        public static ExceptionHolder? read(Rustorb.CdrInput __in)
        {
            var r = __in.read_value(__id);
            if (r != null && r is not ExceptionHolder)
            {
                __in.fail();
                return null;
            }
            return (ExceptionHolder?)r;
        }

        public static void write(Rustorb.CdrOutput __out, ExceptionHolder? v) => __out.write_value(v);
    }
}

namespace Rustorb
{
    /// <summary>The user exceptions of the generated code, by repository id
    /// (their module initialisers register them), for the ExceptionHolders
    /// of callback AMI.</summary>
    public static class UserExceptions
    {
        private sealed class Entry
        {
            internal Func<CdrInput, Corba.UserException> read = null!;
            internal Action<CdrOutput, Corba.UserException> write = null!;
        }

        private static readonly ConcurrentDictionary<string, Entry> byId = new();

        public static void Register<T>(string id, Func<CdrInput, T> read, Action<CdrOutput, T> write)
            where T : Corba.UserException =>
            byId[id] = new Entry { read = i => read(i), write = (o, e) => write(o, (T)e) };

        internal static Func<CdrInput, Corba.UserException>? Reader(string id) =>
            byId.TryGetValue(id, out var e) ? e.read : null;

        internal static Action<CdrOutput, Corba.UserException>? Writer(string id) =>
            byId.TryGetValue(id, out var e) ? e.write : null;
    }

    /// <summary>The sendc_ operations of the generated stubs (rustorb-idl
    /// -Wbami).</summary>
    public static unsafe class Ami
    {
        /// <summary>The minor code of UNKNOWN for a user exception that is not
        /// in the list of raise_exception_with_list or is not known here.</summary>
        internal const uint UNLISTED = 0x4f4d0001;

        private sealed class Request : Call
        {
            internal bool hasHandler;
            internal Action reply = null!;
            internal Action<Messaging.ExceptionHolder> excep = null!;
            internal GCHandle self;
        }

        private static readonly delegate* unmanaged[Cdecl]<nint, int, RorbException*, void> Done = &DoneCb;

        /// <summary>Sends the operation op without waiting (callback AMI). write
        /// writes the arguments (the stub copies them first, as it may be
        /// called more than once, later, on another thread); read reads the
        /// reply, then reply calls the handler, or excep with the holder of
        /// the exception; with a null handler the reply is dropped. Throws
        /// only what is known before sending (BAD_INV_ORDER once the ORB is
        /// shut down).</summary>
        public static void Sendc(Corba.Object target, string op, Corba.IObject? handler, Action<CdrOutput>? write,
            Action<CdrInput>? read, Func<string, CdrInput, Exception?>? readUserException, Action reply,
            Action<Messaging.ExceptionHolder> excep)
        {
            var r = new Request
            {
                write = write,
                read = read,
                readUserException = readUserException,
                hasHandler = handler != null,
                reply = reply,
                excep = excep,
            };
            r.self = GCHandle.Alloc(r);
            byte[] name = Strings.ToUtf8(op);
            RorbException exc;
            int st;
            fixed (byte* p = name)
            {
                var inv = new RorbInvocation
                {
                    operation = p,
                    operation_len = (nuint)name.Length,
                    oneway = 0,
                    timeout_us = -1,
                    context = GCHandle.ToIntPtr(r.self),
                    write_args = write == null ? 0 : (nint)Callbacks.Write,
                    read_reply = read == null ? 0 : (nint)Callbacks.Read,
                    read_user_exception = (nint)Callbacks.ReadUserException,
                };
                st = Ffi.rorb_object_sendc(target.Raw, &inv, (nint)Done, &exc);
            }
            GC.KeepAlive(target);
            if (st != Status.OK)
            {
                r.self.Free();
                throw Errors.Take(st, ref exc);
            }
        }

        [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
        private static void DoneCb(nint context, int status, RorbException* exc)
        {
            GCHandle h = GCHandle.FromIntPtr(context);
            var r = (Request)h.Target!;
            h.Free();
            if (!r.hasHandler)
            {
                return;
            }
            try
            {
                switch (status)
                {
                    case Status.OK:
                        r.reply();
                        break;
                    case Status.USER_EXCEPTION:
                        r.excep(Holder(r.userException!));
                        break;
                    case Status.SYSTEM_EXCEPTION:
                    {
                        string id = exc == null || exc->repository_id == null
                            ? "IDL:omg.org/CORBA/UNKNOWN:1.0"
                            : Strings.FromUtf8(exc->repository_id, exc->repository_id_len);
                        r.excep(Holder(SystemExceptions.Make(id, exc == null ? 0 : exc->minor,
                            exc == null ? Corba.CompletionStatus.COMPLETED_MAYBE : (Corba.CompletionStatus)exc->completed)));
                        break;
                    }
                    default:
                        // A callback failed: its exception, or UNKNOWN.
                        r.excep(Holder(r.error as Corba.SystemException
                            ?? new Corba.UNKNOWN(r.error?.ToString() ?? "a callback failed", 0,
                                Corba.CompletionStatus.COMPLETED_MAYBE)));
                        break;
                }
            }
            catch (Exception)
            {
                // Dropped: the handler's failure is its own.
            }
        }

        /// <summary>The holder of an exception raised in this process.</summary>
        internal static Messaging.ExceptionHolder Holder(Exception e)
        {
            if (e is Corba.SystemException s)
            {
                string id = s._id();
                byte[] data = Encode(o =>
                {
                    o.write_string(id);
                    o.write_ulong(s.minor);
                    o.write_ulong((uint)s.completed);
                });
                return new Messaging.ExceptionHolder(true, data, s);
            }
            var u = (Corba.UserException)e;
            string uid = u._id();
            Action<CdrOutput, Corba.UserException>? w = UserExceptions.Writer(uid);
            // Without the code of the exception only this process can raise it.
            byte[] udata = Encode(o =>
            {
                o.write_string(uid);
                w?.Invoke(o, u);
            });
            return new Messaging.ExceptionHolder(false, udata, u);
        }

        private static byte[] Encode(Action<CdrOutput> write)
        {
            var c = new Call { write = write };
            return Callbacks.With(c, ctx =>
            {
                RorbBytes b;
                RorbException exc;
                int st = Ffi.rorb_cdr_encode((nint)Callbacks.Write, ctx, BitConverter.IsLittleEndian ? 1 : 0, &b, &exc);
                if (st == Status.CALLBACK_FAILED)
                {
                    Ffi.rorb_exception_clear(&exc);
                    c.ThrowError();
                }
                Errors.Check(st, ref exc);
                return Strings.Bytes(b);
            });
        }

        internal static bool Listed(IList<Corba.TypeCode> list, string id)
        {
            foreach (Corba.TypeCode tc in list)
            {
                try
                {
                    if (tc != null && tc.id() == id)
                    {
                        return true;
                    }
                }
                catch (Corba.TypeCode.BadKind)
                {
                    // not the TypeCode of an exception
                }
            }
            return false;
        }

        internal static Corba.UNKNOWN Unlisted(string id) =>
            new Corba.UNKNOWN("not in the list or not known here: " + id, UNLISTED, Corba.CompletionStatus.COMPLETED_YES);

        /// <summary>raise_exception of a holder that has only its state (one
        /// from another process).</summary>
        internal static void Raise(bool system, bool little, byte[] data, IList<Corba.TypeCode>? list)
        {
            string id = "";
            uint minor = 0, completed = 0;
            Exception? user = null;
            bool known = true;
            Read(data, little, i =>
            {
                id = i.read_string();
                if (system)
                {
                    minor = i.read_ulong();
                    completed = i.read_ulong();
                    return;
                }
                if (list != null && !Listed(list, id))
                {
                    known = false;
                    return;
                }
                var read = UserExceptions.Reader(id);
                if (read == null)
                {
                    known = false;
                    return;
                }
                user = read(i);
            });
            if (system)
            {
                throw SystemExceptions.Make(id, minor, (Corba.CompletionStatus)(completed % 3));
            }
            if (!known || user == null)
            {
                throw Unlisted(id);
            }
            Errors.Rethrow(user);
        }

        private static void Read(byte[] data, bool little, Action<CdrInput> read)
        {
            nint input;
            RorbException exc;
            fixed (byte* p = data)
            {
                Errors.Check(Ffi.rorb_in_from_bytes(p, (nuint)data.Length, little ? 1 : 0, 0, &input, &exc), ref exc);
            }
            try
            {
                var i = new CdrInput(input);
                read(i);
                if (i.failed)
                {
                    throw new Corba.MARSHAL("marshaled_exception does not read", 0, Corba.CompletionStatus.COMPLETED_YES);
                }
            }
            catch (Corba.SystemException e) when (e is not Corba.MARSHAL)
            {
                throw new Corba.MARSHAL("marshaled_exception: " + e.Message, 0, Corba.CompletionStatus.COMPLETED_YES);
            }
            finally
            {
                Ffi.rorb_in_release(input);
            }
        }
    }
}
