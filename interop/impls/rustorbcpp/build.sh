#!/usr/bin/env bash
# Builds the rustorb C++ client, server, load client, POA probe server, DynAny probe,
# naming check and PI probe on the host, into bin/: the stubs and skeletons of InteropTest.idl, AnyTest.idl,
# AnyTestHidden.idl (the client only), ValueTest.idl, CustomTest.idl, AbstractTest.idl, CosNaming.idl, PoaProbe.idl (the
# probe server only) and PiProbe.idl (the PI probe only) from rustorb-idl -bc++11 (in gen/), linked with
# librustorb_ffi statically. run-matrix.sh, run-poa-probe.sh, run-dynany-probe.sh,
# run-naming-check.sh, run-pi-probe.sh and run-load-local.py run it; rustorb-idl must be importable (tools/rustorb-idl.txt), or IDL_PARSER_DIR set to a
# checkout of it. CXX picks the compiler (default g++, c++ on macOS).
#
# The binaries run in the interop image (Ubuntu 24.04), so the host needs a
# glibc no newer than that image's; libstdc++ is linked statically.
set -euo pipefail

# Built already (interop.yml builds every implementation once, then sets
# INTEROP_PREBUILT for the steps that run the scripts).
[ -n "${INTEROP_PREBUILT:-}" ] && exit 0

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../../.."
case "$(uname -s)" in
  # The system compiler and libc++; Apple's linker links no libraries
  # statically.
  Darwin) cxx="${CXX:-c++}"; libs=(-framework CoreFoundation -framework Security -lpthread -lm) ;;
  *) cxx="${CXX:-g++}"; libs=(-static-libstdc++ -static-libgcc -lpthread -ldl -lm) ;;
esac

cargo build --release --locked -p rustorb-ffi --manifest-path "$root/Cargo.toml"

rm -rf "$here/gen" "$here/bin"
mkdir -p "$here/gen" "$here/bin"
pythonpath="${RUSTORB_IDL_DIR:+$RUSTORB_IDL_DIR/python}"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR${pythonpath:+:$pythonpath}"
PYTHONPATH="$pythonpath" python3 -m rustorb_idl -bc++11 -C "$here/gen" \
  "$root/interop/idl/InteropTest.idl" "$root/interop/idl/AnyTest.idl" \
  "$root/interop/idl/AnyTestHidden.idl" "$root/tests/mapping/idl/CosNaming.idl" \
  "$root/interop/idl/PoaProbe.idl" "$root/interop/idl/ValueTest.idl" \
  "$root/interop/idl/CustomTest.idl" "$root/interop/idl/AbstractTest.idl" \
  "$root/interop/idl/PiProbe.idl"

flags=(-std=c++17 -O2 -Wall -Wextra -I"$root/cpp/include" -I"$root/crates/rustorb-ffi/include"
       -I"$here/gen" -I"$here")
for f in InteropTest AnyTest AnyTestHidden CosNaming PoaProbe ValueTest CustomTest AbstractTest \
         PiProbe; do
  "$cxx" "${flags[@]}" -c "$here/gen/$f.cc" -o "$here/gen/$f.o"
done
objs=("$here/gen/InteropTest.o" "$here/gen/AnyTest.o" "$here/gen/CosNaming.o"
      "$here/gen/ValueTest.o" "$here/gen/CustomTest.o" "$here/gen/AbstractTest.o")
link() {
  local out=$1
  shift
  "$cxx" "${flags[@]}" "$@" "$root/target/release/librustorb_ffi.a" "${libs[@]}" \
    -o "$here/bin/$out"
}
link server "$here/server.cc" "${objs[@]}"
link client "$here/client.cc" "${objs[@]}" "$here/gen/AnyTestHidden.o"
link load_client "$here/load_client.cc" "$here/gen/InteropTest.o" "$here/gen/CosNaming.o"
link poa_probe_server "$here/poa_probe_server.cc" "$here/gen/PoaProbe.o"
link dynany_probe "$here/dynany_probe.cc"
link naming_check "$here/naming_check.cc" "$here/gen/CosNaming.o"
link pi_probe "$here/pi_probe.cc" "$here/gen/PiProbe.o"
