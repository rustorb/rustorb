#!/usr/bin/env bash
# Closing idle connections between rustorb and the other ORBs
# (docs/connections.md): in each case one side closes a connection that was
# idle longer than its timeout, and the next call opens a new connection.
#
#   ./run-idle-probe.sh [CASE ...]
#
# A CASE is CLIENT:SERVER:SIDE, SIDE being the side that closes (client or
# server); by default every case below. Each case runs omniNames, the Echo
# server and the probe client (scripts/idle-probe on omniORBpy and rustorbpy,
# interop.IdleProbe on JacORB) in one container (scripts/idle-pair). The
# rustorb server and rustorbpy are built on the host and mounted, as in
# run-matrix.sh. omniORBpy runs on omniORB's C++ core, so it stands for
# omniORB C++ as a client too. An omniORB client leaves a connection the
# server closed in CLOSE-WAIT until it next uses it, so for it the probe
# checks only that the connection is no longer established.
set -euo pipefail

cd "$(dirname "$0")"
image=${IMAGE:-rustorb-interop-omniorb:latest}
cases=("$@")
[ ${#cases[@]} -eq 0 ] && cases=(
  rustorbpy:omniorb-cpp:client rustorbpy:omniorb-cpp:server
  rustorbpy:omniorbpy:server
  rustorbpy:jacorb:client rustorbpy:jacorb:server
  omniorbpy:rustorb:client omniorbpy:rustorb:server
  jacorb:rustorb:client jacorb:rustorb:server
  rustorbpy:rustorb:client rustorbpy:rustorb:server
)

# The idle timeout of each ORB, as its options: 1 second (omniORB scans
# every scanGranularity seconds).
idle_args() {
  local impl=$1 side=$2
  case "$impl:$side" in
    rustorb*:client) echo "-ORBoutConScanPeriod 1 -ORBinConScanPeriod 0" ;;
    rustorb*:server) echo "-ORBinConScanPeriod 1 -ORBoutConScanPeriod 0" ;;
    omniorb*:client) echo "-ORBoutConScanPeriod 1 -ORBscanGranularity 1" ;;
    omniorb*:server) echo "-ORBinConScanPeriod 1 -ORBscanGranularity 1" ;;
    jacorb:client) echo "-Djacorb.connection.client.idle_timeout=1000" ;;
    jacorb:server) echo "-Djacorb.connection.server.timeout=1000" ;;
  esac
}
# The other side keeps its connections (rustorb closes idle ones by default).
keep_args() {
  case "$1" in
    rustorb*) echo "-ORBoutConScanPeriod 0 -ORBinConScanPeriod 0" ;;
  esac
}

cargo build --release --locked -p rustorb-interop --manifest-path ../Cargo.toml
mkdir -p impls/rustorb/bin
cp ../target/release/rustorb-interop-server impls/rustorb/bin/
impls/rustorbpy/build.sh

failed=0
for c in "${cases[@]}"; do
  IFS=: read -r client server side <<< "$c"
  if [ "$side" = client ]; then
    client_args=$(idle_args "$client" client)
    server_args=$(keep_args "$server")
  else
    client_args=$(keep_args "$client")
    server_args=$(idle_args "$server" server)
    # omniORB sees the server's CloseConnection only when it next uses the
    # connection (the same with an omniORB server): until then its socket
    # stays in CLOSE-WAIT.
    [ "$client" = omniorbpy ] && client_args="--allow-close-wait"
  fi
  echo "== $client -> $server, the $side closes"
  if docker run --rm \
      -e CLIENT_IDLE_ARGS="$client_args" -e SERVER_IDLE_ARGS="$server_args" \
      -v "$PWD/impls/rustorb:/opt/interop/impls/rustorb:ro" \
      -v "$PWD/impls/rustorbpy:/opt/interop/impls/rustorbpy:ro" \
      -v "$PWD/scripts/idle-probe:/opt/interop/scripts/idle-probe:ro" \
      -v "$PWD/scripts/idle-pair:/opt/interop/scripts/idle-pair:ro" \
      "$image" scripts/idle-pair "$client" "$server"; then
    echo "PASS $c"
  else
    echo "FAIL $c"
    failed=1
  fi
done
exit $failed
