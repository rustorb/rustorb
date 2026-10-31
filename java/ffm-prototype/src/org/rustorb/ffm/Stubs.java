// rorb_object_invoke with Java callbacks.
//
// The callbacks run on the calling thread, inside rorb_object_invoke, so
// the context is the depth of the call on a per-thread stack (calls nest
// when a servant calls out).
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb.ffm;

import static java.lang.foreign.ValueLayout.ADDRESS;
import static java.lang.foreign.ValueLayout.JAVA_INT;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.MemoryLayout.PathElement;
import java.lang.foreign.MemorySegment;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.VarHandle;

public final class Stubs {
    private Stubs() {}

    private static final MemorySegment WRITE = Ffi.upcall(MethodHandles.lookup(), "writeArgs",
            FunctionDescriptor.of(JAVA_INT, ADDRESS, ADDRESS));
    private static final MemorySegment READ = Ffi.upcall(MethodHandles.lookup(), "readReply",
            FunctionDescriptor.of(JAVA_INT, ADDRESS, ADDRESS));

    private static final VarHandle OPERATION = field("operation");
    private static final VarHandle OPERATION_LEN = field("operation_len");
    private static final VarHandle ONEWAY = field("oneway");
    private static final VarHandle TIMEOUT = field("timeout_us");
    private static final VarHandle CONTEXT = field("context");

    private static VarHandle field(String name) {
        return Ffi.INVOCATION.varHandle(PathElement.groupElement(name));
    }

    /** The calls in progress on a thread, with their native structs. */
    private static final class Frames {
        Call[] calls = new Call[4];
        MemorySegment[] invocations = new MemorySegment[4];
        final MemorySegment exception = Arena.ofAuto().allocate(Ffi.EXCEPTION);
        int depth;

        MemorySegment push(Call call) {
            if (depth == calls.length) {
                calls = java.util.Arrays.copyOf(calls, depth * 2);
                invocations = java.util.Arrays.copyOf(invocations, depth * 2);
            }
            MemorySegment inv = invocations[depth];
            if (inv == null) {
                inv = Arena.ofAuto().allocate(Ffi.INVOCATION);
                CONTEXT.set(inv, 0L, MemorySegment.ofAddress(depth));
                Ffi.INVOCATION.varHandle(PathElement.groupElement("write_args")).set(inv, 0L, WRITE);
                Ffi.INVOCATION.varHandle(PathElement.groupElement("read_reply")).set(inv, 0L, READ);
                invocations[depth] = inv;
            }
            calls[depth++] = call;
            return inv;
        }
    }

    private static final ThreadLocal<Frames> FRAMES = ThreadLocal.withInitial(Frames::new);

    /** Invokes op on obj (a rorb_object*). Throws Ffi.SystemError. */
    public static void invoke(MemorySegment obj, Operation op, Call call) {
        Frames frames = FRAMES.get();
        MemorySegment inv = frames.push(call);
        OPERATION.set(inv, 0L, op.name);
        OPERATION_LEN.set(inv, 0L, op.length);
        ONEWAY.set(inv, 0L, op.oneway ? 1 : 0);
        TIMEOUT.set(inv, 0L, -1L);
        int status;
        try {
            status = (int) Ffi.objectInvoke.invokeExact(obj, inv, frames.exception);
        } catch (Throwable t) {
            throw Ffi.rethrow(t);
        } finally {
            frames.calls[--frames.depth] = null;
        }
        if (status == Ffi.CALLBACK_FAILED) {
            throw Ffi.rethrow(call.error);
        }
        Ffi.check(status, frames.exception);
    }

    // The upcalls must not throw: an exception ends the JVM.

    private static int writeArgs(MemorySegment context, MemorySegment out) {
        Call call = FRAMES.get().calls[(int) context.address()];
        try {
            call.write(new Out(out));
            return Ffi.OK;
        } catch (Throwable t) {
            call.error = t;
            return Ffi.CALLBACK_FAILED;
        }
    }

    private static int readReply(MemorySegment context, MemorySegment in) {
        Call call = FRAMES.get().calls[(int) context.address()];
        try {
            call.read(new In(in));
            return Ffi.OK;
        } catch (Throwable t) {
            call.error = t;
            return Ffi.CALLBACK_FAILED;
        }
    }
}
