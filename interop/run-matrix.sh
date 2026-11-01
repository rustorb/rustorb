#!/usr/bin/env bash
# Runs the interop test matrix with Docker Compose.
#
#   ./run-matrix.sh [--build] [--capture] [--giop VER] [--jobs N] [--naming IMPL]
#                   [--out DIR] [--ssl] [--suite SUITE] [CLIENT:SERVER ...]
#
# Without pairs, every client in $CLIENT_IMPLS is paired with every server in
# $SERVER_IMPLS. Both default to $IMPLS (omniorbpy, omniorb-cpp and, but for
# --suite any, jacorb, in the image) plus rustorb, rustorbpy, rustorbcpp and
# rustorbjava (the Rust, the Python, the C++ and the Java client and server of
# rustorb), which are built on the host. rustorbjava-portable (the Java client
# and server on rustorb with JacORB's generated stubs) and rustorbpy-dii (the
# Python client and server through the DII and the DSI) run only when named
# in a pair.
# --build    builds the omniORB image first
# --capture  captures each client's traffic and splits it into GIOP messages
# --giop VER limits the client to GIOP VER (e.g. 1.1); the results go to
#            <client>-to-<server>-giop<VER>
# --jobs N   runs N pairs at a time (default: $INTEROP_JOBS, or 1); each pair
#            has its own Compose project, and its output is printed when it
#            ends
# --naming IMPL  the naming service: omninames (default) or rustorb; with
#            rustorb the results go to <client>-to-<server>-naming-rustorb
# --out DIR  where logs and captures go (default: interop/out)
# --ssl      the servers and the clients' callback objects listen on TLS
#            endpoints only (giop:ssl), with the certificates of
#            crates/rustorb-core/tests/tls, and require certificates both
#            ways (omniORB's default); jacorb is left out unless named in a
#            pair. The results go to <client>-to-<server>-ssl
# --suite SUITE  basic (default): the InteropTest.idl cases; any: the
#            AnyTest.idl cases (Any and TypeCode), whose results go to
#            <client>-to-<server>-any; value: the ValueTest.idl cases
#            (valuetypes), whose results go to <client>-to-<server>-value;
#            custom: the CustomTest.idl cases (custom valuetypes, which
#            omniORB does not support: of the image's implementations only
#            jacorb runs), whose results go to <client>-to-<server>-custom;
#            abstract: the AbstractTest.idl cases (abstract interfaces,
#            which the Rust client and server leave out), whose results go
#            to <client>-to-<server>-abstract
#
# Implementations live in impls/<name>/ and provide `server` and `client`
# executables that accept the options in README.md. To use an implementation
# from another image, set SERVER_IMAGE / CLIENT_IMAGE.
set -euo pipefail

cd "$(dirname "$0")"

IMAGE="rustorb-interop-omniorb:latest"
build=0
capture=0
giop=
naming=omninames
suite=basic
out="$PWD/out"
ssl=0
jobs="${INTEROP_JOBS:-1}"
pairs=()

while [ $# -gt 0 ]; do
  case "$1" in
    --build) build=1 ;;
    --capture) capture=1 ;;
    --giop) giop="$2"; shift ;;
    --jobs) jobs="$2"; shift ;;
    --naming) naming="$2"; shift ;;
    --suite) suite="$2"; shift ;;
    --out) out="$(realpath -m "$2")"; shift ;;
    --ssl) ssl=1 ;;
    -h|--help) sed -n '2,38p' "$0"; exit 0 ;;
    *:*) pairs+=("$1") ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

case "$suite" in
  basic) export ECHO_NAME=interop/Echo ;;
  any) export ECHO_NAME=interop/AnyEcho ;;
  value) export ECHO_NAME=interop/ValueEcho ;;
  custom) export ECHO_NAME=interop/CustomEcho ;;
  abstract) export ECHO_NAME=interop/AbstractEcho ;;
  *) echo "unknown suite: $suite" >&2; exit 2 ;;
esac
export SUITE="$suite"
# JacORB's generated Helpers do not follow AnyTest.idl (see
# docs/java-binding.md), so the any suite leaves it out unless asked for.
if [ "$suite" = any ]; then
  IMPLS="${IMPLS:-omniorbpy omniorb-cpp}"
elif [ "$suite" = custom ]; then
  IMPLS="${IMPLS:-jacorb}"
elif [ $ssl -eq 1 ]; then
  # The JacORB implementation has no TLS set-up.
  IMPLS="${IMPLS:-omniorbpy omniorb-cpp}"
else
  IMPLS="${IMPLS:-omniorbpy omniorb-cpp jacorb}"
fi
if [ $ssl -eq 1 ]; then
  export INTEROP_SSL=/opt/interop/tls
else
  export INTEROP_SSL=
fi
rustorb_impls="rustorb rustorbpy rustorbcpp rustorbjava"
CLIENT_IMPLS="${CLIENT_IMPLS:-$IMPLS $rustorb_impls}"
SERVER_IMPLS="${SERVER_IMPLS:-$IMPLS $rustorb_impls}"

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
# The rustorb Python package with its native module, and the stubs from
# rustorb-idl (idl_parser must be importable, or IDL_PARSER_DIR set).
for pair in "${pairs[@]}"; do
  case " ${pair%%:*} ${pair##*:} " in
    *" rustorbpy "*|*" rustorbpy-dii "*)
      impls/rustorbpy/build.sh
      break
      ;;
  esac
done
# The rustorb C++ client and server, with stubs from rustorb-idl -bc++11.
for pair in "${pairs[@]}"; do
  if [ "${pair%%:*}" = rustorbcpp ] || [ "${pair##*:}" = rustorbcpp ]; then
    impls/rustorbcpp/build.sh
    break
  fi
done

# The rustorb Java client and server, with stubs from rustorb-idl -bjava.
for pair in "${pairs[@]}"; do
  if [ "${pair%%:*}" = rustorbjava ] || [ "${pair##*:}" = rustorbjava ]; then
    impls/rustorbjava/build.sh
    break
  fi
done

# The Java client and server on rustorb with the stubs of JacORB's IDL
# compiler (from the image).
for pair in "${pairs[@]}"; do
  if [ "${pair%%:*}" = rustorbjava-portable ] || [ "${pair##*:}" = rustorbjava-portable ]; then
    IMAGE="$IMAGE" impls/rustorbjava-portable/build.sh
    break
  fi
done

# Prints the cases listed in known-failures.txt for a client / server pair,
# the suite of the run ($suite) and its GIOP version limit ($giop, empty for
# none).
known_failures() {
  local client="$1" server="$2" c s case rest want
  while read -r c s case rest; do
    case "$c" in ''|'#'*) continue ;; esac
    # A leading suite=SUITE applies the line to that suite instead of basic.
    want=basic
    if [[ "$c" == suite=* ]]; then
      want="${c#suite=}"
      read -r c s case rest <<< "$s $case $rest"
    fi
    [ "$want" = "$suite" ] || continue
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

# Runs one CLIENT:SERVER pair and writes its verdict line to $dir/verdict.
run_pair() {
  local pair="$1" client server name dir project status verdict detail
  client="${pair%%:*}"
  server="${pair##*:}"
  name="$(pair_name "$pair")"
  dir="$out/$name"
  project="interop-$(echo "$name" | tr -c 'a-z0-9\n' '-')"

  echo "=== client $client -> server $server"
  export CLIENT_IMPL="$client" SERVER_IMPL="$server" OUT_DIR="$dir" CLIENT_MAX_GIOP="$giop"
  if [ $capture -eq 1 ]; then
    export CAPTURE_DIR=/out/capture
  else
    export CAPTURE_DIR=
  fi

  local compose=(docker compose -p "$project" -f compose.yaml)
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
  local line="$verdict  $name${detail:+ ($detail)}"
  [ "$verdict" = FAIL ] && line+=" logs in $dir"
  echo "$line" > "$dir/verdict"
}

pair_name() {
  local pair="$1" name
  name="${pair%%:*}-to-${pair##*:}${giop:+-giop$giop}"
  if [ "$suite" != basic ]; then
    name+="-$suite"
  fi
  if [ $ssl -eq 1 ]; then
    name+="-ssl"
  fi
  if [ "$naming" != omninames ]; then
    name+="-naming-$naming"
  fi
  echo "$name"
}

mkdir -p "$out"
for pair in "${pairs[@]}"; do
  dir="$out/$(pair_name "$pair")"
  rm -rf "$dir"
  mkdir -p "$dir"
done

if [ "$jobs" -le 1 ]; then
  for pair in "${pairs[@]}"; do
    run_pair "$pair"
  done
else
  # Each pair's output goes to $dir/run.log and is printed when it ends.
  print_done() {
    local pid
    for pid in "${!running[@]}"; do
      if ! kill -0 "$pid" 2>/dev/null; then
        wait "$pid" || true
        cat "${running[$pid]}/run.log"
        unset "running[$pid]"
      fi
    done
  }
  declare -A running=()
  for pair in "${pairs[@]}"; do
    while [ ${#running[@]} -ge "$jobs" ]; do
      wait -n || true
      print_done
    done
    dir="$out/$(pair_name "$pair")"
    run_pair "$pair" > "$dir/run.log" 2>&1 &
    running[$!]="$dir"
  done
  while [ ${#running[@]} -gt 0 ]; do
    wait -n || true
    print_done
  done
fi

summary=()
failed=0
for pair in "${pairs[@]}"; do
  dir="$out/$(pair_name "$pair")"
  if [ -s "$dir/verdict" ]; then
    summary+=("$(cat "$dir/verdict")")
  else
    summary+=("FAIL  $(pair_name "$pair") (did not finish) logs in $dir")
  fi
  case "${summary[-1]}" in FAIL*) failed=1 ;; esac
done

echo
echo "=== summary"
printf '%s\n' "${summary[@]}"
exit $failed
