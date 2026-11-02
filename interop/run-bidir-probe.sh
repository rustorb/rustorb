#!/usr/bin/env bash
# Bidirectional GIOP between rustorb and omniORB (docs/bidir-giop.md): the
# client offers the connection it opens, publishes its callback object at
# an address nobody listens on, and the server calls it back over that
# connection.
#
#   ./run-bidir-probe.sh [CLIENT:SERVER ...]
#
# By default every pair below. Each pair runs omniNames, the Echo server
# and the probe client (scripts/bidir-probe on omniORBpy and rustorbpy) in
# one container (scripts/bidir-pair). The rustorb server and rustorbpy are
# built on the host and mounted, as in run-matrix.sh. omniORBpy runs on
# omniORB's C++ core, so it stands for omniORB C++ as a client too.
set -euo pipefail

cd "$(dirname "$0")"
image=${IMAGE:-rustorb-interop-omniorb:latest}
pairs=("$@")
[ ${#pairs[@]} -eq 0 ] && pairs=(
  rustorbpy:omniorb-cpp rustorbpy:omniorbpy
  omniorbpy:rustorb omniorbpy:rustorbpy
  rustorbpy:rustorb
  omniorbpy:omniorb-cpp
)

client_args="-ORBofferBiDirectionalGIOP 1 -ORBendPoint giop:tcp:127.0.0.1: \
-ORBendPointPublish giop:tcp:127.0.0.1:1"
server_args="-ORBacceptBiDirectionalGIOP 1"
# omniORB takes bidirectional GIOP only on the transports whose rule says
# "bidir" (it logs that it rejected the switch otherwise). An omniORB
# server also needs the BidirectionalPolicy BOTH on the POA of the object
# that receives the callback reference: INTEROP_BIDIR=1 makes the omniORB
# Echo servers create it so.
omniorb_rule="* unix,ssl,tcp,bidir"

cargo build --release --locked -p rustorb-interop --manifest-path ../Cargo.toml
mkdir -p impls/rustorb/bin
cp ../target/release/rustorb-interop-server impls/rustorb/bin/
impls/rustorbpy/build.sh

failed=0
for p in "${pairs[@]}"; do
  IFS=: read -r client server <<< "$p"
  echo "== $client -> $server"
  client_rule="" server_rule=""
  [[ $client = omniorb* ]] && client_rule=$omniorb_rule
  [[ $server = omniorb* ]] && server_rule=$omniorb_rule
  if docker run --rm \
      -e CLIENT_ARGS="$client_args" -e SERVER_ARGS="$server_args" \
      -e CLIENT_RULE="$client_rule" -e SERVER_RULE="$server_rule" -e INTEROP_BIDIR=1 \
      -v "$PWD/impls/rustorb:/opt/interop/impls/rustorb:ro" \
      -v "$PWD/impls/rustorbpy:/opt/interop/impls/rustorbpy:ro" \
      -v "$PWD/scripts/bidir-probe:/opt/interop/scripts/bidir-probe:ro" \
      -v "$PWD/scripts/bidir-pair:/opt/interop/scripts/bidir-pair:ro" \
      "$image" scripts/bidir-pair "$client" "$server"; then
    echo "PASS $p"
  else
    echo "FAIL $p"
    failed=1
  fi
done
exit $failed
