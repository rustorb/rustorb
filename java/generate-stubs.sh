#!/usr/bin/env bash
# Regenerates the stubs of the runtime in src/main/java with rustorb-idl
# -bjava: the CosNaming ones (org.omg.CosNaming), and org.omg.IOP,
# org.omg.Dynamic and org.omg.PortableInterceptor with the few
# org.omg.CORBA classes they need that the runtime does not write by hand
# (from idl/PortableInterceptor.idl). Needs idl_parser importable (or
# IDL_PARSER_DIR set to a checkout of it). CI checks that the committed stubs
# are up to date (it runs this and git diff).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/.."
out="${1:-$here/src/main/java}"
pythonpath="$root/crates/rustorb-codegen/python"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR:$pythonpath"
idl() {
  PYTHONPATH="$pythonpath${PYTHONPATH:+:$PYTHONPATH}" python3 -m rustorb_idl -bjava \
    -Wbpackage=org.omg "$@"
}

rm -rf "$out/org/omg/CosNaming"
idl -C "$out" "$root/crates/rustorb-codegen/tests/idl/CosNaming.idl"

# The CORBA classes of the generated ones that the runtime lacks.
corba=(OctetSeqHelper ParameterMode ParameterModeHelper ParameterModeHolder
       PolicyTypeHelper RepositoryIdHelper StringSeqHelper StringSeqHolder)
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
idl -C "$tmp" "$here/idl/PortableInterceptor.idl"
for pkg in IOP Dynamic PortableInterceptor; do
  rm -rf "$out/org/omg/$pkg"
  cp -R "$tmp/org/omg/$pkg" "$out/org/omg/$pkg"
done
for c in "${corba[@]}"; do
  cp "$tmp/org/omg/CORBA/$c.java" "$out/org/omg/CORBA/$c.java"
done
