// A DII request (org.omg.CORBA.Request) on an object of rustorb: each send
// makes an rorb_request of the C ABI from the arguments, return type and
// exception list as they are then. See docs/dii-dsi.md.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;
import java.util.List;

import org.omg.CORBA.ARG_IN;
import org.omg.CORBA.ARG_INOUT;
import org.omg.CORBA.ARG_OUT;
import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.Context;
import org.omg.CORBA.ContextList;
import org.omg.CORBA.Environment;
import org.omg.CORBA.ExceptionList;
import org.omg.CORBA.NO_IMPLEMENT;
import org.omg.CORBA.NVList;
import org.omg.CORBA.NamedValue;
import org.omg.CORBA.Request;
import org.omg.CORBA.SystemException;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.UnknownUserException;
import org.omg.CORBA.WrongTransaction;

final class RequestImpl extends Request {
    private final org.omg.CORBA.Object target;
    private final ObjectDelegateImpl delegate;
    private final String operation;
    private final NVList arguments;
    private final NamedValue result;
    private final Environment env = new Dii.EnvironmentImpl();
    private final ExceptionList exceptions;
    private final ContextList contexts;

    /** The rorb_request sent deferred whose response is not taken yet
     * (0 for none), the arguments it was sent with, and whether a thread
     * is waiting for it in get_response. */
    private long deferred;
    private List<NamedValue> deferredItems;
    private boolean waiting;

    RequestImpl(org.omg.CORBA.Object target, ObjectDelegateImpl delegate, String operation,
            NVList arguments, NamedValue result, ExceptionList exceptions, ContextList contexts) {
        if (operation == null) {
            throw new org.omg.CORBA.BAD_PARAM("null operation", 0,
                    CompletionStatus.COMPLETED_NO);
        }
        this.target = target;
        this.delegate = delegate;
        this.operation = operation;
        this.arguments = arguments != null ? arguments : Dii.createList();
        Dii.items(this.arguments);
        if (result == null) {
            Any v = new AnyImpl();
            v.type(TypeCodes.primitive(TCKind.tk_void));
            result = Dii.createNamedValue("", v, 0);
        }
        this.result = result;
        this.exceptions = exceptions != null ? exceptions : Dii.createExceptionList();
        Dii.items(this.exceptions);
        this.contexts = contexts != null ? contexts : Dii.createContextList();
    }

    @Override
    public org.omg.CORBA.Object target() {
        return target;
    }

    @Override
    public String operation() {
        return operation;
    }

    @Override
    public NVList arguments() {
        return arguments;
    }

    @Override
    public NamedValue result() {
        return result;
    }

    @Override
    public Environment env() {
        return env;
    }

    @Override
    public ExceptionList exceptions() {
        return exceptions;
    }

    @Override
    public ContextList contexts() {
        return contexts;
    }

    /** rustorb has no contexts. */
    @Override
    public Context ctx() {
        throw new NO_IMPLEMENT("contexts", 0, CompletionStatus.COMPLETED_NO);
    }

    @Override
    public void ctx(Context c) {
        throw new NO_IMPLEMENT("contexts", 0, CompletionStatus.COMPLETED_NO);
    }

    @Override
    public Any add_in_arg() {
        return arguments.add(ARG_IN.value).value();
    }

    @Override
    public Any add_named_in_arg(String name) {
        return arguments.add_item(name, ARG_IN.value).value();
    }

    @Override
    public Any add_inout_arg() {
        return arguments.add(ARG_INOUT.value).value();
    }

    @Override
    public Any add_named_inout_arg(String name) {
        return arguments.add_item(name, ARG_INOUT.value).value();
    }

    @Override
    public Any add_out_arg() {
        return arguments.add(ARG_OUT.value).value();
    }

    @Override
    public Any add_named_out_arg(String name) {
        return arguments.add_item(name, ARG_OUT.value).value();
    }

    @Override
    public void set_return_type(TypeCode tc) {
        result.value().type(tc);
    }

    @Override
    public Any return_value() {
        return result.value();
    }

    // --- sending -----------------------------------------------------------------

    /** The mode of the flags (ARG_IN, ARG_OUT, ARG_INOUT; else what the C
     * ABI rejects). */
    private static int mode(NamedValue nv) {
        return nv.flags() & 3;
    }

    /** A new rorb_request of the current arguments, return type and
     * exceptions. */
    private long build(List<NamedValue> items) {
        long h;
        try {
            h = Native.requestNew(delegate.handle(), operation);
        } finally {
            Reference.reachabilityFence(delegate);
        }
        try {
            for (NamedValue nv : items) {
                AnyImpl a = AnyImpl.cast(nv.value());
                int mode = mode(nv);
                if (mode == ARG_OUT.value) {
                    // Only the type of an out argument is sent.
                    TypeCodeImpl tc = TypeCodeImpl.cast(a.type());
                    long typed;
                    try {
                        typed = Native.anyTyped(tc.handle());
                    } finally {
                        Reference.reachabilityFence(tc);
                    }
                    try {
                        Native.requestAddArg(h, nv.name(), mode, typed);
                    } finally {
                        Native.anyRelease(typed);
                    }
                } else {
                    try {
                        Native.requestAddArg(h, nv.name(), mode, a.handle());
                    } finally {
                        Reference.reachabilityFence(a);
                    }
                }
            }
            TypeCodeImpl rt = TypeCodeImpl.cast(result.value().type());
            if (rt.kind().value() == TCKind._tk_null) {
                rt = (TypeCodeImpl) TypeCodes.primitive(TCKind.tk_void);
            }
            try {
                Native.requestSetReturnType(h, rt.handle());
            } finally {
                Reference.reachabilityFence(rt);
            }
            for (TypeCode t : Dii.items(exceptions)) {
                TypeCodeImpl tc = TypeCodeImpl.cast(t);
                try {
                    Native.requestAddException(h, tc.handle());
                } finally {
                    Reference.reachabilityFence(tc);
                }
            }
            return h;
        } catch (RuntimeException | Error e) {
            Native.requestRelease(h);
            throw e;
        }
    }

    /** Takes the outcome of the call of h into the anys and env(). */
    private void outcome(long h, boolean userException, List<NamedValue> items) {
        if (userException) {
            env.exception(new UnknownUserException(AnyImpl.of(Native.requestException(h))));
            return;
        }
        AnyImpl.cast(result.value()).assign(Native.requestResult(h));
        for (int i = 0; i < items.size(); i++) {
            NamedValue nv = items.get(i);
            int mode = mode(nv);
            if (mode == ARG_OUT.value || mode == ARG_INOUT.value) {
                AnyImpl.cast(nv.value()).assign(Native.requestArgument(h, i));
            }
        }
    }

    /** A system exception of the call goes to env() (as JacORB does, and
     * the javadoc says); anything else is thrown. */
    private void failed(RuntimeException e) {
        if (!(e instanceof SystemException)) {
            throw e;
        }
        env.exception(e);
    }

    @Override
    public void invoke() {
        env.clear();
        try {
            List<NamedValue> items = Dii.items(arguments);
            long h = build(items);
            try {
                outcome(h, Native.requestInvoke(h), items);
            } finally {
                Native.requestRelease(h);
            }
        } catch (RuntimeException e) {
            failed(e);
        }
    }

    @Override
    public void send_oneway() {
        env.clear();
        try {
            long h = build(Dii.items(arguments));
            try {
                Native.requestSendOneway(h);
            } finally {
                Native.requestRelease(h);
            }
        } catch (RuntimeException e) {
            failed(e);
        }
    }

    private static BAD_INV_ORDER badInvOrder(String what) {
        return new BAD_INV_ORDER(what, 0, CompletionStatus.COMPLETED_NO);
    }

    @Override
    public void send_deferred() {
        synchronized (this) {
            if (deferred != 0) {
                throw badInvOrder("a response is outstanding");
            }
        }
        env.clear();
        try {
            List<NamedValue> items = Dii.items(arguments);
            long h = build(items);
            try {
                Native.requestSendDeferred(h);
            } catch (RuntimeException e) {
                Native.requestRelease(h);
                throw e;
            }
            synchronized (this) {
                deferred = h;
                deferredItems = items;
            }
            Dii.deferred(delegate.orb, this);
        } catch (RuntimeException e) {
            failed(e);
        }
    }

    /** Whether the deferred response has arrived (false while a thread
     * waits for it). */
    synchronized boolean ready() {
        return deferred != 0 && !waiting && Native.requestPollResponse(deferred);
    }

    @Override
    public synchronized boolean poll_response() {
        if (deferred == 0) {
            throw badInvOrder("nothing was sent deferred");
        }
        return ready();
    }

    @Override
    public void get_response() throws WrongTransaction {
        long h;
        List<NamedValue> items;
        synchronized (this) {
            if (deferred == 0 || waiting) {
                throw badInvOrder(deferred == 0 ? "nothing was sent deferred"
                        : "another thread waits for the response");
            }
            waiting = true;
            h = deferred;
            items = deferredItems;
        }
        complete(h, items);
    }

    /** get_response for ORB.get_next_response; false if another thread
     * takes it. */
    boolean take() {
        long h;
        List<NamedValue> items;
        synchronized (this) {
            if (deferred == 0 || waiting) {
                return false;
            }
            waiting = true;
            h = deferred;
            items = deferredItems;
        }
        complete(h, items);
        return true;
    }

    /** Waits for the response of h (claimed by this thread) and takes it. */
    private void complete(long h, List<NamedValue> items) {
        try {
            outcome(h, Native.requestGetResponse(h), items);
        } catch (RuntimeException e) {
            failed(e);
        } finally {
            synchronized (this) {
                deferred = 0;
                deferredItems = null;
                waiting = false;
            }
            Native.requestRelease(h);
            Dii.taken(delegate.orb, this);
        }
    }
}
