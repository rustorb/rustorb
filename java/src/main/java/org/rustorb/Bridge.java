// The Java side of librustorb_jni: the methods it calls back, and the
// exceptions it throws.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.SystemException;
import org.omg.CORBA.UNKNOWN;
import org.omg.CORBA.portable.InvokeHandler;
import org.omg.PortableServer.Servant;

final class Bridge {
    private Bridge() {}

    /** Call.exceptionMode: drop the first string written. */
    static final int SKIP_ID = 1;

    // rorb_status
    private static final int OK = 0;
    private static final int SYSTEM_EXCEPTION = 1;
    private static final int USER_EXCEPTION = 2;
    private static final int CALLBACK_FAILED = 4;

    // --- exceptions --------------------------------------------------------

    static RuntimeException systemException(String id, int minor, int completed) {
        return SystemExceptions.make(id, minor, CompletionStatus.from_int(completed));
    }

    /** The exception of a local interface (RORB_LOCAL_EXCEPTION). */
    static Exception localException(int code, int index) {
        switch (code) {
            case 1:
                return new org.omg.PortableServer.POAPackage.AdapterAlreadyExists();
            case 2:
                return new org.omg.PortableServer.POAPackage.AdapterNonExistent();
            case 3:
                return new org.omg.PortableServer.POAPackage.InvalidPolicy((short) index);
            case 4:
                return new org.omg.PortableServer.POAPackage.ServantAlreadyActive();
            case 5:
                return new org.omg.PortableServer.POAPackage.ObjectAlreadyActive();
            case 6:
                return new org.omg.PortableServer.POAPackage.ServantNotActive();
            case 7:
                return new org.omg.PortableServer.POAPackage.ObjectNotActive();
            case 8:
                return new org.omg.PortableServer.POAPackage.WrongPolicy();
            case 9:
                return new org.omg.PortableServer.POAPackage.WrongAdapter();
            case 10:
                return new org.omg.PortableServer.POAManagerPackage.AdapterInactive();
            case 11:
                return new org.omg.PortableServer.POAPackage.NoServant();
            case 12:
                return new org.omg.PortableServer.CurrentPackage.NoContext();
            case 13:
                return new org.omg.CORBA.ORBPackage.InvalidName();
            case 14:
                return new org.omg.CORBA.TypeCodePackage.BadKind();
            case 15:
                return new org.omg.CORBA.TypeCodePackage.Bounds();
            default:
                return new org.omg.CORBA.INTERNAL("local exception " + code, 0,
                        CompletionStatus.COMPLETED_MAYBE);
        }
    }

    /** A user exception a native method threw that the operation does not
     * declare. */
    static org.omg.CORBA.INTERNAL unexpected(org.omg.CORBA.UserException e) {
        org.omg.CORBA.INTERNAL i = new org.omg.CORBA.INTERNAL("unexpected " + e, 0,
                CompletionStatus.COMPLETED_MAYBE);
        i.initCause(e);
        return i;
    }

    /** Throws the Java exception a callback of call threw, if any. */
    static void rethrow(Call call) {
        Throwable t = call.error;
        if (t == null) {
            return;
        }
        call.error = null;
        if (t instanceof RuntimeException) {
            throw (RuntimeException) t;
        }
        if (t instanceof Error) {
            throw (Error) t;
        }
        UNKNOWN u = new UNKNOWN(t.toString(), 0, CompletionStatus.COMPLETED_MAYBE);
        u.initCause(t);
        throw u;
    }

    // --- invocation callbacks -----------------------------------------------

    static boolean callWrite(Call call, long out) {
        NativeOutputStream s = new NativeOutputStream(out, call.orb, call.exceptionMode == SKIP_ID);
        try {
            call.write(s);
            return true;
        } catch (Throwable t) {
            if (call.error == null) {
                call.error = t;
            }
            return false;
        } finally {
            s.invalidate();
        }
    }

    static boolean callRead(Call call, long in) {
        NativeInputStream s = new NativeInputStream(in, call.orb, call.injectId);
        try {
            call.read(s);
            return true;
        } catch (Throwable t) {
            if (call.error == null) {
                call.error = t;
            }
            return false;
        } finally {
            s.invalidate();
        }
    }

    static int callReadException(Call call, String id, long in) {
        NativeInputStream s = new NativeInputStream(in, call.orb, id);
        try {
            org.omg.CORBA.UserException e = call.readException(id, s);
            if (e == null) {
                return SYSTEM_EXCEPTION;
            }
            call.userException = e;
            return USER_EXCEPTION;
        } catch (Throwable t) {
            if (call.error == null) {
                call.error = t;
            }
            return CALLBACK_FAILED;
        } finally {
            s.invalidate();
        }
    }

    // --- servants -------------------------------------------------------------

    static int dispatch(Object servant, String op, long in, long reply) {
        OrbImpl orb = OrbImpl.defaultOrb();
        NativeInputStream input = new NativeInputStream(in, orb, null);
        ResponseHandlerImpl handler = new ResponseHandlerImpl(reply, orb);
        try {
            if (!(servant instanceof InvokeHandler)) {
                throw new org.omg.CORBA.BAD_OPERATION(op, 0, CompletionStatus.COMPLETED_NO);
            }
            ((InvokeHandler) servant)._invoke(op, input, handler);
            return OK;
        } catch (SystemException e) {
            // Raised while writing the reply (a bound, say): the operation
            // has completed.
            int completed = handler.replying() ? CompletionStatus._COMPLETED_YES
                    : e.completed == null ? CompletionStatus._COMPLETED_MAYBE : e.completed.value();
            Native.replySystemException(reply, SystemExceptions.id(e), e.minor, completed);
            return SYSTEM_EXCEPTION;
        } catch (Throwable t) {
            Native.replySystemException(reply, "IDL:omg.org/CORBA/UNKNOWN:1.0", 0,
                    CompletionStatus._COMPLETED_MAYBE);
            return SYSTEM_EXCEPTION;
        } finally {
            input.invalidate();
            handler.invalidate();
        }
    }

    static boolean isA(Object servant, String id) {
        if ("IDL:omg.org/CORBA/Object:1.0".equals(id)) {
            return true;
        }
        try {
            for (String s : ((Servant) servant)._all_interfaces(null, null)) {
                if (s.equals(id)) {
                    return true;
                }
            }
        } catch (RuntimeException e) {
            return false;
        }
        return false;
    }
}
