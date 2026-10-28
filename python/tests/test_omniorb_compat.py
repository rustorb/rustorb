"""Tests of the omniORBpy compatibility package (python/omniorb-compat):
``from omniORB import ...`` on rustorb.

The expected values (descriptors of ``findType``, the bytes of
``cdrMarshal``, the TypeCodes ``any.to_any`` chooses) are what omniORBpy
4.3 gives for the same calls, measured by running it. Run like
test_orb.py, with the native module built and idl_parser importable.
"""

import filecmp
import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
COMPAT = os.path.join(ROOT, "python", "omniorb-compat")
for p in (
    COMPAT,
    os.path.join(ROOT, "python"),
    os.path.join(ROOT, "crates", "rustorb-codegen", "python"),
):
    if p not in sys.path:
        sys.path.insert(0, p)

from rustorb import CORBA as RCORBA  # noqa: E402

try:
    from rustorb_idl import cli
    import idl_parser  # noqa: F401
except ImportError:
    cli = None

MISSING = None
if RCORBA._native is None:
    MISSING = "the native module is not built (python/build-native.sh)"
elif cli is None:
    MISSING = "idl_parser is not importable"
if MISSING and os.environ.get("RUSTORB_REQUIRE_NATIVE"):
    raise ImportError(MISSING)

OUT = tempfile.mkdtemp(prefix="rustorb-compat-test-")

orb = None
C = C__POA = None


def setUpModule():
    global orb, C, C__POA
    if MISSING:
        raise unittest.SkipTest(MISSING)
    idl = os.path.join(HERE, "idl", "CompatTest.idl")
    if cli.main(["-bpython", "-C", OUT, idl]) != 0:
        raise AssertionError("rustorb-idl failed for %s" % idl)
    sys.path.insert(0, OUT)
    import CompatTest
    import CompatTest__POA
    from omniORB import CORBA

    C, C__POA = CompatTest, CompatTest__POA
    orb = CORBA.ORB_init(
        ["prog", "-ORBendPoint", "giop:tcp:127.0.0.1:", "-ORBendPointPublish", "all(addr)"],
        "compat",
    )
    orb.resolve_initial_references("RootPOA")._get_the_POAManager().activate()


def tearDownModule():
    if orb is not None:
        orb.destroy()


def time_desc():
    return (15, C.Time, "IDL:CompatTest/Time:1.0", "Time", "sec", 5, "nsec", 5)


class ModuleTest(unittest.TestCase):
    def test_same_modules(self):
        import omniORB
        import omniORB.CORBA
        import omniORB.PortableServer
        import CORBA
        import PortableServer
        from rustorb import CORBA as R
        from rustorb import PortableServer as RP

        self.assertIs(omniORB.CORBA, R)
        self.assertIs(sys.modules["omniORB.CORBA"], R)
        self.assertIs(omniORB.PortableServer, RP)
        self.assertIs(CORBA, R)
        self.assertIs(PortableServer, RP)

    def test_nil(self):
        from omniORB import CORBA

        self.assertIsNone(CORBA.Object._nil)
        self.assertIsNone(C.Echo._nil)

    def test_minor_code(self):
        import omniORB

        self.assertEqual(omniORB.COMM_FAILURE_WaitingForReply, 0x41540007)

    def test_cosnaming(self):
        import CosNaming

        self.assertEqual(CosNaming.NamingContext._NP_RepositoryId,
                         "IDL:omg.org/CosNaming/NamingContext:1.0")

    def test_cosnaming_stubs_up_to_date(self):
        out = tempfile.mkdtemp(prefix="rustorb-compat-cosnaming-")
        idl = os.path.join(ROOT, "crates", "rustorb-codegen", "tests", "idl", "CosNaming.idl")
        self.assertEqual(cli.main(["-bpython", "-C", out, idl]), 0)
        for rel in ("CosNaming_idl.py", "CosNaming/__init__.py", "CosNaming/__init__.pyi",
                    "CosNaming__POA/__init__.py", "CosNaming__POA/__init__.pyi"):
            self.assertTrue(
                filecmp.cmp(os.path.join(out, rel), os.path.join(COMPAT, rel), shallow=False),
                "%s is out of date: run python/omniorb-compat/generate-stubs.sh" % rel,
            )


class FindTypeTest(unittest.TestCase):
    def test_structs(self):
        from omniORB import findType

        self.assertEqual(findType("IDL:CompatTest/Time:1.0"), time_desc())
        self.assertEqual(
            findType("IDL:CompatTest/TimedLong:1.0"),
            (15, C.TimedLong, "IDL:CompatTest/TimedLong:1.0", "TimedLong",
             "tm", time_desc(), "data", 3),
        )
        self.assertEqual(findType("IDL:CompatTest/TimedString:1.0")[-1], (18, 0))
        self.assertEqual(findType("IDL:CompatTest/TimedWString:1.0")[-1], (27, 0))
        self.assertEqual(findType("IDL:CompatTest/TimedShortSeq:1.0")[-1], (19, 2, 0))
        state = (17, "IDL:CompatTest/State:1.0", "State", (C.IDLE, C.ACTIVE, C.ERROR))
        self.assertEqual(findType("IDL:CompatTest/TimedState:1.0")[-1], state)
        self.assertEqual(
            findType("IDL:CompatTest/Mixed:1.0"),
            (15, C.Mixed, "IDL:CompatTest/Mixed:1.0", "Mixed",
             "h", (21, "IDL:CompatTest/Handle:1.0", "Handle", 3),
             "s", state,
             "e", (14, "IDL:CompatTest/Echo:1.0", "Echo"),
             "arr", (20, 3, 2),
             "a", 11),
        )

    def test_other_types(self):
        from omniORB import findType

        name_value = (15, C.NameValue, "IDL:CompatTest/NameValue:1.0", "NameValue",
                      "name", (18, 0), "value", 11)
        self.assertEqual(findType("IDL:CompatTest/NameValue:1.0"), name_value)
        self.assertEqual(findType("IDL:CompatTest/NVList:1.0"),
                         (21, "IDL:CompatTest/NVList:1.0", "NVList", (19, name_value, 0)))
        self.assertEqual(findType("IDL:CompatTest/Echo:1.0"),
                         (14, "IDL:CompatTest/Echo:1.0", "Echo"))
        self.assertIsNone(findType("IDL:CompatTest/Nothing:1.0"))

    def test_tc_internal(self):
        from omniORB import tcInternal

        self.assertEqual((tcInternal.tv_struct, tcInternal.tv_alias, tcInternal.tv_wstring),
                         (15, 21, 27))


class AnyTest(unittest.TestCase):
    def kind(self, value):
        from omniORB import CORBA, any

        tc = any.to_any(value).typecode()
        return tc.kind(), tc

    def test_to_any_kinds(self):
        from omniORB import CORBA, any

        cases = [
            (5, CORBA.tk_long),
            (-1, CORBA.tk_long),
            (2**33, CORBA.tk_longlong),
            (2**63, CORBA.tk_ulonglong),
            (1.5, CORBA.tk_double),
            (True, CORBA.tk_boolean),
            ("s", CORBA.tk_string),
            ("あ", CORBA.tk_string),
            (None, CORBA.tk_null),
            (b"ab", CORBA.tk_alias),
            (C.ACTIVE, CORBA.tk_enum),
            (C.Time(1, 2), CORBA.tk_struct),
            ({"k": 1}, CORBA.tk_struct),
        ]
        for value, kind in cases:
            self.assertIs(self.kind(value)[0], kind, value)
        self.assertEqual(any.to_any(C.Time(1, 2)).typecode().id(), "IDL:CompatTest/Time:1.0")
        self.assertEqual(any.to_any(b"").typecode().id(), "IDL:omg.org/CORBA/OctetSeq:1.0")
        for value in (2**64, 3 + 4j, CORBA.TC_long):
            self.assertRaises(CORBA.BAD_PARAM, any.to_any, value)
        a = CORBA.Any(CORBA.TC_long, 3)
        self.assertIs(any.to_any(a).typecode(), a.typecode())

    def test_to_any_sequences(self):
        from omniORB import CORBA, any

        k, tc = self.kind([True])
        self.assertIs(k, CORBA.tk_sequence)
        self.assertIs(tc.content_type().kind(), CORBA.tk_boolean)
        self.assertIs(self.kind([[1, 2], [3]])[1].content_type().kind(), CORBA.tk_sequence)
        self.assertIs(self.kind((C.Time(1, 2),))[1].content_type().kind(), CORBA.tk_struct)
        a = any.to_any(["a", 1])
        self.assertIs(a.typecode().content_type().kind(), CORBA.tk_any)
        self.assertEqual(a.value(), [CORBA.Any(CORBA.TC_string, "a"), CORBA.Any(CORBA.TC_long, 1)])

    def test_from_any(self):
        from omniORB import CORBA, any

        a = CORBA.Any(C._tc_TimedLong, C.TimedLong(C.Time(1, 2), 3))
        self.assertEqual(any.from_any(a), {"tm": {"sec": 1, "nsec": 2}, "data": 3})
        self.assertEqual(any.from_any(a, keep_structs=True), C.TimedLong(C.Time(1, 2), 3))
        nv = CORBA.Any(C._tc_NVList, [C.NameValue("a", any.to_any("b"))])
        self.assertEqual(any.from_any(nv), [{"name": "a", "value": "b"}])
        kept = any.from_any(nv, keep_structs=True)
        self.assertEqual(kept, [C.NameValue("a", CORBA.Any(CORBA.TC_string, "b"))])
        self.assertEqual(any.from_any(any.to_any(CORBA.Any(CORBA.TC_long, 3))), 3)
        self.assertEqual(any.from_any(CORBA.Any(CORBA.TC_any, any.to_any(4))), 4)
        self.assertEqual(any.from_any(any.to_any((1, "x"))), [1, "x"])
        self.assertEqual(any.from_any(any.to_any({"a": "b", "c": [1]})), {"a": "b", "c": [1]})
        self.assertEqual(any.from_any(CORBA.Any(C._tc_State, C.ERROR)), C.ERROR)

    def test_any_through_the_orb(self):
        # A dict made into a struct crosses the ORB like any other.
        from omniORB import CORBA, any, cdrMarshal, cdrUnmarshal

        a = any.to_any({"a": "b", "c": [1, 2]})
        back = cdrUnmarshal(CORBA.TC_any, cdrMarshal(CORBA.TC_any, a))
        self.assertEqual(any.from_any(back), {"a": "b", "c": [1, 2]})


class CdrTest(unittest.TestCase):
    def test_bytes(self):
        from omniORB import CORBA, any, cdrMarshal

        little = sys.byteorder == "little"
        self.assertEqual(cdrMarshal(CORBA.TC_ulong, 7, True), b"\x07\x00\x00\x00")
        self.assertEqual(cdrMarshal(CORBA.TC_ulong, 7, False), b"\x00\x00\x00\x07")
        encap = cdrMarshal(CORBA.TC_ulong, 7)
        self.assertEqual(encap[0], 1 if little else 0)
        self.assertEqual(encap[4:], cdrMarshal(CORBA.TC_ulong, 7, little))
        self.assertEqual(
            cdrMarshal(C._tc_TimedLong, C.TimedLong(C.Time(1, 2), 3), True),
            b"\x01\x00\x00\x00\x02\x00\x00\x00\x03\x00\x00\x00",
        )
        self.assertEqual(
            cdrMarshal(C._tc_NVList, [C.NameValue("a", any.to_any("b"))], True),
            b"\x01\x00\x00\x00\x02\x00\x00\x00a\x00\x00\x00\x12\x00\x00\x00"
            b"\x00\x00\x00\x00\x02\x00\x00\x00b\x00",
        )
        # ISO 8859-1 and UTF-16, as omniORB.
        self.assertEqual(cdrMarshal(CORBA.TC_string, "\xe9", True), b"\x02\x00\x00\x00\xe9\x00")
        self.assertEqual(cdrMarshal(CORBA.TC_wstring, "あ", True),
                         b"\x04\x00\x00\x00\xff\xfeB0")
        self.assertRaises(CORBA.DATA_CONVERSION, cdrMarshal, CORBA.TC_string, "あ", True)
        self.assertRaises(CORBA.BAD_PARAM, cdrMarshal, CORBA.TC_ulong, "x", True)

    def test_round_trip(self):
        from omniORB import CORBA, cdrMarshal, cdrUnmarshal

        values = [
            (C._tc_TimedLong, C.TimedLong(C.Time(1, 2), -3)),
            (C._tc_TimedString, C.TimedString(C.Time(1, 2), "caf\xe9")),
            (C._tc_TimedWString, C.TimedWString(C.Time(1, 2), "あい")),
            (C._tc_TimedShortSeq, C.TimedShortSeq(C.Time(1, 2), [1, -2, 3])),
            (C._tc_TimedState, C.TimedState(C.Time(1, 2), C.ERROR)),
        ]
        for tc, value in values:
            for endian in (-1, True, False):
                self.assertEqual(cdrUnmarshal(tc, cdrMarshal(tc, value, endian), endian), value)

    def test_errors(self):
        from omniORB import CORBA, cdrUnmarshal

        self.assertRaises(CORBA.MARSHAL, cdrUnmarshal, CORBA.TC_ulong, b"\x07\x00\x00\x00\x00",
                          True)
        self.assertRaises(CORBA.MARSHAL, cdrUnmarshal, CORBA.TC_ulong, b"\x07\x00", True)
        self.assertRaises(TypeError, cdrUnmarshal, CORBA.TC_ulong, "\x07\x00\x00\x00", True)

    def test_object_reference(self):
        from omniORB import CORBA, cdrMarshal, cdrUnmarshal

        poa = orb.resolve_initial_references("RootPOA")
        obj = poa.servant_to_reference(make_echo())
        back = cdrUnmarshal(C._tc_Echo, cdrMarshal(C._tc_Echo, obj))
        self.assertTrue(back._is_equivalent(obj))
        self.assertEqual(back.say("hi"), "hi")
        self.assertIsNone(cdrUnmarshal(C._tc_Echo, cdrMarshal(C._tc_Echo, None)))


def make_echo():
    class Echo(C__POA.Echo):
        def say(self, s):
            return s

    return Echo()


class InsPoaTest(unittest.TestCase):
    def test_object_key_is_the_id(self):
        from omniORB import CORBA, PortableServer

        ins = orb.resolve_initial_references("omniINSPOA")
        self.assertIs(orb.resolve_initial_references("omniINSPOA"), ins)
        self.assertEqual(ins._get_the_name(), "omniINSPOA")
        self.assertEqual(ins._get_the_parent()._get_the_name(), "RootPOA")
        manager = ins._get_the_POAManager()
        self.assertEqual(manager.get_state(), PortableServer.POAManager.HOLDING)
        manager.activate()
        self.assertRaises(PortableServer.POA.WrongPolicy, ins.activate_object, make_echo())

        servant = make_echo()
        ins.activate_object_with_id("example.Echo0", servant)
        self.assertEqual(ins.servant_to_id(servant), b"example.Echo0")
        ref = ins.id_to_reference("example.Echo0")
        port = orb._native.endpoints()[0][1]
        loc = orb.string_to_object("corbaloc::127.0.0.1:%d/example.Echo0" % port)
        echo = loc._narrow(C.Echo)
        self.assertEqual(echo.say("x"), "x")
        self.assertTrue(ref._is_equivalent(ins.servant_to_reference(servant)))
        self.assertEqual(ref._narrow(C.Echo).say("y"), "y")

        ins.deactivate_object("example.Echo0")
        self.assertTrue(loc._non_existent())

    def test_str_ids(self):
        poa = orb.resolve_initial_references("RootPOA")
        child = poa.create_POA(
            "compat-user-ids", None, [poa.create_id_assignment_policy(
                __import__("rustorb").PortableServer.USER_ID)]
        )
        servant = make_echo()
        child.activate_object_with_id("\xe9", servant)
        self.assertEqual(child.servant_to_id(servant), b"\xc3\xa9")
        self.assertIs(child.id_to_servant("\xe9"), servant)
        self.assertIs(child.id_to_servant(b"\xc3\xa9"), servant)
        child.deactivate_object("\xe9")
        child.destroy(False, True)


if __name__ == "__main__":
    unittest.main()
