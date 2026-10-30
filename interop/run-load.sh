#!/usr/bin/env bash
# Puts load on each Echo server with each load client and records the
# rates, as a rough guide to the performance of rustorb next to omniORB.
#
#   ./run-load.sh [--seconds S] [--threads N] [--connections N] [SERVER ...]
#
# SERVER is omniorb-cpp, omniorbpy, rustorb (Rust) or rustorbpy (Python)
# (default: all four). The clients are the Rust load client of rustorb
# (impls/rustorb/src/load_client.rs) and scripts/load-client on omniORBpy
# and on rustorbpy; each runs its phases (calls from many threads on one
# connection, strings, many connections, 1 MiB messages) against each
# server in turn, with omniNames as the naming service. Every server and
# client runs in its own container of the omniORB image on one Docker
# network. The results go to out/load/ and, as a table, to out/load.md.
# A call that fails makes the script fail; the rates are only recorded:
# they depend on the machine.
set -euo pipefail

cd "$(dirname "$0")"
seconds=5
threads=8
connections=16
servers=()
while [ $# -gt 0 ]; do
  case "$1" in
    --seconds) seconds="$2"; shift ;;
    --threads) threads="$2"; shift ;;
    --connections) connections="$2"; shift ;;
    -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
    *) servers+=("$1") ;;
  esac
  shift
done
[ ${#servers[@]} -eq 0 ] && servers=(omniorb-cpp omniorbpy rustorb rustorbpy)
clients=(rustorb omniorbpy rustorbpy)
image=${SERVER_IMAGE:-rustorb-interop-omniorb:latest}
network=interop-load
naming=corbaloc::naming:2809/NameService

cargo build --release --locked -p rustorb-interop --manifest-path ../Cargo.toml
mkdir -p impls/rustorb/bin
cp ../target/release/rustorb-interop-server ../target/release/rustorb-load-client impls/rustorb/bin/
impls/rustorbpy/build.sh

mounts=(-v "$PWD/impls/rustorb:/opt/interop/impls/rustorb:ro"
        -v "$PWD/impls/rustorbpy:/opt/interop/impls/rustorbpy:ro"
        -v "$PWD/scripts/load-client:/opt/interop/scripts/load-client:ro")

cleanup() {
  docker rm -f load-naming load-server >/dev/null 2>&1 || true
  docker network rm "$network" >/dev/null 2>&1 || true
}
trap cleanup EXIT
cleanup
docker network create "$network" >/dev/null
docker run -d --name load-naming --hostname naming --network "$network" \
  -e NAMING_HOST=naming "$image" run-omninames 2809 >/dev/null

client_cmd() {
  case "$1" in
    rustorb)
      echo impls/rustorb/bin/rustorb-load-client --connections "$connections" ;;
    omniorbpy | rustorbpy)
      echo scripts/load-client --impl "$1" ;;
  esac
}

out=out/load
rm -rf "$out"
mkdir -p "$out"
failed=0
for server in "${servers[@]}"; do
  docker rm -f load-server >/dev/null 2>&1 || true
  docker run -d --name load-server --hostname server --network "$network" "${mounts[@]}" \
    "$image" "impls/$server/server" --naming "$naming" --name "load/$server" \
    --host server --port 2810 >/dev/null
  for client in "${clients[@]}"; do
    log="$out/$client-to-$server.txt"
    echo "== $client -> $server"
    # shellcheck disable=SC2046
    if docker run --rm --network "$network" "${mounts[@]}" "$image" \
        $(client_cmd "$client") --naming "$naming" --name "load/$server" \
        --seconds "$seconds" --threads "$threads" > "$log" 2> "$log.err"; then
      cat "$log"
    else
      cat "$log" "$log.err"
      echo "load: $client -> $server failed" >&2
      failed=1
    fi
  done
  docker logs load-server > "$out/server-$server.log" 2>&1 || true
done

# The table: one row per pair, one column per phase (calls per second).
{
  echo "| client | server | small | string | connections | large (1 MiB) |"
  echo "| --- | --- | ---: | ---: | ---: | ---: |"
  for server in "${servers[@]}"; do
    for client in "${clients[@]}"; do
      f="$out/$client-to-$server.txt"
      row="| $client | $server |"
      for p in small string connections large; do
        r=$(sed -n "s/^$p .*rate=\([0-9]*\)\/s.*/\1/p" "$f" 2>/dev/null)
        row="$row ${r:--} |"
      done
      echo "$row"
    done
  done
} > out/load.md
echo
echo "threads=$threads connections=$connections seconds=$seconds per phase (calls/s)"
cat out/load.md
exit $failed
