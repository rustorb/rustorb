// The stage 2 tests (idl/AnyTest.idl) on the IDL4 to C# mapping: the table
// of values, their comparison, the AnyEcho servant and the client cases.
// The values and cases are those of impls/omniorbpy/anytest.py (and
// impls/java/interop/AnyTests.java).
//
// The servers never use the AnyTestHidden types (they hold its values as
// TypeCodes and anys only), although they are in the same assembly.
//
// Licensed under MIT OR Apache-2.0, like the rest of the rustorb core.

using System;
using System.Linq;
using AnyTest;
using Corba;
using DynamicAny;
using Omg.Types;
using TypeCode = Corba.TypeCode;

namespace Interop
{
    public static class AnyTests
    {
        /// <summary>The ORB's DynAnyFactory (set before the first use).</summary>
        public static IDynAnyFactory Factory;

        public static readonly string[] Names =
        {
            "long", "short", "ulonglong", "float", "double", "longdouble", "boolean", "char", "octet",
            "string", "bounded", "count", "money", "fixed", "point", "shade", "choice_l", "choice_s",
            "choice_default", "longseq", "octets", "shortarray", "node", "nvlist", "any",
            "typecode", "objref", "nil", "oops", "empty",
        };

        private const decimal Money = -123.45m;
        // The other clients and servers send the double 0.1 as a long
        // double: this is that value, to the 28 digits of a decimal.
        private const decimal LongDouble = 0.1000000000000000055511151231m;
        // 31 digits, more than a decimal holds: the value goes in and out of
        // its any through a DynFixed, as a string.
        private const string FixedText = "123456789012345678901234567.8901";

        private static Node MakeNode(string name, params Node[] children) => new Node(name, new Sequence<Node>(children));

        private static Node TheNode() => MakeNode("root", MakeNode("a"), MakeNode("b", MakeNode("c")));

        private static Choice MakeChoice(string name)
        {
            var c = new Choice();
            switch (name)
            {
                case "choice_l": c.l = 10; break;
                case "choice_s": c.Sets("three", 3); break;
                default: c.Setb(true, 9); break;
            }
            return c;
        }

        private static ISequence<NameValue> TheNvlist()
        {
            var a = new Any();
            a.insert_long(1);
            var b = new Any();
            b.insert_string("x");
            var p = new Any();
            PointHelper.insert(p, new AnyTest.Point(3, -0.5));
            return new Sequence<NameValue> { new NameValue("a", a), new NameValue("b", b), new NameValue("p", p) };
        }

        /// <summary>The value named name of the table in AnyTest.idl; self is
        /// the AnyEcho object (for "objref").</summary>
        public static Any Make(string name, IAnyEcho self)
        {
            var a = new Any();
            switch (name)
            {
                case "long": a.insert_long(42); break;
                case "short": a.insert_short(-7); break;
                case "ulonglong": a.insert_ulonglong(ulong.MaxValue); break;
                case "float": a.insert_float(0.25f); break;
                case "double": a.insert_double(1.5); break;
                case "longdouble": a.insert_longdouble(LongDouble); break;
                case "boolean": a.insert_boolean(true); break;
                case "char": a.insert_char('x'); break;
                case "octet": a.insert_octet(255); break;
                case "string": a.insert_string("hello any"); break;
                case "bounded": Short5Helper.insert(a, "abc"); break;
                case "count": CountHelper.insert(a, 7); break;
                case "money": MoneyHelper.insert(a, Money); break;
                case "fixed":
                    {
                        var d = (IDynFixed)Factory.create_dyn_any_from_type_code(Rustorb.TypeCodes.Fixed(31, 4));
                        d.set_value(FixedText);
                        a = d.to_any();
                        d.destroy();
                        break;
                    }
                case "point": PointHelper.insert(a, new AnyTest.Point(1, 2.5)); break;
                case "shade": ShadeHelper.insert(a, Shade.DIM); break;
                case "choice_l":
                case "choice_s":
                case "choice_default":
                    ChoiceHelper.insert(a, MakeChoice(name));
                    break;
                case "longseq": LongSeqHelper.insert(a, new Sequence<int> { 1, 2, 3 }); break;
                case "octets": OctetsHelper.insert(a, new Sequence<byte> { 0, 1, 255 }); break;
                case "shortarray": ShortArrayHelper.insert(a, new short[] { 1, -2, 3 }); break;
                case "node": NodeHelper.insert(a, TheNode()); break;
                case "nvlist": NVListHelper.insert(a, TheNvlist()); break;
                case "any":
                    {
                        var inner = new Any();
                        inner.insert_long(5);
                        a.insert_any(inner);
                        break;
                    }
                case "typecode": a.insert_TypeCode(NodeHelper.type()); break;
                case "objref": AnyEchoHelper.insert(a, self); break;
                case "nil": AnyEchoHelper.insert(a, null); break;
                case "oops": OopsHelper.insert(a, new Oops("bad", 3)); break;
                case "empty": break;
                default:
                    if (!Hidden.Make(a, name))
                    {
                        throw new BAD_PARAM("no such value: " + name, 0, CompletionStatus.COMPLETED_NO);
                    }
                    break;
            }
            return a;
        }

        private static bool SameNodes(Node a, Node b) =>
            a.name == b.name && a.children.Count == b.children.Count &&
            a.children.Zip(b.children, SameNodes).All(x => x);

        private static bool SameChoice(Choice a, Choice b)
        {
            if (a.Discriminator != b.Discriminator)
            {
                return false;
            }
            switch (a.Discriminator)
            {
                case 1: return a.l == b.l;
                case 2:
                case 3: return a.s == b.s;
                default: return a.b == b.b;
            }
        }

        private static bool IsLong(Any a, int v) =>
            a.type.equal(Rustorb.TypeCodes.Simple(TCKind.tk_long)) && a.extract_long() == v;

        /// <summary>Whether got is the value named name: an equal TypeCode and
        /// the same value.</summary>
        public static bool Same(string name, Any got, IAnyEcho self)
        {
            Any want = Make(name, self);
            if (!got.type.equal(want.type))
            {
                return false;
            }
            switch (name)
            {
                case "long": return got.extract_long() == 42;
                case "count": return CountHelper.extract(got) == 7;
                case "short": return got.extract_short() == -7;
                case "ulonglong": return got.extract_ulonglong() == ulong.MaxValue;
                case "float": return got.extract_float() == 0.25f;
                case "double": return got.extract_double() == 1.5;
                case "longdouble": return got.extract_longdouble() == LongDouble;
                case "boolean": return got.extract_boolean();
                case "char": return got.extract_char() == 'x';
                case "octet": return got.extract_octet() == 255;
                case "string": return got.extract_string() == "hello any";
                case "bounded": return Short5Helper.extract(got) == "abc";
                case "money": return MoneyHelper.extract(got) == Money;
                case "fixed":
                    {
                        IDynAny d = Factory.create_dyn_any(got);
                        string v = ((IDynFixed)d).get_value();
                        d.destroy();
                        return v == FixedText;
                    }
                case "point":
                    {
                        AnyTest.Point p = PointHelper.extract(got);
                        return p.x == 1 && p.y == 2.5;
                    }
                case "shade": return ShadeHelper.extract(got) == Shade.DIM;
                case "choice_l":
                case "choice_s":
                case "choice_default":
                    return SameChoice(ChoiceHelper.extract(got), MakeChoice(name));
                case "longseq": return LongSeqHelper.extract(got).SequenceEqual(new[] { 1, 2, 3 });
                case "octets": return OctetsHelper.extract(got).SequenceEqual(new byte[] { 0, 1, 255 });
                case "shortarray": return ShortArrayHelper.extract(got).SequenceEqual(new short[] { 1, -2, 3 });
                case "node": return SameNodes(NodeHelper.extract(got), TheNode());
                case "nvlist":
                    {
                        ISequence<NameValue> l = NVListHelper.extract(got);
                        if (l.Count != 3)
                        {
                            return false;
                        }
                        AnyTest.Point p = l[2].value.type.equal(PointHelper.type()) ? PointHelper.extract(l[2].value) : null;
                        return l[0].name == "a" && IsLong(l[0].value, 1)
                            && l[1].name == "b"
                            && l[1].value.type.equal(Rustorb.TypeCodes.Simple(TCKind.tk_string))
                            && l[1].value.extract_string() == "x"
                            && l[2].name == "p" && p != null && p.x == 3 && p.y == -0.5;
                    }
                case "any": return IsLong(got.extract_any(), 5);
                case "typecode":
                    {
                        TypeCode t = got.extract_TypeCode();
                        return t != null && t.equal(NodeHelper.type());
                    }
                case "objref":
                    {
                        IAnyEcho v = AnyEchoHelper.extract(got);
                        return v != null && v._is_equivalent(self);
                    }
                case "nil": return AnyEchoHelper.extract(got) == null;
                case "oops":
                    {
                        Oops o = OopsHelper.extract(got);
                        return o.why == "bad" && o.code == 3;
                    }
                case "empty": return got.type.kind() == TCKind.tk_null;
                default: return Hidden.Same(got, name);
            }
        }

        private static readonly string[] Kinds =
        {
            "null", "void", "short", "long", "ushort", "ulong", "float", "double", "boolean",
            "char", "octet", "any", "TypeCode", "Principal", "objref", "struct", "union",
            "enum", "string", "sequence", "array", "alias", "except", "longlong",
            "ulonglong", "longdouble", "wchar", "wstring", "fixed", "value", "value_box",
            "native", "abstract_interface", "local_interface",
        };

        /// <summary>The servant of the servers.</summary>
        public sealed class AnyEchoImpl : AnyEchoPOA
        {
            /// <summary>The reference of this object: _this, or that of the
            /// DSI servant that calls this one (DiiAdapters.cs).</summary>
            public Func<IAnyEcho> Self { get; set; }

            private IAnyEcho Me() => Self != null ? Self() : AnyEchoHelper.narrow(_this_object());

            public override Any echo(Any a) => a;

            public override Any make(string name) => Make(name, Me());

            public override bool check(string name, Any a) => Same(name, a, Me());

            public override void out_inout(string name, out Any b, ref Any c)
            {
                if (!Same(name, c, Me()))
                {
                    throw new BAD_PARAM("", 0, CompletionStatus.COMPLETED_NO);
                }
                b = Make(name, Me());
                c = Make("long", Me());
            }

            public override TypeCode echo_tc(TypeCode tc) => tc;

            public override TypeCode make_tc(string name) => Make(name, Me()).type;

            public override bool check_tc(string name, TypeCode tc) => Make(name, Me()).type.equal(tc);

            public override ISequence<NameValue> echo_nvlist(ISequence<NameValue> l) => l;

            public override string describe(Any a)
            {
                TypeCode tc = a.type;
                string id = "";
                try
                {
                    id = tc.id();
                }
                catch (TypeCode.BadKind)
                {
                    // no repository id
                }
                int k = (int)tc.kind();
                return (k < Kinds.Length ? Kinds[k] : "?") + " " + id;
            }
        }

        private static void Check(bool cond, string what)
        {
            if (!cond)
            {
                throw new CheckFailed(what);
            }
        }

        /// <summary>Runs the stage 2 cases.</summary>
        public static void Run(Runner r, IAnyEcho echo)
        {
            foreach (string n in Names)
            {
                r.Run("make[" + n + "]", () => Check(Same(n, echo.make(n), echo), "wrong value"));
                r.Run("check[" + n + "]", () => Check(echo.check(n, Make(n, echo)), "server says no"));
                r.Run("echo[" + n + "]", () => Check(Same(n, echo.echo(Make(n, echo)), echo), "wrong value"));
            }
            foreach (string n in Names)
            {
                r.Run("make_tc[" + n + "]", () => Check(echo.make_tc(n).equal(Make(n, echo).type), "wrong TypeCode"));
                r.Run("check_tc[" + n + "]", () => Check(echo.check_tc(n, Make(n, echo).type), "server says no"));
                r.Run("echo_tc[" + n + "]", () =>
                {
                    TypeCode t = Make(n, echo).type;
                    Check(echo.echo_tc(t).equal(t), "wrong TypeCode");
                });
            }
            foreach (string n in new[] { "point", "nvlist", "node" })
            {
                r.Run("out_inout[" + n + "]", () =>
                {
                    Any c = Make(n, echo);
                    echo.out_inout(n, out Any b, ref c);
                    Check(Same(n, b, echo), "wrong out value");
                    Check(Same("long", c, echo), "wrong inout value");
                });
            }
            r.Run("echo_nvlist", () =>
            {
                var a = new Any();
                NVListHelper.insert(a, echo.echo_nvlist(TheNvlist()));
                Check(Same("nvlist", a, echo), "wrong value");
            });
            string[][] hidden =
            {
                new[] { "secret", "struct IDL:AnyTestHidden/Secret:1.0" },
                new[] { "pick", "union IDL:AnyTestHidden/Pick:1.0" },
                new[] { "moods", "alias IDL:AnyTestHidden/Moods:1.0" },
            };
            foreach (string[] h in hidden)
            {
                r.Run("echo[" + h[0] + "]", () => Check(Same(h[0], echo.echo(Make(h[0], echo)), echo), "wrong value"));
                r.Run("describe[" + h[0] + "]", () =>
                {
                    string d = echo.describe(Make(h[0], echo));
                    Check(d == h[1], d);
                });
            }
            // The values taken apart and built again with DynAny, from their
            // TypeCodes alone (docs/dynany.md).
            foreach (string n in Names)
            {
                r.Run("dyn_check[" + n + "]", () => Check(echo.check(n, DynRebuild(Make(n, echo))), "server says no"));
                r.Run("dyn_make[" + n + "]", () => Check(Same(n, DynRebuild(echo.make(n)), echo), "wrong value"));
            }
            foreach (string[] h in hidden)
            {
                r.Run("dyn_echo[" + h[0] + "]", () =>
                    Check(Same(h[0], echo.echo(DynRebuild(Make(h[0], echo))), echo), "wrong value"));
            }
            r.Run("describe[long]", () =>
            {
                string d = echo.describe(Make("long", echo));
                Check(d == "long ", d);
            });
            r.Run("describe[node]", () =>
            {
                string d = echo.describe(Make("node", echo));
                Check(d == "struct IDL:AnyTest/Node:1.0", d);
            });
            r.Run("check_wrong_type", () =>
            {
                var a = new Any();
                a.insert_short(42);
                Check(!echo.check("long", a), "server says yes");
            });
            r.Run("check_unaliased", () =>
            {
                var a = new Any();
                a.insert_long(7);
                Check(!echo.check("count", a), "server says yes");
            });
            r.Run("check_tc_unaliased", () =>
                Check(!echo.check_tc("count", Rustorb.TypeCodes.Simple(TCKind.tk_long)), "server says yes"));
            r.Run("out_inout_wrong", () =>
            {
                Any c = Make("long", echo);
                try
                {
                    echo.out_inout("point", out _, ref c);
                }
                catch (BAD_PARAM)
                {
                    return;
                }
                Check(false, "no BAD_PARAM");
            });
        }

        /// <summary>value taken apart with a DynAny and built again in a DynAny
        /// made from its TypeCode alone, component by component.</summary>
        private static Any DynRebuild(Any value)
        {
            IDynAnyFactory factory = Factory;
            IDynAny src = factory.create_dyn_any(value);
            IDynAny dst = factory.create_dyn_any_from_type_code(value.type);
            DynCopy(src, dst);
            Any rebuilt = dst.to_any();
            src.destroy();
            dst.destroy();
            return rebuilt;
        }

        private static void DynCopy(IDynAny src, IDynAny dst)
        {
            TypeCode tc = src.type().unaliased();
            switch (tc.kind())
            {
                case TCKind.tk_struct:
                case TCKind.tk_except:
                case TCKind.tk_sequence:
                case TCKind.tk_array:
                case TCKind.tk_value:
                case TCKind.tk_value_box:
                    if (src is IDynSequence ss)
                    {
                        ((IDynSequence)dst).set_length(ss.get_length());
                    }
                    else if (src is IDynValueCommon sv)
                    {
                        if (sv.is_null())
                        {
                            ((IDynValueCommon)dst).set_to_null();
                            return;
                        }
                        ((IDynValueCommon)dst).set_to_value();
                    }
                    for (int i = 0; i < src.component_count(); i++)
                    {
                        src.seek(i);
                        dst.seek(i);
                        DynCopy(src.current_component(), dst.current_component());
                    }
                    return;
                case TCKind.tk_union:
                    {
                        var s = (IDynUnion)src;
                        var d = (IDynUnion)dst;
                        d.set_discriminator(s.get_discriminator());
                        if (!s.has_no_active_member())
                        {
                            DynCopy(s.member(), d.member());
                        }
                        return;
                    }
                case TCKind.tk_enum:
                    ((IDynEnum)dst).set_as_ulong(((IDynEnum)src).get_as_ulong());
                    return;
                case TCKind.tk_fixed:
                    ((IDynFixed)dst).set_value(((IDynFixed)src).get_value());
                    return;
                case TCKind.tk_any: dst.insert_any(DynRebuild(src.get_any())); return;
                case TCKind.tk_long: dst.insert_long(src.get_long()); return;
                case TCKind.tk_short: dst.insert_short(src.get_short()); return;
                case TCKind.tk_ulong: dst.insert_ulong(src.get_ulong()); return;
                case TCKind.tk_ushort: dst.insert_ushort(src.get_ushort()); return;
                case TCKind.tk_longlong: dst.insert_longlong(src.get_longlong()); return;
                case TCKind.tk_ulonglong: dst.insert_ulonglong(src.get_ulonglong()); return;
                case TCKind.tk_float: dst.insert_float(src.get_float()); return;
                case TCKind.tk_double: dst.insert_double(src.get_double()); return;
                case TCKind.tk_boolean: dst.insert_boolean(src.get_boolean()); return;
                case TCKind.tk_char: dst.insert_char(src.get_char()); return;
                case TCKind.tk_wchar: dst.insert_wchar(src.get_wchar()); return;
                case TCKind.tk_octet: dst.insert_octet(src.get_octet()); return;
                case TCKind.tk_string: dst.insert_string(src.get_string()); return;
                case TCKind.tk_wstring: dst.insert_wstring(src.get_wstring()); return;
                case TCKind.tk_TypeCode: dst.insert_typecode(src.get_typecode()); return;
                case TCKind.tk_objref: dst.insert_reference(src.get_reference()); return;
                default:
                    // tk_null, tk_void and long double (whose DynAny value is
                    // a decimal, as in the Any)
                    dst.from_any(src.to_any());
                    return;
            }
        }
    }

    /// <summary>The values of AnyTestHidden.idl, which only the clients use.</summary>
    internal static class Hidden
    {
        private static AnyTestHidden.Secret Secret() =>
            new("s", new Sequence<AnyTest.Point> { new AnyTest.Point(1, 2.0) }, Shade.DARK);

        private static AnyTestHidden.Pick Pick()
        {
            var p = new AnyTestHidden.Pick();
            p.s = new AnyTestHidden.Secret("p", new Sequence<AnyTest.Point>(), Shade.LIGHT);
            return p;
        }

        public static bool Make(Any a, string name)
        {
            switch (name)
            {
                case "secret": AnyTestHidden.SecretHelper.insert(a, Secret()); return true;
                case "pick": AnyTestHidden.PickHelper.insert(a, Pick()); return true;
                case "moods":
                    AnyTestHidden.MoodsHelper.insert(a,
                        new Sequence<AnyTestHidden.Mood> { AnyTestHidden.Mood.SAD, AnyTestHidden.Mood.HAPPY });
                    return true;
                default: return false;
            }
        }

        private static bool SameSecret(AnyTestHidden.Secret a, AnyTestHidden.Secret b) =>
            a.text == b.text && a.shade == b.shade && a.points.Count == b.points.Count &&
            a.points.Zip(b.points, (x, y) => x.x == y.x && x.y == y.y).All(v => v);

        public static bool Same(Any got, string name)
        {
            switch (name)
            {
                case "secret": return SameSecret(AnyTestHidden.SecretHelper.extract(got), Secret());
                case "pick":
                    {
                        AnyTestHidden.Pick p = AnyTestHidden.PickHelper.extract(got);
                        return p.Discriminator == 'b' && SameSecret(p.s, Pick().s);
                    }
                case "moods":
                    {
                        ISequence<AnyTestHidden.Mood> m = AnyTestHidden.MoodsHelper.extract(got);
                        return m.Count == 2 && m[0] == AnyTestHidden.Mood.SAD && m[1] == AnyTestHidden.Mood.HAPPY;
                    }
                default: return false;
            }
        }
    }
}
