#!/usr/bin/env bash
# Portable Interceptors between rustorb and JacORB
# (docs/portable-interceptors.md): service contexts both ways, a tagged
# component from the server's IOR interceptor and IOP::Codec data, with
# impls/rustorb/src/pi_probe.rs, impls/rustorbpy/pi_probe.py,
# impls/rustorbcpp/pi_probe.cc, impls/rustorbjava/probe/interop/PiProbe.java
# and impls/jacorb/pi/PiProbe.java.
#
#   ./run-pi-probe.sh [CLIENT:SERVER ...]
#
# CLIENT and SERVER are rustorb, rustorbpy, rustorbcpp, rustorbjava or
# jacorb (default: every pair). The server prints its IOR and serves; the client calls echo once
# and prints the reply context, the component and the result, which must
# read
#
#   reply context: SERVER got: from CLIENT
#   component: SERVER component
#   result: hello
#
# The rustorb probes are built on the host and mounted, as in
# run-matrix.sh. The servers add their reply context in send_reply, after
# the servant ran: before GIOP 1.2 the core sends it only when it keeps the
# body aligned (docs/portable-interceptors.md), so the probe runs on GIOP
# 1.2, the default.
set -euo pipefail

cd "$(dirname "$0")"
image=${IMAGE:-rustorb-interop-omniorb:latest}
pairs=("$@")
if [ ${#pairs[@]} -eq 0 ]; then
  for c in rustorb rustorbpy rustorbcpp rustorbjava jacorb; do
    for s in rustorb rustorbpy rustorbcpp rustorbjava jacorb; do pairs+=("$c:$s"); done
  done
fi

cargo build --release --locked -p rustorb-interop --bin rustorb-pi-probe --manifest-path ../Cargo.toml
mkdir -p impls/rustorb/bin
cp ../target/release/rustorb-pi-probe impls/rustorb/bin/
impls/rustorbpy/build.sh
impls/rustorbcpp/build.sh > /dev/null
impls/rustorbjava/build.sh > /dev/null

failed=0
for pair in "${pairs[@]}"; do
  IFS=: read -r client server <<< "$pair"
  expected=$(printf 'reply context: %s got: from %s\ncomponent: %s component\nresult: hello' \
    "$server" "$client" "$server")
  echo "== $client -> $server"
  # Inside the container: start the server, wait for its IOR, run the client.
  if actual=$(docker run --rm \
      -v "$PWD/impls/rustorb:/opt/interop/impls/rustorb:ro" \
      -v "$PWD/impls/rustorbpy:/opt/interop/impls/rustorbpy:ro" \
      -v "$PWD/impls/rustorbcpp:/opt/interop/impls/rustorbcpp:ro" \
      -v "$PWD/impls/rustorbjava:/opt/interop/impls/rustorbjava:ro" \
      "$image" bash -c '
        probe() {
          case "$1" in
            rustorb) shift; impls/rustorb/bin/rustorb-pi-probe "$@" ;;
            rustorbpy) shift; impls/rustorbpy/pi-probe "$@" ;;
            rustorbcpp) shift; impls/rustorbcpp/pi-probe "$@" ;;
            rustorbjava) shift; impls/rustorbjava/pi-probe "$@" ;;
            jacorb) shift; impls/jacorb/pi-probe "$@" ;;
          esac
        }
        probe "$2" server > /tmp/ior 2> /tmp/server.err &
        for _ in $(seq 100); do grep -q IOR: /tmp/ior && break; sleep 0.1; done
        probe "$1" client "$(grep IOR: /tmp/ior | head -1)" 2> /tmp/client.err \
          || { cat /tmp/server.err /tmp/client.err >&2; exit 1; }
      ' _ "$client" "$server") && [ "$actual" = "$expected" ]; then
    echo "PASS $pair"
  else
    printf '%s\n' "$actual"
    echo "FAIL $pair (expected:"
    printf '%s\n' "$expected"
    echo ")"
    failed=1
  fi
done
exit $failed
