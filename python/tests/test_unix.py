"""Tests of ``giop:unix`` endpoints (omniORB's Unix domain sockets, see
docs/unix-transport.md) through the ORB_init options of the Python
binding: a server listening on a Unix socket only, called from another
ORB in the process.

Without the native module the tests are skipped, unless
``RUSTORB_REQUIRE_NATIVE`` is set (as in CI). Unix only.
"""

import os
import shutil
import stat
import sys
import tempfile
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
if os.name != "posix":
    MISSING = "giop:unix is for Linux and macOS"


class Nothing(PortableServer.Servant):
    _NP_RepositoryId = "IDL:test/Nothing:1.0"


@unittest.skipIf(MISSING, MISSING)
class UnixTest(unittest.TestCase):
    def setUp(self):
        # Under /tmp: a socket path has at most 104 bytes on macOS.
        self.dir = tempfile.mkdtemp(prefix="rustorb-unix-", dir="/tmp")
        self.addCleanup(shutil.rmtree, self.dir, True)
        self.path = os.path.join(self.dir, "s.sock")

    def test_call_over_a_unix_socket(self):
        server = CORBA.ORB_init(
            ["test", "-ORBendPoint", "giop:unix:" + self.path], "test_unix_server")
        root = server.resolve_initial_references("RootPOA")
        root._get_the_POAManager().activate()
        ior = server.object_to_string(root.servant_to_reference(Nothing()))
        self.assertTrue(stat.S_ISSOCK(os.lstat(self.path).st_mode))

        client = CORBA.ORB_init(["test"], "test_unix_client")
        obj = client.string_to_object(ior)
        self.assertFalse(obj._non_existent())
        self.assertTrue(obj._is_a("IDL:test/Nothing:1.0"))
        client.destroy()
        server.destroy()
        # The socket goes with the ORB.
        self.assertFalse(os.path.lexists(self.path))

    def test_a_file_in_the_way(self):
        with open(self.path, "w") as f:
            f.write("keep me")
        orb = CORBA.ORB_init(
            ["test", "-ORBendPoint", "giop:unix:" + self.path], "test_unix_in_the_way")
        self.addCleanup(orb.destroy)
        with self.assertRaises(CORBA.INITIALIZE):
            orb.resolve_initial_references("RootPOA")
        with open(self.path) as f:
            self.assertEqual(f.read(), "keep me")


if __name__ == "__main__":
    unittest.main()
