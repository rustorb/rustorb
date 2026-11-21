// Finds librustorb_ffi: the path in RUSTORB_FFI_LIBRARY, else the
// runtimes/<rid>/native directory of the package and the directories the
// OS loader searches.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Rustorb.Native;

internal static class Library
{
    /// <summary>The ABI version this binding was written for.</summary>
    internal const uint ABI_VERSION = 3;

#pragma warning disable CA2255 // the resolver must be set before the first P/Invoke
    [ModuleInitializer]
    internal static void Init()
    {
        NativeLibrary.SetDllImportResolver(typeof(Library).Assembly, Resolve);
    }
#pragma warning restore CA2255

    private static nint Resolve(string name, Assembly assembly, DllImportSearchPath? path)
    {
        if (name != Ffi.Lib)
        {
            return 0;
        }
        string? env = Environment.GetEnvironmentVariable("RUSTORB_FFI_LIBRARY");
        if (!string.IsNullOrEmpty(env))
        {
            return NativeLibrary.Load(env);
        }
        if (NativeLibrary.TryLoad(name, assembly, path, out nint handle))
        {
            return handle;
        }
        return 0;
    }

    private static int checkedVersion;

    /// <summary>Fails with INITIALIZE if the library has another ABI.</summary>
    internal static void Check()
    {
        if (checkedVersion != 0)
        {
            return;
        }
        uint v = Ffi.rorb_abi_version();
        if (v != ABI_VERSION)
        {
            throw new Corba.INITIALIZE(
                $"librustorb_ffi has ABI version {v}, this binding needs {ABI_VERSION}", 0,
                Corba.CompletionStatus.COMPLETED_NO);
        }
        checkedVersion = 1;
    }
}
