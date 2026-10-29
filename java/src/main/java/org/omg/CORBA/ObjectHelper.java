// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The Helper of CORBA::Object. */
public abstract class ObjectHelper {
    private static final String _id = "IDL:omg.org/CORBA/Object:1.0";

    public static void insert(Any a, Object that) {
        a.insert_Object(that);
    }

    public static Object extract(Any a) {
        return a.extract_Object();
    }

    public static TypeCode type() {
        return ORB.init().create_interface_tc(_id, "Object");
    }

    public static String id() {
        return _id;
    }

    public static Object read(org.omg.CORBA.portable.InputStream in) {
        return in.read_Object();
    }

    public static void write(org.omg.CORBA.portable.OutputStream out, Object that) {
        out.write_Object(that);
    }

    public static Object narrow(Object obj) {
        return obj;
    }
}
