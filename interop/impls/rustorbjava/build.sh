#!/usr/bin/env bash
# Builds the rustorb Java client, server, POA probe server and DynAny probe on
# the host:
# librustorb_jni (in lib/) and, in classes/, the rustorb Java runtime
# (java/), the stubs and skeletons of InteropTest.idl, AnyTest.idl,
# AnyTestHidden.idl, ValueTest.idl and PoaProbe.idl from rustorb-idl -bjava (in gen/), the
# Java sources shared with the jacorb implementation (impls/java) and the
# probes (probe/). run-matrix.sh, run-poa-probe.sh and run-dynany-probe.sh run it;
# idl_parser must be importable, or IDL_PARSER_DIR set to a checkout of it.
# The classes are for Java 11, and the interop image has a Java 17 runtime.
#
# librustorb_jni runs in the interop image (Ubuntu 24.04), so the host needs
# a glibc no newer than that image's.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../../.."

cargo build --release --locked -p rustorb-jni --manifest-path "$root/Cargo.toml"

rm -rf "$here/gen" "$here/classes" "$here/lib"
mkdir -p "$here/gen" "$here/classes" "$here/lib"
pythonpath="$root/crates/rustorb-codegen/python"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR:$pythonpath"
PYTHONPATH="$pythonpath" python3 -m rustorb_idl -bjava -I "$root/interop/idl" -C "$here/gen" \
  "$root/interop/idl/InteropTest.idl" "$root/interop/idl/AnyTest.idl" \
  "$root/interop/idl/AnyTestHidden.idl" "$root/interop/idl/ValueTest.idl" \
  "$root/interop/idl/PoaProbe.idl"
find "$root/java/src/main/java" "$here/gen" "$here/../java" "$here/probe" -name '*.java' \
  > "$here/gen/sources.txt"
javac --release 11 -encoding UTF-8 -nowarn -d "$here/classes" @"$here/gen/sources.txt"
cp "$root/target/release/librustorb_jni.so" "$here/lib/"
