// The DynAny probe of interop/impls/omniorb-cpp/dynany_probe.cc on the
// rustorb .NET binding (DynamicAny): the same cases, whose output
// run-dynany-probe.sh compares with expected/dynany-probe-rustorb.txt.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Linq;
using System.Text;
using Corba;
using DynamicAny;
using Any = Corba.Any;
using TypeCode = Corba.TypeCode;
using TCKind = Corba.TCKind;

namespace Interop
{
    public static class DynAnyProbe
    {
        private static readonly string[] KINDS = {
            "tk_null", "tk_void", "tk_short", "tk_long", "tk_ushort", "tk_ulong",
            "tk_float", "tk_double", "tk_boolean", "tk_char", "tk_octet", "tk_any",
            "tk_TypeCode", "tk_Principal", "tk_objref", "tk_struct", "tk_union",
            "tk_enum", "tk_string", "tk_sequence", "tk_array", "tk_alias",
            "tk_except", "tk_longlong", "tk_ulonglong", "tk_longdouble", "tk_wchar",
            "tk_wstring", "tk_fixed", "tk_value", "tk_value_box", "tk_native",
            "tk_abstract_interface", "tk_local_interface", "tk_component",
            "tk_home", "tk_event"};

        private static ORB orb = null!;
        private static IDynAnyFactory factory = null!;
        private static readonly StringBuilder o = new StringBuilder();

        private static void C(string name, Action body)
        {
            o.Append(name).Append(": ");
            try
            {
                body();
            }
            catch (DynAny.TypeMismatch)
            {
                o.Append("TypeMismatch");
            }
            catch (DynAny.InvalidValue)
            {
                o.Append("InvalidValue");
            }
            catch (DynAnyFactory.InconsistentTypeCode)
            {
                o.Append("InconsistentTypeCode");
            }
            catch (MustTruncate)
            {
                o.Append("MustTruncate");
            }
            catch (Corba.SystemException e)
            {
                o.Append(e.GetType().Name);
            }
            catch (Exception e)
            {
                o.Append("unexpected ").Append(e);
            }
            o.Append('\n');
        }

        private static void Out(bool b) => o.Append(b ? "true" : "false");
        private static void Out(long v) => o.Append(v);
        private static void Out(string s) => o.Append('"').Append(s).Append('"');
        private static void Out(TCKind k) => o.Append(KINDS[(int)k]);
        private static void Sp() => o.Append(' ');

        private static Omg.Types.ISequence<T> Seq<T>(T[] a) => new Omg.Types.Sequence<T>(a);

        private static TypeCode tc(TCKind k) => orb.get_primitive_tc(k);

        private static Any anyLong(int v) { Any a = orb.create_any(); a.insert_long(v); return a; }
        private static Any anyShort(int v) { Any a = orb.create_any(); a.insert_short((short)v); return a; }
        private static Any anyOctet(int v) { Any a = orb.create_any(); a.insert_octet((byte)v); return a; }
        private static Any anyBoolean(bool v) { Any a = orb.create_any(); a.insert_boolean(v); return a; }

        // The current position, found by counting the components after it (this
        // moves the position to -1, so it comes last in a case).
        private static long position(IDynAny d)
        {
            if (d.current_component() == null)
            {
                return -1;
            }
            int after = 0;
            while (d.next())
            {
                after++;
            }
            return d.component_count() - 1 - after;
        }

        private static void describe(IDynAny d) => Out(d.to_any().type.kind());

        public static int Run(string[] args)
        {
            orb = ORB.init(args);
            factory = DynAnyFactoryHelper.narrow(orb.resolve_initial_references("DynAnyFactory"))!;

            TypeCode tc_long = tc(TCKind.tk_long);
            TypeCode tc_string = tc(TCKind.tk_string);
            TypeCode tc_principal = tc(TCKind.tk_Principal);
            TypeCode tc_longdouble = tc(TCKind.tk_longdouble);
            TypeCode count = orb.create_alias_tc("IDL:P/Count:1.0", "Count", tc_long);
            TypeCode s_tc = orb.create_struct_tc("IDL:P/S:1.0", "S", new StructMember[] {
                new StructMember("a", tc_long), new StructMember("b", tc_string),
                new StructMember("c", count)});
            TypeCode e_tc = orb.create_exception_tc("IDL:P/E:1.0", "E", new StructMember[0]);
            TypeCode color = orb.create_enum_tc("IDL:P/Color:1.0", "Color",
                    new String[] {"RED", "GREEN", "BLUE"});
            TypeCode tc_boolean = tc(TCKind.tk_boolean);
            // union U switch (long) { case 1: long x; case 2: case 3: string y;
            //                         default: boolean z; };
            TypeCode u_tc = orb.create_union_tc("IDL:P/U:1.0", "U", tc_long, new UnionMember[] {
                new UnionMember("x", anyLong(1), tc_long),
                new UnionMember("y", anyLong(2), tc_string),
                new UnionMember("y", anyLong(3), tc_string),
                new UnionMember("z", anyOctet(0), tc_boolean)});
            // union D switch (long) { default: boolean z; case 0: long x; };
            TypeCode d_tc = orb.create_union_tc("IDL:P/D:1.0", "D", tc_long, new UnionMember[] {
                new UnionMember("z", anyOctet(0), tc_boolean),
                new UnionMember("x", anyLong(0), tc_long)});
            // union N switch (short) { case 5: long x; case 2: string y; };
            TypeCode n_tc = orb.create_union_tc("IDL:P/N:1.0", "N", tc(TCKind.tk_short),
                    new UnionMember[] {
                        new UnionMember("x", anyShort(5), tc_long),
                        new UnionMember("y", anyShort(2), tc_string)});
            // union B switch (boolean) { case TRUE: long t; case FALSE: string f; };
            TypeCode b_tc = orb.create_union_tc("IDL:P/B:1.0", "B", tc_boolean, new UnionMember[] {
                new UnionMember("t", anyBoolean(true), tc_long),
                new UnionMember("f", anyBoolean(false), tc_string)});
            TypeCode seq_tc = orb.create_sequence_tc(0, tc_long);
            TypeCode bseq_tc = orb.create_sequence_tc(2, tc_long);
            TypeCode arr_tc = orb.create_array_tc(3, tc_long);
            TypeCode money = orb.create_fixed_tc((ushort) 5, (short) 2);
            TypeCode bstr = orb.create_string_tc(3);
            TypeCode v_tc = orb.create_value_tc("IDL:P/V:1.0", "V", Corba.Constants.VM_NONE, null,
                    new ValueMember[] {new ValueMember("n", tc_long, Corba.Constants.PUBLIC_MEMBER)});
            TypeCode box_tc = orb.create_value_box_tc("IDL:P/LBox:1.0", "LBox", tc_long);

          // -- creation from TypeCodes and positions
          C("long.count", () => { var d = factory.create_dyn_any_from_type_code(tc_long); Out(d.component_count()); });
          C("long.get", () => { var d = factory.create_dyn_any_from_type_code(tc_long); Out(d.get_long()); });
          C("long.seek0", () => { var d = factory.create_dyn_any_from_type_code(tc_long); Out(d.seek(0)); });
          C("long.next", () => { var d = factory.create_dyn_any_from_type_code(tc_long); Out(d.next()); });
          C("long.current_component", () => { var d = factory.create_dyn_any_from_type_code(tc_long); var c = d.current_component(); Out(c == null); });
          C("long.get_short", () => { var d = factory.create_dyn_any_from_type_code(tc_long); d.get_short(); });
          C("long.insert_short", () => { var d = factory.create_dyn_any_from_type_code(tc_long); d.insert_short((short) 1); });
          C("alias.insert_long", () => { var d = factory.create_dyn_any_from_type_code(count); d.insert_long(7); Out(d.get_long()); });
          C("alias.type", () => { var d = factory.create_dyn_any_from_type_code(count); var t = d.type(); Out(t.kind()); });
          C("string.get", () => { var d = factory.create_dyn_any_from_type_code(tc_string); string s = d.get_string(); Out(s); });
          C("bstring.insert_long", () => { var d = factory.create_dyn_any_from_type_code(bstr); d.insert_string("abcd"); });
          C("bstring.insert_ok", () => { var d = factory.create_dyn_any_from_type_code(bstr); d.insert_string("abc"); Out(true); });
          C("typecode.get", () => { var d = factory.create_dyn_any_from_type_code(tc(TCKind.tk_TypeCode)); var t = d.get_typecode(); Out(t.kind()); });
          C("any.get", () => { var d = factory.create_dyn_any_from_type_code(tc(TCKind.tk_any)); Any a = d.get_any(); var t = a.type; Out(t.kind()); });
          C("any.count", () => { var d = factory.create_dyn_any_from_type_code(tc(TCKind.tk_any)); Out(d.component_count()); });
          C("objref.get", () => { var d = factory.create_dyn_any_from_type_code(orb.create_interface_tc("IDL:omg.org/CORBA/Object:1.0", "Object")); var obj = d.get_reference(); Out(obj == null); });
          C("principal", () => { var d = factory.create_dyn_any_from_type_code(tc_principal); describe(d); });
          C("longdouble", () => { var d = factory.create_dyn_any_from_type_code(tc_longdouble); describe(d); });
    
          // -- get_dyn_any: shared with the any or a copy
          C("any.get_dyn_any_shared", () => {
            var d = factory.create_dyn_any_from_type_code(tc(TCKind.tk_any));
            Any a = anyLong(5); d.insert_any(a);
            var inner = d.get_dyn_any(); inner.insert_long(9);
            Out(d.get_any().extract_long());
          });
    
          // -- struct
          C("struct.count", () => { var d = factory.create_dyn_any_from_type_code(s_tc); Out(d.component_count()); });
          C("struct.position", () => { var d = factory.create_dyn_any_from_type_code(s_tc); Out(position(d)); });
          C("struct.insert_long", () => { var d = factory.create_dyn_any_from_type_code(s_tc); d.insert_long(3); var s = DynStructHelper.narrow(d); string n = s.current_member_name(); Out(n); });
          C("struct.insert_long_moves", () => { var d = factory.create_dyn_any_from_type_code(s_tc); d.insert_long(3); Out(d.seek(0)); var c = d.current_component(); Out(c.get_long()); });
          C("struct.insert_string_at_0", () => { var d = factory.create_dyn_any_from_type_code(s_tc); d.insert_string("x"); });
          C("struct.member_kind_alias", () => { var d = factory.create_dyn_any_from_type_code(s_tc); d.seek(2); var s = DynStructHelper.narrow(d); Out(s.current_member_kind()); });
          C("struct.member_name_end", () => { var d = factory.create_dyn_any_from_type_code(s_tc); d.seek(5); var s = DynStructHelper.narrow(d); string n = s.current_member_name(); Out(n); });
          C("struct.insert_at_end", () => { var d = factory.create_dyn_any_from_type_code(s_tc); d.seek(5); d.insert_long(1); });
          C("struct.next_past_end", () => { var d = factory.create_dyn_any_from_type_code(s_tc); d.seek(2); Out(d.next()); var c = d.current_component(); Out(c == null); });
          C("struct.next_from_minus1", () => { var d = factory.create_dyn_any_from_type_code(s_tc); d.seek(-1); Out(d.next()); Out(position(d)); });
          C("struct.seek_minus1", () => { var d = factory.create_dyn_any_from_type_code(s_tc); Out(d.seek(-1)); });
          C("struct.component_shared", () => {
            var d = factory.create_dyn_any_from_type_code(s_tc);
            var c = d.current_component(); c.insert_long(11);
            var s = DynStructHelper.narrow(d); var m = s.get_members();
            Out(m[0].value.extract_long());
          });
          C("struct.members_as_dyn_any_shared", () => {
            var d = factory.create_dyn_any_from_type_code(s_tc);
            var s = DynStructHelper.narrow(d); var m = s.get_members_as_dyn_any();
            m[0].value.insert_long(12);
            var c = d.current_component(); Out(c.get_long());
          });
          C("struct.set_members_wrong_name", () => {
            var d = factory.create_dyn_any_from_type_code(s_tc);
            var s = DynStructHelper.narrow(d); var m = s.get_members();
            m[1].id = "zz"; s.set_members(m);
          });
          C("struct.set_members_empty_names", () => {
            var d = factory.create_dyn_any_from_type_code(s_tc);
            var s = DynStructHelper.narrow(d); var m = s.get_members();
            foreach (var p in m) p.id = "";
            s.set_members(m); Out(true);
          });
          C("struct.set_members_short", () => {
            var d = factory.create_dyn_any_from_type_code(s_tc);
            var s = DynStructHelper.narrow(d); var m = s.get_members();
            m = new Omg.Types.Sequence<NameValuePair>(m.Take(2)); s.set_members(m);
          });
          C("struct.set_members_wrong_type", () => {
            var d = factory.create_dyn_any_from_type_code(s_tc);
            var s = DynStructHelper.narrow(d); var m = s.get_members();
            m[0].value = anyShort(1); s.set_members(m);
          });
          C("struct.set_members_unaliased", () => {
            var d = factory.create_dyn_any_from_type_code(s_tc);
            var s = DynStructHelper.narrow(d); var m = s.get_members();
            m[2].value = anyLong(4); s.set_members(m); Out(true);
          });
          C("struct.set_members_position", () => {
            var d = factory.create_dyn_any_from_type_code(s_tc);
            d.seek(2);
            var s = DynStructHelper.narrow(d); var m = s.get_members();
            s.set_members(m); Out(position(d));
          });
          C("struct.from_any_wrong", () => { var d = factory.create_dyn_any_from_type_code(s_tc); Any a = anyLong(1); d.from_any(a); });
          C("struct.from_any_position", () => {
            var d = factory.create_dyn_any_from_type_code(s_tc);
            Any a = d.to_any(); d.seek(2); d.from_any(a); Out(position(d));
          });
          C("struct.assign_wrong", () => { var d = factory.create_dyn_any_from_type_code(s_tc); var e = factory.create_dyn_any_from_type_code(tc_long); d.assign(e); });
          C("struct.destroy_component", () => {
            var d = factory.create_dyn_any_from_type_code(s_tc);
            var c = d.current_component(); c.destroy(); c.insert_long(2); Out(c.get_long());
          });
          C("struct.destroyed", () => { var d = factory.create_dyn_any_from_type_code(s_tc); var c = d.current_component(); d.destroy(); c.get_long(); });
          C("struct.destroyed_top", () => { var d = factory.create_dyn_any_from_type_code(s_tc); d.destroy(); d.component_count(); });
          C("struct.copy_independent", () => {
            var d = factory.create_dyn_any_from_type_code(s_tc);
            var e = d.copy(); e.insert_long(5); var c = d.current_component(); Out(c.get_long());
          });
          C("struct.equal", () => { var d = factory.create_dyn_any_from_type_code(s_tc); var e = d.copy(); Out(d.equal(e)); e.insert_long(1); Out(d.equal(e)); });
          C("except.count", () => { var d = factory.create_dyn_any_from_type_code(e_tc); Out(d.component_count()); });
          C("except.current_component", () => { var d = factory.create_dyn_any_from_type_code(e_tc); var c = d.current_component(); Out(c == null); });
          C("except.member_name", () => { var d = factory.create_dyn_any_from_type_code(e_tc); var s = DynStructHelper.narrow(d); string n = s.current_member_name(); Out(n); });
    
          // -- enum
          C("enum.get", () => { var d = factory.create_dyn_any_from_type_code(color); var e = DynEnumHelper.narrow(d); string s = e.get_as_string(); Out(s); });
          C("enum.set_as_string_bad", () => { var d = factory.create_dyn_any_from_type_code(color); var e = DynEnumHelper.narrow(d); e.set_as_string("PINK"); });
          C("enum.set_as_ulong_bad", () => { var d = factory.create_dyn_any_from_type_code(color); var e = DynEnumHelper.narrow(d); e.set_as_ulong(3); });
          C("enum.set_as_ulong", () => { var d = factory.create_dyn_any_from_type_code(color); var e = DynEnumHelper.narrow(d); e.set_as_ulong(2); string s = e.get_as_string(); Out(s); });
          C("enum.current_component", () => { var d = factory.create_dyn_any_from_type_code(color); var c = d.current_component(); Out(c == null); });
          C("enum.count", () => { var d = factory.create_dyn_any_from_type_code(color); Out(d.component_count()); });
          C("enum.insert_ulong", () => { var d = factory.create_dyn_any_from_type_code(color); d.insert_ulong(1); });
    
          // -- fixed
          C("fixed.get", () => { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); string s = f.get_value(); Out(s); });
          C("fixed.set", () => { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); Out(f.set_value("-12.5")); string s = f.get_value(); Out(s); });
          C("fixed.set_d", () => { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); Out(f.set_value("1.25d")); string s = f.get_value(); Out(s); });
          C("fixed.set_blank", () => { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); Out(f.set_value(" 1.25 ")); string s = f.get_value(); Out(s); });
          C("fixed.set_truncates", () => { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); Out(f.set_value("1.239")); string s = f.get_value(); Out(s); });
          C("fixed.set_zero_tail", () => { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); Out(f.set_value("1.230")); });
          C("fixed.set_too_big", () => { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); f.set_value("1234.5"); });
          C("fixed.set_bad", () => { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); f.set_value("abc"); });
          C("fixed.set_int", () => { var d = factory.create_dyn_any_from_type_code(money); var f = DynFixedHelper.narrow(d); Out(f.set_value("7")); string s = f.get_value(); Out(s); });
    
          // -- union
          C("union.initial", () => {
            var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
            var disc = u.get_discriminator(); Out(disc.get_long()); Sp();
            Out(u.has_no_active_member()); Sp();
            string n = u.member_name(); Out(n);
          });
          C("union.count", () => { var d = factory.create_dyn_any_from_type_code(u_tc); Out(d.component_count()); });
          C("union.position", () => { var d = factory.create_dyn_any_from_type_code(u_tc); Out(position(d)); });
          C("union.default_first", () => {
            var d = factory.create_dyn_any_from_type_code(d_tc); var u = DynUnionHelper.narrow(d);
            var disc = u.get_discriminator(); Out(disc.get_long()); Sp();
            string n = u.member_name(); Out(n);
          });
          C("union.no_default_initial", () => {
            var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d);
            var disc = u.get_discriminator(); o.Append(disc.get_short()).Append(" ");
            string n = u.member_name(); Out(n);
          });
          C("union.set_discriminator", () => {
            var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
            var disc = factory.create_dyn_any_from_type_code(tc_long); disc.insert_long(3);
            u.set_discriminator(disc); string n = u.member_name(); Out(n); Sp(); Out(position(d));
          });
          C("union.set_discriminator_keeps", () => {
            var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
            var disc = factory.create_dyn_any_from_type_code(tc_long); disc.insert_long(2);
            u.set_discriminator(disc); var m = u.member(); m.insert_string("kept");
            disc.insert_long(3); u.set_discriminator(disc); m = u.member(); string s = m.get_string(); Out(s);
          });
          C("union.set_discriminator_default", () => {
            var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
            var disc = factory.create_dyn_any_from_type_code(tc_long); disc.insert_long(42);
            u.set_discriminator(disc); string n = u.member_name(); Out(n);
          });
          C("union.set_discriminator_wrong", () => {
            var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
            var disc = factory.create_dyn_any_from_type_code(tc(TCKind.tk_short)); u.set_discriminator(disc);
          });
          C("union.disc_component", () => {
            var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
            var disc = u.get_discriminator(); disc.insert_long(2);
            string n = u.member_name(); Out(n); Sp(); Out(d.component_count());
          });
          C("union.disc_component_to_none", () => {
            var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d);
            var disc = u.get_discriminator(); disc.insert_short((short) 9);
            Out(u.has_no_active_member()); Sp(); Out(d.component_count());
          });
          C("union.set_to_default_member", () => {
            var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d);
            u.set_to_default_member(); string n = u.member_name(); Out(n); Sp();
            var disc = u.get_discriminator(); Out(disc.get_long()); Sp(); Out(position(d));
          });
          C("union.set_to_default_member_none", () => { var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d); u.set_to_default_member(); });
          C("union.set_to_no_active_member", () => {
            var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d);
            u.set_to_no_active_member(); Out(u.has_no_active_member()); Sp();
            var disc = u.get_discriminator(); o.Append(disc.get_short()).Append(" "); Out(d.component_count()); Sp(); Out(position(d));
          });
          C("union.set_to_no_active_member_default", () => { var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d); u.set_to_no_active_member(); });
          C("union.set_to_no_active_member_full", () => { var d = factory.create_dyn_any_from_type_code(b_tc); var u = DynUnionHelper.narrow(d); u.set_to_no_active_member(); });
          C("union.member_none", () => { var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d); u.set_to_no_active_member(); u.member(); });
          C("union.member_name_none", () => { var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d); u.set_to_no_active_member(); u.member_name(); });
          C("union.discriminator_kind", () => { var d = factory.create_dyn_any_from_type_code(n_tc); var u = DynUnionHelper.narrow(d); Out(u.discriminator_kind()); });
          C("union.member_kind", () => { var d = factory.create_dyn_any_from_type_code(u_tc); var u = DynUnionHelper.narrow(d); Out(u.member_kind()); });
          C("union.insert_long", () => { var d = factory.create_dyn_any_from_type_code(u_tc); d.insert_long(1); Out(position(d)); });
          C("union.boolean_initial", () => {
            var d = factory.create_dyn_any_from_type_code(b_tc); var u = DynUnionHelper.narrow(d);
            var disc = u.get_discriminator(); Out(disc.get_boolean()); Sp(); string n = u.member_name(); Out(n);
          });
    
          // -- sequence and array
          C("seq.count", () => { var d = factory.create_dyn_any_from_type_code(seq_tc); Out(d.component_count()); });
          C("seq.current_component", () => { var d = factory.create_dyn_any_from_type_code(seq_tc); var c = d.current_component(); Out(c == null); });
          C("seq.insert_empty", () => { var d = factory.create_dyn_any_from_type_code(seq_tc); d.insert_long(1); });
          C("seq.grow_position", () => { var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d); s.set_length(3); Out(position(d)); });
          C("seq.grow_keeps_position", () => {
            var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
            s.set_length(2); d.seek(1); d.insert_long(4); s.set_length(4);
            var c = d.current_component(); Out(c.get_long());
          });
          C("seq.grow_from_minus1", () => {
            var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
            s.set_length(2); d.seek(-1); s.set_length(4); var c = d.current_component();
            Out(c == null); Sp(); d.insert_long(8); var e = d.copy(); e.seek(2); Out(e.get_long());
          });
          C("seq.shrink_position", () => {
            var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
            s.set_length(4); d.seek(3); s.set_length(2); var c = d.current_component(); Out(c == null);
          });
          C("seq.shrink_keeps", () => {
            var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
            s.set_length(4); d.seek(1); d.insert_long(6); s.set_length(2); var c = d.current_component(); Out(c.get_long());
          });
          C("seq.shrink_to_0", () => {
            var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
            s.set_length(4); s.set_length(0); var c = d.current_component(); Out(c == null);
          });
          C("seq.bound", () => { var d = factory.create_dyn_any_from_type_code(bseq_tc); var s = DynSequenceHelper.narrow(d); s.set_length(3); });
          C("seq.set_elements_bound", () => {
            var d = factory.create_dyn_any_from_type_code(bseq_tc); var s = DynSequenceHelper.narrow(d);
            var e = new Any[3]; for (int i = 0; i < 3; i++) e[i] = anyLong(i); s.set_elements(Seq(e));
          });
          C("seq.set_elements_type", () => {
            var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
            var e = new Any[1]; e[0] = anyShort(1); s.set_elements(Seq(e));
          });
          C("seq.set_elements_position", () => {
            var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
            var e = new Any[2]; e[0] = anyLong(1); e[1] = anyLong(2); s.set_elements(Seq(e)); Out(position(d));
          });
          C("seq.insert_long_seq", () => {
            var d = factory.create_dyn_any_from_type_code(seq_tc);
            int[] v = new int[2]; v[0] = 1; v[1] = 2; d.insert_long_seq(Seq(v)); Out(d.component_count()); Sp(); Out(position(d));
          });
          C("seq.get_long_seq", () => {
            var d = factory.create_dyn_any_from_type_code(seq_tc); var s = DynSequenceHelper.narrow(d);
            s.set_length(2); var v = d.get_long_seq(); Out(v.Count);
          });
          C("seq.insert_short_seq", () => {
            var d = factory.create_dyn_any_from_type_code(seq_tc);
            short[] v = {1}; d.insert_short_seq(Seq(v));
          });
          C("seq.insert_long_seq_bound", () => {
            var d = factory.create_dyn_any_from_type_code(bseq_tc);
            int[] v = new int[3]; d.insert_long_seq(Seq(v));
          });
          C("struct.insert_long_seq", () => {
            // A struct whose current component is a long: not a sequence.
            var d = factory.create_dyn_any_from_type_code(s_tc);
            int[] v = new int[1]; d.insert_long_seq(Seq(v));
          });
          C("arr.count", () => { var d = factory.create_dyn_any_from_type_code(arr_tc); Out(d.component_count()); Sp(); Out(position(d)); });
          C("arr.set_elements_count", () => {
            var d = factory.create_dyn_any_from_type_code(arr_tc); var a = DynArrayHelper.narrow(d);
            var e = new Any[2]; e[0] = anyLong(1); e[1] = anyLong(1); a.set_elements(Seq(e));
          });
          C("arr.insert_long_seq_count", () => {
            var d = factory.create_dyn_any_from_type_code(arr_tc);
            int[] v = new int[2]; d.insert_long_seq(Seq(v));
          });
    
          // -- valuetypes
          C("value.null", () => { var d = factory.create_dyn_any_from_type_code(v_tc); var v = DynValueHelper.narrow(d); Out(v.is_null()); Sp(); Out(d.component_count()); });
          C("value.set_to_value", () => { var d = factory.create_dyn_any_from_type_code(v_tc); var v = DynValueHelper.narrow(d); v.set_to_value(); Out(v.is_null()); Sp(); Out(d.component_count()); Sp(); Out(position(d)); });
          C("value.get_members_null", () => { var d = factory.create_dyn_any_from_type_code(v_tc); var v = DynValueHelper.narrow(d); var m = v.get_members(); Out(m.Count); });
          C("value.member_name_null", () => { var d = factory.create_dyn_any_from_type_code(v_tc); var v = DynValueHelper.narrow(d); string n = v.current_member_name(); Out(n); });
          C("value.current_component_null", () => { var d = factory.create_dyn_any_from_type_code(v_tc); var c = d.current_component(); Out(c == null); });
          C("value.set_members", () => {
            var d = factory.create_dyn_any_from_type_code(v_tc); var v = DynValueHelper.narrow(d);
            NameValuePair[] m = {new NameValuePair("n", anyLong(3))};
            v.set_members(Seq(m)); Out(v.is_null()); Sp(); var c = d.current_component(); Out(c.get_long());
          });
          C("box.null", () => { var d = factory.create_dyn_any_from_type_code(box_tc); var b = DynValueBoxHelper.narrow(d); Out(b.is_null()); Sp(); Out(d.component_count()); });
          C("box.get_null", () => { var d = factory.create_dyn_any_from_type_code(box_tc); var b = DynValueBoxHelper.narrow(d); Any a = b.get_boxed_value(); });
          C("box.set", () => {
            var d = factory.create_dyn_any_from_type_code(box_tc); var b = DynValueBoxHelper.narrow(d);
            Any a = anyLong(4); b.set_boxed_value(a); Out(b.is_null()); Sp(); Out(d.component_count()); Sp(); Out(position(d));
          });
          C("box.set_wrong", () => {
            var d = factory.create_dyn_any_from_type_code(box_tc); var b = DynValueBoxHelper.narrow(d);
            Any a = anyShort(4); b.set_boxed_value(a);
          });
          C("box.set_to_value", () => { var d = factory.create_dyn_any_from_type_code(box_tc); var b = DynValueBoxHelper.narrow(d); b.set_to_value(); var c = d.current_component(); Out(c.get_long()); });
    
            Console.Out.Write(o.ToString());
            Console.Out.Flush();
            orb.destroy();
            return 0;
        }
    }
}
