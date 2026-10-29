// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** Gives a skeleton the stream of the reply. */
public interface ResponseHandler {
    /** The stream of a normal reply. */
    OutputStream createReply();

    /** The stream of a user exception reply; the exception's Helper writes
     * its repository id first. */
    OutputStream createExceptionReply();
}
