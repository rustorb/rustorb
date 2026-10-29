#!/usr/bin/env bash
# Regenerates the CosNaming stubs of the runtime (org.omg.CosNaming, in
# src/main/java) with rustorb-idl -bjava. Needs idl_parser importable (or
# IDL_PARSER_DIR set to a checkout of it). CI checks that the committed stubs
# are up to date (it runs this and git diff).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/.."
out="${1:-$here/src/main/java}"
pythonpath="$root/crates/rustorb-codegen/python"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR:$pythonpath"
rm -rf "$out/org/omg/CosNaming"
PYTHONPATH="$pythonpath${PYTHONPATH:+:$PYTHONPATH}" python3 -m rustorb_idl -bjava \
  -Wbpackage=org.omg -C "$out" "$root/crates/rustorb-codegen/tests/idl/CosNaming.idl"
