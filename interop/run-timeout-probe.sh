#!/usr/bin/env bash
# Runs scripts/timeout-probe with each Python ORB and compares the output
# with expected/timeout-probe-<impl>.txt.
#
#   ./run-timeout-probe.sh [--update] [IMPL ...]
#
# --update   rewrites the expected files with the current output
#
# IMPL is omniorbpy or rustorbpy (default: both). The probe records how
# the client call and connect timeouts (omniORB.setClientCallTimeout and
# friends) behave: which one wins, the exception, and what the server
# side reads. The two expected files differ only where rustorb chose to:
# it sends no LocateRequest before the first call through a corbaloc
# reference, and on a timeout it sends CancelRequest where omniORB closes
# the connection. rustorbpy is built on the host (impls/rustorbpy/build.sh,
# as in run-matrix.sh) and mounted into the container.
set -euo pipefail

cd "$(dirname "$0")"
update=0
if [ "${1:-}" = "--update" ]; then
  update=1
  shift
fi
impls=("$@")
[ ${#impls[@]} -eq 0 ] && impls=(omniorbpy rustorbpy)
image=${CLIENT_IMAGE:-rustorb-interop-omniorb:latest}

for impl in "${impls[@]}"; do
  [ "$impl" = rustorbpy ] && impls/rustorbpy/build.sh
done

mkdir -p out
failed=0
for impl in "${impls[@]}"; do
  expected="expected/timeout-probe-$impl.txt"
  actual="out/timeout-probe-$impl.txt"
  docker run --rm -v "$PWD/impls/rustorbpy:/opt/interop/impls/rustorbpy:ro" \
    -v "$PWD/scripts/timeout-probe:/opt/interop/scripts/timeout-probe:ro" "$image" \
    scripts/timeout-probe --impl "$impl" > "$actual"

  if [ $update -eq 1 ]; then
    cp "$actual" "$expected"
    echo "updated $expected"
  elif diff -u "$expected" "$actual"; then
    echo "timeout probe $impl: output matches $expected"
  else
    echo "timeout probe $impl: output differs from $expected" >&2
    failed=1
  fi
done
exit $failed
