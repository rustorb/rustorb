#!/usr/bin/env bash
# Runs the CosNaming operation sequence against the naming service with each
# client and compares the output with the expected result for omniNames.
#
#   ./run-naming-check.sh [--update] [--client IMPL]...
#
# --update        rewrites expected/naming-check.txt with the output of the
#                 omniorbpy client
# --client IMPL   runs only this client (omniorbpy, rustorb); may be repeated.
#                 Default: CLIENT_IMPLS, or "omniorbpy rustorb"
#
# NAMING_IMAGE / NAMING_CMD select the naming service as in compose.yaml, so
# the same check can later be pointed at the rustorb Naming Service. The
# rustorb client is built with cargo on the host and mounted into the client
# container, as in run-matrix.sh.
set -euo pipefail

cd "$(dirname "$0")"
expected=expected/naming-check.txt
update=0
clients=()
while [ $# -gt 0 ]; do
  case "$1" in
    --update) update=1 ;;
    --client) clients+=("$2"); shift ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done
if [ ${#clients[@]} -eq 0 ]; then
  read -r -a clients <<< "${CLIENT_IMPLS:-omniorbpy rustorb}"
fi
if [ $update -eq 1 ]; then
  clients=(omniorbpy)
fi

for client in "${clients[@]}"; do
  if [ "$client" = rustorb ]; then
    cargo build --release --locked -p rustorb-interop --manifest-path ../Cargo.toml
    mkdir -p impls/rustorb/bin
    cp ../target/release/rustorb-naming-check impls/rustorb/bin/
  fi
done

mkdir -p out
project=interop-naming-check
compose=(docker compose -p "$project" -f compose.yaml)
trap '"${compose[@]}" down -t 2 >/dev/null 2>&1' EXIT

status=0
for client in "${clients[@]}"; do
  # A fresh naming service for each client, as the sequence expects an
  # empty root context.
  "${compose[@]}" down -t 2 >/dev/null 2>&1 || true
  "${compose[@]}" up -d naming
  out="out/naming-check-$client.txt"
  "${compose[@]}" run --rm -T --no-deps client \
    "impls/$client/naming-check" --naming corbaloc::naming:2809/NameService \
    > "$out" || true
  "${compose[@]}" logs --no-color naming > "out/naming-check-$client-naming.log" 2>&1

  if [ $update -eq 1 ]; then
    mkdir -p "$(dirname "$expected")"
    cp "$out" "$expected"
    echo "updated $expected"
  elif diff -u "$expected" "$out"; then
    echo "naming check ($client): output matches $expected"
  else
    echo "naming check ($client): output differs from $expected" >&2
    status=1
  fi
done
exit $status
