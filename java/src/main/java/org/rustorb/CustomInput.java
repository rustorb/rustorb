// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.rustorb;

import org.omg.CORBA.Any;
import org.omg.CORBA.TypeCode;

/** The DataInputStream a custom valuetype's unmarshal reads from. */
final class CustomInput implements org.omg.CORBA.DataInputStream {
    private static final long serialVersionUID = 1L;
    private static final String[] IDS = {"IDL:omg.org/CORBA/DataInputStream:1.0"};

    private final transient org.omg.CORBA_2_3.portable.InputStream in;

    CustomInput(org.omg.CORBA_2_3.portable.InputStream in) {
        this.in = in;
    }

    @Override
    public String[] _truncatable_ids() {
        return IDS;
    }

    @Override
    public Any read_any() {
        return in.read_any();
    }

    @Override
    public boolean read_boolean() {
        return in.read_boolean();
    }

    @Override
    public char read_char() {
        return in.read_char();
    }

    @Override
    public char read_wchar() {
        return in.read_wchar();
    }

    @Override
    public byte read_octet() {
        return in.read_octet();
    }

    @Override
    public short read_short() {
        return in.read_short();
    }

    @Override
    public short read_ushort() {
        return in.read_ushort();
    }

    @Override
    public int read_long() {
        return in.read_long();
    }

    @Override
    public int read_ulong() {
        return in.read_ulong();
    }

    @Override
    public long read_longlong() {
        return in.read_longlong();
    }

    @Override
    public long read_ulonglong() {
        return in.read_ulonglong();
    }

    @Override
    public float read_float() {
        return in.read_float();
    }

    @Override
    public double read_double() {
        return in.read_double();
    }

    @Override
    public String read_string() {
        return in.read_string();
    }

    @Override
    public String read_wstring() {
        return in.read_wstring();
    }

    @Override
    public org.omg.CORBA.Object read_Object() {
        return in.read_Object();
    }

    @Override
    public java.lang.Object read_Abstract() {
        return in.read_abstract_interface();
    }

    @Override
    public java.io.Serializable read_Value() {
        return in.read_value();
    }

    @Override
    public TypeCode read_TypeCode() {
        return in.read_TypeCode();
    }

    @Override
    public void read_any_array(org.omg.CORBA.AnySeqHolder seq, int offset, int length) {
        seq.value = grow(seq.value, offset + length, Any[]::new);
        for (int k = 0; k < length; k++) {
            seq.value[offset + k] = in.read_any();
        }
    }

    @Override
    public void read_boolean_array(org.omg.CORBA.BooleanSeqHolder seq, int offset, int length) {
        if (seq.value == null || seq.value.length < offset + length) {
            boolean[] grown = new boolean[offset + length];
            if (seq.value != null) {
                System.arraycopy(seq.value, 0, grown, 0, seq.value.length);
            }
            seq.value = grown;
        }
        in.read_boolean_array(seq.value, offset, length);
    }

    @Override
    public void read_char_array(org.omg.CORBA.CharSeqHolder seq, int offset, int length) {
        if (seq.value == null || seq.value.length < offset + length) {
            char[] grown = new char[offset + length];
            if (seq.value != null) {
                System.arraycopy(seq.value, 0, grown, 0, seq.value.length);
            }
            seq.value = grown;
        }
        in.read_char_array(seq.value, offset, length);
    }

    @Override
    public void read_wchar_array(org.omg.CORBA.WCharSeqHolder seq, int offset, int length) {
        if (seq.value == null || seq.value.length < offset + length) {
            char[] grown = new char[offset + length];
            if (seq.value != null) {
                System.arraycopy(seq.value, 0, grown, 0, seq.value.length);
            }
            seq.value = grown;
        }
        in.read_wchar_array(seq.value, offset, length);
    }

    @Override
    public void read_octet_array(org.omg.CORBA.OctetSeqHolder seq, int offset, int length) {
        if (seq.value == null || seq.value.length < offset + length) {
            byte[] grown = new byte[offset + length];
            if (seq.value != null) {
                System.arraycopy(seq.value, 0, grown, 0, seq.value.length);
            }
            seq.value = grown;
        }
        in.read_octet_array(seq.value, offset, length);
    }

    @Override
    public void read_short_array(org.omg.CORBA.ShortSeqHolder seq, int offset, int length) {
        if (seq.value == null || seq.value.length < offset + length) {
            short[] grown = new short[offset + length];
            if (seq.value != null) {
                System.arraycopy(seq.value, 0, grown, 0, seq.value.length);
            }
            seq.value = grown;
        }
        in.read_short_array(seq.value, offset, length);
    }

    @Override
    public void read_ushort_array(org.omg.CORBA.UShortSeqHolder seq, int offset, int length) {
        if (seq.value == null || seq.value.length < offset + length) {
            short[] grown = new short[offset + length];
            if (seq.value != null) {
                System.arraycopy(seq.value, 0, grown, 0, seq.value.length);
            }
            seq.value = grown;
        }
        in.read_ushort_array(seq.value, offset, length);
    }

    @Override
    public void read_long_array(org.omg.CORBA.LongSeqHolder seq, int offset, int length) {
        if (seq.value == null || seq.value.length < offset + length) {
            int[] grown = new int[offset + length];
            if (seq.value != null) {
                System.arraycopy(seq.value, 0, grown, 0, seq.value.length);
            }
            seq.value = grown;
        }
        in.read_long_array(seq.value, offset, length);
    }

    @Override
    public void read_ulong_array(org.omg.CORBA.ULongSeqHolder seq, int offset, int length) {
        if (seq.value == null || seq.value.length < offset + length) {
            int[] grown = new int[offset + length];
            if (seq.value != null) {
                System.arraycopy(seq.value, 0, grown, 0, seq.value.length);
            }
            seq.value = grown;
        }
        in.read_ulong_array(seq.value, offset, length);
    }

    @Override
    public void read_longlong_array(org.omg.CORBA.LongLongSeqHolder seq, int offset, int length) {
        if (seq.value == null || seq.value.length < offset + length) {
            long[] grown = new long[offset + length];
            if (seq.value != null) {
                System.arraycopy(seq.value, 0, grown, 0, seq.value.length);
            }
            seq.value = grown;
        }
        in.read_longlong_array(seq.value, offset, length);
    }

    @Override
    public void read_ulonglong_array(org.omg.CORBA.ULongLongSeqHolder seq, int offset, int length) {
        if (seq.value == null || seq.value.length < offset + length) {
            long[] grown = new long[offset + length];
            if (seq.value != null) {
                System.arraycopy(seq.value, 0, grown, 0, seq.value.length);
            }
            seq.value = grown;
        }
        in.read_ulonglong_array(seq.value, offset, length);
    }

    @Override
    public void read_float_array(org.omg.CORBA.FloatSeqHolder seq, int offset, int length) {
        if (seq.value == null || seq.value.length < offset + length) {
            float[] grown = new float[offset + length];
            if (seq.value != null) {
                System.arraycopy(seq.value, 0, grown, 0, seq.value.length);
            }
            seq.value = grown;
        }
        in.read_float_array(seq.value, offset, length);
    }

    @Override
    public void read_double_array(org.omg.CORBA.DoubleSeqHolder seq, int offset, int length) {
        if (seq.value == null || seq.value.length < offset + length) {
            double[] grown = new double[offset + length];
            if (seq.value != null) {
                System.arraycopy(seq.value, 0, grown, 0, seq.value.length);
            }
            seq.value = grown;
        }
        in.read_double_array(seq.value, offset, length);
    }

    private static <T> T[] grow(T[] a, int len, java.util.function.IntFunction<T[]> make) {
        if (a != null && a.length >= len) {
            return a;
        }
        T[] grown = make.apply(len);
        if (a != null) {
            System.arraycopy(a, 0, grown, 0, a.length);
        }
        return grown;
    }
}
