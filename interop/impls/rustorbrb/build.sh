#!/usr/bin/env bash
# Builds what the rustorb Ruby client, server and naming check need: rt/
# (the runtime of the rustorb gem, ruby/lib, with its rustorb_ext extension)
# and gen/ (InteropTest.idl, AnyTest.idl and AnyTestHidden.idl through
# rustorb-idl -bruby). run-matrix.sh,
# run-naming-check.sh and run-wire-probe.sh run it; rustorb-idl must be
# importable (tools/rustorb-idl.txt), or RUSTORB_IDL_DIR / IDL_PARSER_DIR set
# to checkouts.
#
# The extension is built for the Ruby that runs it: in the interop image
# (IMAGE, default rustorb-interop-omniorb:latest, which has its Ruby with
# the headers and a C compiler), against librustorb_ffi built on the host;
# with RUSTORB_RB_HOST=1, for the host's Ruby (to run the programs on the
# host).
set -euo pipefail

[ -n "${INTEROP_PREBUILT:-}" ] && exit 0

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
image="${IMAGE:-rustorb-interop-omniorb:latest}"

cargo build --release --locked -p rustorb-ffi --manifest-path "$root/Cargo.toml"

rm -rf "$here/rt" "$here/gen"
mkdir -p "$here/rt" "$here/gen"
cp -R "$root/ruby/lib/." "$here/rt/"
pythonpath="${RUSTORB_IDL_DIR:+$RUSTORB_IDL_DIR/python}"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR${pythonpath:+:$pythonpath}"
PYTHONPATH="$pythonpath" python3 -m rustorb_idl -bruby -C "$here/gen" -I"$root/interop/idl" \
  "$root/interop/idl/InteropTest.idl" "$root/interop/idl/AnyTest.idl" "$root/interop/idl/AnyTestHidden.idl"

ext="$here/rt/rustorb/rustorb_ext.so"
if [ -n "${RUSTORB_RB_HOST:-}" ]; then
  build="$here/rt-build"
  rm -rf "$build"
  mkdir -p "$build"
  (cd "$build" && ruby "$root/ruby/ext/rustorb_ext/extconf.rb" > /dev/null && make > /dev/null)
  cp "$build/rustorb_ext.$(ruby -rrbconfig -e 'print RbConfig::CONFIG["DLEXT"]')" "$here/rt/rustorb/"
  rm -rf "$build"
else
  # Built in a throwaway directory of the container; the library comes back
  # on stdout, so that nothing in the checkout belongs to the container's
  # user.
  docker run --rm -v "$root:/src:ro" -e RUSTORB_FFI_LIBRARY=/src/target/release/librustorb_ffi.a \
    -e RUSTORB_FFI_INCLUDE=/src/crates/rustorb-ffi/include "$image" sh -c \
    'mkdir /tmp/ext && cd /tmp/ext && ruby /src/ruby/ext/rustorb_ext/extconf.rb > /dev/null &&
     make > /dev/null && cat rustorb_ext.so' > "$ext"
fi
