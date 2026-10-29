#!/usr/bin/env bash
# Runs the DynAny probe with each implementation and compares the output
# with expected/dynany-probe-<impl>.txt.
#
#   ./run-dynany-probe.sh [--update] [IMPL ...]
#
# --update   rewrites the expected files with the current output
#
# IMPL is omniorb-cpp (impls/omniorb-cpp/dynany_probe.cc, in the image),
# rustorb (impls/rustorb/src/dynany_probe.rs) or rustorbpy
# (impls/rustorbpy/dynany-probe, the Python binding on the same core, compared
# with the rustorb file) (default: all of them). omniORBpy has no DynAny.
# The probe records how DynAny behaves where the specification leaves room
# or where rustorb had to choose; the two expected files differ where
# rustorb chose otherwise (see docs/dynany.md). The rustorb probes run on
# the host.
set -euo pipefail

cd "$(dirname "$0")"
update=0
if [ "${1:-}" = "--update" ]; then
  update=1
  shift
fi
impls=("$@")
[ ${#impls[@]} -eq 0 ] && impls=(omniorb-cpp rustorb rustorbpy)
image=${CLIENT_IMAGE:-rustorb-interop-omniorb:latest}

expected_name() {
  case "$1" in
    rustorbpy) echo rustorb ;;
    *) echo "$1" ;;
  esac
}

mkdir -p out
failed=0
for impl in "${impls[@]}"; do
  expected="expected/dynany-probe-$(expected_name "$impl").txt"
  actual="out/dynany-probe-$impl.txt"
  case "$impl" in
    omniorb-cpp)
      docker run --rm "$image" impls/omniorb-cpp/dynany_probe > "$actual" ;;
    rustorb)
      cargo build --release --locked -q -p rustorb-interop --bin rustorb-dynany-probe \
        --manifest-path ../Cargo.toml
      ../target/release/rustorb-dynany-probe > "$actual" ;;
    rustorbpy)
      impls/rustorbpy/build.sh
      python3 impls/rustorbpy/dynany-probe > "$actual" ;;
    *) echo "unknown implementation: $impl" >&2; exit 2 ;;
  esac

  if [ $update -eq 1 ] && [ "$(expected_name "$impl")" = "$impl" ]; then
    cp "$actual" "$expected"
    echo "updated $expected"
  elif diff -u "$expected" "$actual"; then
    echo "dynany probe $impl: output matches $expected"
  else
    echo "dynany probe $impl: output differs from $expected" >&2
    failed=1
  fi
done
exit $failed
