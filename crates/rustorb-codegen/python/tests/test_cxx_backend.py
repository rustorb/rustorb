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
    os.path.join(INTEROP_IDL, "AnyTest.idl"),
    os.path.join(CPP_IDL, "Idl4.idl"),
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

    def test_typecodes(self):
        header = read(os.path.join(self.outdir, "AnyTest.hh"))
        source = read(os.path.join(self.outdir, "AnyTest.cc"))
        # Every type has _tc_<name>, made by _rorb_tc_<name>().
        for name in ("Point", "Shade", "Choice", "Count", "NodeSeq", "Node", "Oops", "AnyEcho"):
            self.assertIn("extern const ::IDL::traits<::CORBA::TypeCode>::ref_type _tc_%s;" % name, header)
            self.assertIn("AnyTest::_tc_%s = AnyTest::_rorb_tc_%s();" % (name, name), source)
        # Types with a C++ type of their own have a type_code specialisation.
        self.assertIn("template <> struct type_code<::AnyTest::Point>", header)
        self.assertNotIn("type_code<::AnyTest::Count>", header)
        # Inside Node, NodeSeq is made in place and refers back to Node.
        self.assertIn(
            '"children", ::rustorb::detail::tc_alias(::CORBA::TCKind::tk_alias, '
            '"IDL:AnyTest/NodeSeq:1.0", "NodeSeq", ::rustorb::detail::tc_sequence('
            '::rustorb::detail::tc_recursive("IDL:AnyTest/Node:1.0"), 0))',
            source,
        )
        # Union labels, the default member last with its index.
        self.assertIn('{3ULL, "s", ::rustorb::detail::tc_string(false, 0)}, {0, "b", ', source)
        # any and TypeCode arguments.
        self.assertIn("virtual ::CORBA::Any echo(const ::CORBA::Any& a);", header)
        self.assertIn(
            "virtual ::IDL::traits<::CORBA::TypeCode>::ref_type echo_tc("
            "::IDL::traits<::CORBA::TypeCode>::ref_type tc);",
            header,
        )

    def test_options(self):
        path = os.path.join(INTEROP_IDL, "InteropTest.idl")
        self.assertEqual(cli.main(["-bc++11", "-Wbglobal=x", "-C", OUT, path]), 2)
        self.assertEqual(cli.main(["-bpython", "-Wbh=.h", "-C", OUT, path]), 2)
        self.assertEqual(cli.main(["-bada", "-C", OUT, path]), 2)

    def test_unsupported(self):
        cases = {
            "supports_local": "module U { local interface I {}; valuetype V supports I { public long x; }; };",
            # IDL 4 constructs that GIOP has no form for.
            "optional": "module U { struct S { @optional long x; }; };",
            "external": "module U { struct S { @external long x; }; };",
            "value": "module U { enum E { @value(3) A, B }; };",
            "map": "module U { typedef map<string, long> M; };",
            "int8_discriminator": "module U { union X switch (int8) { case 1: long a; }; };",
            "enum_default": "module U { enum E { A, B }; struct S { @default(B) E e; }; };",
            "bitset_base": "module U { bitset A { bitfield<2> a; }; bitset B : A { bitfield<2> b; }; };",
        }
        for name, text in cases.items():
            with self.subTest(name):
                path = write_idl("Unsupported_%s.idl" % name, text)
                self.assertEqual(cli.main(["-bc++11", "-C", OUT, path]), 1)

    def test_idl4(self):
        hh = read(os.path.join(self.outdir, "Idl4.hh"))
        cc = read(os.path.join(self.outdir, "Idl4.cc"))
        self.assertIn("enum class Flags : uint8_t", hh)
        self.assertIn("WRITE = static_cast<uint8_t>(1) << 3", hh)
        self.assertIn("enum class Wide : uint64_t", hh)
        self.assertIn("int16_t delta : 5;", hh)
        self.assertIn("template <> struct type_code<::Idl4::Header> : type_code<uint16_t> {};", hh)
        self.assertIn("int32_t _m_retries {7};", hh)
        self.assertIn("bool _m_enabled {true};", hh)
        self.assertIn("v.level() < static_cast<int16_t>(0) || v.level() > static_cast<int16_t>(10)", cc)
        self.assertIn("inline constexpr int8_t SMALLEST = static_cast<int8_t>(-128);", hh)

    def test_idl4_only_in_cxx(self):
        cases = {
            "int8": "module U { struct S { int8 x; }; };",
            "bitmask": "module U { bitmask F { A }; };",
            "bitset": "module U { bitset B { bitfield<2> a; }; };",
        }
        for name, text in cases.items():
            path = write_idl("Idl4Only_%s.idl" % name, text)
            for backend in ("-bpython", "-bjava", "-brust"):
                with self.subTest(name, backend=backend):
                    self.assertEqual(cli.main([backend, "-C", OUT, path]), 1)
            with self.subTest(name, backend="-bc++11"):
                self.assertEqual(cli.main(["-bc++11", "-C", OUT, path]), 0)

    def test_abstract_interfaces(self):
        path = write_idl(
            "Abstracts.idl",
            """
            module Abs {
              abstract interface A { string a(); };
              abstract interface B : A { long b(); };
              interface I : B { void i(); };
              interface J : I, A { void j(); };
              interface K : A {};
              valuetype V supports B { public long x; };
              valuetype W supports A, J { public long y; };
              interface Use { A swap(in B b, inout A x, out B y); };
            };
        """,
        )
        outdir = os.path.join(OUT, "abstracts")
        self.assertEqual(cli.main(["-bc++11", "-C", outdir, path]), 0)
        header = read(os.path.join(outdir, "Abstracts.hh"))
        source = read(os.path.join(outdir, "Abstracts.cc"))
        self.assertIn("class A : public virtual ::CORBA::AbstractBase {", header)
        self.assertIn("class B : public virtual ::Abs::A {", header)
        self.assertIn("virtual std::string a() = 0;", header)
        # The stub of a reference received as B implements A's operations.
        self.assertIn(
            "class B::_rorb_stub : public virtual B, public virtual ::CORBA::Object {", header
        )
        self.assertIn("std::string Abs::B::_rorb_stub::a() {", source)
        self.assertIn("int32_t Abs::B::_rorb_stub::b() {", source)
        # The first concrete interface implements the abstract operations,
        # in its stub and its skeleton.
        self.assertIn(
            "class I : public virtual ::Abs::B, public virtual ::CORBA::Object {", header
        )
        self.assertIn("class J : public virtual ::Abs::I, public virtual ::Abs::A {", header)
        self.assertIn("std::string Abs::I::a() {", source)
        self.assertNotIn("std::string Abs::J::a() {", source)
        self.assertIn("class I : public virtual ::PortableServer::ServantBase {", header)
        self.assertIn('if (op == "a") {', source)
        self.assertNotIn("_rorb_ops_Abs_A", source)
        self.assertIn("::CORBA::TCKind::tk_abstract_interface", source)
        # A valuetype is a value of the abstract interfaces it supports.
        self.assertIn(
            "class V : public virtual ::CORBA::ValueBase, public virtual ::Abs::B {", header
        )
        self.assertIn(
            "class W : public virtual ::CORBA::ValueBase, public virtual ::Abs::A {", header
        )
        self.assertIn(
            "class W : public virtual ::POA_Abs::J, public virtual ::Abs::W {", header
        )
        self.assertNotIn("class V : public virtual ::POA_Abs", header)
        if CXX:
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
                    "-I" + outdir,
                    os.path.join(outdir, "Abstracts.cc"),
                ],
                check=True,
            )

    def test_value_behaviour(self):
        path = write_idl(
            "Behave.idl",
            """
            module Behave {
              interface Meter { long reading(); };
              custom valuetype Tagged { public long tag; };
              valuetype Gauge supports Meter {
                public long level;
                long scaled(in long by);
                factory init(in long level);
              };
            };
        """,
        )
        outdir = os.path.join(OUT, "behave")
        self.assertEqual(cli.main(["-bc++11", "-C", outdir, path]), 0)
        header = read(os.path.join(outdir, "Behave.hh"))
        self.assertIn(
            "class Tagged : public virtual ::CORBA::ValueBase, public virtual ::CORBA::CustomMarshal {",
            header,
        )
        # Operations, those of the supported interface included, are pure.
        self.assertIn("virtual int32_t scaled(int32_t by) = 0;", header)
        self.assertIn("virtual int32_t reading() = 0;", header)
        self.assertIn("class Gauge_init : public ::CORBA::ValueFactoryBase {", header)
        self.assertIn(
            "virtual ::IDL::traits<::Behave::Gauge>::ref_type init(int32_t level) = 0;", header
        )
        self.assertIn(
            "class Gauge : public virtual ::POA_Behave::Meter, public virtual ::Behave::Gauge {",
            header,
        )

    def test_fixed(self):
        header = read(os.path.join(self.outdir, "CxxMapping.hh"))
        source = read(os.path.join(self.outdir, "CxxMapping.cc"))
        self.assertIn("using Money = ::IDL::Fixed<5, 2>;", header)
        # A constant of type fixed has the digits and scale of its literal.
        self.assertIn('inline const ::IDL::Fixed<4, 2> C_FIXED = ::IDL::Fixed<4, 2>("12.5");', header)
        self.assertIn('inline const ::CxxMapping::Money C_MONEY = ::IDL::Fixed<5, 2>("-1.5");', header)
        self.assertIn("::IDL::Fixed<3, 0>& whole", header)
        self.assertIn("::rustorb::cdr::type_code<::IDL::Fixed<31, 31>>::get()", source)
        path = write_idl("FixedRange.idl", "module U { typedef fixed<3, 1> F; const F C = 123.4d; };")
        self.assertEqual(cli.main(["-bc++11", "-C", OUT, path]), 1)

    def test_valuetypes(self):
        path = write_idl(
            "Values.idl",
            """
            module Values {
              valuetype Later;
              abstract valuetype Named {};
              valuetype Base { public long a; private string from; };
              valuetype Derived : truncatable Base, Named {
                struct In { long q; };
                public In inner;
                public Later later;
              };
              valuetype Later { public Later next; };
              valuetype Box sequence<long>;
              interface Store { Derived swap(in Base b, inout Box x, out Later l); };
            };
        """,
        )
        outdir = os.path.join(OUT, "values")
        self.assertEqual(cli.main(["-bc++11", "-C", outdir, path]), 0)
        header = read(os.path.join(outdir, "Values.hh"))
        source = read(os.path.join(outdir, "Values.cc"))
        self.assertIn(
            "class Derived : public virtual ::Values::Base, public virtual ::Values::Named {", header
        )
        self.assertIn("virtual const ::Values::Derived::In& inner() const = 0;", header)
        # Private members have protected accessors.
        protected = header.index("protected:", header.index("class Base"))
        self.assertGreater(header.index("virtual std::string& from() = 0;"), protected)
        self.assertIn("namespace OBV_Values {", header)
        self.assertIn(
            "Derived(int32_t a, const std::string& from, const ::Values::Derived::In& inner,"
            " ::IDL::traits<::Values::Later>::ref_type later)",
            header,
        )
        self.assertIn("explicit Box(std::vector<int32_t> _v)", header)
        self.assertIn(
            '_ids{"IDL:Values/Derived:1.0", "IDL:Values/Base:1.0"};', source
        )
        self.assertIn('::rustorb::detail::tc_recursive("IDL:Values/Later:1.0")', source)
        self.assertIn("::CORBA::VM_TRUNCATABLE, ::Values::_rorb_tc_Base()", source)
        self.assertIn("::CORBA::VM_ABSTRACT, nullptr, {}", source)
        self.assertIn(
            'ValueRegistration _rorb_value_0("IDL:Values/Named:1.0", nullptr,', source
        )
        self.assertIn("DefaultValueFactory<::OBV_Values::Later>", source)
        self.assertIn("DefaultValueFactory<::Values::Box>", source)
        if CXX:
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
                    "-I" + outdir,
                    os.path.join(outdir, "Values.cc"),
                ],
                check=True,
            )

    @unittest.skipUnless(CXX, "no C++ compiler")
    def test_compiles(self):
        for base in ("CxxMapping", "CosNaming", "InteropTest", "AnyTest"):
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
