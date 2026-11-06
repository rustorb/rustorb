#!/usr/bin/env bash
# Callback AMI (docs/ami.md) from the rustorb clients to the Echo servers
# of omniORB C++, omniORBpy, JacORB and rustorb: impls/rustorbpy/ami_probe.py,
# impls/rustorbcpp/ami_probe.cc and
# impls/rustorbjava/probe/interop/AmiProbe.java send sendc_ requests
# (results, inout and out arguments, a user exception, a system exception,
# attributes, a nil handler and a call timeout) and print what their
# AMI_EchoHandler got, which must match expected/ami-probe.txt.
#
#   ./run-ami-probe.sh [CLIENT:SERVER ...]
#
# CLIENT is rustorbpy, rustorbcpp or rustorbjava; SERVER a directory under
# impls/ (default: each client against omniorb-cpp, omniorbpy, jacorb and
# rustorb). AMI requests and replies are ordinary GIOP requests and
# replies on the wire, so the servers need nothing for AMI. Each pair runs
# omniNames, the server and the probe in one container (scripts/ami-pair).
# The rustorb implementations are built on the host (with rustorb-idl
# -Wbami) and mounted, as in run-matrix.sh.
#
# The timeout case: the client registers a callback object that answers
# after 2 s, then calls sendc_trigger with a call timeout of 500 ms, so the
# server is still calling the callback when the client gives up; the
# handler gets TIMEOUT, COMPLETED_MAYBE.
set -euo pipefail

cd "$(dirname "$0")"
image=${IMAGE:-rustorb-interop-omniorb:latest}
pairs=("$@")
if [ ${#pairs[@]} -eq 0 ]; then
  for c in rustorbpy rustorbcpp rustorbjava; do
    for s in omniorb-cpp omniorbpy jacorb rustorb; do pairs+=("$c:$s"); done
  done
fi

cargo build --release --locked -p rustorb-interop --manifest-path ../Cargo.toml
mkdir -p impls/rustorb/bin
cp ../target/release/rustorb-interop-server impls/rustorb/bin/
impls/rustorbpy/build.sh
impls/rustorbcpp/build.sh > /dev/null
impls/rustorbjava/build.sh > /dev/null

failed=0
for p in "${pairs[@]}"; do
  IFS=: read -r client server <<< "$p"
  echo "== $client -> $server"
  if actual=$(docker run --rm \
      -v "$PWD/impls/rustorb:/opt/interop/impls/rustorb:ro" \
      -v "$PWD/impls/rustorbpy:/opt/interop/impls/rustorbpy:ro" \
      -v "$PWD/impls/rustorbcpp:/opt/interop/impls/rustorbcpp:ro" \
      -v "$PWD/impls/rustorbjava:/opt/interop/impls/rustorbjava:ro" \
      -v "$PWD/scripts/ami-pair:/opt/interop/scripts/ami-pair:ro" \
      "$image" scripts/ami-pair "$client" "$server") \
      && [ "$actual" = "$(cat expected/ami-probe.txt)" ]; then
    echo "PASS $p"
  else
    printf '%s\n' "$actual"
    echo "FAIL $p (expected expected/ami-probe.txt)"
    diff <(printf '%s\n' "$actual") expected/ami-probe.txt || true
    failed=1
  fi
done
exit $failed
