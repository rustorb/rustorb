"""Tests of TLS (SSLIOP) through the ORB_init options of the Python
binding: a server with a TLS endpoint only, called by clients with and
without the test CA and a client certificate (the certificates of
crates/rustorb-core/tests/tls).

Without the native module the tests are skipped, unless
``RUSTORB_REQUIRE_NATIVE`` is set (as in CI).
"""

import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
if os.path.join(ROOT, "python") not in sys.path:
    sys.path.insert(0, os.path.join(ROOT, "python"))

from rustorb import CORBA, PortableServer  # noqa: E402

MISSING = None
if CORBA._native is None:
    MISSING = "the native module is not built (python/build-native.sh)"
    if os.environ.get("RUSTORB_REQUIRE_NATIVE"):
        raise ImportError(MISSING)

CERTS = os.path.join(ROOT, "crates", "rustorb-core", "tests", "tls")


def cert(name):
    return os.path.join(CERTS, name)


class Nothing(PortableServer.Servant):
    _NP_RepositoryId = "IDL:test/Nothing:1.0"


@unittest.skipIf(MISSING, MISSING)
class TlsTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = CORBA.ORB_init(
            ["test", "-ORBendPoint", "giop:ssl:127.0.0.1:",
             "-ORBsslCAFile", cert("ca.pem"),
             "-ORBsslKeyFile", cert("server-bundle.pem"),
             "-ORBsslVerifyClient", "required"],
            "test_tls_server",
        )
        root = cls.server.resolve_initial_references("RootPOA")
        root._get_the_POAManager().activate()
        cls.ior = cls.server.object_to_string(root.servant_to_reference(Nothing()))

    @classmethod
    def tearDownClass(cls):
        cls.server.destroy()

    def client(self, name, *options):
        orb = CORBA.ORB_init(["test"] + list(options), name)
        self.addCleanup(orb.destroy)
        return orb.string_to_object(self.ior)

    def test_with_a_client_certificate(self):
        obj = self.client("test_tls_cert", "-ORBsslCAFile", cert("ca.pem"),
                          "-ORBsslKeyFile", cert("client-bundle.pem"))
        self.assertFalse(obj._non_existent())
        self.assertTrue(obj._is_a("IDL:test/Nothing:1.0"))

    def test_without_a_client_certificate(self):
        obj = self.client("test_tls_nocert", "-ORBsslCAFile", cert("ca.pem"))
        with self.assertRaises((CORBA.TRANSIENT, CORBA.COMM_FAILURE)):
            obj._is_a("IDL:test/Nothing:1.0")

    def test_without_tls(self):
        obj = self.client("test_tls_plain")
        with self.assertRaises(CORBA.TRANSIENT):
            obj._is_a("IDL:test/Nothing:1.0")

    def test_trusting_another_ca(self):
        obj = self.client("test_tls_other", "-ORBsslCAFile", cert("other-ca.pem"),
                          "-ORBsslKeyFile", cert("client-bundle.pem"))
        with self.assertRaises(CORBA.TRANSIENT):
            obj._is_a("IDL:test/Nothing:1.0")

    def test_bad_option(self):
        with self.assertRaises(CORBA.BAD_PARAM):
            CORBA.ORB_init(["test", "-ORBsslVerifyHost", "maybe"], "test_tls_bad")


if __name__ == "__main__":
    unittest.main()
