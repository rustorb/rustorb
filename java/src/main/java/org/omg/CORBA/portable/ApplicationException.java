// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** A user exception reply, for portable stubs. */
public class ApplicationException extends Exception {
    private final String id;
    private final InputStream ins;

    public ApplicationException(String id, InputStream ins) {
        super(id);
        this.id = id;
        this.ins = ins;
    }

    public String getId() {
        return id;
    }

    public InputStream getInputStream() {
        return ins;
    }
}
