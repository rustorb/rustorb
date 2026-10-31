#!/usr/bin/env bash
# Builds the rustorb Java client, server, load client, POA probe server,
# DynAny probe and naming check on the host:
# librustorb_jni (in lib/) and, in classes/, the rustorb Java runtime
# (java/), the stubs and skeletons of InteropTest.idl, AnyTest.idl,
# AnyTestHidden.idl, ValueTest.idl, CustomTest.idl, AbstractTest.idl and PoaProbe.idl from rustorb-idl -bjava (in gen/), the
# Java sources shared with the jacorb implementation (impls/java) and the
# probes (probe/). run-matrix.sh, run-poa-probe.sh, run-dynany-probe.sh,
# run-naming-check.sh and run-load-local.py run it;
# idl_parser must be importable, or IDL_PARSER_DIR set to a checkout of it.
# The classes are for Java 11, and the interop image has a Java 17 runtime.
#
# librustorb_jni runs in the interop image (Ubuntu 24.04), so the host needs
# a glibc no newer than that image's.
set -euo pipefail

# Built already (interop.yml builds every implementation once, then sets
# INTEROP_PREBUILT for the steps that run the scripts).
[ -n "${INTEROP_PREBUILT:-}" ] && exit 0

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
  "$root/interop/idl/CustomTest.idl" "$root/interop/idl/AbstractTest.idl" \
  "$root/interop/idl/PoaProbe.idl"
find "$root/java/src/main/java" "$here/gen" "$here/../java" "$here/probe" -name '*.java' \
  > "$here/gen/sources.txt"
javac --release 11 -encoding UTF-8 -nowarn -d "$here/classes" @"$here/gen/sources.txt"
case "$(uname -s)" in
  Darwin) cp "$root/target/release/librustorb_jni.dylib" "$here/lib/" ;;
  *) cp "$root/target/release/librustorb_jni.so" "$here/lib/" ;;
esac
