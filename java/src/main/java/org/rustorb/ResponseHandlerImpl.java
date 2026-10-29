// The ResponseHandler a skeleton gets: the streams of the reply.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.BAD_INV_ORDER;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.portable.OutputStream;
import org.omg.CORBA.portable.ResponseHandler;

final class ResponseHandlerImpl implements ResponseHandler {
    private long reply;
    private final OrbImpl orb;
    private NativeOutputStream stream;

    ResponseHandlerImpl(long reply, OrbImpl orb) {
        this.reply = reply;
        this.orb = orb;
    }

    private long reply() {
        if (reply == 0) {
            throw new BAD_INV_ORDER("the reply is used outside its request", 0,
                    CompletionStatus.COMPLETED_MAYBE);
        }
        return reply;
    }

    @Override
    public OutputStream createReply() {
        invalidateStream();
        stream = new NativeOutputStream(Native.replyOutput(reply()), orb, false);
        return stream;
    }

    @Override
    public OutputStream createExceptionReply() {
        invalidateStream();
        stream = NativeOutputStream.exceptionReply(reply(), orb);
        return stream;
    }

    /** Whether the skeleton has started the reply. */
    boolean replying() {
        return stream != null;
    }

    private void invalidateStream() {
        if (stream != null) {
            stream.invalidate();
            stream = null;
        }
    }

    void invalidate() {
        invalidateStream();
        reply = 0;
    }
}
