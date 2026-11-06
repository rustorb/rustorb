// Callback AMI (docs/ami.md): the sendc_ of generated stubs on
// rorb_object_sendc, and the ExceptionHolders handed to AMI_<I>Handlers.
//
// marshaled_exception holds the exception as in a reply body (the
// repository id, then the members of a user exception or the minor code and
// completion status of a system exception), plain CDR from offset 0 in the
// byte order byte_order says (TRUE: little endian), with the code sets
// UTF-8 and UTF-16: what rorb_cdr_encode writes and rorb_in_from_bytes
// reads, the same in every language of rustorb.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.ByteOrder;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.concurrent.ThreadFactory;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.MARSHAL;
import org.omg.CORBA.SystemException;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.UNKNOWN;
import org.omg.CORBA.UserException;
import org.omg.CORBA.portable.InputStream;
import org.omg.CORBA.portable.ObjectImpl;
import org.omg.CORBA.portable.OutputStream;

final class Ami {
    private Ami() {}

    /** Status of the done callback (rorb_status). */
    private static final int OK = 0;
    private static final int SYSTEM_EXCEPTION = 1;
    private static final int USER_EXCEPTION = 2;

    /** The minor code of UNKNOWN for a user exception that is not in the
     * list of raise_exception_with_list or has no Helper here. */
    static final int UNLISTED = 0x4f4d0001;

    static final boolean LITTLE = ByteOrder.nativeOrder() == ByteOrder.LITTLE_ENDIAN;

    /**
     * One sendc_ request: writes the arguments recorded when sendc_ was
     * called (so that changing them later changes nothing), reads into the
     * generated SendcCall.
     */
    static final class Request extends Call {
        final SendcCall call;
        final boolean hasHandler;
        private final PortableOutputStream args;
        /** The repository id of the user exception read. */
        String exceptionId;

        Request(SendcCall call, boolean hasHandler, PortableOutputStream args) {
            this.call = call;
            this.hasHandler = hasHandler;
            this.args = args;
        }

        @Override
        public void write(OutputStream out) {
            args.writeTo(out);
        }

        @Override
        public void read(InputStream in) {
            call.read(in);
        }

        @Override
        public UserException readException(String id, InputStream in) {
            UserException e = call.readException(id, in);
            if (e != null) {
                exceptionId = id;
            }
            return e;
        }
    }

    /** Stubs.sendc. */
    static void sendc(ObjectImpl self, String op, org.omg.CORBA.Object handler, SendcCall call) {
        ObjectDelegateImpl d = ObjectDelegateImpl.of(self);
        call.orb = d.orb;
        PortableOutputStream args = new PortableOutputStream(d.orb);
        call.write(args);
        Request r = new Request(call, handler != null, args);
        r.orb = d.orb;
        try {
            Native.objectSendc(d.handle(), op, -1, r);
        } finally {
            Reference.reachabilityFence(d);
        }
    }

    /** The number of threads that call handlers. */
    private static final int HANDLER_THREADS = 4;

    /**
     * The handlers run here: daemon threads of the JVM, at most
     * HANDLER_THREADS, ended after a minute unused; replies wait in a queue
     * for one. The done upcall, on a thread of the ORB's reply pool, only
     * hands the reply over. A handler mostly calls on (its reference is a
     * CORBA object, often remote). Made on the reply pool's native threads
     * inside the upcall, or on as many JVM threads as replies came at once,
     * those calls crashed the JVM on Windows now and then; on a few threads
     * they did not.
     */
    private static final ExecutorService HANDLERS = handlers();

    private static ExecutorService handlers() {
        ThreadFactory factory = new ThreadFactory() {
            private final AtomicInteger next = new AtomicInteger();

            @Override
            public Thread newThread(Runnable task) {
                Thread t = new Thread(task, "rustorb-ami-" + next.incrementAndGet());
                t.setDaemon(true);
                return t;
            }
        };
        ThreadPoolExecutor e = new ThreadPoolExecutor(HANDLER_THREADS, HANDLER_THREADS, 60L,
                TimeUnit.SECONDS, new LinkedBlockingQueue<>(), factory);
        e.allowCoreThreadTimeOut(true);
        return e;
    }

    /** The done upcall: the handler is called on a thread of HANDLERS (a
     * nil handler drops the reply at once). */
    static void deliver(Request r, int status, RuntimeException system) {
        if (!r.hasHandler) {
            return;
        }
        HANDLERS.execute(() -> done(r, status, system));
    }

    /** Calls the handler. What the handler throws is dropped. */
    static void done(Request r, int status, RuntimeException system) {
        if (!r.hasHandler) {
            return;
        }
        try {
            switch (status) {
                case OK:
                    r.call.reply();
                    return;
                case USER_EXCEPTION:
                    r.call.excep(userHolder(r.userException, r.exceptionId));
                    return;
                case SYSTEM_EXCEPTION:
                    r.call.excep(systemHolder(system instanceof SystemException
                            ? (SystemException) system
                            : new UNKNOWN(String.valueOf(system), 0, CompletionStatus.COMPLETED_MAYBE)));
                    return;
                default:
                    // A callback failed: its exception, or UNKNOWN.
                    Throwable t = r.error;
                    if (t instanceof SystemException) {
                        r.call.excep(systemHolder((SystemException) t));
                    } else {
                        UNKNOWN u = new UNKNOWN(String.valueOf(t), 0, CompletionStatus.COMPLETED_MAYBE);
                        if (t != null) {
                            u.initCause(t);
                        }
                        r.call.excep(systemHolder(u));
                    }
            }
        } catch (Throwable t) {
            // dropped: the handler's failure is its own
        }
    }

    // --- holders ---------------------------------------------------------------

    /** A holder made in this process: it keeps the exception, which
     * raise_exception throws. */
    static final class LocalHolder extends org.omg.Messaging.ExceptionHolder {
        private static final long serialVersionUID = 1L;
        private final transient Exception exception;
        private final transient String id;

        LocalHolder(boolean system, byte[] data, Exception exception, String id) {
            this.is_system_exception = system;
            this.byte_order = LITTLE;
            this.marshaled_exception = data;
            this.exception = exception;
            this.id = id;
        }

        @Override
        public void raise_exception() throws UserException {
            raise(null);
        }

        @Override
        public void raise_exception_with_list(TypeCode[] exc_list) throws UserException {
            raise(exc_list);
        }

        private void raise(TypeCode[] list) throws UserException {
            if (exception == null) {
                Ami.raise(is_system_exception, byte_order, marshaled_exception, list);
                return;
            }
            if (exception instanceof SystemException) {
                throw (SystemException) exception;
            }
            if (list != null && !listed(list, id)) {
                throw new UNKNOWN("not in the list: " + id, UNLISTED, CompletionStatus.COMPLETED_YES);
            }
            throw (UserException) exception;
        }
    }

    static org.omg.Messaging.ExceptionHolder systemHolder(SystemException e) {
        String id = SystemExceptions.id(e);
        byte[] data = encode(new Call() {
            @Override
            public void write(OutputStream out) {
                out.write_string(id);
                out.write_ulong(e.minor);
                out.write_ulong(e.completed.value());
            }
        });
        return new LocalHolder(true, data, e, id);
    }

    static org.omg.Messaging.ExceptionHolder userHolder(UserException e, String id) {
        byte[] data;
        try {
            Method write = helper(e.getClass()).getMethod("write", OutputStream.class, e.getClass());
            data = encode(new Call() {
                @Override
                public void write(OutputStream out) {
                    try {
                        write.invoke(null, out, e);
                    } catch (InvocationTargetException x) {
                        throw Library.sneaky(x.getCause());
                    } catch (IllegalAccessException x) {
                        throw new MARSHAL(x.toString(), 0, CompletionStatus.COMPLETED_YES);
                    }
                }
            });
        } catch (ReflectiveOperationException | RuntimeException x) {
            // No Helper to write it with: only this process can raise it.
            data = encode(new Call() {
                @Override
                public void write(OutputStream out) {
                    out.write_string(id);
                }
            });
        }
        return new LocalHolder(false, data, e, id);
    }

    private static Class<?> helper(Class<?> c) throws ClassNotFoundException {
        return Class.forName(c.getName() + "Helper", true, c.getClassLoader());
    }

    private static byte[] encode(Call call) {
        OrbImpl orb = OrbImpl.defaultOrb();
        call.orb = orb;
        byte[] data = Native.cdrEncode(call, LITTLE);
        Bridge.rethrow(call);
        return data;
    }

    static boolean listed(TypeCode[] list, String id) {
        for (TypeCode tc : list) {
            try {
                if (tc != null && tc.id().equals(id)) {
                    return true;
                }
            } catch (org.omg.CORBA.TypeCodePackage.BadKind e) {
                // not an exception's TypeCode
            }
        }
        return false;
    }

    /**
     * ExceptionHolder.raise_exception(_with_list) of a holder that has only
     * its state (one from another process): reads marshaled_exception and
     * throws the system exception, or the user exception with the Helper
     * generated for it. A user exception that is not in list (when given)
     * or has no Helper here is UNKNOWN; data that does not read is MARSHAL.
     */
    static void raise(boolean system, boolean little, byte[] data, TypeCode[] list)
            throws UserException {
        if (data == null) {
            throw new MARSHAL("no marshaled_exception", 0, CompletionStatus.COMPLETED_YES);
        }
        OrbImpl orb = OrbImpl.defaultOrb();
        String id;
        int minor = 0;
        int completed = 0;
        OwnedInputStream in = input(orb, data, little);
        try {
            id = in.read_string();
            if (system) {
                minor = in.read_ulong();
                completed = in.read_ulong();
            }
        } catch (SystemException e) {
            throw new MARSHAL("marshaled_exception: " + e, 0, CompletionStatus.COMPLETED_YES);
        } finally {
            in.release();
        }
        if (system) {
            throw SystemExceptions.make(id, minor,
                    CompletionStatus.from_int(Integer.remainderUnsigned(completed, 3)));
        }
        if (list != null && !listed(list, id)) {
            throw new UNKNOWN("not in the list: " + id, UNLISTED, CompletionStatus.COMPLETED_YES);
        }
        Class<?> h = Values.generated(id, "Helper");
        Method read;
        try {
            if (h == null) {
                throw new NoSuchMethodException(id);
            }
            read = h.getMethod("read", InputStream.class);
        } catch (NoSuchMethodException e) {
            throw new UNKNOWN("no Helper for " + id, UNLISTED, CompletionStatus.COMPLETED_YES);
        }
        Object e;
        in = input(orb, data, little);
        try {
            e = read.invoke(null, in);
        } catch (InvocationTargetException x) {
            throw new MARSHAL("marshaled_exception: " + x.getCause(), 0,
                    CompletionStatus.COMPLETED_YES);
        } catch (IllegalAccessException x) {
            throw new UNKNOWN("no Helper for " + id, UNLISTED, CompletionStatus.COMPLETED_YES);
        } finally {
            in.release();
        }
        if (!(e instanceof UserException)) {
            throw new UNKNOWN("not a user exception: " + id, UNLISTED, CompletionStatus.COMPLETED_YES);
        }
        throw (UserException) e;
    }

    private static OwnedInputStream input(OrbImpl orb, byte[] data, boolean little) {
        long h;
        try {
            h = Native.inFromBytes(data, little, orb != null ? orb.handle() : 0);
        } finally {
            Reference.reachabilityFence(orb);
        }
        return new OwnedInputStream(h, orb, null);
    }
}
