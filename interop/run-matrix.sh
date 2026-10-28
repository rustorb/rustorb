#!/usr/bin/env bash
# Runs the interop test matrix with Docker Compose.
#
#   ./run-matrix.sh [--build] [--capture] [--giop VER] [--naming IMPL] [--out DIR]
#                   [CLIENT:SERVER ...]
#
# Without pairs, every client in $CLIENT_IMPLS is paired with every server in
# $SERVER_IMPLS. Both default to $IMPLS plus rustorb, whose client and server
# are built with cargo on the host.
# --build    builds the omniORB image first
# --capture  captures each client's traffic and splits it into GIOP messages
# --giop VER limits the client to GIOP VER (e.g. 1.1); the results go to
#            <client>-to-<server>-giop<VER>
# --naming IMPL  the naming service: omninames (default) or rustorb; with
#            rustorb the results go to <client>-to-<server>-naming-rustorb
# --out DIR  where logs and captures go (default: interop/out)
#
# Implementations live in impls/<name>/ and provide `server` and `client`
# executables that accept the options in README.md. To use an implementation
# from another image, set SERVER_IMAGE / CLIENT_IMAGE.
set -euo pipefail

cd "$(dirname "$0")"

IMPLS="${IMPLS:-omniorbpy omniorb-cpp}"
CLIENT_IMPLS="${CLIENT_IMPLS:-$IMPLS rustorb}"
SERVER_IMPLS="${SERVER_IMPLS:-$IMPLS rustorb}"
IMAGE="rustorb-interop-omniorb:latest"
build=0
capture=0
giop=
naming=omninames
out="$PWD/out"
pairs=()

while [ $# -gt 0 ]; do
  case "$1" in
    --build) build=1 ;;
    --capture) capture=1 ;;
    --giop) giop="$2"; shift ;;
    --naming) naming="$2"; shift ;;
    --out) out="$(realpath -m "$2")"; shift ;;
    -h|--help) sed -n '2,21p' "$0"; exit 0 ;;
    *:*) pairs+=("$1") ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

if [ ${#pairs[@]} -eq 0 ]; then
  for c in $CLIENT_IMPLS; do
    for s in $SERVER_IMPLS; do
      pairs+=("$c:$s")
    done
  done
fi

if [ $build -eq 1 ]; then
  docker build -f docker/omniorb/Dockerfile -t "$IMAGE" .
fi

# The rustorb client and server run in the omniORB image (Ubuntu 24.04), so
# they are built on the host and mounted into the containers (see
# compose.yaml).
case "$naming" in
  omninames) unset NAMING_CMD ;;
  rustorb)
    cargo build --release --locked -p rustorb-naming --bin rustorb-names \
      --manifest-path ../Cargo.toml
    mkdir -p impls/rustorb/bin
    cp ../target/release/rustorb-names impls/rustorb/bin/
    export NAMING_CMD="impls/rustorb/naming-service 2809"
    ;;
  *) echo "unknown naming service: $naming" >&2; exit 2 ;;
esac

for pair in "${pairs[@]}"; do
  if [ "${pair%%:*}" = rustorb ] || [ "${pair##*:}" = rustorb ]; then
    cargo build --release --locked -p rustorb-interop --manifest-path ../Cargo.toml
    mkdir -p impls/rustorb/bin
    cp ../target/release/rustorb-interop-client ../target/release/rustorb-interop-server \
      impls/rustorb/bin/
    break
  fi
done

# Prints the cases listed in known-failures.txt for a client / server pair
# and the GIOP version limit of the run ($giop, empty for none).
known_failures() {
  local client="$1" server="$2" c s case rest
  while read -r c s case rest; do
    case "$c" in ''|'#'*) continue ;; esac
    # A leading giop=VER applies the line only to runs with --giop VER.
    if [[ "$c" == giop=* ]]; then
      [ "${c#giop=}" = "$giop" ] || continue
      read -r c s case rest <<< "$s $case $rest"
    fi
    # shellcheck disable=SC2053 # CLIENT / SERVER are glob patterns
    if [[ "$client" == $c && "$server" == $s ]]; then
      echo "$case"
    fi
  done < known-failures.txt
}

# Prints "PASS|XFAIL|FAIL detail" for one run, from the client's exit status
# and the "FAIL <case>:" lines in its log.
judge() {
  local client="$1" server="$2" log="$3" status="$4"
  if ! grep -qE '^[0-9]+ passed, [0-9]+ failed' "$log"; then
    echo "FAIL client did not finish (exit $status)"
    return
  fi
  local failed known unexpected fixed
  failed="$(sed -n 's/^FAIL \([^:]*\):.*/\1/p' "$log" | sort -u)"
  known="$(known_failures "$client" "$server" | sort -u)"
  unexpected="$(comm -23 <(echo "$failed") <(echo "$known") | grep . || true)"
  fixed="$(comm -13 <(echo "$failed") <(echo "$known") | grep . || true)"
  if [ -n "$unexpected" ]; then
    echo "FAIL unexpected: $(echo $unexpected)"
  elif [ -n "$fixed" ]; then
    echo "FAIL known failures now pass, update known-failures.txt: $(echo $fixed)"
  elif [ -n "$failed" ]; then
    echo "XFAIL known: $(echo $failed)"
  elif [ "$status" -ne 0 ]; then
    echo "FAIL client exited with $status"
  else
    echo "PASS"
  fi
}

mkdir -p "$out"
summary=()
failed=0

for pair in "${pairs[@]}"; do
  client="${pair%%:*}"
  server="${pair##*:}"
  name="${client}-to-${server}${giop:+-giop$giop}"
  if [ "$naming" != omninames ]; then
    name+="-naming-$naming"
  fi
  dir="$out/$name"
  project="interop-$(echo "$name" | tr -c 'a-z0-9\n' '-')"
  rm -rf "$dir"
  mkdir -p "$dir"

  echo "=== client $client -> server $server"
  export CLIENT_IMPL="$client" SERVER_IMPL="$server" OUT_DIR="$dir" CLIENT_MAX_GIOP="$giop"
  if [ $capture -eq 1 ]; then
    export CAPTURE_DIR=/out/capture
  else
    export CAPTURE_DIR=
  fi

  compose=(docker compose -p "$project" -f compose.yaml)
  "${compose[@]}" up -d naming server
  set +e
  "${compose[@]}" run --rm -T client 2>&1 | tee "$dir/client.log"
  status=${PIPESTATUS[0]}
  set -e
  # The client asks the server to shut down; give it a moment to log that.
  timeout 10 docker wait "$("${compose[@]}" ps -aq server)" >/dev/null 2>&1 || true
  "${compose[@]}" logs --no-color naming > "$dir/naming.log" 2>&1
  "${compose[@]}" logs --no-color server > "$dir/server.log" 2>&1
  "${compose[@]}" down -t 2 >/dev/null 2>&1

  read -r verdict detail < <(judge "$client" "$server" "$dir/client.log" "$status")
  summary+=("$verdict  $name${detail:+ ($detail)}")
  if [ "$verdict" = FAIL ]; then
    summary[-1]+=" logs in $dir"
    failed=1
  fi
done

echo
echo "=== summary"
printf '%s\n' "${summary[@]}"
exit $failed
