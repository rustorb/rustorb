#!/usr/bin/env bash
# Builds the rustorb C++ client and server on the host, into bin/: the stubs
# and skeletons of InteropTest.idl, AnyTest.idl, AnyTestHidden.idl (the
# client only) and CosNaming.idl from rustorb-idl
# -bc++11 (in gen/), linked with librustorb_ffi statically. run-matrix.sh
# runs it; idl_parser must be importable, or IDL_PARSER_DIR set to a
# checkout of it. CXX picks the compiler (default g++).
#
# The binaries run in the interop image (Ubuntu 24.04), so the host needs a
# glibc no newer than that image's; libstdc++ is linked statically.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../../.."
cxx="${CXX:-g++}"

cargo build --release --locked -p rustorb-ffi --manifest-path "$root/Cargo.toml"

rm -rf "$here/gen" "$here/bin"
mkdir -p "$here/gen" "$here/bin"
pythonpath="$root/crates/rustorb-codegen/python"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR:$pythonpath"
PYTHONPATH="$pythonpath" python3 -m rustorb_idl -bc++11 -C "$here/gen" \
  "$root/interop/idl/InteropTest.idl" "$root/interop/idl/AnyTest.idl" \
  "$root/interop/idl/AnyTestHidden.idl" "$root/crates/rustorb-codegen/tests/idl/CosNaming.idl"

flags=(-std=c++17 -O2 -Wall -Wextra -I"$root/cpp/include" -I"$root/crates/rustorb-ffi/include"
       -I"$here/gen" -I"$here")
for f in InteropTest AnyTest AnyTestHidden CosNaming; do
  "$cxx" "${flags[@]}" -c "$here/gen/$f.cc" -o "$here/gen/$f.o"
done
objs=("$here/gen/InteropTest.o" "$here/gen/AnyTest.o" "$here/gen/CosNaming.o")
for f in server client; do
  extra=()
  [ "$f" = client ] && extra=("$here/gen/AnyTestHidden.o")
  "$cxx" "${flags[@]}" "$here/$f.cc" "${objs[@]}" "${extra[@]}" \
    "$root/target/release/librustorb_ffi.a" -static-libstdc++ -static-libgcc \
    -lpthread -ldl -lm -o "$here/bin/$f"
done
