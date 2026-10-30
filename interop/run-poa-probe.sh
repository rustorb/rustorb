#!/usr/bin/env bash
# Runs scripts/poa-probe against each POA probe server and compares the
# output with expected/poa-probe-<server>.txt.
#
#   ./run-poa-probe.sh [--update] [SERVER ...]
#
# --update   rewrites the expected files with the current output
#
# SERVER is omniorbpy, omniorb-cpp, rustorb (Rust), rustorbpy (Python),
# rustorbcpp (C++) or rustorbjava (Java) (default: all of them). The rustorb
# servers are built on the host and mounted into the container, like in
# run-matrix.sh; the Python, C++ and Java ones run on the same core as the
# Rust one and are compared with its expected file. Each server is started
# three times: once to record the object keys, once after a restart to check
# which keys survive it and to run the probe (the last stage shuts the ORB
# down), and once more for ORB::shutdown(false) called inside an upcall.
# This records the POA and thread model behaviour that docs/poa-design.md
# relies on.
set -euo pipefail

cd "$(dirname "$0")"
update=0
if [ "${1:-}" = "--update" ]; then
  update=1
  shift
fi
servers=("$@")
[ ${#servers[@]} -eq 0 ] && servers=(omniorbpy omniorb-cpp rustorb rustorbpy rustorbcpp rustorbjava)
image=${SERVER_IMAGE:-rustorb-interop-omniorb:latest}
network=interop-poa-probe

server_cmd() {
  case "$1" in
    omniorbpy) echo impls/omniorbpy/poa-probe-server ;;
    omniorb-cpp) echo impls/omniorb-cpp/poa_probe_server ;;
    rustorb) echo impls/rustorb/poa-probe-server ;;
    rustorbpy) echo impls/rustorbpy/poa-probe-server ;;
    rustorbcpp) echo impls/rustorbcpp/poa-probe-server ;;
    rustorbjava) echo impls/rustorbjava/poa-probe-server ;;
    *) echo "unknown server: $1" >&2; exit 2 ;;
  esac
}

# The expected output: the bindings of the rustorb core share one.
expected_name() {
  case "$1" in
    rustorbpy | rustorbcpp | rustorbjava) echo rustorb ;;
    *) echo "$1" ;;
  esac
}

cleanup() {
  docker rm -f poa-probe-server >/dev/null 2>&1 || true
  docker network rm "$network" >/dev/null 2>&1 || true
}
trap cleanup EXIT

start_server() {
  local cmd=$1 dir=$2
  docker rm -f poa-probe-server >/dev/null 2>&1 || true
  rm -f "$dir/control.ior"
  docker run -d --name poa-probe-server --hostname server --network "$network" \
    -v "$PWD/out:/out" -v "$PWD/impls/rustorb:/opt/interop/impls/rustorb:ro" \
    -v "$PWD/impls/rustorbpy:/opt/interop/impls/rustorbpy:ro" \
    -v "$PWD/impls/rustorbcpp:/opt/interop/impls/rustorbcpp:ro" \
    -v "$PWD/impls/rustorbjava:/opt/interop/impls/rustorbjava:ro" "$image" \
    "$cmd" --host server --port 2820 --ior-file "/out/${dir#out/}/control.ior" >/dev/null
  for _ in $(seq 100); do
    [ -f "$dir/control.ior" ] && return 0
    sleep 0.2
  done
  echo "the probe server did not start" >&2
  docker logs poa-probe-server >&2
  exit 1
}

probe() {
  local dir=$1
  shift
  docker run --rm --network "$network" -v "$PWD/out:/out" "$image" \
    scripts/poa-probe --ior-file "/out/${dir#out/}/control.ior" "$@"
}

for server in "${servers[@]}"; do
  case "$server" in
    rustorb)
      cargo build --release --locked -p rustorb-interop --manifest-path ../Cargo.toml
      mkdir -p impls/rustorb/bin
      cp ../target/release/rustorb-poa-probe-server impls/rustorb/bin/
      ;;
    rustorbpy | rustorbcpp | rustorbjava) "impls/$server/build.sh" ;;
  esac
done

mkdir -p out
docker network create "$network" >/dev/null 2>&1 || true
failed=0
for server in "${servers[@]}"; do
  cmd=$(server_cmd "$server")
  dir="out/poa-probe-$server"
  actual="out/poa-probe-$server.txt"
  expected="expected/poa-probe-$(expected_name "$server").txt"
  mkdir -p "$dir"

  start_server "$cmd" "$dir"
  probe "$dir" keys --out "/out/poa-probe-$server/keys.json"
  docker rm -f poa-probe-server >/dev/null
  # Transient object keys carry the start time in seconds.
  sleep 2
  start_server "$cmd" "$dir"
  probe "$dir" run --previous "/out/poa-probe-$server/keys.json" > "$actual"
  docker logs poa-probe-server > "$dir/server.log" 2>&1
  start_server "$cmd" "$dir"
  probe "$dir" shutdown-inside >> "$actual"
  docker logs poa-probe-server >> "$dir/server.log" 2>&1

  if [ $update -eq 1 ]; then
    cp "$actual" "$expected"
    echo "updated $expected"
  elif diff -u "$expected" "$actual"; then
    echo "poa probe $server: output matches $expected"
  else
    echo "poa probe $server: output differs from $expected" >&2
    failed=1
  fi
done
exit $failed
