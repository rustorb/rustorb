"""The custom valuetype tests (idl/CustomTest.idl): the Blob
implementation, the CustomEcho servant and the client cases.

omniORBpy does not support custom valuetypes, so only rustorbpy uses it.
"""

from common import CORBA
import CustomTest
import CustomTest__POA
import ValueTest

C, V = CustomTest, ValueTest


class Blob(C.Blob):
    """CustomTest::Blob, marshaled as CustomTest.idl says."""

    def marshal(self, os):
        os.write_long(self.n)
        os.write_string(self.label)
        os.write_Value(self.pt)
        os.write_ulong(len(self.values))
        os.write_long_array(self.values, 0, len(self.values))

    def unmarshal(self, is_):
        self.n = is_.read_long()
        self.label = is_.read_string()
        self.pt = is_.read_Value()
        count = is_.read_ulong()
        self.values = is_.read_long_array([], 0, count)


def register(orb):
    """Registers the value factory of Blob with orb."""
    orb.register_value_factory(C.Blob._NP_RepositoryId, lambda: Blob.__new__(Blob))


class CustomEchoServant(CustomTest__POA.CustomEcho):

    def __init__(self, orb):
        self.orb = orb

    def echo_blob(self, b):
        return b

    def echo_blob_pair(self, p):
        return p

    def echo_blobs(self, s):
        return s

    def shutdown(self):
        self.orb.shutdown(False)


def run_cases(r, echo, check):
    """Runs the custom valuetype cases with the Runner r against the
    CustomEcho echo."""

    def blob(b, n, label, values):
        check(isinstance(b, Blob), "got %r" % (b,))
        check((b.n, b.label, list(b.values)) == (n, label, values),
              "got %r" % ((b.n, b.label, b.values),))

    def plain():
        got = echo.echo_blob(Blob(7, "seven", V.Point(1, -2), [1, 2, 3]))
        blob(got, 7, "seven", [1, 2, 3])
        check(type(got.pt) is V.Point and (got.pt.x, got.pt.y) == (1, -2), "got %r" % (got.pt,))

    def empty():
        got = echo.echo_blob(Blob(0, "", None, []))
        blob(got, 0, "", [])
        check(got.pt is None, "got %r" % (got.pt,))

    def derived():
        got = echo.echo_blob(Blob(3, "three", V.Point3(1, 2, 3, "three"), [-1]))
        blob(got, 3, "three", [-1])
        p = got.pt
        check(type(p) is V.Point3, "got %r" % (p,))
        check((p.x, p.y, p.z, p.label) == (1, 2, 3, "three"), "got %r" % (p,))

    r.case("echo_blob[plain]", plain)
    r.case("echo_blob[null]", lambda: check(echo.echo_blob(None) is None))
    r.case("echo_blob[empty]", empty)
    r.case("echo_blob[derived]", derived)

    def pair_shared():
        p = V.Point(5, 6)
        got = echo.echo_blob_pair(C.BlobPair(Blob(1, "one", p, [4, 5]), p))
        blob(got.blob, 1, "one", [4, 5])
        check(got.blob.pt is got.pt, "not one instance")
        check((got.pt.x, got.pt.y) == (5, 6), "got %r" % (got.pt,))

    def pair_distinct():
        got = echo.echo_blob_pair(C.BlobPair(Blob(1, "one", V.Point(5, 6), []), V.Point(5, 6)))
        check(got.blob.pt is not got.pt, "one instance")

    r.case("echo_blob_pair[shared]", pair_shared)
    r.case("echo_blob_pair[distinct]", pair_distinct)

    def blobs():
        b = Blob(2, "two", None, list(range(100)))
        c = Blob(9, "nine", V.Point(9, 9), [])
        got = echo.echo_blobs([b, c, b, None])
        check(len(got) == 4, "got %r" % (got,))
        blob(got[0], 2, "two", list(range(100)))
        blob(got[1], 9, "nine", [])
        check(got[0] is got[2], "sharing lost")
        check(got[0] is not got[1] and got[3] is None, "got %r" % (got,))

    r.case("echo_blobs", blobs)
