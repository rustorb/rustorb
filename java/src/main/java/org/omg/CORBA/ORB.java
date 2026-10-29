// Part of the IDL to Java mapping (org.omg.CORBA), written for rustorb from
// the OMG specification.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

package org.omg.CORBA;

import org.rustorb.TypeCodes;

/**
 * The ORB. {@link #init(String[], java.util.Properties)} makes a rustorb
 * ORB; {@link #init()} returns the singleton ORB, which only makes
 * TypeCodes and anys (as generated Helpers use it).
 *
 * The ORB options are the omniORB style arguments (-ORBInitRef,
 * -ORBendPoint...) in args, and properties named after them without the
 * dash (ORBInitRef...) or as org.omg.CORBA.ORBInitRef.
 */
public abstract class ORB {
    private static ORB singleton;

    protected ORB() {}

    public static synchronized ORB init() {
        if (singleton == null) {
            singleton = new ORB() {
                @Override
                public String[] list_initial_services() {
                    throw new NO_IMPLEMENT();
                }

                @Override
                public Object resolve_initial_references(String object_name) {
                    throw new NO_IMPLEMENT();
                }

                @Override
                public String object_to_string(Object obj) {
                    throw new NO_IMPLEMENT();
                }

                @Override
                public Object string_to_object(String str) {
                    throw new NO_IMPLEMENT();
                }
            };
        }
        return singleton;
    }

    public static ORB init(String[] args, java.util.Properties props) {
        return org.rustorb.OrbImpl.create(args, props);
    }

    public abstract String[] list_initial_services();

    public abstract Object resolve_initial_references(String object_name)
            throws org.omg.CORBA.ORBPackage.InvalidName;

    public abstract String object_to_string(Object obj);

    public abstract Object string_to_object(String str);

    /**
     * Makes resolve_initial_references(id) return obj (a reference or a
     * local object). InvalidName for an empty id or one that is already an
     * initial reference, BAD_PARAM (minor 27) for null.
     */
    public void register_initial_reference(String id, Object obj)
            throws org.omg.CORBA.ORBPackage.InvalidName {
        throw new NO_IMPLEMENT();
    }

    /**
     * A POA policy from its enumerator, or a
     * Messaging::RelativeRoundtripTimeoutPolicy from an unsigned long long
     * (TimeBase::TimeT, in units of 100 ns), in the any val.
     */
    public Policy create_policy(int type, Any val) throws PolicyError {
        throw new NO_IMPLEMENT();
    }

    /** The ORB offers no services such as security: false. */
    public boolean get_service_information(short service_type,
            ServiceInformationHolder service_info) {
        service_info.value = new ServiceInformation(new int[0], new ServiceDetail[0]);
        return false;
    }

    public void run() {
        throw new NO_IMPLEMENT();
    }

    public void shutdown(boolean wait_for_completion) {
        throw new NO_IMPLEMENT();
    }

    public boolean work_pending() {
        throw new NO_IMPLEMENT();
    }

    public void perform_work() {
        throw new NO_IMPLEMENT();
    }

    public void destroy() {
        throw new NO_IMPLEMENT();
    }

    public void connect(Object obj) {
        throw new NO_IMPLEMENT();
    }

    public void disconnect(Object obj) {
        throw new NO_IMPLEMENT();
    }

    public org.omg.CORBA.portable.OutputStream create_output_stream() {
        throw new NO_IMPLEMENT();
    }

    // --- TypeCodes and anys ----------------------------------------------

    public Any create_any() {
        return TypeCodes.newAny();
    }

    public TypeCode get_primitive_tc(TCKind tcKind) {
        return TypeCodes.primitive(tcKind);
    }

    public TypeCode create_struct_tc(String id, String name, StructMember[] members) {
        return TypeCodes.struct(TCKind._tk_struct, id, name, members);
    }

    public TypeCode create_exception_tc(String id, String name, StructMember[] members) {
        return TypeCodes.struct(TCKind._tk_except, id, name, members);
    }

    public TypeCode create_union_tc(String id, String name, TypeCode discriminator_type,
            UnionMember[] members) {
        return TypeCodes.union(id, name, discriminator_type, members);
    }

    public TypeCode create_enum_tc(String id, String name, String[] members) {
        return TypeCodes.enumeration(id, name, members);
    }

    public TypeCode create_alias_tc(String id, String name, TypeCode original_type) {
        return TypeCodes.alias(TCKind._tk_alias, id, name, original_type);
    }

    public TypeCode create_value_box_tc(String id, String name, TypeCode boxed_type) {
        return TypeCodes.alias(TCKind._tk_value_box, id, name, boxed_type);
    }

    public TypeCode create_interface_tc(String id, String name) {
        return TypeCodes.iface(TCKind._tk_objref, id, name);
    }

    public TypeCode create_abstract_interface_tc(String id, String name) {
        return TypeCodes.iface(TCKind._tk_abstract_interface, id, name);
    }

    public TypeCode create_local_interface_tc(String id, String name) {
        return TypeCodes.iface(TCKind._tk_local_interface, id, name);
    }

    public TypeCode create_native_tc(String id, String name) {
        return TypeCodes.iface(TCKind._tk_native, id, name);
    }

    public TypeCode create_string_tc(int bound) {
        return TypeCodes.string(false, bound);
    }

    public TypeCode create_wstring_tc(int bound) {
        return TypeCodes.string(true, bound);
    }

    public TypeCode create_fixed_tc(short digits, short scale) {
        return TypeCodes.fixed(digits, scale);
    }

    public TypeCode create_sequence_tc(int bound, TypeCode element_type) {
        return TypeCodes.sequence(element_type, bound);
    }

    public TypeCode create_array_tc(int length, TypeCode element_type) {
        return TypeCodes.array(element_type, length);
    }

    public TypeCode create_value_tc(String id, String name, short type_modifier,
            TypeCode concrete_base, ValueMember[] members) {
        return TypeCodes.value(id, name, type_modifier, concrete_base, members);
    }

    public TypeCode create_recursive_tc(String id) {
        return TypeCodes.recursive(id);
    }
}
