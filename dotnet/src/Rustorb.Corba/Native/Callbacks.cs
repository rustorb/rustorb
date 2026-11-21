// The C ABI's write and read callbacks: static [UnmanagedCallersOnly]
// trampolines whose context is a GCHandle of a Call. A .NET exception never
// unwinds into Rust: it is kept on the Call and the callback reports
// RORB_CALLBACK_FAILED; the caller rethrows it.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Rustorb.Native;

/// <summary>The state of one call into the C ABI with callbacks.</summary>
internal class Call
{
    internal Action<Rustorb.CdrOutput>? write;
    internal Action<Rustorb.CdrInput>? read;
    /// <summary>Reads a user exception of the given id; null when the
    /// operation does not declare it.</summary>
    internal Func<string, Rustorb.CdrInput, Exception?>? readUserException;
    internal Exception? error;
    internal Exception? userException;

    internal void ThrowError()
    {
        if (error != null)
        {
            Errors.Rethrow(error);
        }
    }
}

internal static unsafe class Callbacks
{
    internal static readonly delegate* unmanaged[Cdecl]<nint, nint, int> Write = &WriteCb;
    internal static readonly delegate* unmanaged[Cdecl]<nint, nint, int> Read = &ReadCb;
    internal static readonly delegate* unmanaged[Cdecl]<nint, byte*, nuint, nint, int> ReadUserException = &ReadUserExceptionCb;

    internal static Call Of(nint context) => (Call)GCHandle.FromIntPtr(context).Target!;

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    private static int WriteCb(nint context, nint output)
    {
        Call c = Of(context);
        try
        {
            c.write?.Invoke(new Rustorb.CdrOutput(output));
            return Status.OK;
        }
        catch (Exception e)
        {
            c.error = e;
            return Status.CALLBACK_FAILED;
        }
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    private static int ReadCb(nint context, nint input)
    {
        Call c = Of(context);
        try
        {
            c.read?.Invoke(new Rustorb.CdrInput(input));
            return Status.OK;
        }
        catch (Exception e)
        {
            c.error = e;
            return Status.CALLBACK_FAILED;
        }
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    private static int ReadUserExceptionCb(nint context, byte* id, nuint len, nint input)
    {
        Call c = Of(context);
        try
        {
            if (c.readUserException == null)
            {
                return Status.SYSTEM_EXCEPTION;
            }
            Exception? e = c.readUserException(Strings.FromUtf8(id, len), new Rustorb.CdrInput(input));
            if (e == null)
            {
                return Status.SYSTEM_EXCEPTION;
            }
            c.userException = e;
            return Status.USER_EXCEPTION;
        }
        catch (Exception e)
        {
            c.error = e;
            return Status.CALLBACK_FAILED;
        }
    }

    /// <summary>Runs f with a GCHandle of c as the context.</summary>
    internal static T With<T>(Call c, Func<nint, T> f)
    {
        GCHandle h = GCHandle.Alloc(c);
        try
        {
            return f(GCHandle.ToIntPtr(h));
        }
        finally
        {
            h.Free();
        }
    }
}
