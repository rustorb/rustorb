"""Tests of ``rustorb-idl -brust``: the command line, the generated code
and the constructs the Rust backend refuses. The generated code itself is
compiled and run by ``cargo test -p rustorb-codegen``
(tests/rust_mapping.rs) and by the interop client and servers.

Run from the repository root with idl_parser importable::

    PYTHONPATH=path/to/idl_parser python3 -m unittest discover -s crates/rustorb-codegen/python/tests
"""

import os
import shutil
import sys
import tempfile
import textwrap
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", "..", ".."))
CODEGEN = os.path.join(ROOT, "crates", "rustorb-codegen", "python")
TEST_IDL = os.path.join(ROOT, "crates", "rustorb-codegen", "tests", "idl")
INTEROP_IDL = os.path.join(ROOT, "interop", "idl")

if CODEGEN not in sys.path:
    sys.path.insert(0, CODEGEN)

from rustorb_idl import cli  # noqa: E402

OUT = tempfile.mkdtemp(prefix="rustorb-idl-rust-test-")

IDLS = [
    os.path.join(TEST_IDL, "RustMapping.idl"),
    os.path.join(TEST_IDL, "CosNaming.idl"),
    os.path.join(INTEROP_IDL, "InteropTest.idl"),
    os.path.join(INTEROP_IDL, "AnyTest.idl"),
    os.path.join(INTEROP_IDL, "ValueTest.idl"),
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


class RustBackendTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.outdir = os.path.join(OUT, "gen")
        assert cli.main(["-brust", "-C", cls.outdir] + IDLS) == 0

    def gen(self, name):
        return read(os.path.join(self.outdir, name))

    def test_outputs(self):
        self.assertEqual(
            sorted(os.listdir(self.outdir)),
            ["any_test.rs", "cos_naming.rs", "interop_test.rs", "rust_mapping.rs", "value_test.rs"],
        )
        text = self.gen("interop_test.rs")
        self.assertIn("Do not edit", text)
        self.assertIn("const _: () = ::rustorb_core::idl::GENERATED_CODE_VERSION_1;", text)
        self.assertIn("pub mod interop_test {", text)

    def test_names(self):
        text = self.gen("rust_mapping.rs")
        # Rust keywords are raw identifiers, and self gets a suffix.
        self.assertIn("pub r#type: i32,", text)
        self.assertIn("pub self_: bool,", text)
        self.assertIn("pub camel_case_name: String,", text)
        self.assertIn("pub const C_ENUM: Fruit = Fruit::Banana;", text)
        self.assertIn("pub const MAX_COUNT: i32 = 5;", text)
        self.assertIn("CherryPie", text)
        # The operation name on the wire keeps the IDL spelling.
        self.assertIn('request("getValue")', text)
        self.assertIn("pub fn get_value(", text)

    def test_operations(self):
        text = self.gen("interop_test.rs")
        # out values come back in a tuple, inout ones are &mut.
        self.assertIn("pub fn divide(&self, a: i32, b: i32) -> Result<(i32, i32), echo::DivideError>", text)
        self.assertIn("a: &mut i32,", text)
        self.assertIn("fn echo_records(&self, v: &[Record]) -> Result<RecordSeq,", text)
        self.assertIn("pub trait EchoServant: Send + Sync {", text)
        self.assertIn("pub struct EchoPoa<T>(pub T);", text)
        self.assertIn("pub fn activate_with_id(", text)
        self.assertIn("DivideByZero(super::DivideByZero),", text)

    def test_typecodes(self):
        text = self.gen("any_test.rs")
        self.assertIn('TypeCode::recursive("IDL:AnyTest/Node:1.0")', text)
        self.assertIn("pub type Money = ::rustorb_core::Fixed;", text)
        self.assertIn("pub mod alias {", text)
        self.assertIn("pub struct NvList;", text)

    def test_valuetypes(self):
        text = self.gen("value_test.rs")
        self.assertIn("pub struct Point3State {", text)
        self.assertIn("pub base: PointState,", text)
        self.assertIn("impl TryFrom<Point> for Point3 {", text)
        self.assertIn("pub struct LongBox(::std::sync::Arc<i32>);", text)
        self.assertIn("pub fn register_value_types() {", text)
        self.assertIn('sent_ids: &["IDL:ValueTest/Point3:1.0", "IDL:ValueTest/Point:1.0"]', text)

    def test_extern_and_core(self):
        outdir = os.path.join(OUT, "extern")
        path = os.path.join(INTEROP_IDL, "AnyTestHidden.idl")
        args = ["-brust", "-I", INTEROP_IDL, "-Wbextern=AnyTest=crate::any", "-Wbcore=::core_alias"]
        self.assertEqual(cli.main(args + ["-C", outdir, path]), 0)
        self.assertEqual(os.listdir(outdir), ["any_test_hidden.rs"])
        text = read(os.path.join(outdir, "any_test_hidden.rs"))
        self.assertIn("pub type PointSeq = Vec<crate::any::Point>;", text)
        self.assertNotIn("pub mod any_test {", text)
        self.assertIn("::core_alias::idl::", text)
        self.assertNotIn("::rustorb_core", text)
        # A path without a prefix is absolute.
        self.assertEqual(cli.main(["-brust", "-I", INTEROP_IDL, "-Wbextern=AnyTest=other::any",
                                   "-C", outdir, path]), 0)
        self.assertIn("Vec<::other::any::Point>", read(os.path.join(outdir, "any_test_hidden.rs")))

    def test_no_rustfmt(self):
        outdir = os.path.join(OUT, "raw")
        path = os.path.join(INTEROP_IDL, "InteropTest.idl")
        self.assertEqual(cli.main(["-brust", "-Wbno_rustfmt", "-C", outdir, path]), 0)
        self.assertIn("pub struct Echo(", read(os.path.join(outdir, "interop_test.rs")))

    def test_options(self):
        path = os.path.join(INTEROP_IDL, "InteropTest.idl")
        self.assertEqual(cli.main(["-brust", "-Wbextern=AnyTest", "-C", OUT, path]), 2)
        self.assertEqual(cli.main(["-brust", "-Wbh=.h", "-C", OUT, path]), 2)
        self.assertEqual(cli.main(["-bpython", "-Wbcore=x", "-C", OUT, path]), 2)

    def test_unsupported(self):
        cases = {
            "abstract": "module U { abstract interface A { void f(); }; };",
            "local": "module U { local interface L { void f(); }; };",
            "supports_local": "module U { local interface I {}; valuetype V supports I { public long x; }; };",
            "value_raises": "module U { exception E {}; valuetype V { public long x; long f() raises (E); }; };",
            "value_op_clash": "module U { valuetype V { public long x; long get(); }; };",
            "abstract_value": "module U { abstract valuetype A {}; valuetype V : A { public long x; }; };",
            "members": "module U { struct S { long aB; long a_b; }; };",
            "enumerators": "module U { enum E { FOO_BAR, FooBar }; };",
            "items": "module U { struct my_pair { long x; }; struct MyPair { long y; }; };",
            "base_member": "module U { valuetype B { public long x; }; valuetype D : B { public long base; }; };",
        }
        for name, text in cases.items():
            with self.subTest(name):
                path = write_idl("Unsupported_%s.idl" % name, text)
                self.assertEqual(cli.main(["-brust", "-C", OUT, path]), 1)


if __name__ == "__main__":
    unittest.main()
