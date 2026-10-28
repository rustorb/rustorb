#!/usr/bin/env bash
# Sends hand-built GIOP requests (scripts/wire-probe) to each Echo server and
# compares what it answers with expected/wire-probe-<server>.txt.
#
#   ./run-wire-probe.sh [--update] [SERVER ...]
#
# --update   rewrites the expected files with the current output
#
# This records how the servers read wire forms that their own clients never
# send, such as a wstring without a BOM.
set -euo pipefail

cd "$(dirname "$0")"
update=0
if [ "${1:-}" = "--update" ]; then
  update=1
  shift
fi
servers=("$@")
[ ${#servers[@]} -eq 0 ] && servers=(omniorb-cpp omniorbpy)

mkdir -p out
failed=0
for server in "${servers[@]}"; do
  expected="expected/wire-probe-$server.txt"
  actual="out/wire-probe-$server.txt"
  project="interop-wire-probe-$server"
  compose=(docker compose -p "$project" -f compose.yaml)
  opts=()
  # omniORBpy cannot read wchar (see known-failures.txt).
  [ "$server" = omniorbpy ] && opts+=(--no-wchar)
  export SERVER_IMPL="$server"
  "${compose[@]}" up -d naming server
  # The server is ready once it has bound itself in the naming service.
  for _ in $(seq 60); do
    "${compose[@]}" logs server 2>/dev/null | grep -q "server ready" && break
    sleep 1
  done
  "${compose[@]}" run --rm -T --no-deps client \
    scripts/wire-probe --host server --port 2810 "${opts[@]}" > "$actual"
  "${compose[@]}" down -t 2 >/dev/null 2>&1

  if [ $update -eq 1 ]; then
    cp "$actual" "$expected"
    echo "updated $expected"
  elif diff -u "$expected" "$actual"; then
    echo "wire probe $server: output matches $expected"
  else
    echo "wire probe $server: output differs from $expected" >&2
    failed=1
  fi
done
exit $failed
