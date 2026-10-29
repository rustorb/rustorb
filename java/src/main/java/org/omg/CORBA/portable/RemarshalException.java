// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA.portable;

/** Tells a portable stub to marshal the request again. */
public final class RemarshalException extends Exception {
    public RemarshalException() {}
}
