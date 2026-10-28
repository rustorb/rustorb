#!/usr/bin/env bash
# Regenerates the CosNaming stubs of the package with rustorb-idl. Needs
# idl_parser importable (or IDL_PARSER_DIR set to a checkout of it). The
# Python tests (python/tests/test_omniorb_compat.py) check that the
# committed stubs are up to date.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../.."
pythonpath="$root/crates/rustorb-codegen/python"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR:$pythonpath"
rm -rf "$here/CosNaming" "$here/CosNaming__POA" "$here/CosNaming_idl.py"
PYTHONPATH="$pythonpath${PYTHONPATH:+:$PYTHONPATH}" python3 -m rustorb_idl -bpython -C "$here" \
  "$root/crates/rustorb-codegen/tests/idl/CosNaming.idl"
