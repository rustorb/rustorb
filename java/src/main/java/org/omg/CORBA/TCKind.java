// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** The kind of a TypeCode. */
public class TCKind {
    public static final int _tk_null = 0;
    public static final int _tk_void = 1;
    public static final int _tk_short = 2;
    public static final int _tk_long = 3;
    public static final int _tk_ushort = 4;
    public static final int _tk_ulong = 5;
    public static final int _tk_float = 6;
    public static final int _tk_double = 7;
    public static final int _tk_boolean = 8;
    public static final int _tk_char = 9;
    public static final int _tk_octet = 10;
    public static final int _tk_any = 11;
    public static final int _tk_TypeCode = 12;
    public static final int _tk_Principal = 13;
    public static final int _tk_objref = 14;
    public static final int _tk_struct = 15;
    public static final int _tk_union = 16;
    public static final int _tk_enum = 17;
    public static final int _tk_string = 18;
    public static final int _tk_sequence = 19;
    public static final int _tk_array = 20;
    public static final int _tk_alias = 21;
    public static final int _tk_except = 22;
    public static final int _tk_longlong = 23;
    public static final int _tk_ulonglong = 24;
    public static final int _tk_longdouble = 25;
    public static final int _tk_wchar = 26;
    public static final int _tk_wstring = 27;
    public static final int _tk_fixed = 28;
    public static final int _tk_value = 29;
    public static final int _tk_value_box = 30;
    public static final int _tk_native = 31;
    public static final int _tk_abstract_interface = 32;
    public static final int _tk_local_interface = 33;
    public static final int _tk_component = 34;
    public static final int _tk_home = 35;
    public static final int _tk_event = 36;

    public static final TCKind tk_null = new TCKind(_tk_null);
    public static final TCKind tk_void = new TCKind(_tk_void);
    public static final TCKind tk_short = new TCKind(_tk_short);
    public static final TCKind tk_long = new TCKind(_tk_long);
    public static final TCKind tk_ushort = new TCKind(_tk_ushort);
    public static final TCKind tk_ulong = new TCKind(_tk_ulong);
    public static final TCKind tk_float = new TCKind(_tk_float);
    public static final TCKind tk_double = new TCKind(_tk_double);
    public static final TCKind tk_boolean = new TCKind(_tk_boolean);
    public static final TCKind tk_char = new TCKind(_tk_char);
    public static final TCKind tk_octet = new TCKind(_tk_octet);
    public static final TCKind tk_any = new TCKind(_tk_any);
    public static final TCKind tk_TypeCode = new TCKind(_tk_TypeCode);
    public static final TCKind tk_Principal = new TCKind(_tk_Principal);
    public static final TCKind tk_objref = new TCKind(_tk_objref);
    public static final TCKind tk_struct = new TCKind(_tk_struct);
    public static final TCKind tk_union = new TCKind(_tk_union);
    public static final TCKind tk_enum = new TCKind(_tk_enum);
    public static final TCKind tk_string = new TCKind(_tk_string);
    public static final TCKind tk_sequence = new TCKind(_tk_sequence);
    public static final TCKind tk_array = new TCKind(_tk_array);
    public static final TCKind tk_alias = new TCKind(_tk_alias);
    public static final TCKind tk_except = new TCKind(_tk_except);
    public static final TCKind tk_longlong = new TCKind(_tk_longlong);
    public static final TCKind tk_ulonglong = new TCKind(_tk_ulonglong);
    public static final TCKind tk_longdouble = new TCKind(_tk_longdouble);
    public static final TCKind tk_wchar = new TCKind(_tk_wchar);
    public static final TCKind tk_wstring = new TCKind(_tk_wstring);
    public static final TCKind tk_fixed = new TCKind(_tk_fixed);
    public static final TCKind tk_value = new TCKind(_tk_value);
    public static final TCKind tk_value_box = new TCKind(_tk_value_box);
    public static final TCKind tk_native = new TCKind(_tk_native);
    public static final TCKind tk_abstract_interface = new TCKind(_tk_abstract_interface);
    public static final TCKind tk_local_interface = new TCKind(_tk_local_interface);
    public static final TCKind tk_component = new TCKind(_tk_component);
    public static final TCKind tk_home = new TCKind(_tk_home);
    public static final TCKind tk_event = new TCKind(_tk_event);

    private static final TCKind[] ALL = {
        tk_null,
        tk_void,
        tk_short,
        tk_long,
        tk_ushort,
        tk_ulong,
        tk_float,
        tk_double,
        tk_boolean,
        tk_char,
        tk_octet,
        tk_any,
        tk_TypeCode,
        tk_Principal,
        tk_objref,
        tk_struct,
        tk_union,
        tk_enum,
        tk_string,
        tk_sequence,
        tk_array,
        tk_alias,
        tk_except,
        tk_longlong,
        tk_ulonglong,
        tk_longdouble,
        tk_wchar,
        tk_wstring,
        tk_fixed,
        tk_value,
        tk_value_box,
        tk_native,
        tk_abstract_interface,
        tk_local_interface,
        tk_component,
        tk_home,
        tk_event,
    };

    private final int value;

    protected TCKind(int value) {
        this.value = value;
    }

    public int value() {
        return value;
    }

    public static TCKind from_int(int i) {
        if (i < 0 || i >= ALL.length) {
            throw new BAD_PARAM();
        }
        return ALL[i];
    }
}
