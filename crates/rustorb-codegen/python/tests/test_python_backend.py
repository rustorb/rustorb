"""Tests of ``rustorb-idl -bpython``: generate code for the test IDLs and
use it with the rustorb runtime (``python/rustorb``).

Run from the repository root with idl_parser importable::

    PYTHONPATH=path/to/idl_parser python3 -m unittest discover -s crates/rustorb-codegen/python/tests
"""

import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", "..", ".."))
CODEGEN = os.path.join(ROOT, "crates", "rustorb-codegen", "python")
RUNTIME = os.path.join(ROOT, "python")
TEST_IDL = os.path.join(ROOT, "crates", "rustorb-codegen", "tests", "idl")
INTEROP_IDL = os.path.join(ROOT, "interop", "idl")

for p in (CODEGEN, RUNTIME):
    if p not in sys.path:
        sys.path.insert(0, p)

from rustorb_idl import cli  # noqa: E402
from rustorb import CORBA, PortableServer, _rt  # noqa: E402

OUT = tempfile.mkdtemp(prefix="rustorb-idl-test-")


def generate(*paths, **kw):
    argv = ["-bpython", "-C", kw.get("outdir", OUT)]
    for d in kw.get("include", ()):
        argv += ["-I", d]
    argv += list(paths)
    status = cli.main(argv)
    if status != 0:
        raise AssertionError("rustorb-idl failed for %s" % (paths,))


def write_idl(name, text):
    d = os.path.join(OUT, "idl")
    os.makedirs(d, exist_ok=True)
    path = os.path.join(d, name)
    with open(path, "w") as f:
        f.write(textwrap.dedent(text))
    return path


def setUpModule():
    generate(os.path.join(TEST_IDL, "Mapping.idl"))
    generate(os.path.join(TEST_IDL, "CosNaming.idl"))
    generate(os.path.join(INTEROP_IDL, "InteropTest.idl"))
    generate(os.path.join(INTEROP_IDL, "PoaProbe.idl"))
    sys.path.insert(0, OUT)


def tearDownModule():
    shutil.rmtree(OUT, ignore_errors=True)


class FakeOrbObject(object):
    """Stands for the ORB's object of a reference: records invocations."""

    def __init__(self, repoIds=(), results=None):
        self.repoIds = set(repoIds)
        self.calls = []
        self.results = results or {}

    def invoke(self, op, args):
        self.calls.append((op, args))
        return self.results.get(op.name)

    def is_a(self, repoId):
        return repoId in self.repoIds


class LayoutTest(unittest.TestCase):
    def test_files(self):
        for f in (
            "InteropTest_idl.py",
            "InteropTest/__init__.py",
            "InteropTest/__init__.pyi",
            "InteropTest__POA/__init__.py",
            "Mapping/Inner/__init__.py",
            "Mapping__POA/Inner/__init__.py",
            "_GlobalIDL/__init__.py",
            "_GlobalIDL__POA/__init__.py",
            "CosNaming_idl.py",
        ):
            self.assertTrue(os.path.exists(os.path.join(OUT, f)), f)

    def test_packages(self):
        import Mapping
        import Mapping.Inner
        import Mapping__POA

        self.assertIs(Mapping.Inner, sys.modules["Mapping.Inner"])
        self.assertIs(Mapping__POA.Inner, sys.modules["Mapping__POA.Inner"])
        self.assertEqual(Mapping.Inner.INNER_NAME, "inner")
        self.assertEqual(Mapping.REOPENED, 1)  # module opened again

    def test_import_of_the_idl_file_first(self):
        # A *_idl.py file imported before the packages fills them when they
        # are imported.
        code = textwrap.dedent(
            """
            import sys
            sys.path[:0] = [%r, %r]
            import CosNaming_idl
            import CosNaming, CosNaming__POA
            assert CosNaming.NameComponent("a", "b").kind == "b"
            assert CosNaming__POA.NamingContext._NP_RepositoryId == \\
                "IDL:omg.org/CosNaming/NamingContext:1.0"
            print("ok")
        """
            % (OUT, RUNTIME)
        )
        r = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True)
        self.assertEqual(r.stdout.strip(), "ok", r.stderr)


class ValueTest(unittest.TestCase):
    def test_constants(self):
        import Mapping

        self.assertEqual(Mapping.C_SHORT, -3)
        self.assertEqual(Mapping.C_ULONGLONG, 18000000000000000000)
        self.assertEqual(Mapping.C_DOUBLE, 0.25)
        self.assertIs(Mapping.C_TRUE, True)
        self.assertEqual(Mapping.C_CHAR, "q")
        self.assertEqual(Mapping.C_WSTRING, "wide")
        self.assertEqual(Mapping.C_STRING, "abcd")
        self.assertEqual(Mapping.C_EXPR, (1 << 21) - 3)
        self.assertIs(Mapping.C_ENUM, Mapping.BANANA)
        self.assertEqual(Mapping.C_FIXED, CORBA.fixed("12.5"))
        self.assertEqual(Mapping.Base.BASE_CONST, 7)

    def test_enum(self):
        import InteropTest

        self.assertEqual([i._n for i in InteropTest.Color._items], ["RED", "GREEN", "BLUE"])
        self.assertIs(InteropTest.Color._item(1), InteropTest.GREEN)
        self.assertEqual(InteropTest.BLUE._v, 2)
        self.assertLess(InteropTest.RED, InteropTest.BLUE)
        self.assertEqual(repr(InteropTest.RED), "RED")

    def test_struct(self):
        import InteropTest

        p = InteropTest.Point(1.0, 2.0)
        r = InteropTest.Record(1, "a", InteropTest.RED, p, [1, 2], True)
        self.assertEqual(r.pos.y, 2.0)
        self.assertEqual(
            r,
            InteropTest.Record(1, "a", InteropTest.RED, InteropTest.Point(1.0, 2.0), [1, 2], True),
        )
        self.assertEqual(repr(p), "Point(x=1.0, y=2.0)")
        self.assertRaises(TypeError, InteropTest.Point, 1.0)
        self.assertRaises(TypeError, InteropTest.Point, 1.0, 2.0, 3.0)

    def test_nested_types(self):
        import Mapping

        o = Mapping.Outer(Mapping.Outer.Inner(1), Mapping.Outer.ON)
        self.assertEqual(o.nested.v, 1)
        self.assertEqual(Mapping.Outer.Inner.__qualname__, "Outer.Inner")
        self.assertEqual(
            Mapping.Outer.Inner._NP_RepositoryId, "IDL:rustorb.org/Mapping/Outer/Inner:1.0"
        )

    def test_typedef_classes(self):
        import Mapping

        self.assertTrue(issubclass(Mapping.PairAlias, Mapping.Pair))
        self.assertEqual(Mapping.PairAlias(1, 2).second, 2)
        self.assertEqual(
            Mapping.PairAlias._NP_RepositoryId, "IDL:rustorb.org/Mapping/PairAlias:1.0"
        )
        self.assertRaises(RuntimeError, Mapping.Count)
        self.assertRaises(RuntimeError, Mapping.BaseAlias)

    def test_union(self):
        import InteropTest
        import Mapping

        v = InteropTest.Value(l=5)
        self.assertEqual((v._d, v._v, v.l), (1, 5, 5))
        self.assertRaises(CORBA.BAD_PARAM, getattr, v, "s")
        v.s = "x"
        self.assertEqual((v._d, v._v), (2, "x"))
        self.assertEqual(InteropTest.Value(seq=[1])._d, 5)  # the last label
        self.assertEqual(InteropTest.Value(4, [1]).seq, [1])
        d = InteropTest.Value(flag=True)
        self.assertEqual(d._d, -(1 << 31))
        self.assertTrue(d.flag)
        self.assertEqual(InteropTest.Value(7, False).flag, False)  # default by value
        self.assertIs(Mapping.ByEnum(other=1)._d, Mapping.BANANA)
        self.assertEqual(Mapping.ByChar(bc="v")._d, "c")
        self.assertEqual(Mapping.ByBool(True, 3).yes, 3)
        self.assertEqual(repr(InteropTest.Value(l=5)), "Value(l=5)")
        self.assertRaises(TypeError, InteropTest.Value)

    def test_exception(self):
        import InteropTest
        import Mapping

        e = InteropTest.Oops(3, "bad")
        self.assertIsInstance(e, CORBA.UserException)
        self.assertEqual((e.code, e.reason), (3, "bad"))
        self.assertEqual(e, InteropTest.Oops(3, "bad"))
        with self.assertRaises(InteropTest.Oops):
            raise e
        self.assertEqual(Mapping.Base.BaseError("x").why, "x")
        InteropTest.DivideByZero()


class TypeCodeTest(unittest.TestCase):
    def test_struct(self):
        import InteropTest

        tc = InteropTest._tc_Record
        self.assertIs(tc.kind(), CORBA.tk_struct)
        self.assertEqual(tc.id(), "IDL:rustorb.org/InteropTest/Record:1.0")
        self.assertEqual(
            [tc.member_name(i) for i in range(tc.member_count())],
            ["id", "name", "hue", "pos", "values", "active"],
        )
        self.assertIs(tc.member_type(2), InteropTest._tc_Color)
        self.assertIs(tc.member_type(4).content_type().kind(), CORBA.tk_sequence)
        self.assertIs(tc._NP_class(), InteropTest.Record)
        self.assertIs(_rt.typecodes[tc.id()], tc)

    def test_recursive(self):
        import Mapping

        tc = Mapping._tc_Tree
        self.assertIs(tc.member_type(1).content_type(), tc)
        self.assertTrue(tc.equal(tc))

    def test_union(self):
        import InteropTest
        import Mapping

        tc = InteropTest._tc_Value
        self.assertEqual(tc.member_count(), 6)
        self.assertEqual(tc.default_index(), 5)
        self.assertEqual(tc.member_label(3).value(), 4)
        self.assertEqual(tc.member_name(4), "seq")
        self.assertIs(tc.member_label(5).typecode(), CORBA.TC_octet)
        self.assertIs(tc.discriminator_type(), CORBA.TC_long)
        self.assertIs(Mapping._tc_ByEnum.member_label(1).value(), Mapping.CHERRY)

    def test_arrays_and_bounds(self):
        import InteropTest
        import Mapping

        m = InteropTest._tc_Matrix.content_type()
        self.assertEqual((m.kind(), m.length(), m.content_type().length()), (CORBA.tk_array, 2, 3))
        self.assertEqual(Mapping._tc_BoundedSeq.content_type().length(), 5)
        self.assertEqual(Mapping._tc_ShortString.content_type().length(), 8)
        f = Mapping._tc_Money.content_type()
        self.assertEqual((f.fixed_digits(), f.fixed_scale()), (5, 2))

    def test_interfaces(self):
        import Mapping

        self.assertIs(Mapping._tc_Base.kind(), CORBA.tk_objref)
        self.assertIs(Mapping._tc_AbstractIface.kind(), CORBA.tk_abstract_interface)
        self.assertIs(Mapping._tc_LocalIface.kind(), CORBA.tk_local_interface)
        # a forward declaration and the definition share one TypeCode
        self.assertIs(Mapping.Base._tc_LaterSeq.content_type().content_type(), Mapping._tc_Later)

    def test_equal(self):
        import InteropTest

        built = CORBA.TypeCode(
            CORBA.tk_struct,
            "IDL:rustorb.org/InteropTest/Point:1.0",
            "Point",
            members=[("x", CORBA.TC_double), ("y", CORBA.TC_double)],
        )
        self.assertTrue(InteropTest._tc_Point.equal(built))
        self.assertTrue(InteropTest._tc_LongSeq.equivalent(_rt.tc_sequence(CORBA.TC_long)))
        self.assertFalse(InteropTest._tc_LongSeq.equal(_rt.tc_sequence(CORBA.TC_long)))
        self.assertRaises(CORBA.TypeCode.BadKind, CORBA.TC_long.member_count)


class InterfaceTest(unittest.TestCase):
    def test_classes(self):
        import InteropTest
        import InteropTest__POA

        self.assertRaises(RuntimeError, InteropTest.Echo)
        self.assertIsNone(InteropTest.Echo._nil)
        self.assertTrue(issubclass(InteropTest._objref_Echo, CORBA.Object))
        self.assertTrue(issubclass(InteropTest__POA.Echo, PortableServer.Servant))
        self.assertIs(
            _rt.objref_classes[InteropTest.Echo._NP_RepositoryId], InteropTest._objref_Echo
        )
        self.assertIs(
            _rt.skeleton_classes[InteropTest.Echo._NP_RepositoryId], InteropTest__POA.Echo
        )

    def test_operations(self):
        import InteropTest

        ops = InteropTest.Echo._NP_operations
        divide = ops["divide"]
        self.assertEqual(divide.in_types, (CORBA.TC_long, CORBA.TC_long))
        self.assertEqual(divide.out_types, (CORBA.TC_long, CORBA.TC_long))
        self.assertEqual(divide.exceptions, (InteropTest._tc_DivideByZero,))
        swap = ops["swap"]
        self.assertEqual((len(swap.in_types), len(swap.out_types)), (2, 3))
        make = ops["make"]
        self.assertEqual(make.in_types, (CORBA.TC_long,))
        self.assertEqual(make.out_types, (InteropTest._tc_Point, InteropTest._tc_Record))
        self.assertTrue(ops["post"].oneway)
        self.assertFalse(ops["posted"].oneway)
        self.assertEqual(ops["_get_counter"].out_types, (CORBA.TC_long,))
        self.assertEqual(ops["_set_counter"].in_types, (CORBA.TC_long,))
        self.assertNotIn("_set_name", ops)
        self.assertIs(ops["echo_ushort"].in_types[0], CORBA.TC_ushort)  # idl_parser#159
        self.assertIs(ops["echo_ulonglong"].out_types[0], CORBA.TC_ulonglong)

    def test_stub_calls(self):
        import InteropTest

        orb_obj = FakeOrbObject(results={"echo_long": 5, "_get_counter": 3})
        echo = InteropTest._objref_Echo(orb_obj)
        self.assertEqual(echo.echo_long(5), 5)
        self.assertEqual(echo.counter, 3)
        echo.counter = 4
        echo.divide(7, 2)
        names = [(op.name, args) for op, args in orb_obj.calls]
        self.assertEqual(
            names,
            [("echo_long", (5,)), ("_get_counter", ()), ("_set_counter", (4,)), ("divide", (7, 2))],
        )
        self.assertRaises(TypeError, echo.echo_long)

    def test_inheritance(self):
        import Mapping
        import Mapping__POA

        self.assertTrue(issubclass(Mapping.Multi, Mapping.Derived))
        self.assertTrue(issubclass(Mapping._objref_Multi, Mapping._objref_Other))
        self.assertTrue(issubclass(Mapping__POA.Multi, Mapping__POA.Base))
        self.assertTrue(issubclass(Mapping.Inner._objref_InnerIface, Mapping._objref_Base))
        self.assertEqual(_rt.operation(Mapping__POA.Multi, "base_op").name, "base_op")
        self.assertEqual(_rt.operation(Mapping._objref_Multi, "_get_id").name, "_get_id")
        self.assertIsNone(_rt.operation(Mapping__POA.Other, "base_op"))

    def test_narrow(self):
        import CosNaming

        ext = "IDL:omg.org/CosNaming/NamingContextExt:1.0"
        obj = CORBA.Object(FakeOrbObject([ext]))
        nc = obj._narrow(CosNaming.NamingContextExt)
        self.assertIsInstance(nc, CosNaming._objref_NamingContextExt)
        self.assertIs(nc._narrow(CosNaming.NamingContext), nc)
        self.assertIsNone(obj._narrow(CosNaming.BindingIterator))
        self.assertIsNone(CORBA.Object(FakeOrbObject())._narrow(CosNaming.NamingContext))

    def test_servant(self):
        import InteropTest
        import InteropTest__POA

        class EchoImpl(InteropTest__POA.Echo):
            def echo_long(self, v):
                return v

        s = EchoImpl()
        self.assertTrue(s._is_a(InteropTest.Echo._NP_RepositoryId))
        self.assertFalse(s._is_a(InteropTest.Callback._NP_RepositoryId))


class GeneratorTest(unittest.TestCase):
    def test_keywords(self):
        path = write_idl(
            "Keywords.idl",
            """
            module KwTest {
              struct S { long def; long pass; };
              exception E { string lambda; };
              enum Kind { from, import };
              interface I {
                attribute long class;
                void yield(in long return, out long self);
              };
            };
        """,
        )
        generate(path)
        import KwTest

        s = KwTest.S(1, 2)
        self.assertEqual((s._def, s._pass), (1, 2))
        self.assertEqual(KwTest.E("x")._lambda, "x")
        self.assertEqual(KwTest._from._n, "from")
        self.assertEqual(KwTest._tc_S.member_name(0), "def")
        o = FakeOrbObject()
        i = KwTest._objref_I(o)
        i._yield(1)
        i._class = 2
        self.assertEqual([c[0].name for c in o.calls], ["yield", "_set_class"])

    def test_edge_cases(self):
        path = write_idl(
            "Edge.idl",
            """
            module Edge {
              exception Bad { long code; };
              typedef long Disc;
              union ByTypedef switch (Disc) { case 1: long a; case 2: default: string rest; };
              union ByWchar switch (wchar) { case L'x': long x; };
              union WithType switch (long) { case 1: struct In { long q; } inn; };
              module Other { enum E { E1, E2 }; };
              const Other::E C_OTHER = Other::E2;
              interface I {
                attribute long x getraises (Bad) setraises (Bad);
                void ctx(in long a) context ("A", "B*");
                any get_any(in any a, in CORBA::TypeCode t);
                Object get_obj();
              };
            };
        """,
        )
        generate(path)
        import Edge

        ops = Edge.I._NP_operations
        self.assertEqual(ops["_get_x"].exceptions, (Edge._tc_Bad,))
        self.assertEqual(ops["_set_x"].exceptions, (Edge._tc_Bad,))
        self.assertEqual(ops["ctx"].contexts, ("A", "B*"))
        self.assertEqual(ops["get_any"].in_types, (CORBA.TC_any, CORBA.TC_TypeCode))
        self.assertIs(ops["get_obj"].out_types[0], CORBA.TC_Object)
        self.assertTrue(Edge._tc_ByTypedef.discriminator_type().equivalent(CORBA.TC_long))
        self.assertEqual(Edge.ByTypedef(7, "r").rest, "r")
        self.assertEqual(Edge.ByWchar(x=1)._d, "x")
        self.assertEqual(Edge.WithType(inn=Edge.WithType.In(2)).inn.q, 2)
        self.assertIs(Edge.C_OTHER, Edge.Other.E2)

    def test_includes_and_shared_modules(self):
        base = write_idl(
            "SharedBase.idl",
            """
            #ifndef SHARED_BASE
            #define SHARED_BASE
            module Shared {
              struct Item { long id; };
              interface Store { Item get(); };
            };
            #endif
        """,
        )
        top = write_idl(
            "SharedTop.idl",
            """
            #include "SharedBase.idl"
            module Shared {
              typedef sequence<Item> ItemSeq;
              interface BigStore : Store { ItemSeq all(); };
            };
        """,
        )
        generate(base)
        generate(top)
        with open(os.path.join(OUT, "SharedTop_idl.py")) as f:
            self.assertIn("import SharedBase_idl", f.read())
        with open(os.path.join(OUT, "Shared", "__init__.py")) as f:
            init = f.read()
        self.assertIn("import SharedBase_idl", init)
        self.assertIn("import SharedTop_idl", init)
        with open(os.path.join(OUT, "Shared", "__init__.pyi")) as f:
            pyi = f.read()
        self.assertIn("# rustorb-idl: begin SharedBase.idl", pyi)
        self.assertIn("# rustorb-idl: begin SharedTop.idl", pyi)
        # generating a file again replaces its own section only
        generate(top)
        with open(os.path.join(OUT, "Shared", "__init__.pyi")) as f:
            self.assertEqual(f.read(), pyi)
        import Shared
        import Shared__POA

        self.assertIs(Shared._tc_ItemSeq.content_type().content_type(), Shared._tc_Item)
        self.assertTrue(issubclass(Shared__POA.BigStore, Shared__POA.Store))
        self.assertEqual(Shared.Item(4).id, 4)

    def test_unsupported(self):
        path = write_idl(
            "Unsupported.idl",
            """
            module Unsup { valuetype V { public long x; }; };
        """,
        )
        self.assertEqual(cli.main(["-bpython", "-C", OUT, path]), 1)

    def test_pyi_compiles(self):
        for root, _, files in os.walk(OUT):
            for f in files:
                if f.endswith(".pyi"):
                    with open(os.path.join(root, f)) as fh:
                        compile(fh.read(), f, "exec")


TYPED_USER = """
import CosNaming
import InteropTest
import InteropTest__POA
from rustorb import CORBA


def use(obj: CORBA.Object) -> None:
    echo = obj._narrow(InteropTest.Echo)
    if echo is not None:
        n: int = echo.echo_long(3)
        q, r = echo.divide(7, 2)
        p: InteropTest.Point = echo.echo_point(InteropTest.Point(1.0, 2.0))
        echo.counter = n + q + r
    nc = obj._narrow(CosNaming.NamingContextExt)
    if nc is not None:
        kind: str = nc.to_name("a/b")[0].kind


class EchoImpl(InteropTest__POA.Echo):
    def echo_long(self, v: int) -> int:
        return v
"""


@unittest.skipUnless(importlib.util.find_spec("mypy"), "mypy is not installed")
class StubTypeCheckTest(unittest.TestCase):
    """The .pyi stubs pass mypy, and so does code that uses them."""

    def test_mypy(self):
        user = os.path.join(OUT, "typed_user.py")
        with open(user, "w") as f:
            f.write(TYPED_USER)
        stubs = [
            os.path.relpath(os.path.join(root, f), OUT)
            for root, _, files in os.walk(OUT)
            for f in files
            if f == "__init__.pyi" and not root.endswith("idl")
        ]
        r = subprocess.run(
            [sys.executable, "-m", "mypy", "--no-incremental", "typed_user.py"] + sorted(stubs),
            cwd=OUT,
            capture_output=True,
            text=True,
            env=dict(os.environ, MYPYPATH=RUNTIME),
        )
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)


class DumpTest(unittest.TestCase):
    """The public API printed by interop/scripts/pymapping-dump is the one
    recorded in interop/expected/pymapping-rustorb.txt (which differs from
    the omniORBpy one only as interop/README.md says)."""

    def test_dump(self):
        expected = os.path.join(ROOT, "interop", "expected", "pymapping-rustorb.txt")
        r = subprocess.run(
            [
                sys.executable,
                os.path.join(ROOT, "interop", "scripts", "pymapping-dump"),
                "--runtime",
                "rustorb",
            ]
            + DUMP_MODULES,
            cwd=OUT,
            capture_output=True,
            text=True,
            env=dict(os.environ, PYTHONPATH=RUNTIME),
        )
        self.assertEqual(r.returncode, 0, r.stderr)
        with open(expected) as f:
            self.assertEqual(r.stdout, f.read())


DUMP_MODULES = [
    "Mapping",
    "_GlobalIDL",
    "Mapping__POA",
    "_GlobalIDL__POA",
    "CosNaming",
    "CosNaming__POA",
    "InteropTest",
    "InteropTest__POA",
    "PoaProbe",
    "PoaProbe__POA",
]

if __name__ == "__main__":
    unittest.main()
