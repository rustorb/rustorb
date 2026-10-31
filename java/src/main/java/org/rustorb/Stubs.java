// What generated stubs call.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;

import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.portable.InputStream;
import org.omg.CORBA.portable.ObjectImpl;

public final class Stubs {
    private Stubs() {}

    /** Status of Native.objectInvoke. */
    private static final int USER_EXCEPTION = 2;
    private static final int CALLBACK_FAILED = 4;

    /**
     * Invokes op on the object of self: call writes the arguments and reads
     * the reply. A system exception (or an exception a callback threw) is
     * thrown; a user exception is left in call.userException().
     */
    public static void invoke(ObjectImpl self, String op, boolean oneway, Call call) {
        ObjectDelegateImpl d = ObjectDelegateImpl.of(self);
        call.orb = d.orb;
        int status;
        try {
            status = Native.objectInvoke(d.handle(), op, oneway, -1, call);
        } finally {
            Reference.reachabilityFence(d);
        }
        if (status == CALLBACK_FAILED) {
            Bridge.rethrow(call);
            throw new org.omg.CORBA.UNKNOWN("a callback failed", 0, CompletionStatus.COMPLETED_MAYBE);
        }
        if (status != USER_EXCEPTION) {
            call.userException = null;
        }
    }

    /** ORB.create_output_stream: a stream whose create_input_stream reads
     * back what was written (object references belong to orb). */
    public static org.omg.CORBA.portable.OutputStream newOutputStream(org.omg.CORBA.ORB orb) {
        return new PortableOutputStream(orb instanceof OrbImpl ? (OrbImpl) orb : null);
    }

    /** The length of a sequence read from in, checked against the bytes
     * left when in is a stream of rustorb (elements of at least
     * minElementSize bytes). */
    public static int readLength(InputStream in, int minElementSize) {
        if (in instanceof NativeInputStream) {
            return ((NativeInputStream) in).readLength(minElementSize);
        }
        int n = in.read_ulong();
        if (n < 0) {
            throw new org.omg.CORBA.MARSHAL("sequence too long", 0, CompletionStatus.COMPLETED_MAYBE);
        }
        return n;
    }

    /**
     * Writes obj as an abstract interface: true and an object reference
     * (null too), or false and a value.
     */
    static void writeAbstract(org.omg.CORBA_2_3.portable.OutputStream out, java.lang.Object obj) {
        if (obj == null || obj instanceof org.omg.CORBA.Object) {
            out.write_boolean(true);
            out.write_Object((org.omg.CORBA.Object) obj);
        } else if (obj instanceof java.io.Serializable) {
            out.write_boolean(false);
            out.write_value((java.io.Serializable) obj);
        } else {
            throw new BAD_PARAM("neither an object nor a value: " + obj.getClass().getName(), 0,
                    CompletionStatus.COMPLETED_NO);
        }
    }

    /** Reads an abstract interface: an object reference or a value. */
    static java.lang.Object readAbstract(org.omg.CORBA_2_3.portable.InputStream in) {
        return in.read_boolean() ? in.read_Object() : in.read_value();
    }

    /** obj as an instance of clz (a stub class or an interface whose stub
     * is _NameStub in the same package), without asking the object. */
    static org.omg.CORBA.Object narrowTo(org.omg.CORBA.Object obj, Class<?> clz) {
        if (clz.isInstance(obj)) {
            return obj;
        }
        Class<?> stub = clz;
        try {
            if (clz.isInterface()) {
                String pkg = clz.getPackage() == null ? "" : clz.getPackage().getName() + ".";
                stub = Class.forName(pkg + "_" + clz.getSimpleName() + "Stub", true,
                        clz.getClassLoader());
            }
            ObjectImpl s = (ObjectImpl) stub.getDeclaredConstructor().newInstance();
            s._set_delegate(((ObjectImpl) obj)._get_delegate());
            return s;
        } catch (ReflectiveOperationException | ClassCastException e) {
            throw new BAD_PARAM("no stub class for " + clz.getName(), 0,
                    CompletionStatus.COMPLETED_NO);
        }
    }
}
