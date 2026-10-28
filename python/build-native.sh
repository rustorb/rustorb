#!/usr/bin/env bash
# Builds the native module (crates/rustorb-python) and puts it into
# python/rustorb/, so that the package works from the source tree, as the
# tests use it. `pip install ./python` builds a wheel with maturin instead.
#
#   python/build-native.sh [--release]
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
profile=debug
args=()
if [ "${1:-}" = "--release" ]; then
  profile=release
  args+=(--release)
fi
# Built as an extension module: not linked to libpython, which the Python
# that loads it provides.
PYO3_BUILD_EXTENSION_MODULE=1 cargo build "${args[@]}" -p rustorb-python \
  --manifest-path "$root/Cargo.toml"
case "$(uname -s)" in
  Darwin) lib="$root/target/$profile/lib_native.dylib" ;;
  MINGW*|MSYS*|CYGWIN*) lib="$root/target/$profile/_native.dll" ;;
  *) lib="$root/target/$profile/lib_native.so" ;;
esac
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*) dest="$root/python/rustorb/_native.pyd" ;;
  *) dest="$root/python/rustorb/_native.abi3.so" ;;
esac
cp "$lib" "$dest"
echo "built $dest"
