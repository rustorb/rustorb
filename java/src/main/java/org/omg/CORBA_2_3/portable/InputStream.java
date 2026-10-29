// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA_2_3.portable;

/** An input stream that reads valuetypes. */
public abstract class InputStream extends org.omg.CORBA.portable.InputStream {
    /** A value of any type (its repository id is sent). */
    public java.io.Serializable read_value() {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    /** A value whose declared type has the repository id rep_id. */
    public java.io.Serializable read_value(String rep_id) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    /** A value whose declared type is the class clz. */
    public java.io.Serializable read_value(Class<?> clz) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    /** A value box, read by factory. */
    public java.io.Serializable read_value(org.omg.CORBA.portable.BoxedValueHelper factory) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    /** Reads the state of value, made by a value factory, and returns it. */
    public java.io.Serializable read_value(java.io.Serializable value) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    public java.lang.Object read_abstract_interface() {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }

    public java.lang.Object read_abstract_interface(Class<?> clz) {
        throw new org.omg.CORBA.NO_IMPLEMENT();
    }
}
