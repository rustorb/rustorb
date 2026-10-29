#!/usr/bin/env bash
# Runs scripts/orb-ops-probe with each Python ORB and compares the output
# with expected/orb-ops-probe-<impl>.txt.
#
#   ./run-orb-ops-probe.sh [--update] [IMPL ...]
#
# --update   rewrites the expected files with the current output
#
# IMPL is omniorbpy or rustorbpy (default: both). The probe records how
# the remaining standard ORB operations behave through the omniORBpy API:
# ORB.id, list_initial_services, register_initial_reference, create_policy
# and the policy operations of CORBA.Object. The two expected files differ
# where omniORBpy lacks an operation (the Messaging timeout policy and the
# Object policy operations) and where rustorb chose to follow the
# specification (see docs/orb-operations.md). rustorbpy is built on the
# host (impls/rustorbpy/build.sh, as in run-matrix.sh) and mounted into the
# container.
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
  expected="expected/orb-ops-probe-$impl.txt"
  actual="out/orb-ops-probe-$impl.txt"
  docker run --rm -v "$PWD/impls/rustorbpy:/opt/interop/impls/rustorbpy:ro" \
    -v "$PWD/scripts/orb-ops-probe:/opt/interop/scripts/orb-ops-probe:ro" "$image" \
    scripts/orb-ops-probe --impl "$impl" > "$actual"

  if [ $update -eq 1 ]; then
    cp "$actual" "$expected"
    echo "updated $expected"
  elif diff -u "$expected" "$actual"; then
    echo "orb ops probe $impl: output matches $expected"
  else
    echo "orb ops probe $impl: output differs from $expected" >&2
    failed=1
  fi
done
exit $failed
