#!/usr/bin/env bash
# Builds the Java client and server of impls/java on the rustorb Java runtime
# with the portable stubs and skeletons of JacORB's IDL compiler (Helpers,
# _XStub on ObjectImpl._request / _invoke, XPOA on InvokeHandler) in place
# of those of rustorb-idl -bjava: the check that code from another IDL
# compiler runs on rustorb (docs/java-binding.md).
#
# The JacORB-generated sources come from the interop image
# (/opt/interop/impls/jacorb/gen, made by its jacorb-build stage), so the
# image must be built first. librustorb_jni (in lib/) and, in classes/, the
# rustorb Java runtime (java/), the generated sources and impls/java are built
# on the host, for Java 11; the image runs them on Java 17.
set -euo pipefail

# Built already (interop.yml builds every implementation once, then sets
# INTEROP_PREBUILT for the steps that run the scripts).
[ -n "${INTEROP_PREBUILT:-}" ] && exit 0

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../../.."
image="${IMAGE:-rustorb-interop-omniorb:latest}"

cargo build --release --locked -p rustorb-jni --manifest-path "$root/Cargo.toml"

rm -rf "$here/gen" "$here/classes" "$here/lib"
mkdir -p "$here/gen" "$here/classes" "$here/lib"
docker run --rm "$image" tar -C /opt/interop/impls/jacorb/gen -cf - . | tar -C "$here/gen" -xf -
find "$root/java/src/main/java" "$here/gen" "$here/../java" -name '*.java' > "$here/sources.txt"
javac --release 11 -encoding UTF-8 -nowarn -d "$here/classes" @"$here/sources.txt"
rm "$here/sources.txt"
cp "$root/target/release/librustorb_jni.so" "$here/lib/"
