#!/usr/bin/env bash
# Runs the interop test matrix with Docker Compose.
#
#   ./run-matrix.sh [--build] [--capture] [--out DIR] [CLIENT:SERVER ...]
#
# Without pairs, every combination of $IMPLS is run as client and server.
# --build    builds the omniORB image first
# --capture  captures each client's traffic and splits it into GIOP messages
# --out DIR  where logs and captures go (default: interop/out)
#
# Implementations live in impls/<name>/ and provide `server` and `client`
# executables that accept the options in README.md. To use an implementation
# from another image, set SERVER_IMAGE / CLIENT_IMAGE.
set -euo pipefail

cd "$(dirname "$0")"

IMPLS="${IMPLS:-omniorbpy omniorb-cpp}"
IMAGE="rustorb-interop-omniorb:latest"
build=0
capture=0
out="$PWD/out"
pairs=()

while [ $# -gt 0 ]; do
  case "$1" in
    --build) build=1 ;;
    --capture) capture=1 ;;
    --out) out="$(realpath -m "$2")"; shift ;;
    -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
    *:*) pairs+=("$1") ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

if [ ${#pairs[@]} -eq 0 ]; then
  for c in $IMPLS; do
    for s in $IMPLS; do
      pairs+=("$c:$s")
    done
  done
fi

if [ $build -eq 1 ]; then
  docker build -f docker/omniorb/Dockerfile -t "$IMAGE" .
fi

# Prints the cases listed in known-failures.txt for a client / server pair.
known_failures() {
  local client="$1" server="$2" c s case rest
  while read -r c s case rest; do
    case "$c" in ''|'#'*) continue ;; esac
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
  name="${client}-to-${server}"
  dir="$out/$name"
  project="interop-$(echo "$name" | tr -c 'a-z0-9\n' '-')"
  rm -rf "$dir"
  mkdir -p "$dir"

  echo "=== client $client -> server $server"
  export CLIENT_IMPL="$client" SERVER_IMPL="$server" OUT_DIR="$dir"
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
