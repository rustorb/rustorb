"""Tests of ``rustorb-idl -bc++11``: the command line, the generated code
and the constructs the C++ backend refuses. The generated code is compiled
(syntax only) when a C++ compiler is found; cpp/tests runs it.

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

if CODEGEN not in sys.path:
    sys.path.insert(0, CODEGEN)

from rustorb_idl import cli  # noqa: E402

OUT = tempfile.mkdtemp(prefix="rustorb-idl-cxx-test-")
CXX = os.environ.get("CXX") or shutil.which("g++") or shutil.which("clang++")

IDLS = [
    os.path.join(CPP_IDL, "CxxMapping.idl"),
    os.path.join(TEST_IDL, "CosNaming.idl"),
    os.path.join(INTEROP_IDL, "InteropTest.idl"),
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


class CxxBackendTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.outdir = os.path.join(OUT, "gen")
        assert cli.main(["-bc++11", "-C", cls.outdir] + IDLS) == 0

    def test_outputs(self):
        for base in ("CxxMapping", "CosNaming", "InteropTest"):
            header = read(os.path.join(self.outdir, base + ".hh"))
            source = read(os.path.join(self.outdir, base + ".cc"))
            self.assertIn("Do not edit", header)
            self.assertIn("#include <rustorb/cdr.h>", header)
            self.assertIn('#include "%s.hh"' % base, source)

    def test_suffixes(self):
        outdir = os.path.join(OUT, "suffixes")
        path = os.path.join(INTEROP_IDL, "InteropTest.idl")
        self.assertEqual(cli.main(["-bcxx11", "-Wbh=.h", "-Wbs=.cpp", "-C", outdir, path]), 0)
        self.assertEqual(sorted(os.listdir(outdir)), ["InteropTest.cpp", "InteropTest.h"])
        self.assertIn('#include "InteropTest.h"', read(os.path.join(outdir, "InteropTest.cpp")))

    def test_mapping(self):
        header = read(os.path.join(self.outdir, "CxxMapping.hh"))
        # C++ keywords get the _cxx_ prefix.
        self.assertIn("int32_t _cxx_class() const", header)
        self.assertIn("virtual void _cxx_delete(const std::string& what);", header)
        # Enums, constants, skeletons and the servant traits.
        self.assertIn("enum class Fruit : uint32_t", header)
        self.assertIn('inline const std::string C_STRING = "abcd";', header)
        self.assertIn("class Types : public virtual ::PortableServer::ServantBase", header)
        self.assertIn("struct servant_traits<::CxxMapping::Types>", header)
        # The operation name on the wire keeps the IDL spelling.
        self.assertIn('op == "delete"', read(os.path.join(self.outdir, "CxxMapping.cc")))

    def test_options(self):
        path = os.path.join(INTEROP_IDL, "InteropTest.idl")
        self.assertEqual(cli.main(["-bc++11", "-Wbglobal=x", "-C", OUT, path]), 2)
        self.assertEqual(cli.main(["-bpython", "-Wbh=.h", "-C", OUT, path]), 2)
        self.assertEqual(cli.main(["-bjava", "-C", OUT, path]), 2)

    def test_unsupported(self):
        cases = {
            "any": "module U { interface I { void f(in any a); }; };",
            "fixed": "module U { typedef fixed<5, 2> F; };",
            "valuetype": "module U { valuetype V { public long x; }; };",
            "abstract": "module U { abstract interface A { void f(); }; };",
            "longdouble": "module U { struct S { long double d; }; };",
        }
        for name, text in cases.items():
            with self.subTest(name):
                path = write_idl("Unsupported_%s.idl" % name, text)
                self.assertEqual(cli.main(["-bc++11", "-C", OUT, path]), 1)

    @unittest.skipUnless(CXX, "no C++ compiler")
    def test_compiles(self):
        for base in ("CxxMapping", "CosNaming", "InteropTest"):
            with self.subTest(base):
                subprocess.run(
                    [
                        CXX,
                        "-std=c++17",
                        "-fsyntax-only",
                        "-Wall",
                        "-Wextra",
                        "-Werror",
                        "-I" + os.path.join(ROOT, "cpp", "include"),
                        "-I" + os.path.join(ROOT, "crates", "rustorb-ffi", "include"),
                        "-I" + self.outdir,
                        os.path.join(self.outdir, base + ".cc"),
                    ],
                    check=True,
                )


if __name__ == "__main__":
    unittest.main()
