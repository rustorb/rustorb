#!/usr/bin/env bash
# Regenerates the CosNaming stubs of librustorb_c (include/rustorb/CosNaming.h
# and src/CosNaming.c) with rustorb-idl -bc. rustorb-idl must be importable
# (tools/rustorb-idl.txt), or RUSTORB_IDL_DIR / IDL_PARSER_DIR set to
# checkouts. CI checks that the committed files match.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
pythonpath="${RUSTORB_IDL_DIR:+$RUSTORB_IDL_DIR/python}"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR${pythonpath:+:$pythonpath}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
idl="$(PYTHONPATH="$pythonpath" python3 -m rustorb_idl --idl-dir)/CosNaming.idl"
PYTHONPATH="$pythonpath" python3 -m rustorb_idl -bc -C "$tmp" "$idl"
cp "$tmp/CosNaming.h" "$here/include/rustorb/CosNaming.h"
cp "$tmp/CosNaming.c" "$here/src/CosNaming.c"
