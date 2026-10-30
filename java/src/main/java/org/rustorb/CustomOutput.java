// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.Any;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.TypeCode;

/** The DataOutputStream a custom valuetype's marshal writes to: the
 * stream of the message, so that the valuetypes written keep their sharing
 * with the rest of it. */
final class CustomOutput implements org.omg.CORBA.DataOutputStream {
    private static final long serialVersionUID = 1L;
    private static final String[] IDS = {"IDL:omg.org/CORBA/DataOutputStream:1.0"};

    private final transient org.omg.CORBA_2_3.portable.OutputStream out;

    CustomOutput(org.omg.CORBA_2_3.portable.OutputStream out) {
        this.out = out;
    }

    @Override
    public String[] _truncatable_ids() {
        return IDS;
    }

    private static void check(int have, int offset, int length) {
        if (offset < 0 || length < 0 || offset > have - length) {
            throw new BAD_PARAM("array bounds", 0, CompletionStatus.COMPLETED_NO);
        }
    }

    @Override
    public void write_any(Any value) {
        out.write_any(value);
    }

    @Override
    public void write_boolean(boolean value) {
        out.write_boolean(value);
    }

    @Override
    public void write_char(char value) {
        out.write_char(value);
    }

    @Override
    public void write_wchar(char value) {
        out.write_wchar(value);
    }

    @Override
    public void write_octet(byte value) {
        out.write_octet(value);
    }

    @Override
    public void write_short(short value) {
        out.write_short(value);
    }

    @Override
    public void write_ushort(short value) {
        out.write_ushort(value);
    }

    @Override
    public void write_long(int value) {
        out.write_long(value);
    }

    @Override
    public void write_ulong(int value) {
        out.write_ulong(value);
    }

    @Override
    public void write_longlong(long value) {
        out.write_longlong(value);
    }

    @Override
    public void write_ulonglong(long value) {
        out.write_ulonglong(value);
    }

    @Override
    public void write_float(float value) {
        out.write_float(value);
    }

    @Override
    public void write_double(double value) {
        out.write_double(value);
    }

    @Override
    public void write_string(String value) {
        out.write_string(value);
    }

    @Override
    public void write_wstring(String value) {
        out.write_wstring(value);
    }

    @Override
    public void write_Object(org.omg.CORBA.Object value) {
        out.write_Object(value);
    }

    @Override
    public void write_Abstract(java.lang.Object value) {
        out.write_abstract_interface(value);
    }

    @Override
    public void write_Value(java.io.Serializable value) {
        out.write_value(value);
    }

    @Override
    public void write_TypeCode(TypeCode value) {
        out.write_TypeCode(value);
    }

    @Override
    public void write_any_array(Any[] seq, int offset, int length) {
        check(seq.length, offset, length);
        for (int i = 0; i < length; i++) {
            out.write_any(seq[offset + i]);
        }
    }

    @Override
    public void write_boolean_array(boolean[] seq, int offset, int length) {
        check(seq.length, offset, length);
        out.write_boolean_array(seq, offset, length);
    }

    @Override
    public void write_char_array(char[] seq, int offset, int length) {
        check(seq.length, offset, length);
        out.write_char_array(seq, offset, length);
    }

    @Override
    public void write_wchar_array(char[] seq, int offset, int length) {
        check(seq.length, offset, length);
        out.write_wchar_array(seq, offset, length);
    }

    @Override
    public void write_octet_array(byte[] seq, int offset, int length) {
        check(seq.length, offset, length);
        out.write_octet_array(seq, offset, length);
    }

    @Override
    public void write_short_array(short[] seq, int offset, int length) {
        check(seq.length, offset, length);
        out.write_short_array(seq, offset, length);
    }

    @Override
    public void write_ushort_array(short[] seq, int offset, int length) {
        check(seq.length, offset, length);
        out.write_ushort_array(seq, offset, length);
    }

    @Override
    public void write_long_array(int[] seq, int offset, int length) {
        check(seq.length, offset, length);
        out.write_long_array(seq, offset, length);
    }

    @Override
    public void write_ulong_array(int[] seq, int offset, int length) {
        check(seq.length, offset, length);
        out.write_ulong_array(seq, offset, length);
    }

    @Override
    public void write_longlong_array(long[] seq, int offset, int length) {
        check(seq.length, offset, length);
        out.write_longlong_array(seq, offset, length);
    }

    @Override
    public void write_ulonglong_array(long[] seq, int offset, int length) {
        check(seq.length, offset, length);
        out.write_ulonglong_array(seq, offset, length);
    }

    @Override
    public void write_float_array(float[] seq, int offset, int length) {
        check(seq.length, offset, length);
        out.write_float_array(seq, offset, length);
    }

    @Override
    public void write_double_array(double[] seq, int offset, int length) {
        check(seq.length, offset, length);
        out.write_double_array(seq, offset, length);
    }
}
