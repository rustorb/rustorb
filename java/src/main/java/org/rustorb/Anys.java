// Putting values in anys and taking them out, for generated Helpers.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_OPERATION;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.portable.InputStream;
import org.omg.CORBA.portable.OutputStream;

public final class Anys {
    private Anys() {}

    /** Writes a value to a stream. */
    public interface Writer {
        void write(OutputStream out);
    }

    /** Reads a value from a stream. */
    public interface Reader<T> {
        T read(InputStream in);
    }

    /** Puts what w writes, a value of type tc, in a. */
    public static void insert(Any a, TypeCode tc, Writer w) {
        AnyImpl.cast(a).insert(tc, w, false);
    }

    /** Like insert, for an exception whose Helper writes its repository id
     * first (the any holds the members only). */
    public static void insertException(Any a, TypeCode tc, Writer w) {
        AnyImpl.cast(a).insert(tc, w, true);
    }

    /** The value r reads from a, which must hold a value of type tc
     * (BAD_OPERATION otherwise). */
    public static <T> T extract(Any a, TypeCode tc, Reader<T> r) {
        return AnyImpl.cast(a).extract(tc, r, false);
    }

    /** Like extract, for an exception whose Helper reads its repository id
     * first. */
    public static <T> T extractException(Any a, TypeCode tc, Reader<T> r) {
        return AnyImpl.cast(a).extract(tc, r, true);
    }

    static BAD_OPERATION badOperation() {
        return new BAD_OPERATION("the any does not hold a value of that type", 0,
                CompletionStatus.COMPLETED_NO);
    }
}
