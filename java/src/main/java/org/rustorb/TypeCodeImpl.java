// A TypeCode of the rustorb core (an rorb_typecode handle).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.TypeCodePackage.BadKind;
import org.omg.CORBA.TypeCodePackage.Bounds;

final class TypeCodeImpl extends TypeCode {
    private final Owned handle;

    private TypeCodeImpl(long handle) {
        this.handle = Owned.register(this, handle, Native::tcRelease);
    }

    /** Takes a handle; null for 0. */
    static TypeCodeImpl of(long handle) {
        return handle == 0 ? null : new TypeCodeImpl(handle);
    }

    long handle() {
        return handle.get();
    }

    static TypeCodeImpl cast(TypeCode tc) {
        if (tc instanceof TypeCodeImpl) {
            return (TypeCodeImpl) tc;
        }
        throw new BAD_PARAM(tc == null ? "null TypeCode" : "not a rustorb TypeCode", 0,
                org.omg.CORBA.CompletionStatus.COMPLETED_NO);
    }

    /** The kind with aliases removed. */
    int unaliasedKind() {
        long u = Native.tcUnaliased(handle());
        try {
            return Native.tcKind(u);
        } finally {
            Native.tcRelease(u);
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public boolean equal(TypeCode tc) {
        TypeCodeImpl other = cast(tc);
        try {
            return Native.tcEqual(handle(), other.handle());
        } finally {
            Reference.reachabilityFence(this);
            Reference.reachabilityFence(other);
        }
    }

    @Override
    public boolean equivalent(TypeCode tc) {
        TypeCodeImpl other = cast(tc);
        try {
            return Native.tcEquivalent(handle(), other.handle());
        } finally {
            Reference.reachabilityFence(this);
            Reference.reachabilityFence(other);
        }
    }

    @Override
    public TypeCode get_compact_typecode() {
        try {
            return of(Native.tcCompact(handle()));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public TCKind kind() {
        try {
            return TCKind.from_int(Native.tcKind(handle()));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public String id() throws BadKind {
        try {
            return Native.tcId(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public String name() throws BadKind {
        try {
            return Native.tcName(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public int member_count() throws BadKind {
        try {
            return Native.tcMemberCount(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public String member_name(int index) throws BadKind, Bounds {
        try {
            return Native.tcMemberName(handle(), index);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public TypeCode member_type(int index) throws BadKind, Bounds {
        try {
            return of(Native.tcMemberType(handle(), index));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public Any member_label(int index) throws BadKind, Bounds {
        try {
            return AnyImpl.of(Native.tcMemberLabel(handle(), index));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public TypeCode discriminator_type() throws BadKind {
        try {
            return of(Native.tcDiscriminatorType(handle()));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public int default_index() throws BadKind {
        try {
            return Native.tcDefaultIndex(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public int length() throws BadKind {
        try {
            return Native.tcLength(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public TypeCode content_type() throws BadKind {
        try {
            return of(Native.tcContentType(handle()));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public short fixed_digits() throws BadKind {
        try {
            return Native.tcFixedDigits(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public short fixed_scale() throws BadKind {
        try {
            return Native.tcFixedScale(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public short member_visibility(int index) throws BadKind, Bounds {
        try {
            return Native.tcMemberVisibility(handle(), index);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public short type_modifier() throws BadKind {
        try {
            return Native.tcTypeModifier(handle());
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public TypeCode concrete_base_type() throws BadKind {
        try {
            return of(Native.tcConcreteBaseType(handle()));
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    @Override
    public boolean equals(Object o) {
        return o instanceof TypeCodeImpl && equal((TypeCode) o);
    }

    @Override
    public int hashCode() {
        return Native.tcKind(handle());
    }

    @Override
    public String toString() {
        String s = "TypeCode " + kind().value();
        try {
            s += " " + id();
        } catch (BadKind e) {
            // no id
        }
        return s;
    }
}
