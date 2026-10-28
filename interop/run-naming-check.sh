#!/usr/bin/env bash
# Runs the CosNaming operation sequence against the naming service with each
# client and compares the output with the expected result for omniNames.
#
#   ./run-naming-check.sh [--update] [--client IMPL]... [--naming IMPL]
#
# --update        rewrites expected/naming-check.txt with the output of the
#                 omniorbpy client
# --client IMPL   runs only this client (omniorbpy, rustorb); may be repeated.
#                 Default: CLIENT_IMPLS, or "omniorbpy rustorb"
# --naming IMPL   the naming service: omninames (default) or rustorb. The
#                 output of each must match the same expected file. With
#                 rustorb, the service is also restarted after the sequence,
#                 and naming_restart.py checks that the bindings and the
#                 references handed out before the restart still work
#                 (expected/naming-restart.txt).
#
# The rustorb client and naming service are built with cargo on the host and
# mounted into the containers, as in run-matrix.sh.
set -euo pipefail

cd "$(dirname "$0")"
expected=expected/naming-check.txt
update=0
naming=omninames
clients=()
while [ $# -gt 0 ]; do
  case "$1" in
    --update) update=1 ;;
    --client) clients+=("$2"); shift ;;
    --naming) naming="$2"; shift ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done
if [ ${#clients[@]} -eq 0 ]; then
  read -r -a clients <<< "${CLIENT_IMPLS:-omniorbpy rustorb}"
fi
if [ $update -eq 1 ]; then
  clients=(omniorbpy)
  naming=omninames
fi

case "$naming" in
  omninames) unset NAMING_CMD; suffix= ;;
  rustorb)
    cargo build --release --locked -p rustorb-naming --bin rustorb-names \
      --manifest-path ../Cargo.toml
    mkdir -p impls/rustorb/bin
    cp ../target/release/rustorb-names impls/rustorb/bin/
    export NAMING_CMD="impls/rustorb/naming-service 2809"
    suffix=-naming-rustorb
    ;;
  *) echo "unknown naming service: $naming" >&2; exit 2 ;;
esac

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
  out="out/naming-check-$client$suffix.txt"
  "${compose[@]}" run --rm -T --no-deps client \
    "impls/$client/naming-check" --naming corbaloc::naming:2809/NameService \
    > "$out" || true

  if [ "$naming" = rustorb ]; then
    # The state the sequence left, then the same after a restart.
    restart="out/naming-restart-$client.txt"
    restart_check=("${compose[@]}" run --rm -T --no-deps client
      python3 impls/omniorbpy/naming_restart.py --naming corbaloc::naming:2809/NameService)
    "${restart_check[@]}" --save /out/naming-restart.ior > "$restart" || true
    "${compose[@]}" restart -t 2 naming >/dev/null
    "${restart_check[@]}" --load /out/naming-restart.ior >> "$restart" || true
    if diff -u expected/naming-restart.txt "$restart"; then
      echo "naming restart ($client): output matches expected/naming-restart.txt"
    else
      echo "naming restart ($client): output differs from expected/naming-restart.txt" >&2
      status=1
    fi
  fi
  "${compose[@]}" logs --no-color naming > "out/naming-check-$client$suffix-naming.log" 2>&1

  if [ $update -eq 1 ]; then
    mkdir -p "$(dirname "$expected")"
    cp "$out" "$expected"
    echo "updated $expected"
  elif diff -u "$expected" "$out"; then
    echo "naming check ($client$suffix): output matches $expected"
  else
    echo "naming check ($client$suffix): output differs from $expected" >&2
    status=1
  fi
done
exit $status
