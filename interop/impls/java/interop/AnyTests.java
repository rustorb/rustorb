// The stage 2 tests (idl/AnyTest.idl) on the standard IDL to Java mapping:
// the table of values, their comparison, the AnyEcho servant and the client
// cases. The values and cases are those of impls/omniorbpy/anytest.py.
//
// The servers never use the AnyTestHidden classes (they hold its values as
// TypeCodes and anys only), although they are on the same class path.

package interop;

import java.util.Arrays;
import java.util.List;

import AnyTest.AnyEcho;
import AnyTest.AnyEchoHelper;
import AnyTest.AnyEchoPOA;
import AnyTest.Choice;
import AnyTest.ChoiceHelper;
import AnyTest.CountHelper;
import AnyTest.LongSeqHelper;
import AnyTest.NVListHelper;
import AnyTest.NameValue;
import AnyTest.Node;
import AnyTest.NodeHelper;
import AnyTest.OctetsHelper;
import AnyTest.Oops;
import AnyTest.OopsHelper;
import AnyTest.Point;
import AnyTest.PointHelper;
import AnyTest.Shade;
import AnyTest.ShadeHelper;
import AnyTest.ShortArrayHelper;
import AnyTest.Short5Helper;
import org.omg.CORBA.Any;
import org.omg.CORBA.AnyHolder;
import org.omg.CORBA.BAD_PARAM;
import org.omg.CORBA.CompletionStatus;
import org.omg.CORBA.ORB;
import org.omg.CORBA.TCKind;
import org.omg.CORBA.TypeCode;
import org.omg.CORBA.TypeCodePackage.BadKind;

public final class AnyTests {
    private AnyTests() {}

    static final List<String> NAMES = Arrays.asList(
            "long", "short", "ulonglong", "float", "double", "boolean", "char", "octet",
            "string", "bounded", "count", "point", "shade", "choice_l", "choice_s",
            "choice_default", "longseq", "octets", "shortarray", "node", "nvlist", "any",
            "typecode", "objref", "nil", "oops", "empty");

    static Node node(String name, Node... children) {
        return new Node(name, children);
    }

    static Choice choice(String name) {
        Choice c = new Choice();
        if (name.equals("choice_l")) {
            c.l(10);
        } else if (name.equals("choice_s")) {
            c.s(3, "three");
        } else {
            c.b(9, true);
        }
        return c;
    }

    static Node theNode() {
        return node("root", node("a"), node("b", node("c")));
    }

    static NameValue[] theNvlist(ORB orb) {
        Any a = orb.create_any();
        a.insert_long(1);
        Any b = orb.create_any();
        b.insert_string("x");
        Any p = orb.create_any();
        PointHelper.insert(p, new Point(3, -0.5));
        return new NameValue[] {new NameValue("a", a), new NameValue("b", b), new NameValue("p", p)};
    }

    /** The value named name of the table in AnyTest.idl; self is the
     * AnyEcho object (for "objref"). */
    static Any make(ORB orb, String name, AnyEcho self) {
        Any a = orb.create_any();
        switch (name) {
            case "long": a.insert_long(42); break;
            case "short": a.insert_short((short) -7); break;
            case "ulonglong": a.insert_ulonglong(-1L); break;
            case "float": a.insert_float(0.25f); break;
            case "double": a.insert_double(1.5); break;
            case "boolean": a.insert_boolean(true); break;
            case "char": a.insert_char('x'); break;
            case "octet": a.insert_octet((byte) 255); break;
            case "string": a.insert_string("hello any"); break;
            case "bounded": Short5Helper.insert(a, "abc"); break;
            case "count": CountHelper.insert(a, 7); break;
            case "point": PointHelper.insert(a, new Point(1, 2.5)); break;
            case "shade": ShadeHelper.insert(a, Shade.DIM); break;
            case "choice_l":
            case "choice_s":
            case "choice_default":
                ChoiceHelper.insert(a, choice(name));
                break;
            case "longseq": LongSeqHelper.insert(a, new int[] {1, 2, 3}); break;
            case "octets": OctetsHelper.insert(a, new byte[] {0, 1, (byte) 255}); break;
            case "shortarray": ShortArrayHelper.insert(a, new short[] {1, -2, 3}); break;
            case "node": NodeHelper.insert(a, theNode()); break;
            case "nvlist": NVListHelper.insert(a, theNvlist(orb)); break;
            case "any": {
                Any inner = orb.create_any();
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
                if (!Hidden.make(a, name)) {
                    throw new BAD_PARAM(0, CompletionStatus.COMPLETED_NO);
                }
                break;
        }
        return a;
    }

    static boolean sameNodes(Node a, Node b) {
        if (!a.name.equals(b.name) || a.children.length != b.children.length) {
            return false;
        }
        for (int i = 0; i < a.children.length; ++i) {
            if (!sameNodes(a.children[i], b.children[i])) {
                return false;
            }
        }
        return true;
    }

    static boolean sameChoice(Choice a, Choice b) {
        if (a.discriminator() != b.discriminator()) {
            return false;
        }
        switch (a.discriminator()) {
            case 1: return a.l() == b.l();
            case 2:
            case 3: return a.s().equals(b.s());
            default: return a.b() == b.b();
        }
    }

    static boolean isLong(Any a, int v) {
        return a.type().equal(ORB.init().get_primitive_tc(TCKind.tk_long)) && a.extract_long() == v;
    }

    /** Whether got is the value named name: an equal TypeCode and the same
     * value. */
    static boolean same(ORB orb, String name, Any got, AnyEcho self) {
        Any want = make(orb, name, self);
        if (!got.type().equal(want.type())) {
            return false;
        }
        switch (name) {
            case "long": return got.extract_long() == 42;
            case "count": return CountHelper.extract(got) == 7;
            case "short": return got.extract_short() == -7;
            case "ulonglong": return got.extract_ulonglong() == -1L;
            case "float": return got.extract_float() == 0.25f;
            case "double": return got.extract_double() == 1.5;
            case "boolean": return got.extract_boolean();
            case "char": return got.extract_char() == 'x';
            case "octet": return got.extract_octet() == (byte) 255;
            case "string": return got.extract_string().equals("hello any");
            case "bounded": return Short5Helper.extract(got).equals("abc");
            case "point": {
                Point p = PointHelper.extract(got);
                return p.x == 1 && p.y == 2.5;
            }
            case "shade": return ShadeHelper.extract(got).value() == Shade._DIM;
            case "choice_l":
            case "choice_s":
            case "choice_default":
                return sameChoice(ChoiceHelper.extract(got), choice(name));
            case "longseq": return Arrays.equals(LongSeqHelper.extract(got), new int[] {1, 2, 3});
            case "octets": return Arrays.equals(OctetsHelper.extract(got), new byte[] {0, 1, (byte) 255});
            case "shortarray": return Arrays.equals(ShortArrayHelper.extract(got), new short[] {1, -2, 3});
            case "node": return sameNodes(NodeHelper.extract(got), theNode());
            case "nvlist": {
                NameValue[] l = NVListHelper.extract(got);
                if (l.length != 3) {
                    return false;
                }
                Point p = null;
                if (l[2].value.type().equal(PointHelper.type())) {
                    p = PointHelper.extract(l[2].value);
                }
                return l[0].name.equals("a") && isLong(l[0].value, 1)
                        && l[1].name.equals("b")
                        && l[1].value.type().equal(ORB.init().get_primitive_tc(TCKind.tk_string))
                        && l[1].value.extract_string().equals("x")
                        && l[2].name.equals("p") && p != null && p.x == 3 && p.y == -0.5;
            }
            case "any": return isLong(got.extract_any(), 5);
            case "typecode": {
                TypeCode t = got.extract_TypeCode();
                return t != null && t.equal(NodeHelper.type());
            }
            case "objref": {
                AnyEcho v = AnyEchoHelper.extract(got);
                return v != null && v._is_equivalent(self);
            }
            case "nil": return AnyEchoHelper.extract(got) == null;
            case "oops": {
                Oops o = OopsHelper.extract(got);
                return o.why.equals("bad") && o.code == 3;
            }
            case "empty": return got.type().kind().value() == TCKind._tk_null;
            default: return Hidden.same(got, name);
        }
    }

    static final String[] KINDS = {
        "null", "void", "short", "long", "ushort", "ulong", "float", "double", "boolean",
        "char", "octet", "any", "TypeCode", "Principal", "objref", "struct", "union",
        "enum", "string", "sequence", "array", "alias", "except", "longlong",
        "ulonglong", "longdouble", "wchar", "wstring", "fixed", "value", "value_box",
        "native", "abstract_interface", "local_interface"};

    /** The servant of the servers. */
    public static final class AnyEchoImpl extends AnyEchoPOA {
        private final ORB orb;

        public AnyEchoImpl(ORB orb) {
            this.orb = orb;
        }

        public Any echo(Any a) { return a; }

        public Any make(String name) { return AnyTests.make(orb, name, _this()); }

        public boolean check(String name, Any a) { return same(orb, name, a, _this()); }

        public void out_inout(String name, AnyHolder b, AnyHolder c) {
            if (!same(orb, name, c.value, _this())) {
                throw new BAD_PARAM(0, CompletionStatus.COMPLETED_NO);
            }
            b.value = AnyTests.make(orb, name, _this());
            c.value = AnyTests.make(orb, "long", _this());
        }

        public TypeCode echo_tc(TypeCode tc) { return tc; }

        public TypeCode make_tc(String name) { return AnyTests.make(orb, name, _this()).type(); }

        public boolean check_tc(String name, TypeCode tc) {
            return AnyTests.make(orb, name, _this()).type().equal(tc);
        }

        public NameValue[] echo_nvlist(NameValue[] l) { return l; }

        public String describe(Any a) {
            TypeCode tc = a.type();
            String id = "";
            try {
                id = tc.id();
            } catch (BadKind e) {
                // no repository id
            }
            int k = tc.kind().value();
            return (k < KINDS.length ? KINDS[k] : "?") + " " + id;
        }
    }

    /** Runs the stage 2 cases. */
    public static void run(Cases.Runner r, ORB orb, AnyEcho echo) {
        for (String n : NAMES) {
            r.run("make[" + n + "]", () -> Cases.check(same(orb, n, echo.make(n), echo), "wrong value"));
            r.run("check[" + n + "]", () -> Cases.check(echo.check(n, make(orb, n, echo)), "server says no"));
            r.run("echo[" + n + "]",
                    () -> Cases.check(same(orb, n, echo.echo(make(orb, n, echo)), echo), "wrong value"));
        }
        for (String n : NAMES) {
            r.run("make_tc[" + n + "]",
                    () -> Cases.check(echo.make_tc(n).equal(make(orb, n, echo).type()), "wrong TypeCode"));
            r.run("check_tc[" + n + "]",
                    () -> Cases.check(echo.check_tc(n, make(orb, n, echo).type()), "server says no"));
            r.run("echo_tc[" + n + "]", () -> {
                TypeCode t = make(orb, n, echo).type();
                Cases.check(echo.echo_tc(t).equal(t), "wrong TypeCode");
            });
        }
        for (String n : new String[] {"point", "nvlist", "node"}) {
            r.run("out_inout[" + n + "]", () -> {
                AnyHolder b = new AnyHolder();
                AnyHolder c = new AnyHolder(make(orb, n, echo));
                echo.out_inout(n, b, c);
                Cases.check(same(orb, n, b.value, echo), "wrong out value");
                Cases.check(same(orb, "long", c.value, echo), "wrong inout value");
            });
        }
        r.run("echo_nvlist", () -> {
            Any a = orb.create_any();
            NVListHelper.insert(a, echo.echo_nvlist(theNvlist(orb)));
            Cases.check(same(orb, "nvlist", a, echo), "wrong value");
        });
        String[][] hidden = {
            {"secret", "struct IDL:AnyTestHidden/Secret:1.0"},
            {"pick", "union IDL:AnyTestHidden/Pick:1.0"},
            {"moods", "alias IDL:AnyTestHidden/Moods:1.0"},
        };
        for (String[] h : hidden) {
            r.run("echo[" + h[0] + "]",
                    () -> Cases.check(same(orb, h[0], echo.echo(make(orb, h[0], echo)), echo), "wrong value"));
            r.run("describe[" + h[0] + "]", () -> {
                String d = echo.describe(make(orb, h[0], echo));
                Cases.check(d.equals(h[1]), d);
            });
        }
        r.run("describe[long]", () -> {
            String d = echo.describe(make(orb, "long", echo));
            Cases.check(d.equals("long "), d);
        });
        r.run("describe[node]", () -> {
            String d = echo.describe(make(orb, "node", echo));
            Cases.check(d.equals("struct IDL:AnyTest/Node:1.0"), d);
        });
        r.run("check_wrong_type", () -> {
            Any a = orb.create_any();
            a.insert_short((short) 42);
            Cases.check(!echo.check("long", a), "server says yes");
        });
        r.run("check_unaliased", () -> {
            Any a = orb.create_any();
            a.insert_long(7);
            Cases.check(!echo.check("count", a), "server says yes");
        });
        r.run("check_tc_unaliased", () -> Cases.check(
                !echo.check_tc("count", orb.get_primitive_tc(TCKind.tk_long)), "server says yes"));
        r.run("out_inout_wrong", () -> {
            AnyHolder b = new AnyHolder();
            AnyHolder c = new AnyHolder(make(orb, "long", echo));
            try {
                echo.out_inout("point", b, c);
            } catch (BAD_PARAM e) {
                return;
            }
            Cases.check(false, "no BAD_PARAM");
        });
    }
}
