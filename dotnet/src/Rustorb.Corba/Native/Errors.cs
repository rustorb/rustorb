// Turns the rorb_status and rorb_exception of a C call into a .NET exception.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Runtime.ExceptionServices;
using System.Text;

namespace Rustorb.Native;

internal static unsafe class Errors
{
    /// <summary>Throws the exception of a failed call (and clears exc).
    /// RORB_USER_EXCEPTION and RORB_CALLBACK_FAILED are handled by the
    /// callers that expect them; here they are INTERNAL.</summary>
    internal static void Check(int status, ref RorbException exc)
    {
        if (status == Status.OK)
        {
            return;
        }
        throw Take(status, ref exc);
    }

    internal static Exception Take(int status, ref RorbException exc)
    {
        try
        {
            switch (status)
            {
                case Status.SYSTEM_EXCEPTION:
                    {
                        string id = exc.repository_id == null
                            ? "IDL:omg.org/CORBA/UNKNOWN:1.0"
                            : Encoding.UTF8.GetString(exc.repository_id, (int)exc.repository_id_len);
                        return SystemExceptions.Make(id, exc.minor, (Corba.CompletionStatus)exc.completed);
                    }
                case Status.LOCAL_EXCEPTION:
                    return LocalException(exc.local, exc.index);
                default:
                    return new Corba.INTERNAL($"unexpected status {status} from librustorb_ffi", 0,
                        Corba.CompletionStatus.COMPLETED_MAYBE);
            }
        }
        finally
        {
            fixed (RorbException* p = &exc)
            {
                Ffi.rorb_exception_clear(p);
            }
        }
    }

    internal static Exception LocalException(uint local, uint index)
    {
        switch (local)
        {
            case Local.ADAPTER_ALREADY_EXISTS: return new PortableServer.POA.AdapterAlreadyExists();
            case Local.ADAPTER_NON_EXISTENT: return new PortableServer.POA.AdapterNonExistent();
            case Local.INVALID_POLICY: return new PortableServer.POA.InvalidPolicy((ushort)index);
            case Local.SERVANT_ALREADY_ACTIVE: return new PortableServer.POA.ServantAlreadyActive();
            case Local.OBJECT_ALREADY_ACTIVE: return new PortableServer.POA.ObjectAlreadyActive();
            case Local.SERVANT_NOT_ACTIVE: return new PortableServer.POA.ServantNotActive();
            case Local.OBJECT_NOT_ACTIVE: return new PortableServer.POA.ObjectNotActive();
            case Local.WRONG_POLICY: return new PortableServer.POA.WrongPolicy();
            case Local.WRONG_ADAPTER: return new PortableServer.POA.WrongAdapter();
            case Local.ADAPTER_INACTIVE: return new PortableServer.POAManager.AdapterInactive();
            case Local.NO_SERVANT: return new PortableServer.POA.NoServant();
            case Local.NO_CONTEXT: return new PortableServer.Current.NoContext();
            case Local.INVALID_NAME: return new Corba.ORB.InvalidName();
            case Local.TC_BAD_KIND: return new Corba.TypeCode.BadKind();
            case Local.TC_BOUNDS: return new Corba.TypeCode.Bounds();
            case Local.POLICY_ERROR: return new Corba.PolicyError((short)index);
            case Local.DYN_INVALID_VALUE: return new DynamicAny.DynAny.InvalidValue();
            case Local.DYN_TYPE_MISMATCH: return new DynamicAny.DynAny.TypeMismatch();
            case Local.DYN_INCONSISTENT_TYPECODE: return new DynamicAny.DynAnyFactory.InconsistentTypeCode();
            case Local.DYN_MUST_TRUNCATE: return new DynamicAny.MustTruncate();
            case Local.MANAGER_ALREADY_EXISTS: return new PortableServer.POAManagerFactory.ManagerAlreadyExists();
            default:
                return new Corba.INTERNAL($"unknown local exception {local} from librustorb_ffi", 0,
                    Corba.CompletionStatus.COMPLETED_MAYBE);
        }
    }

    /// <summary>Rethrows an exception kept by a callback, with its stack.</summary>
    internal static void Rethrow(Exception e)
    {
        ExceptionDispatchInfo.Capture(e).Throw();
    }
}

/// <summary>UTF-8 and UTF-16 strings across the ABI.</summary>
internal static unsafe class Strings
{
    internal static readonly UTF8Encoding Utf8 = new UTF8Encoding(false, true);

    internal static string FromUtf8(byte* p, nuint len)
    {
        if (len == 0)
        {
            return "";
        }
        return Encoding.UTF8.GetString(p, checked((int)len));
    }

    /// <summary>The UTF-8 bytes of s; invalid UTF-16 (a lone surrogate) is
    /// DATA_CONVERSION.</summary>
    internal static byte[] ToUtf8(string s)
    {
        try
        {
            return Utf8.GetBytes(s);
        }
        catch (EncoderFallbackException)
        {
            throw new Corba.DATA_CONVERSION("a string that is not valid UTF-16", 0,
                Corba.CompletionStatus.COMPLETED_NO);
        }
    }

    internal static byte[] Bytes(RorbBytes b)
    {
        var a = new byte[(int)b.len];
        if (b.len > 0)
        {
            new ReadOnlySpan<byte>(b.data, (int)b.len).CopyTo(a);
        }
        Ffi.rorb_bytes_free(b);
        return a;
    }

    internal static string Take(RorbBytes b)
    {
        string s = FromUtf8(b.data, b.len);
        Ffi.rorb_bytes_free(b);
        return s;
    }
}
