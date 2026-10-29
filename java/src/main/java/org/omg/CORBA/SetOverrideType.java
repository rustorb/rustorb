// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

/** CORBA::SetOverrideType */
public class SetOverrideType implements org.omg.CORBA.portable.IDLEntity {
    public static final int _SET_OVERRIDE = 0;
    public static final int _ADD_OVERRIDE = 1;

    public static final SetOverrideType SET_OVERRIDE = new SetOverrideType(_SET_OVERRIDE);
    public static final SetOverrideType ADD_OVERRIDE = new SetOverrideType(_ADD_OVERRIDE);

    private final int value;

    protected SetOverrideType(int value) {
        this.value = value;
    }

    public int value() {
        return value;
    }

    public static SetOverrideType from_int(int i) {
        switch (i) {
            case _SET_OVERRIDE:
                return SET_OVERRIDE;
            case _ADD_OVERRIDE:
                return ADD_OVERRIDE;
            default:
                throw new BAD_PARAM();
        }
    }

    private java.lang.Object readResolve() {
        return from_int(value);
    }
}
