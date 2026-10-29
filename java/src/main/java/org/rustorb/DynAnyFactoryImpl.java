// DynamicAny::DynAnyFactory (the initial reference DynAnyFactory), on the
// DynAny of the rustorb core. See docs/dynany.md.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.Any;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.UserException;
import org.omg.DynamicAny.DynAny;
import org.omg.DynamicAny.DynAnyFactory;
import org.omg.DynamicAny.DynAnyFactoryPackage.InconsistentTypeCode;
import org.omg.DynamicAny.MustTruncate;

final class DynAnyFactoryImpl extends org.omg.CORBA.LocalObject implements DynAnyFactory {
    static final DynAnyFactoryImpl INSTANCE = new DynAnyFactoryImpl();

    private DynAnyFactoryImpl() {}

    @Override
    public boolean _is_a(String id) {
        return id.equals("IDL:omg.org/DynamicAny/DynAnyFactory:1.0")
                || id.equals("IDL:omg.org/CORBA/LocalObject:1.0")
                || id.equals("IDL:omg.org/CORBA/Object:1.0");
    }

    private static DynAny create(Any value, boolean truncate)
            throws InconsistentTypeCode, MustTruncate {
        AnyImpl a = AnyImpl.cast(value);
        try {
            return DynAnyImpl.of(Native.dynCreate(a.handle(), truncate));
        } catch (InconsistentTypeCode | MustTruncate e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(a);
        }
    }

    @Override
    public DynAny create_dyn_any(Any value) throws InconsistentTypeCode {
        try {
            return create(value, true);
        } catch (MustTruncate e) {
            throw Bridge.unexpected(e);
        }
    }

    @Override
    public DynAny create_dyn_any_from_type_code(TypeCode type) throws InconsistentTypeCode {
        TypeCodeImpl t = TypeCodeImpl.cast(type);
        try {
            return DynAnyImpl.of(Native.dynFromTypeCode(t.handle()));
        } catch (InconsistentTypeCode e) {
            throw e;
        } catch (UserException e) {
            throw Bridge.unexpected(e);
        } finally {
            Reference.reachabilityFence(t);
        }
    }

    @Override
    public DynAny create_dyn_any_without_truncation(Any value)
            throws InconsistentTypeCode, MustTruncate {
        return create(value, false);
    }

    @Override
    public DynAny[] create_multiple_dyn_anys(Any[] values, boolean allow_truncate)
            throws InconsistentTypeCode, MustTruncate {
        DynAny[] d = new DynAny[values.length];
        for (int i = 0; i < values.length; i++) {
            d[i] = create(values[i], allow_truncate);
        }
        return d;
    }

    @Override
    public Any[] create_multiple_anys(DynAny[] values) {
        Any[] a = new Any[values.length];
        for (int i = 0; i < values.length; i++) {
            a[i] = values[i].to_any();
        }
        return a;
    }
}
