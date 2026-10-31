// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** A value of any IDL type, with its TypeCode. */
public abstract class Any implements org.omg.CORBA.portable.IDLEntity {
    public abstract boolean equal(Any a);

    public abstract TypeCode type();

    /** Sets the TypeCode; the value is lost unless the type is equivalent. */
    public abstract void type(TypeCode t);

    public abstract void read_value(org.omg.CORBA.portable.InputStream is, TypeCode t)
            throws MARSHAL;

    public abstract void write_value(org.omg.CORBA.portable.OutputStream os);

    public abstract org.omg.CORBA.portable.OutputStream create_output_stream();

    public abstract org.omg.CORBA.portable.InputStream create_input_stream();

    public abstract short extract_short() throws BAD_OPERATION;
    public abstract void insert_short(short s);
    public abstract int extract_long() throws BAD_OPERATION;
    public abstract void insert_long(int l);
    public abstract long extract_longlong() throws BAD_OPERATION;
    public abstract void insert_longlong(long l);
    public abstract short extract_ushort() throws BAD_OPERATION;
    public abstract void insert_ushort(short s);
    public abstract int extract_ulong() throws BAD_OPERATION;
    public abstract void insert_ulong(int l);
    public abstract long extract_ulonglong() throws BAD_OPERATION;
    public abstract void insert_ulonglong(long l);
    public abstract float extract_float() throws BAD_OPERATION;
    public abstract void insert_float(float f);
    public abstract double extract_double() throws BAD_OPERATION;
    public abstract void insert_double(double d);
    public abstract boolean extract_boolean() throws BAD_OPERATION;
    public abstract void insert_boolean(boolean b);
    public abstract char extract_char() throws BAD_OPERATION;
    public abstract void insert_char(char c) throws DATA_CONVERSION;
    public abstract char extract_wchar() throws BAD_OPERATION;
    public abstract void insert_wchar(char c);
    public abstract byte extract_octet() throws BAD_OPERATION;
    public abstract void insert_octet(byte b);
    public abstract Any extract_any() throws BAD_OPERATION;
    public abstract void insert_any(Any a);
    public abstract org.omg.CORBA.Object extract_Object() throws BAD_OPERATION;
    public abstract void insert_Object(org.omg.CORBA.Object o);
    public abstract void insert_Object(org.omg.CORBA.Object o, TypeCode t) throws BAD_PARAM;
    public abstract String extract_string() throws BAD_OPERATION;
    public abstract void insert_string(String s) throws DATA_CONVERSION, MARSHAL;
    public abstract String extract_wstring() throws BAD_OPERATION;
    public abstract void insert_wstring(String s) throws MARSHAL;
    public abstract TypeCode extract_TypeCode() throws BAD_OPERATION;
    public abstract void insert_TypeCode(TypeCode t);

    public org.omg.CORBA.portable.Streamable extract_Streamable() throws BAD_INV_ORDER {
        throw new NO_IMPLEMENT();
    }

    public void insert_Streamable(org.omg.CORBA.portable.Streamable s) {
        throw new NO_IMPLEMENT();
    }

    public java.io.Serializable extract_Value() throws BAD_OPERATION {
        throw new NO_IMPLEMENT();
    }

    public void insert_Value(java.io.Serializable v) {
        throw new NO_IMPLEMENT();
    }

    public void insert_Value(java.io.Serializable v, TypeCode t) throws MARSHAL {
        throw new NO_IMPLEMENT();
    }

    /** A long double as the nearest double (rustorb: the Java mapping has
     * no long double). */
    public double extract_longdouble() {
        throw new NO_IMPLEMENT();
    }

    /** Puts value in as a long double (rustorb). */
    public void insert_longdouble(double value) {
        throw new NO_IMPLEMENT();
    }

    public java.math.BigDecimal extract_fixed() {
        throw new NO_IMPLEMENT();
    }

    public void insert_fixed(java.math.BigDecimal value) {
        throw new NO_IMPLEMENT();
    }

    public void insert_fixed(java.math.BigDecimal value, TypeCode type) throws BAD_INV_ORDER {
        throw new NO_IMPLEMENT();
    }
}
