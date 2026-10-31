"""Tests of the rustorb-idl command line that do not depend on a backend.

Run from the repository root with idl_parser importable::

    PYTHONPATH=path/to/idl_parser python3 -m unittest discover -s crates/rustorb-codegen/python/tests
"""

import contextlib
import io
import os
import shutil
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", "..", ".."))
CODEGEN = os.path.join(ROOT, "crates", "rustorb-codegen", "python")
TEST_IDL = os.path.join(ROOT, "crates", "rustorb-codegen", "tests", "idl")

if CODEGEN not in sys.path:
    sys.path.insert(0, CODEGEN)

from rustorb_idl import cli  # noqa: E402


class IdlDirTest(unittest.TestCase):
    def test_prints_the_directory(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(cli.main(["--idl-dir"]), 0)
        self.assertEqual(out.getvalue().strip(), cli.IDL_DIR)
        self.assertTrue(os.path.isfile(os.path.join(cli.IDL_DIR, "CosNaming.idl")))

    def test_bundled_cosnaming_matches_the_test_copy(self):
        # The tests and scripts use tests/idl/CosNaming.idl; the package
        # ships the same file.
        with open(os.path.join(TEST_IDL, "CosNaming.idl"), "rb") as a:
            with open(os.path.join(cli.IDL_DIR, "CosNaming.idl"), "rb") as b:
                self.assertEqual(a.read(), b.read())

    def test_bundled_directory_is_on_the_include_path(self):
        d = tempfile.mkdtemp(prefix="rustorb-idl-cli-")
        self.addCleanup(shutil.rmtree, d)
        idl = os.path.join(d, "UsesNaming.idl")
        with open(idl, "w") as f:
            f.write('#include "CosNaming.idl"\nmodule UsesNaming { interface Finder {\n'
                    "  Object find(in CosNaming::Name n); }; };\n")
        self.assertEqual(cli.main(["-bpython", "-C", d, idl]), 0)
        self.assertTrue(os.path.isdir(os.path.join(d, "UsesNaming")))


if __name__ == "__main__":
    unittest.main()
