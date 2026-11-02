"""Tests of the DDS type support of ``rustorb-idl -brust`` (``-Wbdds``,
``-Wbdds_only``): the annotations the frontend reads and the constructs the
generator refuses. The generated code itself is compiled and run by
``cargo test -p rustorb-codegen`` (tests/dds_mapping.rs).

Run from the repository root with idl_parser importable::

    PYTHONPATH=path/to/idl_parser python3 -m unittest discover -s crates/rustorb-codegen/python/tests
"""

import contextlib
import io
import os
import shutil
import sys
import tempfile
import textwrap
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", "..", ".."))
CODEGEN = os.path.join(ROOT, "crates", "rustorb-codegen", "python")

if CODEGEN not in sys.path:
    sys.path.insert(0, CODEGEN)

from rustorb_idl import cli  # noqa: E402
from rustorb_idl import frontend as fe  # noqa: E402

OUT = tempfile.mkdtemp(prefix="rustorb-idl-dds-test-")


def tearDownModule():
    shutil.rmtree(OUT, ignore_errors=True)


def write_idl(name, text):
    d = os.path.join(OUT, "idl")
    os.makedirs(d, exist_ok=True)
    path = os.path.join(d, name)
    with open(path, "w") as f:
        f.write(textwrap.dedent(text))
    return path


def run(idl_text, *options, name="T.idl"):
    """Runs the generator; returns (status, stderr, generated text)."""
    path = write_idl(name, idl_text)
    outdir = os.path.join(OUT, "gen")
    err = io.StringIO()
    with contextlib.redirect_stderr(err):
        status = cli.main(["-brust", "-Wbno_rustfmt", "-C", outdir] + list(options) + [path])
    text = None
    out = os.path.join(outdir, cli.rust_backend.file_module(path) + ".rs")
    if status == 0:
        with open(out) as f:
            text = f.read()
        os.remove(out)
    return status, err.getvalue(), text


def struct_node(idl_text, name):
    path = write_idl("Node.idl", idl_text)
    spec = fe.load(path, giop=False)
    for it in fe.definitions(spec):
        if it.node.name == name:
            return it.node
    raise KeyError(name)


class MemberIdTest(unittest.TestCase):
    def test_hash_of_the_spec_example(self):
        # DDS-XTypes 7.3.1.2.1.1, example 2.
        self.assertEqual(fe.hashed_id("color"), 0x0FA5DD70)
        self.assertEqual(fe.hashed_id("shapesize"), 0x047790DA)

    def test_sequential_and_explicit_ids(self):
        node = struct_node(
            """
            struct S { long a; @id(10) long b; long c; @hashid long d; long e; };
            """,
            "S",
        )
        ids = fe.member_ids(node, node.members)
        h = fe.hashed_id("d")
        self.assertEqual(ids, [0, 10, 11, h, h + 1])

    def test_autoid_hash_from_the_module(self):
        node = struct_node(
            """
            @autoid(HASH) module M { struct S { long color; @hashid("shapesize") long size; @id(3) long x; }; };
            """,
            "S",
        )
        self.assertEqual(fe.member_ids(node, node.members), [0x0FA5DD70, 0x047790DA, 3])

    def test_autoid_without_a_value_is_hash(self):
        node = struct_node("@autoid struct S { long color; };", "S")
        self.assertEqual(fe.member_ids(node, node.members), [0x0FA5DD70])

    def test_duplicate_ids_are_refused(self):
        node = struct_node("struct S { @id(1) long a; @id(1) long b; };", "S")
        with self.assertRaises(fe.GeneratorError):
            fe.member_ids(node, node.members)

    def test_extensibility(self):
        self.assertEqual(fe.extensibility(struct_node("struct S { long a; };", "S")), "appendable")
        self.assertEqual(fe.extensibility(struct_node("@final struct S { long a; };", "S")), "final")
        self.assertEqual(
            fe.extensibility(struct_node("@extensibility(MUTABLE) struct S { long a; };", "S")),
            "mutable",
        )
        with self.assertRaises(fe.GeneratorError):
            fe.extensibility(struct_node("@final @mutable struct S { long a; };", "S"))

    def test_nested(self):
        idl = """
            @default_nested module M {
              struct A { long a; };
              @topic struct B { long a; };
              @nested(FALSE) struct C { long a; };
            };
            struct D { long a; };
            @nested struct E { long a; };
            """
        self.assertTrue(fe.is_nested(struct_node(idl, "A")))
        self.assertFalse(fe.is_nested(struct_node(idl, "B")))
        self.assertFalse(fe.is_nested(struct_node(idl, "C")))
        self.assertFalse(fe.is_nested(struct_node(idl, "D")))
        self.assertTrue(fe.is_nested(struct_node(idl, "E")))


class GeneratorTest(unittest.TestCase):
    def test_dds_only_has_no_corba_code(self):
        status, err, text = run("@topic struct S { @key long id; @optional string s; };", "-Wbdds_only")
        self.assertEqual(status, 0, err)
        self.assertNotIn("rustorb_core", text)
        self.assertIn("const _: () = ::rustorb_typesupport::GENERATED_CODE_VERSION_1;", text)
        self.assertIn("pub s: Option<String>,", text)
        self.assertIn("impl ::rustorb_typesupport::TopicType for S {", text)
        self.assertIn("const KEYED: bool = true;", text)

    def test_dds_adds_to_the_corba_code(self):
        status, err, text = run("struct S { @key long id; };", "-Wbdds")
        self.assertEqual(status, 0, err)
        self.assertIn("impl ::rustorb_core::idl::IdlType for S {", text)
        self.assertIn("impl ::rustorb_typesupport::TypeSupport for S {", text)

    def test_corba_refuses_what_giop_cannot_send(self):
        for options in ([], ["-Wbdds"]):
            status, err, _ = run("struct S { @optional long a; };", *options)
            self.assertEqual(status, 1)
            self.assertIn("@optional is not supported", err)

    def test_nested_types_are_not_topics(self):
        status, err, text = run("@nested struct S { long a; };", "-Wbdds_only")
        self.assertEqual(status, 0, err)
        self.assertIn("impl ::rustorb_typesupport::TypeSupport for S {", text)
        self.assertNotIn("TopicType", text)

    def test_default_extensibility_option(self):
        status, err, text = run("struct S { long a; };", "-Wbdds_only", "-Wbextensibility=final")
        self.assertEqual(status, 0, err)
        self.assertIn("::rustorb_typesupport::Extensibility::Final", text)
        self.assertNotIn("Appendable", text)
        status, err, _ = run("struct S { long a; };", "-Wbdds_only", "-Wbextensibility=open")
        self.assertEqual(status, 2)

    def test_typesupport_path(self):
        status, err, text = run("struct S { long a; };", "-Wbdds_only", "-Wbtypesupport=crate::ts")
        self.assertEqual(status, 0, err)
        self.assertIn("impl crate::ts::TypeSupport for S {", text)

    def test_refused(self):
        cases = [
            ("struct S { @key @optional long a; };", "both @key and @optional"),
            ("@mutable union U switch (long) { case 1: long a; };", "mutable unions"),
            ("interface I { void f(); };", "no form in DDS"),
            ("exception E { long a; };", "no form in DDS"),
            ("struct S { any a; };", "any has no form in DDS"),
            ("@bit_bound(8) enum E { @value(300) A };", "does not fit"),
            ("struct S { @id(1) long a; @id(1) long b; };", "the same member ID"),
            ("struct S { @non_serialized long a; };", "not supported for DDS yet"),
        ]
        for idl, message in cases:
            status, err, _ = run(idl, "-Wbdds_only")
            self.assertEqual(status, 1, idl)
            self.assertIn(message, err, idl)

    def test_bitmask_needs_dds(self):
        status, err, _ = run("bitmask B { A, B2 };")
        self.assertEqual(status, 1)
        self.assertIn("only for DDS", err)
        status, err, text = run("@bit_bound(16) bitmask B { A, @position(9) FLAG_B };", "-Wbdds_only")
        self.assertEqual(status, 0, err)
        self.assertIn("pub struct B(pub u16);", text)
        self.assertIn("pub const FLAG_B: B = B(1 << 9);", text)


if __name__ == "__main__":
    unittest.main()
