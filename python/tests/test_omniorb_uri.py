"""Tests of ``omniORB.URI`` in the omniORBpy compatibility package
(python/omniorb-compat).

The expected values are what omniORBpy 4.3.4 gives for the same calls,
measured by running it. Only arguments of the wrong Python type differ:
omniORBpy raises ``NameError`` there, the package ``CORBA.BAD_PARAM``.
The module is pure Python, so the tests do not need the native module.
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
for p in (os.path.join(ROOT, "python", "omniorb-compat"), os.path.join(ROOT, "python")):
    if p not in sys.path:
        sys.path.insert(0, p)

import CosNaming  # noqa: E402
from omniORB import CORBA, URI  # noqa: E402

NC = CosNaming.NameComponent
InvalidName = CosNaming.NamingContext.InvalidName
InvalidAddress = CosNaming.NamingContextExt.InvalidAddress


def pairs(name):
    return [(nc.id, nc.kind) for nc in name]


class StringToNameTest(unittest.TestCase):
    def test_valid(self):
        cases = {
            "a.b/c\\/d/.e": [("a", "b"), ("c/d", ""), ("", "e")],
            "x": [("x", "")],
            ".": [("", "")],
            ".k": [("", "k")],
            "a\\.b.c": [("a.b", "c")],
            "a\\\\b": [("a\\b", "")],
            "a.\\.": [("a", ".")],
            "\\.": [(".", "")],
            "\\..": [(".", "")],
            "./a": [("", ""), ("a", "")],
            "a/.": [("a", ""), ("", "")],
            ".\\.b": [("", ".b")],
            "a b/\u00e9": [("a b", ""), ("\u00e9", "")],
            # A `.` before an empty kind, in the last component only.
            "a.": [("a", "")],
            "x/a.": [("x", ""), ("a", "")],
            "a\\./b": [("a.", ""), ("b", "")],
        }
        for s, expected in cases.items():
            with self.subTest(s=s):
                name = URI.stringToName(s)
                self.assertEqual(pairs(name), expected)
                self.assertTrue(all(isinstance(nc, NC) for nc in name))

    def test_invalid(self):
        for s in [
            "", "a//b", "/a", "a/", "a.b/", "a.b.c", "..", "a..b",
            "a./b", "a/b./c", "\\../b", "a\\\\./b",
            "a\\", "a.b\\", "\\x", "a.\\x", "\\a", "\\ ", "\\~",
        ]:
            with self.subTest(s=s):
                self.assertRaises(InvalidName, URI.stringToName, s)

    def test_wrong_type(self):
        for s in [123, None, b"a"]:
            with self.subTest(s=s):
                self.assertRaises(CORBA.BAD_PARAM, URI.stringToName, s)


class NameToStringTest(unittest.TestCase):
    def test_valid(self):
        cases = [
            ([NC("a", "b"), NC("c/d", ""), NC("", "e"), NC("", ""), NC("x.y", "z\\")],
             "a.b/c\\/d/.e/./x\\.y.z\\\\"),
            ([NC("a", "")], "a"),
            ([NC("", "")], "."),
            ([NC("", "x")], ".x"),
            ([NC(".", "")], "\\."),
            ([NC("a/b", "c.d")], "a\\/b.c\\.d"),
            ([NC("\u00e9", " ")], "\u00e9. "),
            ((NC("a", "b"),), "a.b"),
        ]
        for name, expected in cases:
            with self.subTest(expected=expected):
                self.assertEqual(URI.nameToString(name), expected)
                self.assertEqual(
                    pairs(URI.stringToName(expected)), [(nc.id, nc.kind) for nc in name]
                )

    def test_invalid(self):
        self.assertRaises(InvalidName, URI.nameToString, [])

    def test_wrong_type(self):
        for name in [[("a", "b")], "a", None, [NC("a", 1)], [NC(b"a", "b")], [NC("a", "b"), None]]:
            with self.subTest(name=name):
                self.assertRaises(CORBA.BAD_PARAM, URI.nameToString, name)


class AddrAndNameToURITest(unittest.TestCase):
    def test_valid(self):
        cases = [
            (":host:2809", "a/b c", "corbaname::host:2809#a/b%20c"),
            ("iiop:1.2@host,:other", "x.y/z\\/w", "corbaname:iiop:1.2@host,:other#x.y/z%5C/w"),
            ("rir:", "a", "corbaname:rir:#a"),
            (":host", "", "corbaname::host"),
            (":h", ".", "corbaname::h#."),
            (":h", "a.", "corbaname::h#a."),
            (":host", "\u00e9~", "corbaname::host#%C3%A9~"),
            # The address is not checked beyond being empty.
            ("host:2809", "a", "corbaname:host:2809#a"),
            ("iiop:host/x", "a", "corbaname:iiop:host/x#a"),
            (":host#a", "a", "corbaname::host#a#a"),
            ("ssliop:host", "a", "corbaname:ssliop:host#a"),
            ("uiop:/tmp/x", "a", "corbaname:uiop:/tmp/x#a"),
            (" ", "a", "corbaname: #a"),
        ]
        for addr, sname, expected in cases:
            with self.subTest(addr=addr, sname=sname):
                self.assertEqual(URI.addrAndNameToURI(addr, sname), expected)

    def test_escapes(self):
        printable = "".join(chr(c) for c in range(32, 127) if chr(c) not in "/.\\")
        self.assertEqual(
            URI.addrAndNameToURI(":h", printable),
            "corbaname::h#%20%21%22%23%24%25%26%27%28%29%2A%2B%2C-0123456789"
            "%3A%3B%3C%3D%3E%3F%40ABCDEFGHIJKLMNOPQRSTUVWXYZ%5B%5D%5E_%60"
            "abcdefghijklmnopqrstuvwxyz%7B%7C%7D~",
        )
        self.assertEqual(
            URI.addrAndNameToURI(":h", "\\/\\.\\\\\x01\x7f\u00ff\U0001F600"),
            "corbaname::h#%5C/%5C.%5C%5C%01%7F%C3%BF%F0%9F%98%80",
        )

    def test_invalid(self):
        self.assertRaises(InvalidAddress, URI.addrAndNameToURI, "", "a")
        for sname in ["a//b", "\\x", "a./b"]:
            with self.subTest(sname=sname):
                self.assertRaises(InvalidName, URI.addrAndNameToURI, ":host", sname)

    def test_wrong_type(self):
        for addr, sname in [("iiop:host", [NC("a", "b")]), (":host", None), (None, "a")]:
            with self.subTest(addr=addr, sname=sname):
                self.assertRaises(CORBA.BAD_PARAM, URI.addrAndNameToURI, addr, sname)


if __name__ == "__main__":
    unittest.main()
