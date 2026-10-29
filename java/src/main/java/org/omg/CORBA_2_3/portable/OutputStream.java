// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA_2_3.portable;

/** An output stream that writes valuetypes. */
public abstract class OutputStream extends org.omg.CORBA.portable.OutputStream {
    public void write_value(java.io.Serializable value) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    public void write_value(java.io.Serializable value, String repository_id) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    public void write_value(java.io.Serializable value, Class<?> clz) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    public void write_value(java.io.Serializable value,
            org.omg.CORBA.portable.BoxedValueHelper factory) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    public void write_abstract_interface(java.lang.Object obj) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }
}
