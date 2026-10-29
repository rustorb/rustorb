// The Java side of librustorb_jni: the methods it calls back, and the
// exceptions it throws.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.SystemException;
import org.omg.CORBA.UNKNOWN;
import org.omg.CORBA.portable.InvokeHandler;
import org.omg.PortableServer.AdapterActivator;
import org.omg.PortableServer.ForwardRequest;
import org.omg.PortableServer.Servant;
import org.omg.PortableServer.ServantActivator;
import org.omg.PortableServer.ServantLocator;
import org.omg.PortableServer.ServantLocatorPackage.CookieHolder;

final class Bridge {
    private Bridge() {}

    /** Call.exceptionMode: drop the first string written. */
    static final int SKIP_ID = 1;

    // rorb_status
    private static final int OK = 0;
    private static final int SYSTEM_EXCEPTION = 1;
    private static final int USER_EXCEPTION = 2;
    private static final int CALLBACK_FAILED = 4;

    // --- valuetypes --------------------------------------------------------

    /** The core asks for a valuetype it does not know: a new TypeCode
     * handle, or 0. */
    static long resolveValueType(String id) {
        try {
            return Values.resolve(id);
        } catch (RuntimeException e) {
            return 0;
        }
    }

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

    // --- servant managers and adapter activators -------------------------------

    /** A POA handle passed to a callback, which keeps it only during the
     * call. */
    private static PoaImpl borrowedPoa(long poa) {
        return PoaImpl.of(Native.poaClone(poa));
    }

    /** The servant a servant manager returned, as a new rorb_servant handle
     * the core takes over; 0 after reporting an error. */
    private static long provide(Servant servant, long error) {
        if (servant == null) {
            Native.managerErrorSystem(error, "IDL:omg.org/CORBA/OBJ_ADAPTER:1.0", 0,
                    CompletionStatus._COMPLETED_NO);
            return 0;
        }
        return ServantDelegateImpl.rorbServant(servant);
    }

    /** Reports what incarnate or preinvoke threw; returns 0. */
    private static long managerError(Throwable t, long error) {
        if (t instanceof ForwardRequest) {
            org.omg.CORBA.Object target = ((ForwardRequest) t).forward_reference;
            try {
                if (target == null) {
                    throw new org.omg.CORBA.BAD_PARAM("nil forward_reference", 0,
                            CompletionStatus.COMPLETED_NO);
                }
                ObjectDelegateImpl d = ObjectDelegateImpl.of(target);
                try {
                    Native.managerErrorForward(error, d.handle());
                } finally {
                    java.lang.ref.Reference.reachabilityFence(d);
                }
                return 0;
            } catch (Throwable u) {
                t = u;
            }
        }
        if (t instanceof SystemException) {
            SystemException e = (SystemException) t;
            int completed = e.completed == null ? CompletionStatus._COMPLETED_MAYBE
                    : e.completed.value();
            Native.managerErrorSystem(error, SystemExceptions.id(e), e.minor, completed);
        } else {
            Native.managerErrorSystem(error, "IDL:omg.org/CORBA/UNKNOWN:1.0", 0,
                    CompletionStatus._COMPLETED_MAYBE);
        }
        return 0;
    }

    private static void report(String what, Throwable t) {
        System.err.println("rustorb: exception in " + what + ": " + t);
    }

    static long incarnate(Object activator, byte[] oid, long poa, long error) {
        try {
            return provide(((ServantActivator) activator).incarnate(oid, borrowedPoa(poa)), error);
        } catch (Throwable t) {
            if (!(t instanceof ForwardRequest) && !(t instanceof SystemException)) {
                report("incarnate", t);
            }
            return managerError(t, error);
        }
    }

    static void etherealize(Object activator, byte[] oid, long poa, Object servant,
            boolean cleanupInProgress, boolean remainingActivations) {
        try {
            ((ServantActivator) activator).etherealize(oid, borrowedPoa(poa), (Servant) servant,
                    cleanupInProgress, remainingActivations);
        } catch (Throwable t) {
            report("etherealize", t);
        }
    }

    static long preinvoke(Object locator, byte[] oid, long poa, String operation,
            Object[] cookie, long error) {
        try {
            CookieHolder holder = new CookieHolder();
            Servant servant = ((ServantLocator) locator).preinvoke(oid, borrowedPoa(poa),
                    operation, holder);
            long h = provide(servant, error);
            cookie[0] = holder.value;
            return h;
        } catch (Throwable t) {
            if (!(t instanceof ForwardRequest) && !(t instanceof SystemException)) {
                report("preinvoke", t);
            }
            return managerError(t, error);
        }
    }

    static void postinvoke(Object locator, byte[] oid, long poa, String operation,
            Object cookie, Object servant) {
        try {
            ((ServantLocator) locator).postinvoke(oid, borrowedPoa(poa), operation, cookie,
                    (Servant) servant);
        } catch (Throwable t) {
            report("postinvoke", t);
        }
    }

    static boolean unknownAdapter(Object activator, long parent, String name) {
        try {
            return ((AdapterActivator) activator).unknown_adapter(borrowedPoa(parent), name);
        } catch (Throwable t) {
            report("unknown_adapter", t);
            return false;
        }
    }
}
