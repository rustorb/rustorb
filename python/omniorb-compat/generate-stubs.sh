#!/usr/bin/env bash
# Regenerates the CosNaming stubs of the package with rustorb-idl. Needs
# rustorb-idl importable (tools/rustorb-idl.txt) (or IDL_PARSER_DIR set to a checkout of it). The
# Python tests (python/tests/test_omniorb_compat.py) check that the
# committed stubs are up to date.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../.."
pythonpath="${RUSTORB_IDL_DIR:+$RUSTORB_IDL_DIR/python}"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR${pythonpath:+:$pythonpath}"
rm -rf "$here/CosNaming" "$here/CosNaming__POA" "$here/CosNaming_idl.py"
PYTHONPATH="$pythonpath${PYTHONPATH:+${pythonpath:+:}$PYTHONPATH}" python3 -m rustorb_idl -bpython -C "$here" \
  "$root/tests/mapping/idl/CosNaming.idl"
