#!/usr/bin/env bash
# Portable Interceptors between rustorb and JacORB
# (docs/portable-interceptors.md): service contexts both ways, a tagged
# component from the server's IOR interceptor and IOP::Codec data, with
# impls/rustorb/src/pi_probe.rs, impls/rustorbpy/pi_probe.py,
# impls/rustorbcpp/pi_probe.cc, impls/rustorbjava/probe/interop/PiProbe.java,
# impls/jacorb/pi/PiProbe.java, impls/omniorbpy/pi_probe.py and (a client
# only) impls/omniorb-cpp/pi_probe.cc.
#
#   ./run-pi-probe.sh [CLIENT:SERVER ...]
#
# CLIENT and SERVER are rustorb, rustorbpy, rustorbcpp, rustorbjava, jacorb
# or omniorbpy, and CLIENT also omniorbcpp (default: every pair). The server prints its IOR and serves; the client calls echo once
# and prints the reply context, the component and the result, which must
# read
#
#   reply context: SERVER got: from CLIENT
#   component: SERVER component
#   result: hello
#   request context: from CLIENT
#
# where the last line is what the server printed when the request came
# (the one with the client's context, if it printed several).
# The rustorb probes are built on the host and mounted, as in
# run-matrix.sh. Each pair runs with the client limited to GIOP 1.2, 1.1
# and 1.0 in turn (GIOP_VERSIONS, default "1.2 1.1 1.0"), with the
# client's -ORBmaxGIOPVersion. A JacORB client uses the version of the
# server's IOR, which is 1.2 for every server here, so JacORB clients run on
# 1.2 only (the others are skipped). The servers
# add their reply context in send_reply, after the servant ran; before GIOP
# 1.2 that can move the reply body by 4 bytes, which the core makes up for
# (docs/portable-interceptors.md). The names make the context lengths
# differ, so some pairs move the body and some do not.
#
# omniORBpy has only its own request interceptors (omniORB.interceptors),
# which carry the service contexts but cannot add tagged components to IORs:
# with an omniorbpy server the component line is not compared. omniORB sends
# no reply service contexts at all on GIOP 1.1 (seen on the wire, whatever
# their length, also between two omniORBpy probes, while 1.0 and 1.2 carry
# them), so with an omniorbpy server on 1.1 the reply context line is not
# compared either.
#
# omniORB C++ has its own request interceptors too, and its user's guide
# documents only how a client adds request service contexts
# (clientSendRequest), so the omniorbcpp client prints only the result,
# and the server's line shows that its context arrived.
set -euo pipefail

cd "$(dirname "$0")"
image=${IMAGE:-rustorb-interop-omniorb:latest}
pairs=("$@")
if [ ${#pairs[@]} -eq 0 ]; then
  all="rustorb rustorbpy rustorbcpp rustorbjava jacorb omniorbpy"
  for c in $all omniorbcpp; do
    for s in $all; do pairs+=("$c:$s"); done
  done
fi

cargo build --release --locked -p rustorb-interop --bin rustorb-pi-probe --manifest-path ../Cargo.toml
mkdir -p impls/rustorb/bin
cp ../target/release/rustorb-pi-probe impls/rustorb/bin/
impls/rustorbpy/build.sh
impls/rustorbcpp/build.sh > /dev/null
impls/rustorbjava/build.sh > /dev/null

failed=0
for giop in ${GIOP_VERSIONS:-1.2 1.1 1.0}; do
for pair in "${pairs[@]}"; do
  IFS=: read -r client server <<< "$pair"
  if [ "$client" = jacorb ] && [ "$giop" != 1.2 ]; then
    echo "SKIP $pair GIOP $giop"
    continue
  fi
  expected=$(printf 'reply context: %s got: from %s\ncomponent: %s component\nresult: hello\nrequest context: from %s' \
    "$server" "$client" "$server" "$client")
  # The lines not compared for this pair (see above).
  skip() {
    if [ "$client" = omniorbcpp ] || { [ "$server" = omniorbpy ] && [ "$giop" = 1.1 ]; }; then
      grep -v -e '^component: ' -e '^reply context: '
    elif [ "$server" = omniorbpy ]; then
      grep -v '^component: '
    else
      cat
    fi <<< "$1"
  }
  echo "== $client -> $server (GIOP $giop)"
  # Inside the container: start the server, wait for its IOR, run the client.
  if actual=$(docker run --rm \
      -v "$PWD/impls/rustorb:/opt/interop/impls/rustorb:ro" \
      -v "$PWD/impls/rustorbpy:/opt/interop/impls/rustorbpy:ro" \
      -v "$PWD/impls/rustorbcpp:/opt/interop/impls/rustorbcpp:ro" \
      -v "$PWD/impls/rustorbjava:/opt/interop/impls/rustorbjava:ro" \
      "$image" bash -c '
        client=$1 giop=$3
        probe() {
          case "$1" in
            rustorb) shift; impls/rustorb/bin/rustorb-pi-probe "$@" ;;
            rustorbpy) shift; impls/rustorbpy/pi-probe "$@" ;;
            rustorbcpp) shift; impls/rustorbcpp/pi-probe "$@" ;;
            rustorbjava) shift; impls/rustorbjava/pi-probe "$@" ;;
            jacorb) shift; impls/jacorb/pi-probe "$@" ;;
            omniorbpy) shift; impls/omniorbpy/pi-probe "$@" ;;
            omniorbcpp) shift; impls/omniorb-cpp/pi_probe "$@" ;;
          esac
        }
        probe "$2" server > /tmp/ior 2> /tmp/server.err &
        for _ in $(seq 100); do grep -q IOR: /tmp/ior && break; sleep 0.1; done
        ior=$(grep IOR: /tmp/ior | head -1)
        if [ "$1" = jacorb ]; then set --; else set -- -ORBmaxGIOPVersion "$giop"; fi
        probe "$client" client "$ior" "$@" 2> /tmp/client.err \
          || { cat /tmp/server.err /tmp/client.err >&2; exit 1; }
        # With omniORB clients, the JacORB server first sees a message
        # without the context (before the request itself).
        grep -m1 "^request context: from $client\$" /tmp/ior \
          || grep -m1 "^request context: " /tmp/ior
      ' _ "$client" "$server" "$giop") && [ "$(skip "$actual")" = "$(skip "$expected")" ]; then
    echo "PASS $pair GIOP $giop"
  else
    printf '%s\n' "$actual"
    echo "FAIL $pair GIOP $giop (expected:"
    printf '%s\n' "$expected"
    echo ")"
    failed=1
  fi
done
done
exit $failed
