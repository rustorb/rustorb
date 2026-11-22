// The structs and constants of the C ABI (rustorb.h) that the binding fills
// in or reads.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System.Runtime.InteropServices;

namespace Rustorb.Native;

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct RorbException
{
    public byte* repository_id;
    public nuint repository_id_len;
    public uint minor;
    public uint completed;
    public uint local;
    public uint index;
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct RorbBytes
{
    public byte* data;
    public nuint len;
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct RorbInvocation
{
    public byte* operation;
    public nuint operation_len;
    public int oneway;
    public long timeout_us;
    public nint context;
    public nint write_args;
    public nint read_reply;
    public nint read_user_exception;
}

[StructLayout(LayoutKind.Sequential)]
internal struct RorbPolicy
{
    public uint type;
    public uint value;
}

[StructLayout(LayoutKind.Sequential)]
internal struct RorbServantVtable
{
    public nint dispatch;
    public nint is_a;
    public nint add_ref;
    public nint remove_ref;
}

internal static class Status
{
    public const int OK = 0;
    public const int SYSTEM_EXCEPTION = 1;
    public const int USER_EXCEPTION = 2;
    public const int LOCAL_EXCEPTION = 3;
    public const int CALLBACK_FAILED = 4;
}

/// <summary>The local exception codes of rorb_exception.local.</summary>
internal static class Local
{
    public const uint ADAPTER_ALREADY_EXISTS = 1;
    public const uint ADAPTER_NON_EXISTENT = 2;
    public const uint INVALID_POLICY = 3;
    public const uint SERVANT_ALREADY_ACTIVE = 4;
    public const uint OBJECT_ALREADY_ACTIVE = 5;
    public const uint SERVANT_NOT_ACTIVE = 6;
    public const uint OBJECT_NOT_ACTIVE = 7;
    public const uint WRONG_POLICY = 8;
    public const uint WRONG_ADAPTER = 9;
    public const uint ADAPTER_INACTIVE = 10;
    public const uint NO_SERVANT = 11;
    public const uint NO_CONTEXT = 12;
    public const uint INVALID_NAME = 13;
    public const uint TC_BAD_KIND = 14;
    public const uint TC_BOUNDS = 15;
    public const uint POLICY_ERROR = 16;
    public const uint DYN_INVALID_VALUE = 17;
    public const uint DYN_TYPE_MISMATCH = 18;
    public const uint DYN_INCONSISTENT_TYPECODE = 19;
    public const uint DYN_MUST_TRUNCATE = 20;
    public const uint INVALID_SLOT = 21;
    public const uint DUPLICATE_NAME = 22;
    public const uint UNKNOWN_ENCODING = 23;
    public const uint CODEC_INVALID_TYPE_FOR_ENCODING = 24;
    public const uint CODEC_FORMAT_MISMATCH = 25;
    public const uint CODEC_TYPE_MISMATCH = 26;
    public const uint MANAGER_ALREADY_EXISTS = 27;
}

/// <summary>Element kinds of rorb_out_write_array / rorb_in_read_array.</summary>
internal static class Prim
{
    public const int OCTET = 0;
    public const int SHORT = 1;
    public const int USHORT = 2;
    public const int LONG = 3;
    public const int ULONG = 4;
    public const int LONGLONG = 5;
    public const int ULONGLONG = 6;
    public const int FLOAT = 7;
    public const int DOUBLE = 8;
}
