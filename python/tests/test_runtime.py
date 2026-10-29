"""Tests of the pure-Python parts of the rustorb package.

python3 -m unittest discover -s python/tests
"""

import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from rustorb import CORBA, _rt  # noqa: E402


class FixedTest(unittest.TestCase):
    def test_digits_and_scale(self):
        for text, digits, scale, value in (
            ("12.50", 3, 1, 125),
            ("100", 3, 0, 100),
            ("-0.010", 2, 2, -1),
            ("3.14159", 6, 5, 314159),
            # More digits than the default decimal context keeps (28).
            ("123456789012345678901234567.8901", 31, 4, 1234567890123456789012345678901),
            ("-1234567890123456789012345678901", 31, 0, -1234567890123456789012345678901),
        ):
            f = CORBA.fixed(text)
            self.assertEqual((f.precision(), f.decimals(), f.value()), (digits, scale, value), text)
        self.assertEqual(repr(CORBA.fixed("1.20")), 'fixed("1.2")')
        self.assertEqual(CORBA.fixed("1.20"), CORBA.fixed("1.2"))


class ExceptionTest(unittest.TestCase):
    def test_system_exceptions(self):
        e = CORBA.BAD_PARAM(42, CORBA.COMPLETED_MAYBE)
        self.assertIsInstance(e, CORBA.SystemException)
        self.assertIsInstance(e, CORBA.Exception)
        self.assertEqual((e.minor, e.completed), (42, CORBA.COMPLETED_MAYBE))
        self.assertEqual(e._NP_RepositoryId, "IDL:omg.org/CORBA/BAD_PARAM:1.0")
        self.assertIs(CORBA.system_exceptions["IDL:omg.org/CORBA/TRANSIENT:1.0"], CORBA.TRANSIENT)
        self.assertEqual(CORBA.OBJECT_NOT_EXIST(), CORBA.OBJECT_NOT_EXIST(0, CORBA.COMPLETED_NO))


class TypeCodeTest(unittest.TestCase):
    def test_basic(self):
        self.assertIs(CORBA.TC_long.kind(), CORBA.tk_long)
        self.assertEqual(CORBA.TC_Object.id(), "IDL:omg.org/CORBA/Object:1.0")
        self.assertEqual(_rt.tc_string(5).length(), 5)
        self.assertTrue(_rt.tc_string(0).equal(CORBA.TC_string))
        self.assertRaises(CORBA.TypeCode.BadKind, CORBA.TC_long.id)
        self.assertRaises(CORBA.TypeCode.BadKind, CORBA.TC_long.length)

    def test_alias(self):
        alias = CORBA.TypeCode(CORBA.tk_alias, "IDL:T:1.0", "T", content=CORBA.TC_long)
        self.assertFalse(alias.equal(CORBA.TC_long))
        self.assertTrue(alias.equivalent(CORBA.TC_long))

    def test_bounds(self):
        tc = CORBA.TypeCode(CORBA.tk_struct, "IDL:S:1.0", "S", members=[("a", CORBA.TC_long)])
        self.assertRaises(CORBA.TypeCode.Bounds, tc.member_name, 1)


class ModuleTest(unittest.TestCase):
    def test_open_module(self):
        m = _rt.open_module("RustorbTestOuter.Inner")
        self.assertIs(_rt.open_module("RustorbTestOuter").Inner, m)
        self.assertIs(_rt.open_module("RustorbTestOuter.Inner"), m)

    def test_check_version(self):
        _rt.check_version(_rt.GENERATED_CODE_VERSION)
        self.assertRaises(ImportError, _rt.check_version, _rt.GENERATED_CODE_VERSION + 1)


if __name__ == "__main__":
    unittest.main()
