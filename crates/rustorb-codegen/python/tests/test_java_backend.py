"""Tests of ``rustorb-idl -bjava``: the command line, the generated code and
the constructs the Java backend refuses. The generated code is compiled with
the runtime (java/src/main/java) when javac is found; java/ (mvn verify)
runs it.

Run from the repository root with idl_parser importable::

    PYTHONPATH=path/to/idl_parser python3 -m unittest discover -s crates/rustorb-codegen/python/tests
"""

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
TEST_IDL = os.path.join(ROOT, "crates", "rustorb-codegen", "tests", "idl")
CPP_IDL = os.path.join(ROOT, "cpp", "tests", "idl")
INTEROP_IDL = os.path.join(ROOT, "interop", "idl")
RUNTIME = os.path.join(ROOT, "java", "src", "main", "java")

if CODEGEN not in sys.path:
    sys.path.insert(0, CODEGEN)

from rustorb_idl import cli  # noqa: E402

OUT = tempfile.mkdtemp(prefix="rustorb-idl-java-test-")
JAVAC = shutil.which("javac")

IDLS = [
    os.path.join(INTEROP_IDL, "InteropTest.idl"),
    os.path.join(INTEROP_IDL, "AnyTest.idl"),
    os.path.join(INTEROP_IDL, "AnyTestHidden.idl"),
    os.path.join(INTEROP_IDL, "PoaProbe.idl"),
]


def tearDownModule():
    shutil.rmtree(OUT, ignore_errors=True)


def write_idl(name, text):
    d = os.path.join(OUT, "idl")
    os.makedirs(d, exist_ok=True)
    path = os.path.join(d, name)
    with open(path, "w") as f:
        f.write(textwrap.dedent(text))
    return path


def read(path):
    with open(path) as f:
        return f.read()


class JavaBackendTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.outdir = os.path.join(OUT, "gen")
        assert cli.main(["-bjava", "-I", INTEROP_IDL, "-C", cls.outdir] + IDLS) == 0
        path = os.path.join(CPP_IDL, "CxxMapping.idl")
        assert cli.main(["-bjava", "-Wbpackage=mapping", "-C", cls.outdir, path]) == 0

    def java(self, *parts):
        return read(os.path.join(self.outdir, *parts) + ".java")

    def test_outputs(self):
        echo = os.path.join(self.outdir, "InteropTest")
        for name in ("Echo", "EchoOperations", "EchoHelper", "EchoHolder", "_EchoStub",
                     "EchoPOA", "EchoPOATie", "Point", "PointHelper", "PointHolder",
                     "Color", "Value", "LongSeqHelper", "LongSeqHolder", "MAGIC"):
            self.assertTrue(os.path.exists(os.path.join(echo, name + ".java")), name)
        self.assertIn("Do not edit", self.java("InteropTest", "Echo"))
        self.assertIn("package InteropTest;", self.java("InteropTest", "Echo"))

    def test_mapping(self):
        # -Wbpackage puts the definitions outside modules in the package.
        self.assertIn("package mapping;", self.java("mapping", "GlobalPoint"))
        self.assertIn("package mapping.CxxMapping;", self.java("mapping", "CxxMapping", "Types"))
        # Types inside an interface or a struct are in <Name>Package.
        self.assertIn(
            "package mapping.CxxMapping.BasePackage;",
            self.java("mapping", "CxxMapping", "BasePackage", "BaseError"),
        )
        self.assertIn("package mapping.CxxMapping.OuterPackage;",
                      self.java("mapping", "CxxMapping", "OuterPackage", "Mode"))
        # Java keywords and the methods of java.lang.Object get a _.
        self.assertIn("public int _class;", self.java("mapping", "CxxMapping", "Keywords"))
        self.assertIn("void _notify(String msg);",
                      self.java("mapping", "CxxMapping", "DerivedOperations"))
        # The operation name on the wire keeps the IDL spelling.
        self.assertIn('"notify"', self.java("mapping", "CxxMapping", "_DerivedStub"))
        # Constants of a module are interfaces with a value; of an
        # interface, fields of the signature interface.
        self.assertIn('String value = "abcd";', self.java("mapping", "CxxMapping", "C_STRING"))
        self.assertIn("int BASE_CONST = 7;", self.java("mapping", "CxxMapping", "Base"))
        # in, inout and out arguments.
        self.assertIn(
            "int derived_op(int a, org.omg.CORBA.IntHolder b, org.omg.CORBA.IntHolder c)",
            self.java("mapping", "CxxMapping", "DerivedOperations"),
        )
        # Skeletons follow the standard mapping.
        poa = self.java("mapping", "CxxMapping", "TypesPOA")
        self.assertIn("extends org.omg.PortableServer.Servant", poa)
        self.assertIn("implements mapping.CxxMapping.TypesOperations, "
                      "org.omg.CORBA.portable.InvokeHandler", poa)

    def test_typecodes(self):
        helper = self.java("AnyTest", "NodeHelper")
        # Recursion through NodeSeq refers back to Node.
        self.assertIn('create_recursive_tc(_id)', helper)
        choice = self.java("AnyTest", "ChoiceHelper")
        # Union labels in order, the default member labelled by octet 0.
        self.assertLess(choice.index("insert_long(2)"), choice.index("insert_long(3)"))
        self.assertIn("insert_octet((byte) 0)", choice)
        # An alias Helper inserts with the alias TypeCode.
        self.assertIn("create_alias_tc", self.java("AnyTest", "CountHelper"))

    def test_options(self):
        path = os.path.join(INTEROP_IDL, "InteropTest.idl")
        self.assertEqual(cli.main(["-bjava", "-Wbh=.h", "-C", OUT, path]), 2)
        self.assertEqual(cli.main(["-bjava", "-Wbglobal=x", "-C", OUT, path]), 2)

    def test_unsupported(self):
        cases = {
            "fixed": "module U { typedef fixed<5, 2> F; };",
            "valuetype": "module U { valuetype V { public long x; }; };",
            "abstract": "module U { abstract interface A { void f(); }; };",
            "longdouble": "module U { struct S { long double d; }; };",
        }
        for name, text in cases.items():
            with self.subTest(name):
                path = write_idl("Unsupported_%s.idl" % name, text)
                self.assertEqual(cli.main(["-bjava", "-C", OUT, path]), 1)

    @unittest.skipUnless(JAVAC, "no javac")
    def test_compiles(self):
        sources = []
        for d in (RUNTIME, self.outdir):
            for dirpath, _, names in os.walk(d):
                sources += [os.path.join(dirpath, n) for n in names if n.endswith(".java")]
        listing = os.path.join(OUT, "sources.txt")
        with open(listing, "w") as f:
            f.write("\n".join(sources))
        subprocess.run(
            [JAVAC, "--release", "11", "-encoding", "UTF-8", "-Xlint:all,-serial", "-Werror",
             "-d", os.path.join(OUT, "classes"), "@" + listing],
            check=True,
        )


if __name__ == "__main__":
    unittest.main()
