// The DynAny probe of interop/impls/omniorb-cpp/dynany_probe.cc on the
// rustorb Java binding (org.omg.DynamicAny): the same cases, whose output
// run-dynany-probe.sh compares with expected/dynany-probe-rustorb.txt.

package interop;

import org.omg.CORBA.Any;
import org.omg.CORBA.ORB;
import org.omg.CORBA.StructMember;
import org.omg.CORBA.SystemException;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.UnionMember;
import org.omg.CORBA.ValueMember;
import org.omg.DynamicAny.*;
import org.omg.DynamicAny.DynAnyFactoryPackage.InconsistentTypeCode;
import org.omg.DynamicAny.DynAnyPackage.InvalidValue;
import org.omg.DynamicAny.DynAnyPackage.TypeMismatch;

public final class DynAnyProbe {
    private static final String[] KINDS = {
        "tk_null", "tk_void", "tk_short", "tk_long", "tk_ushort", "tk_ulong",
        "tk_float", "tk_double", "tk_boolean", "tk_char", "tk_octet", "tk_any",
        "tk_TypeCode", "tk_Principal", "tk_objref", "tk_struct", "tk_union",
        "tk_enum", "tk_string", "tk_sequence", "tk_array", "tk_alias",
        "tk_except", "tk_longlong", "tk_ulonglong", "tk_longdouble", "tk_wchar",
        "tk_wstring", "tk_fixed", "tk_value", "tk_value_box", "tk_native",
        "tk_abstract_interface", "tk_local_interface", "tk_component",
        "tk_home", "tk_event"};

    interface Body {
        void run() throws Exception;
    }

    private static ORB orb;
    private static DynAnyFactory factory;
    private static final StringBuilder o = new StringBuilder();

    private static void c(String name, Body body) {
        o.append(name).append(": ");
        try {
            body.run();
        } catch (TypeMismatch e) {
            o.append("TypeMismatch");
        } catch (InvalidValue e) {
            o.append("InvalidValue");
        } catch (InconsistentTypeCode e) {
            o.append("InconsistentTypeCode");
        } catch (MustTruncate e) {
            o.append("MustTruncate");
        } catch (SystemException e) {
            o.append(e.getClass().getSimpleName());
        } catch (Exception e) {
            o.append("unexpected ").append(e);
        }
        o.append("\n");
    }

    private static void out(boolean b) {
        o.append(b ? "true" : "false");
    }

    private static void out(int v) {
        o.append(v);
    }

    private static void out(String s) {
        o.append('"').append(s).append('"');
    }

    private static void out(TCKind k) {
        o.append(KINDS[k.value()]);
    }

    private static void sp() {
        o.append(" ");
    }

    private static TypeCode tc(TCKind k) {
        return orb.get_primitive_tc(k);
    }

    private static Any anyLong(int v) {
        Any a = orb.create_any();
        a.insert_long(v);
        return a;
    }

    private static Any anyShort(int v) {
        Any a = orb.create_any();
        a.insert_short((short) v);
        return a;
    }

    private static Any anyOctet(int v) {
        Any a = orb.create_any();
        a.insert_octet((byte) v);
        return a;
    }

    private static Any anyBoolean(boolean v) {
        Any a = orb.create_any();
        a.insert_boolean(v);
        return a;
    }

    // The current position, found by counting the components after it (this
    // moves the position to -1, so it comes last in a case).
    private static int position(DynAny d) throws Exception {
        if (d.current_component() == null) {
            return -1;
        }
        int after = 0;
        while (d.next()) {
            after++;
        }
        return d.component_count() - 1 - after;
    }

    private static void describe(DynAny d) {
        out(d.to_any().type().kind());
    }

    public static void main(String[] args) throws Exception {
        orb = ORB.init(args, null);
        factory = DynAnyFactoryHelper.narrow(orb.resolve_initial_references("DynAnyFactory"));

        TypeCode tc_long = tc(TCKind.tk_long);
        TypeCode tc_string = tc(TCKind.tk_string);
        TypeCode tc_principal = tc(TCKind.tk_Principal);
        TypeCode tc_longdouble = tc(TCKind.tk_longdouble);
        TypeCode count = orb.create_alias_tc("IDL:P/Count:1.0", "Count", tc_long);
        TypeCode s_tc = orb.create_struct_tc("IDL:P/S:1.0", "S", new StructMember[] {
            new StructMember("a", tc_long, null), new StructMember("b", tc_string, null),
            new StructMember("c", count, null)});
        TypeCode e_tc = orb.create_exception_tc("IDL:P/E:1.0", "E", new StructMember[0]);
        TypeCode color = orb.create_enum_tc("IDL:P/Color:1.0", "Color",
                new String[] {"RED", "GREEN", "BLUE"});
        TypeCode tc_boolean = tc(TCKind.tk_boolean);
        // union U switch (long) { case 1: long x; case 2: case 3: string y;
        //                         default: boolean z; };
        TypeCode u_tc = orb.create_union_tc("IDL:P/U:1.0", "U", tc_long, new UnionMember[] {
            new UnionMember("x", anyLong(1), tc_long, null),
            new UnionMember("y", anyLong(2), tc_string, null),
            new UnionMember("y", anyLong(3), tc_string, null),
            new UnionMember("z", anyOctet(0), tc_boolean, null)});
        // union D switch (long) { default: boolean z; case 0: long x; };
        TypeCode d_tc = orb.create_union_tc("IDL:P/D:1.0", "D", tc_long, new UnionMember[] {
            new UnionMember("z", anyOctet(0), tc_boolean, null),
            new UnionMember("x", anyLong(0), tc_long, null)});
        // union N switch (short) { case 5: long x; case 2: string y; };
        TypeCode n_tc = orb.create_union_tc("IDL:P/N:1.0", "N", tc(TCKind.tk_short),
                new UnionMember[] {
                    new UnionMember("x", anyShort(5), tc_long, null),
                    new UnionMember("y", anyShort(2), tc_string, null)});
        // union B switch (boolean) { case TRUE: long t; case FALSE: string f; };
        TypeCode b_tc = orb.create_union_tc("IDL:P/B:1.0", "B", tc_boolean, new UnionMember[] {
            new UnionMember("t", anyBoolean(true), tc_long, null),
            new UnionMember("f", anyBoolean(false), tc_string, null)});
        TypeCode seq_tc = orb.create_sequence_tc(0, tc_long);
        TypeCode bseq_tc = orb.create_sequence_tc(2, tc_long);
        TypeCode arr_tc = orb.create_array_tc(3, tc_long);
        TypeCode money = orb.create_fixed_tc((short) 5, (short) 2);
        TypeCode bstr = orb.create_string_tc(3);
        TypeCode v_tc = orb.create_value_tc("IDL:P/V:1.0", "V", org.omg.CORBA.VM_NONE.value, null,
                new ValueMember[] {new ValueMember("n", "", "", "", tc_long, null,
                        org.omg.CORBA.PUBLIC_MEMBER.value)});
        TypeCode box_tc = orb.create_value_box_tc("IDL:P/LBox:1.0", "LBox", tc_long);

      // -- creation from TypeCodes and positions
      c("long.count", () -> { var d = factory.create_dyn_any_from_type_code(tc_long); out(d.component_count()); });
      c("long.get", () -> { var d = factory.create_dyn_any_from_type_code(tc_long); out(d.get_long()); });
      c("long.seek0", () -> { var d = factory.create_dyn_any_from_type_code(tc_long); out(d.seek(0)); });
      c("long.next", () -> { var d = factory.create_dyn_any_from_type_code(tc_long); out(d.next()); });
      c("long.current_component", () -> { var d = factory.create_dyn_any_from_type_code(tc_long); var c = d.current_component(); out(c == null); });
      c("long.get_short", () -> { var d = factory.create_dyn_any_from_type_code(tc_long); d.get_short(); });
      c("long.insert_short", () -> { var d = factory.create_dyn_any_from_type_code(tc_long); d.insert_short((short) 1); });
      c("alias.insert_long", () -> { var d = factory.create_dyn_any_from_type_code(count); d.insert_long(7); out(d.get_long()); });
      c("alias.type", () -> { var d = factory.create_dyn_any_from_type_code(count); var t = d.type(); out(t.kind()); });
      c("string.get", () -> { var d = factory.create_dyn_any_from_type_code(tc_string); String s = d.get_string(); out(s); });
      c("bstring.insert_long", () -> { var d = factory.create_dyn_any_from_type_code(bstr); d.insert_string("abcd"); });
      c("bstring.insert_ok", () -> { var d = factory.create_dyn_any_from_type_code(bstr); d.insert_string("abc"); out(true); });
      c("typecode.get", () -> { var d = factory.create_dyn_any_from_type_code(tc(TCKind.tk_TypeCode)); var t = d.get_typecode(); out(t.kind()); });
      c("any.get", () -> { var d = factory.create_dyn_any_from_type_code(tc(TCKind.tk_any)); Any a = d.get_any(); var t = a.type(); out(t.kind()); });
      c("any.count", () -> { var d = factory.create_dyn_any_from_type_code(tc(TCKind.tk_any)); out(d.component_count()); });
      c("objref.get", () -> { var d = factory.create_dyn_any_from_type_code(orb.create_interface_tc("IDL:omg.org/CORBA/Object:1.0", "Object")); var obj = d.get_reference(); out(obj == null); });
      c("principal", () -> { var d = factory.create_dyn_any_from_type_code(tc_principal); describe(d); });
      c("longdouble", () -> { var d = factory.create_dyn_any_from_type_code(tc_longdouble); describe(d); });
    
      // -- get_dyn_any: shared with the any or a copy
      c("any.get_dyn_any_shared", () -> {
        var d = factory.create_dyn_any_from_type_code(tc(TCKind.tk_any));
        Any a = anyLong(5); d.insert_any(a);
        var inner = d.get_dyn_any(); inner.insert_long(9);
        out(d.get_any().extract_long());
      });
    
      // -- struct
      c("struct.count", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); out(d.component_count()); });
      c("struct.position", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); out(position(d)); });
      c("struct.insert_long", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); d.insert_long(3); var s = DynStructHelper.narrow(d); String n = s.current_member_name(); out(n); });
      c("struct.insert_long_moves", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); d.insert_long(3); out(d.seek(0)); var c = d.current_component(); out(c.get_long()); });
      c("struct.insert_string_at_0", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); d.insert_string("x"); });
      c("struct.member_kind_alias", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); d.seek(2); var s = DynStructHelper.narrow(d); out(s.current_member_kind()); });
      c("struct.member_name_end", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); d.seek(5); var s = DynStructHelper.narrow(d); String n = s.current_member_name(); out(n); });
      c("struct.insert_at_end", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); d.seek(5); d.insert_long(1); });
      c("struct.next_past_end", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); d.seek(2); out(d.next()); var c = d.current_component(); out(c == null); });
      c("struct.next_from_minus1", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); d.seek(-1); out(d.next()); out(position(d)); });
      c("struct.seek_minus1", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); out(d.seek(-1)); });
      c("struct.component_shared", () -> {
        var d = factory.create_dyn_any_from_type_code(s_tc);
        var c = d.current_component(); c.insert_long(11);
        var s = DynStructHelper.narrow(d); var m = s.get_members();
        out(m[0].value.extract_long());
      });
      c("struct.members_as_dyn_any_shared", () -> {
        var d = factory.create_dyn_any_from_type_code(s_tc);
        var s = DynStructHelper.narrow(d); var m = s.get_members_as_dyn_any();
        m[0].value.insert_long(12);
        var c = d.current_component(); out(c.get_long());
      });
      c("struct.set_members_wrong_name", () -> {
        var d = factory.create_dyn_any_from_type_code(s_tc);
        var s = DynStructHelper.narrow(d); var m = s.get_members();
        m[1].id = "zz"; s.set_members(m);
      });
      c("struct.set_members_empty_names", () -> {
        var d = factory.create_dyn_any_from_type_code(s_tc);
        var s = DynStructHelper.narrow(d); var m = s.get_members();
        for (var p : m) p.id = "";
        s.set_members(m); out(true);
      });
      c("struct.set_members_short", () -> {
        var d = factory.create_dyn_any_from_type_code(s_tc);
        var s = DynStructHelper.narrow(d); var m = s.get_members();
        m = java.util.Arrays.copyOf(m, 2); s.set_members(m);
      });
      c("struct.set_members_wrong_type", () -> {
        var d = factory.create_dyn_any_from_type_code(s_tc);
        var s = DynStructHelper.narrow(d); var m = s.get_members();
        m[0].value = anyShort(1); s.set_members(m);
      });
      c("struct.set_members_unaliased", () -> {
        var d = factory.create_dyn_any_from_type_code(s_tc);
        var s = DynStructHelper.narrow(d); var m = s.get_members();
        m[2].value = anyLong(4); s.set_members(m); out(true);
      });
      c("struct.set_members_position", () -> {
        var d = factory.create_dyn_any_from_type_code(s_tc);
        d.seek(2);
        var s = DynStructHelper.narrow(d); var m = s.get_members();
        s.set_members(m); out(position(d));
      });
      c("struct.from_any_wrong", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); Any a = anyLong(1); d.from_any(a); });
      c("struct.from_any_position", () -> {
        var d = factory.create_dyn_any_from_type_code(s_tc);
        Any a = d.to_any(); d.seek(2); d.from_any(a); out(position(d));
      });
      c("struct.assign_wrong", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); var e = factory.create_dyn_any_from_type_code(tc_long); d.assign(e); });
      c("struct.destroy_component", () -> {
        var d = factory.create_dyn_any_from_type_code(s_tc);
        var c = d.current_component(); c.destroy(); c.insert_long(2); out(c.get_long());
      });
      c("struct.destroyed", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); var c = d.current_component(); d.destroy(); c.get_long(); });
      c("struct.destroyed_top", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); d.destroy(); d.component_count(); });
      c("struct.copy_independent", () -> {
        var d = factory.create_dyn_any_from_type_code(s_tc);
        var e = d.copy(); e.insert_long(5); var c = d.current_component(); out(c.get_long());
      });
      c("struct.equal", () -> { var d = factory.create_dyn_any_from_type_code(s_tc); var e = d.copy(); out(d.equal(e)); e.insert_long(1); out(d.equal(e)); });
      c("except.count", () -> { var d = factory.create_dyn_any_from_type_code(e_tc); out(d.component_count()); });
      c("except.current_component", () -> { var d = factory.create_dyn_any_from_type_code(e_tc); var c = d.current_component(); out(c == null); });
      c("except.member_name", () -> { var d = factory.create_dyn_any_from_type_code(e_tc); var s = DynStructHelper.narrow(d); String n = s.current_member_name(); out(n); });
    
      // -- enum
      c("enum.get", () -> { var d = factory.create_dyn_any_from_type_code(color); var e = DynEnumHelper.narrow(d); String s = e.get_as_string(); out(s); });
      c("enum.set_as_string_bad", () -> { var d = factory.create_dyn_any_from_type_code(color); var e = DynEnumHelper.narrow(d); e.set_as_string("PINK"); });
      c("enum.set_as_ulong_bad", () -> { var d = factory.create_dyn_any_from_type_code(color); var e = DynEnumHelper.narrow(d); e.set_as_ulong(3); });
      c("enum.set_as_ulong", () -> { var d = factory.create_dyn_any_from_type_code(color); var e = DynEnumHelper.narrow(d); e.set_as_ulong(2); String s = e.get_as_string(); out(s); });
      c("enum.current_component", () -> { var d = factory.create_dyn_any_from_type_code(color); var c = d.current_component(); out(c == null); });
      c("enum.count", () -> { var d = factory.create_dyn_any_from_type_code(color); out(d.component_count()); });
      c("enum.insert_ulong", () -> { var d = factory.create_dyn_any_from_type_code(color); d.insert_ulong(1); });
    
      // -- fixed
      c("fixed.get", () -> { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); String s = f.get_value(); out(s); });
      c("fixed.set", () -> { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); out(f.set_value("-12.5")); String s = f.get_value(); out(s); });
      c("fixed.set_d", () -> { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); out(f.set_value("1.25d")); String s = f.get_value(); out(s); });
      c("fixed.set_blank", () -> { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); out(f.set_value(" 1.25 ")); String s = f.get_value(); out(s); });
      c("fixed.set_truncates", () -> { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); out(f.set_value("1.239")); String s = f.get_value(); out(s); });
      c("fixed.set_zero_tail", () -> { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); out(f.set_value("1.230")); });
      c("fixed.set_too_big", () -> { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); f.set_value("1234.5"); });
      c("fixed.set_bad", () -> { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); f.set_value("abc"); });
      c("fixed.set_int", () -> { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); out(f.set_value("7")); String s = f.get_value(); out(s); });
    
      // -- union
      c("union.initial", () -> {
        var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
        var disc = u.get_discriminator(); out(disc.get_long()); sp();
        out(u.has_no_active_member()); sp();
        String n = u.member_name(); out(n);
      });
      c("union.count", () -> { var d = factory.create_dyn_any_from_type_code(u_tc); out(d.component_count()); });
      c("union.position", () -> { var d = factory.create_dyn_any_from_type_code(u_tc); out(position(d)); });
      c("union.default_first", () -> {
        var d = factory.create_dyn_any_from_type_code(d_tc); var u = DynUnionHelper.narrow(d);
        var disc = u.get_discriminator(); out(disc.get_long()); sp();
        String n = u.member_name(); out(n);
      });
      c("union.no_default_initial", () -> {
        var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d);
        var disc = u.get_discriminator(); o.append(disc.get_short()).append(" ");
        String n = u.member_name(); out(n);
      });
      c("union.set_discriminator", () -> {
        var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
        var disc = factory.create_dyn_any_from_type_code(tc_long); disc.insert_long(3);
        u.set_discriminator(disc); String n = u.member_name(); out(n); sp(); out(position(d));
      });
      c("union.set_discriminator_keeps", () -> {
        var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
        var disc = factory.create_dyn_any_from_type_code(tc_long); disc.insert_long(2);
        u.set_discriminator(disc); var m = u.member(); m.insert_string("kept");
        disc.insert_long(3); u.set_discriminator(disc); m = u.member(); String s = m.get_string(); out(s);
      });
      c("union.set_discriminator_default", () -> {
        var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
        var disc = factory.create_dyn_any_from_type_code(tc_long); disc.insert_long(42);
        u.set_discriminator(disc); String n = u.member_name(); out(n);
      });
      c("union.set_discriminator_wrong", () -> {
        var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
        var disc = factory.create_dyn_any_from_type_code(tc(TCKind.tk_short)); u.set_discriminator(disc);
      });
      c("union.disc_component", () -> {
        var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
        var disc = u.get_discriminator(); disc.insert_long(2);
        String n = u.member_name(); out(n); sp(); out(d.component_count());
      });
      c("union.disc_component_to_none", () -> {
        var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d);
        var disc = u.get_discriminator(); disc.insert_short((short) 9);
        out(u.has_no_active_member()); sp(); out(d.component_count());
      });
      c("union.set_to_default_member", () -> {
        var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
        u.set_to_default_member(); String n = u.member_name(); out(n); sp();
        var disc = u.get_discriminator(); out(disc.get_long()); sp(); out(position(d));
      });
      c("union.set_to_default_member_none", () -> { var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d); u.set_to_default_member(); });
      c("union.set_to_no_active_member", () -> {
        var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d);
        u.set_to_no_active_member(); out(u.has_no_active_member()); sp();
        var disc = u.get_discriminator(); o.append(disc.get_short()).append(" "); out(d.component_count()); sp(); out(position(d));
      });
      c("union.set_to_no_active_member_default", () -> { var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d); u.set_to_no_active_member(); });
      c("union.set_to_no_active_member_full", () -> { var d = factory.create_dyn_any_from_type_code(b_tc); var u = DynUnionHelper.narrow(d); u.set_to_no_active_member(); });
      c("union.member_none", () -> { var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d); u.set_to_no_active_member(); u.member(); });
      c("union.member_name_none", () -> { var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d); u.set_to_no_active_member(); u.member_name(); });
      c("union.discriminator_kind", () -> { var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d); out(u.discriminator_kind()); });
      c("union.member_kind", () -> { var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d); out(u.member_kind()); });
      c("union.insert_long", () -> { var d = factory.create_dyn_any_from_type_code(u_tc); d.insert_long(1); out(position(d)); });
      c("union.boolean_initial", () -> {
        var d = factory.create_dyn_any_from_type_code(b_tc); var u = DynUnionHelper.narrow(d);
        var disc = u.get_discriminator(); out(disc.get_boolean()); sp(); String n = u.member_name(); out(n);
      });
    
      // -- sequence and array
      c("seq.count", () -> { var d = factory.create_dyn_any_from_type_code(seq_tc); out(d.component_count()); });
      c("seq.current_component", () -> { var d = factory.create_dyn_any_from_type_code(seq_tc); var c = d.current_component(); out(c == null); });
      c("seq.insert_empty", () -> { var d = factory.create_dyn_any_from_type_code(seq_tc); d.insert_long(1); });
      c("seq.grow_position", () -> { var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d); s.set_length(3); out(position(d)); });
      c("seq.grow_keeps_position", () -> {
        var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
        s.set_length(2); d.seek(1); d.insert_long(4); s.set_length(4);
        var c = d.current_component(); out(c.get_long());
      });
      c("seq.grow_from_minus1", () -> {
        var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
        s.set_length(2); d.seek(-1); s.set_length(4); var c = d.current_component();
        out(c == null); sp(); d.insert_long(8); var e = d.copy(); e.seek(2); out(e.get_long());
      });
      c("seq.shrink_position", () -> {
        var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
        s.set_length(4); d.seek(3); s.set_length(2); var c = d.current_component(); out(c == null);
      });
      c("seq.shrink_keeps", () -> {
        var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
        s.set_length(4); d.seek(1); d.insert_long(6); s.set_length(2); var c = d.current_component(); out(c.get_long());
      });
      c("seq.shrink_to_0", () -> {
        var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
        s.set_length(4); s.set_length(0); var c = d.current_component(); out(c == null);
      });
      c("seq.bound", () -> { var d = factory.create_dyn_any_from_type_code(bseq_tc); var s = DynSequenceHelper.narrow(d); s.set_length(3); });
      c("seq.set_elements_bound", () -> {
        var d = factory.create_dyn_any_from_type_code(bseq_tc); var s = DynSequenceHelper.narrow(d);
        Any[] e = new Any[3]; for (int i = 0; i < 3; i++) e[i] = anyLong(i); s.set_elements(e);
      });
      c("seq.set_elements_type", () -> {
        var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
        Any[] e = new Any[1]; e[0] = anyShort(1); s.set_elements(e);
      });
      c("seq.set_elements_position", () -> {
        var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
        Any[] e = new Any[2]; e[0] = anyLong(1); e[1] = anyLong(2); s.set_elements(e); out(position(d));
      });
      c("seq.insert_long_seq", () -> {
        var d = factory.create_dyn_any_from_type_code(seq_tc);
        int[] v = new int[2]; v[0] = 1; v[1] = 2; d.insert_long_seq(v); out(d.component_count()); sp(); out(position(d));
      });
      c("seq.get_long_seq", () -> {
        var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
        s.set_length(2); var v = d.get_long_seq(); out(v.length);
      });
      c("seq.insert_short_seq", () -> {
        var d = factory.create_dyn_any_from_type_code(seq_tc);
        short[] v = {1}; d.insert_short_seq(v);
      });
      c("seq.insert_long_seq_bound", () -> {
        var d = factory.create_dyn_any_from_type_code(bseq_tc);
        int[] v = new int[3]; d.insert_long_seq(v);
      });
      c("struct.insert_long_seq", () -> {
        // A struct whose current component is a long: not a sequence.
        var d = factory.create_dyn_any_from_type_code(s_tc);
        int[] v = new int[1]; d.insert_long_seq(v);
      });
      c("arr.count", () -> { var d = factory.create_dyn_any_from_type_code(arr_tc); out(d.component_count()); sp(); out(position(d)); });
      c("arr.set_elements_count", () -> {
        var d = factory.create_dyn_any_from_type_code(arr_tc); var a = DynArrayHelper.narrow(d);
        Any[] e = new Any[2]; e[0] = anyLong(1); e[1] = anyLong(1); a.set_elements(e);
      });
      c("arr.insert_long_seq_count", () -> {
        var d = factory.create_dyn_any_from_type_code(arr_tc);
        int[] v = new int[2]; d.insert_long_seq(v);
      });
    
      // -- valuetypes
      c("value.null", () -> { var d = factory.create_dyn_any_from_type_code(v_tc); var v = DynValueHelper.narrow(d); out(v.is_null()); sp(); out(d.component_count()); });
      c("value.set_to_value", () -> { var d = factory.create_dyn_any_from_type_code(v_tc); var v = DynValueHelper.narrow(d); v.set_to_value(); out(v.is_null()); sp(); out(d.component_count()); sp(); out(position(d)); });
      c("value.get_members_null", () -> { var d = factory.create_dyn_any_from_type_code(v_tc); var v = DynValueHelper.narrow(d); var m = v.get_members(); out(m.length); });
      c("value.member_name_null", () -> { var d = factory.create_dyn_any_from_type_code(v_tc); var v = DynValueHelper.narrow(d); String n = v.current_member_name(); out(n); });
      c("value.current_component_null", () -> { var d = factory.create_dyn_any_from_type_code(v_tc); var c = d.current_component(); out(c == null); });
      c("value.set_members", () -> {
        var d = factory.create_dyn_any_from_type_code(v_tc); var v = DynValueHelper.narrow(d);
        NameValuePair[] m = {new NameValuePair("n", anyLong(3))};
        v.set_members(m); out(v.is_null()); sp(); var c = d.current_component(); out(c.get_long());
      });
      c("box.null", () -> { var d = factory.create_dyn_any_from_type_code(box_tc); var b = DynValueBoxHelper.narrow(d); out(b.is_null()); sp(); out(d.component_count()); });
      c("box.get_null", () -> { var d = factory.create_dyn_any_from_type_code(box_tc); var b = DynValueBoxHelper.narrow(d); Any a = b.get_boxed_value(); });
      c("box.set", () -> {
        var d = factory.create_dyn_any_from_type_code(box_tc); var b = DynValueBoxHelper.narrow(d);
        Any a = anyLong(4); b.set_boxed_value(a); out(b.is_null()); sp(); out(d.component_count()); sp(); out(position(d));
      });
      c("box.set_wrong", () -> {
        var d = factory.create_dyn_any_from_type_code(box_tc); var b = DynValueBoxHelper.narrow(d);
        Any a = anyShort(4); b.set_boxed_value(a);
      });
      c("box.set_to_value", () -> { var d = factory.create_dyn_any_from_type_code(box_tc); var b = DynValueBoxHelper.narrow(d); b.set_to_value(); var c = d.current_component(); out(c.get_long()); });
    
        System.out.print(o);
        orb.destroy();
    }
}
