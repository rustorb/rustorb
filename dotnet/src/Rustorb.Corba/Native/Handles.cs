// SafeHandles for the reference counted handles of the C ABI.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System.Runtime.InteropServices;

namespace Rustorb.Native;

internal abstract class Handle : SafeHandle
{
    protected Handle(nint h) : base(0, true)
    {
        SetHandle(h);
    }

    public override bool IsInvalid => handle == 0;

    /// <summary>The raw handle; the caller keeps this object alive (GC.KeepAlive)
    /// while it uses it.</summary>
    internal nint Raw => handle;
}

internal sealed class ObjectHandle : Handle
{
    internal ObjectHandle(nint h) : base(h) { }
    protected override bool ReleaseHandle() { Ffi.rorb_object_release(handle); return true; }
}

internal sealed class OrbHandle : Handle
{
    internal OrbHandle(nint h) : base(h) { }
    protected override bool ReleaseHandle() { Ffi.rorb_orb_release(handle); return true; }
}

internal sealed class TypeCodeHandle : Handle
{
    internal TypeCodeHandle(nint h) : base(h) { }
    protected override bool ReleaseHandle() { Ffi.rorb_tc_release(handle); return true; }
}

internal sealed class AnyHandle : Handle
{
    internal AnyHandle(nint h) : base(h) { }
    protected override bool ReleaseHandle() { Ffi.rorb_any_release(handle); return true; }
}

internal sealed class PoaHandle : Handle
{
    internal PoaHandle(nint h) : base(h) { }
    protected override bool ReleaseHandle() { Ffi.rorb_poa_release(handle); return true; }
}

internal sealed class PoaManagerHandle : Handle
{
    internal PoaManagerHandle(nint h) : base(h) { }
    protected override bool ReleaseHandle() { Ffi.rorb_poa_manager_release(handle); return true; }
}

internal sealed class InHandle : Handle
{
    internal InHandle(nint h) : base(h) { }
    protected override bool ReleaseHandle() { Ffi.rorb_in_release(handle); return true; }
}
