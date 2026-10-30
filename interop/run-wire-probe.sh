#!/usr/bin/env bash
# Sends hand-built GIOP requests (scripts/wire-probe) to each Echo server and
# compares what it answers with expected/wire-probe-<server>.txt.
#
#   ./run-wire-probe.sh [--update] [SERVER ...]
#
# --update   rewrites the expected files with the current output
#
# SERVER is omniorb-cpp, omniorbpy, rustorb (Rust), rustorbpy (Python),
# rustorbcpp (C++) or rustorbjava (Java) (default: all of them). The rustorb
# servers are built on the host and mounted into the container, as in
# run-matrix.sh; the bindings run on the same core, so they all compare with
# expected/wire-probe-rustorb.txt.
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
[ ${#servers[@]} -eq 0 ] && servers=(omniorb-cpp omniorbpy rustorb rustorbpy rustorbcpp rustorbjava)

expected_name() {
  case "$1" in
    rustorbpy | rustorbcpp | rustorbjava) echo rustorb ;;
    *) echo "$1" ;;
  esac
}

for server in "${servers[@]}"; do
  case "$server" in
    omniorb-cpp | omniorbpy) ;;
    rustorb)
      cargo build --release --locked -p rustorb-interop --manifest-path ../Cargo.toml
      mkdir -p impls/rustorb/bin
      cp ../target/release/rustorb-interop-server impls/rustorb/bin/
      ;;
    rustorbpy | rustorbcpp | rustorbjava) "impls/$server/build.sh" ;;
    *) echo "unknown server: $server" >&2; exit 2 ;;
  esac
done

mkdir -p out
failed=0
for server in "${servers[@]}"; do
  expected="expected/wire-probe-$(expected_name "$server").txt"
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
