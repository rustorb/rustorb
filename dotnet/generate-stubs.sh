#!/usr/bin/env bash
# Regenerates the stubs of the runtime in src/Rustorb.Corba with rustorb-idl
# -bcsharp: CosNaming (from tests/mapping/idl/CosNaming.idl). Needs
# rustorb-idl importable (tools/rustorb-idl.txt), or RUSTORB_IDL_DIR set to a
# checkout of it (and IDL_PARSER_DIR to one of idl_parser). CI checks that
# the committed stubs are up to date (it runs this and git diff).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/.."
out="${1:-$here/src/Rustorb.Corba/CosNaming}"
pythonpath="${RUSTORB_IDL_DIR:+$RUSTORB_IDL_DIR/python}"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR${pythonpath:+:$pythonpath}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
PYTHONPATH="$pythonpath${PYTHONPATH:+${pythonpath:+:}$PYTHONPATH}" python3 -m rustorb_idl -bcsharp \
  -C "$tmp" "$root/tests/mapping/idl/CosNaming.idl"
mkdir -p "$out"
cp "$tmp/CosNaming.cs" "$out/CosNaming.g.cs"
