// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** Implemented by skeletons: dispatches one request. */
public interface InvokeHandler {
    OutputStream _invoke(String method, InputStream input, ResponseHandler handler)
            throws org.omg.CORBA.SystemException;
}
