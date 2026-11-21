#!/usr/bin/env bash
# Builds the rustorb C client, server and naming check on the host, into
# bin/: librustorb_c (c/, with CMake; it builds librustorb_ffi with cargo),
# the stubs and skeletons of InteropTest.idl and AnyTest.idl (and, for the
# client, AnyTestHidden.idl) from rustorb-idl -bc (in gen/)
# and the sources in src/, linked statically. run-matrix.sh,
# run-naming-check.sh and run-wire-probe.sh run it; rustorb-idl must be
# importable (tools/rustorb-idl.txt), or RUSTORB_IDL_DIR / IDL_PARSER_DIR set
# to checkouts. CC picks the compiler (default cc).
#
# The binaries run in the interop image (Ubuntu 24.04), so the host needs a
# glibc no newer than that image's.
set -euo pipefail

[ -n "${INTEROP_PREBUILT:-}" ] && exit 0

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../../.."
cc="${CC:-cc}"
case "$(uname -s)" in
  Darwin) libs=(-framework CoreFoundation -framework Security -lpthread -lm) ;;
  *) libs=(-static-libgcc -lpthread -ldl -lm) ;;
esac

cmake -S "$root/c" -B "$here/build" -DCMAKE_BUILD_TYPE=Release -DRUSTORB_BUILD_TESTS=OFF \
  -DCMAKE_C_COMPILER="$cc" > /dev/null
cmake --build "$here/build" > /dev/null

rm -rf "$here/gen" "$here/bin"
mkdir -p "$here/gen" "$here/bin"
pythonpath="${RUSTORB_IDL_DIR:+$RUSTORB_IDL_DIR/python}"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR${pythonpath:+:$pythonpath}"
PYTHONPATH="$pythonpath" python3 -m rustorb_idl -bc -I "$root/interop/idl" -C "$here/gen" \
  "$root/interop/idl/InteropTest.idl" "$root/interop/idl/AnyTest.idl" \
  "$root/interop/idl/AnyTestHidden.idl" "$root/interop/idl/PoaProbe.idl"

flags=(-std=c99 -O2 -Wall -Wextra -Wpedantic -I"$root/c/include" -I"$root/crates/rustorb-ffi/include"
       -I"$here/gen" -I"$here/src")
for f in InteropTest AnyTest AnyTestHidden PoaProbe; do
  "$cc" "${flags[@]}" -c "$here/gen/$f.c" -o "$here/gen/$f.o"
done
"$cc" "${flags[@]}" -c "$here/src/common.c" -o "$here/gen/common.o"
"$cc" "${flags[@]}" -c "$here/src/dii_adapters.c" -o "$here/gen/dii_adapters.o"
link() {
  local out=$1
  shift
  "$cc" "${flags[@]}" "$@" "$here/gen/common.o" "$here/gen/dii_adapters.o" "$here/gen/InteropTest.o" "$here/gen/AnyTest.o" \
    "$here/build/librustorb_c.a" "$root/target/release/librustorb_ffi.a" "${libs[@]}" \
    -o "$here/bin/$out"
}
# The server has no code for the types of AnyTestHidden.idl (the stage 2
# suite sends it values of them).
link server "$here/src/server.c" "$here/src/echo_impl.c" "$here/src/anytest.c"
link client "$here/src/client.c" "$here/src/cases.c" "$here/src/anytest.c" \
  "$here/src/anytest_hidden.c" "$here/gen/AnyTestHidden.o"
link naming_check "$here/src/naming_check.c"
link poa_probe_server "$here/src/poa_probe_server.c" "$here/gen/PoaProbe.o"
