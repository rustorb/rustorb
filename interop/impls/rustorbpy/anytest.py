"""The stage 2 tests (idl/AnyTest.idl): the AnyEcho servant, the table of
values and the client cases.

The same file is in impls/omniorbpy/ and impls/rustorbpy/: it uses only
the standard IDL to Python mapping, and gets CORBA from common.py.
"""

from common import CORBA
import AnyTest
import AnyTest__POA
import AnyTestHidden

#: The names of the table of AnyTest.idl, in the order the client runs them.
NAMES = [
    "long", "short", "ulonglong", "float", "double", "boolean", "char", "octet",
    "string", "bounded", "count", "point", "shade", "choice_l", "choice_s",
    "choice_default", "longseq", "octets", "shortarray", "node", "nvlist", "any",
    "typecode", "objref", "nil", "oops", "empty",
]

#: Values of the types only the clients know (AnyTestHidden.idl), with what
#: AnyEcho::describe answers for them.
HIDDEN = ["secret", "pick", "moods"]
HIDDEN_DESCRIPTION = {
    "secret": "struct IDL:AnyTestHidden/Secret:1.0",
    "pick": "union IDL:AnyTestHidden/Pick:1.0",
    "moods": "alias IDL:AnyTestHidden/Moods:1.0",
}


def node(name, *children):
    return AnyTest.Node(name, list(children))


def value(name, echo):
    """The value named ``name`` as a CORBA.Any. ``echo`` is the AnyEcho
    object (for "objref")."""
    A = AnyTest
    tc = {
        "long": (CORBA.TC_long, lambda: 42),
        "short": (CORBA.TC_short, lambda: -7),
        "ulonglong": (CORBA.TC_ulonglong, lambda: 2**64 - 1),
        "float": (CORBA.TC_float, lambda: 0.25),
        "double": (CORBA.TC_double, lambda: 1.5),
        "boolean": (CORBA.TC_boolean, lambda: True),
        "char": (CORBA.TC_char, lambda: "x"),
        "octet": (CORBA.TC_octet, lambda: 255),
        "string": (CORBA.TC_string, lambda: "hello any"),
        "bounded": (A._tc_Short5, lambda: "abc"),
        "count": (A._tc_Count, lambda: 7),
        "point": (A._tc_Point, lambda: A.Point(1, 2.5)),
        "shade": (A._tc_Shade, lambda: A.DIM),
        "choice_l": (A._tc_Choice, lambda: A.Choice(1, 10)),
        "choice_s": (A._tc_Choice, lambda: A.Choice(3, "three")),
        "choice_default": (A._tc_Choice, lambda: A.Choice(9, True)),
        "longseq": (A._tc_LongSeq, lambda: [1, 2, 3]),
        "octets": (A._tc_Octets, lambda: b"\x00\x01\xff"),
        "shortarray": (A._tc_ShortArray, lambda: [1, -2, 3]),
        "node": (A._tc_Node, lambda: node("root", node("a"), node("b", node("c")))),
        "nvlist": (A._tc_NVList, lambda: [
            A.NameValue("a", CORBA.Any(CORBA.TC_long, 1)),
            A.NameValue("b", CORBA.Any(CORBA.TC_string, "x")),
            A.NameValue("p", CORBA.Any(A._tc_Point, A.Point(3, -0.5))),
        ]),
        "any": (CORBA.TC_any, lambda: CORBA.Any(CORBA.TC_long, 5)),
        "typecode": (CORBA.TC_TypeCode, lambda: A._tc_Node),
        "objref": (A._tc_AnyEcho, lambda: echo),
        "nil": (A._tc_AnyEcho, lambda: None),
        "oops": (A._tc_Oops, lambda: A.Oops("bad", 3)),
        "empty": (CORBA.TC_null, lambda: None),
    }
    H = AnyTestHidden
    tc.update({
        "secret": (H._tc_Secret, lambda: H.Secret("s", [A.Point(1, 2.0)], A.DARK)),
        "pick": (H._tc_Pick, lambda: H.Pick("b", H.Secret("p", [], A.LIGHT))),
        "moods": (H._tc_Moods, lambda: [H.SAD, H.HAPPY]),
    })
    t, v = tc[name]
    return CORBA.Any(t, v())


def same(tc, a, b):
    """Whether a and b are the same value of type tc, following the
    TypeCode (generated classes need not compare by value). omniORBpy's
    member_count() returns a float, hence the int()."""
    k = tc.kind()
    if k == CORBA.tk_alias:
        return same(tc.content_type(), a, b)
    if k in (CORBA.tk_null, CORBA.tk_void):
        return a is None and b is None
    if k in (CORBA.tk_struct, CORBA.tk_except):
        return type(a) is type(b) and all(
            same(tc.member_type(i), getattr(a, attr(tc.member_name(i))),
                 getattr(b, attr(tc.member_name(i))))
            for i in range(int(tc.member_count())))
    if k == CORBA.tk_union:
        if type(a) is not type(b) or a._d != b._d:
            return False
        for i in range(int(tc.member_count())):
            if i != tc.default_index() and tc.member_label(i).value() == a._d:
                return same(tc.member_type(i), a._v, b._v)
        if tc.default_index() >= 0:
            return same(tc.member_type(tc.default_index()), a._v, b._v)
        return True
    if k in (CORBA.tk_sequence, CORBA.tk_array):
        content = tc.content_type()
        if isinstance(a, (bytes, str)) or isinstance(b, (bytes, str)):
            return a == b
        return len(a) == len(b) and all(same(content, x, y) for x, y in zip(a, b))
    if k == CORBA.tk_any:
        return (a.typecode().equal(b.typecode())
                and same(a.typecode(), a.value(), b.value()))
    if k == CORBA.tk_TypeCode:
        return a.equal(b)
    if k == CORBA.tk_objref:
        if a is None or b is None:
            return a is None and b is None
        return a._is_equivalent(b)
    return a == b


def attr(name):
    import keyword
    return "_" + name if keyword.iskeyword(name) else name


def same_any(a, b):
    return a.typecode().equal(b.typecode()) and same(a.typecode(), a.value(), b.value())


class AnyEchoServant(AnyTest__POA.AnyEcho):

    def echo(self, a):
        return a

    def make(self, name):
        return value(name, self._this())

    def check(self, name, a):
        return same_any(value(name, self._this()), a)

    def out_inout(self, name, c):
        if not same_any(value(name, self._this()), c):
            raise CORBA.BAD_PARAM(0, CORBA.COMPLETED_NO)
        return (value(name, self._this()), value("long", None))

    def echo_tc(self, tc):
        return tc

    def make_tc(self, name):
        return value(name, self._this()).typecode()

    def check_tc(self, name, tc):
        return value(name, self._this()).typecode().equal(tc)

    def echo_nvlist(self, l):
        return l

    def describe(self, a):
        tc = a.typecode()
        try:
            rid = tc.id()
        except CORBA.TypeCode.BadKind:
            rid = ""
        kind = str(tc.kind())  # "tk_struct", or "CORBA.tk_struct"
        return "%s %s" % (kind[kind.rfind("tk_") + 3:], rid)


def run_cases(r, echo, check):
    """Runs the stage 2 cases with the Runner r against the AnyEcho echo."""
    def expect(name, got):
        want = value(name, echo)
        check(same_any(want, got), "got %r" % (got,))

    for n in NAMES:
        r.case("make[%s]" % n, lambda n=n: expect(n, echo.make(n)))
        r.case("check[%s]" % n, lambda n=n: check(echo.check(n, value(n, echo)) is True))
        r.case("echo[%s]" % n, lambda n=n: expect(n, echo.echo(value(n, echo))))
    for n in NAMES:
        r.case("make_tc[%s]" % n,
               lambda n=n: check(echo.make_tc(n).equal(value(n, echo).typecode())))
        r.case("check_tc[%s]" % n,
               lambda n=n: check(echo.check_tc(n, value(n, echo).typecode()) is True))
        r.case("echo_tc[%s]" % n, lambda n=n: check(
            echo.echo_tc(value(n, echo).typecode()).equal(value(n, echo).typecode())))
    for n in ("point", "nvlist", "node"):
        def out_inout(n=n):
            b, c = echo.out_inout(n, value(n, echo))
            expect(n, b)
            expect("long", c)
        r.case("out_inout[%s]" % n, out_inout)
    r.case("echo_nvlist", lambda: check(same(
        AnyTest._tc_NVList, echo.echo_nvlist(value("nvlist", echo).value()),
        value("nvlist", echo).value())))
    for n in HIDDEN:
        r.case("echo[%s]" % n, lambda n=n: expect(n, echo.echo(value(n, echo))))
        r.case("describe[%s]" % n, lambda n=n: check(
            echo.describe(value(n, echo)) == HIDDEN_DESCRIPTION[n],
            echo.describe(value(n, echo))))
    r.case("describe[long]", lambda: check(echo.describe(value("long", echo)) == "long "))
    r.case("describe[node]",
           lambda: check(echo.describe(value("node", echo)) == "struct IDL:AnyTest/Node:1.0"))
    # The server compares TypeCodes with equal(): a short 42 or an
    # unaliased long 7 is not the value.
    r.case("check_wrong_type", lambda: check(
        echo.check("long", CORBA.Any(CORBA.TC_short, 42)) is False))
    r.case("check_unaliased", lambda: check(
        echo.check("count", CORBA.Any(CORBA.TC_long, 7)) is False))
    r.case("check_tc_unaliased", lambda: check(echo.check_tc("count", CORBA.TC_long) is False))

    def wrong_inout():
        try:
            echo.out_inout("point", value("long", echo))
        except CORBA.BAD_PARAM:
            return
        raise AssertionError("no BAD_PARAM")
    r.case("out_inout_wrong", wrong_inout)

