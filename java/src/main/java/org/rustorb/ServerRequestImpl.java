// The request a DynamicImplementation servant receives
// (org.omg.CORBA.ServerRequest): an rorb_server_request of the C ABI. See
// docs/dii-dsi.md.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import java.lang.ref.Reference;
import java.util.List;

import org.omg.CORBA.ARG_IN;
import org.omg.CORBA.ARG_OUT;
import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.Context;
import org.omg.CORBA.NO_IMPLEMENT;
import org.omg.CORBA.NVList;
import org.omg.CORBA.NamedValue;
import org.omg.CORBA.ServerRequest;

final class ServerRequestImpl extends ServerRequest {
    private final String operation;
    /** The rorb_server_request; 0 once released. */
    private long handle;
    /** The parameters given to arguments, else null. */
    private List<NamedValue> params;
    private boolean raised;

    ServerRequestImpl(long handle, String operation) {
        this.handle = handle;
        this.operation = operation;
    }

    private long h() {
        if (handle == 0) {
            throw new BAD_INV_ORDER("the request is over", 0, CompletionStatus.COMPLETED_NO);
        }
        return handle;
    }

    @Override
    public String operation() {
        return operation;
    }

    @Override
    public synchronized void arguments(NVList args) {
        long h = h();
        if (params != null) {
            throw new BAD_INV_ORDER("arguments was called", 0, CompletionStatus.COMPLETED_NO);
        }
        List<NamedValue> items = Dii.items(args);
        for (NamedValue nv : items) {
            TypeCodeImpl tc = TypeCodeImpl.cast(nv.value().type());
            try {
                Native.serverRequestAddParam(h, nv.name(), nv.flags() & 3, tc.handle());
            } finally {
                Reference.reachabilityFence(tc);
            }
        }
        Native.serverRequestArguments(h);
        for (int i = 0; i < items.size(); i++) {
            NamedValue nv = items.get(i);
            if ((nv.flags() & 3) != ARG_OUT.value) {
                AnyImpl.cast(nv.value()).assign(Native.serverRequestArgument(h, i));
            }
        }
        params = items;
    }

    private static long anyHandle(Any any) {
        if (any == null) {
            throw new BAD_PARAM("null any", 0, CompletionStatus.COMPLETED_NO);
        }
        return AnyImpl.cast(any).handle();
    }

    @Override
    public synchronized void set_result(Any any) {
        try {
            Native.serverRequestSetResult(h(), anyHandle(any));
        } finally {
            Reference.reachabilityFence(any);
        }
    }

    @Override
    public synchronized void set_exception(Any any) {
        try {
            Native.serverRequestSetException(h(), anyHandle(any));
        } finally {
            Reference.reachabilityFence(any);
        }
        raised = true;
    }

    /** rustorb has no contexts. */
    @Override
    public Context ctx() {
        throw new NO_IMPLEMENT("contexts", 0, CompletionStatus.COMPLETED_NO);
    }

    /** Writes the reply (after invoke): the out and inout values of the
     * parameters, then the result or the exception. An out value not set
     * makes the reply BAD_PARAM. */
    synchronized void reply(long reply) {
        long h = h();
        if (params != null && !raised) {
            for (int i = 0; i < params.size(); i++) {
                NamedValue nv = params.get(i);
                if ((nv.flags() & 3) == ARG_IN.value) {
                    continue;
                }
                AnyImpl a = AnyImpl.cast(nv.value());
                try {
                    long v = a.handleIfValue();
                    if (v != 0) {
                        Native.serverRequestSetArgument(h, i, v);
                    }
                } finally {
                    Reference.reachabilityFence(a);
                }
            }
        }
        Native.serverRequestReply(h, reply);
    }

    synchronized void release() {
        if (handle != 0) {
            Native.serverRequestRelease(handle);
            handle = 0;
        }
    }
}
