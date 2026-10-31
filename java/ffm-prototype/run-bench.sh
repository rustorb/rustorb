#!/usr/bin/env bash
# Builds the FFM prototype and the same benchmark on the JNI binding, and
# runs both on one JVM (Java 22 or later; JAVA_HOME or the java on PATH).
#
#   java/ffm-prototype/run-bench.sh            # needs idl_parser importable,
#   IDL_PARSER_DIR=path/to/idl_parser java/ffm-prototype/run-bench.sh
#
# Prints "binding<TAB>case<TAB>ns per call" lines. -Dbench.rounds=N and
# -Dbench.round=MS (in JAVA_OPTS) change the rounds.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../.."
out="${OUT:-$here/target}"
java="${JAVA_HOME:+$JAVA_HOME/bin/}java"
javac="${JAVA_HOME:+$JAVA_HOME/bin/}javac"

cargo build --release --locked -p rustorb-ffi -p rustorb-jni --manifest-path "$root/Cargo.toml"
mvn -q -f "$root/java/pom.xml" -DskipTests ${IDL_PARSER_DIR:+-Didl.parser.dir=$IDL_PARSER_DIR} package
jar="$(ls "$root"/java/target/rustorb-*.jar | head -1)"

rm -rf "$out"
mkdir -p "$out/gen" "$out/jni" "$out/ffm"
pythonpath="$root/crates/rustorb-codegen/python"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR:$pythonpath"
PYTHONPATH="$pythonpath" python3 -m rustorb_idl -bjava -C "$out/gen" "$here/idl/Bench.idl"

"$javac" -nowarn -cp "$jar" -d "$out/jni" $(find "$out/gen" -name '*.java') \
  "$here/bench/Harness.java" "$here/bench/BenchJni.java"
"$javac" --release 22 -d "$out/ffm" $(find "$here/src" -name '*.java') \
  "$here/bench/Harness.java" "$here/bench/BenchFfm.java"

lib="$root/target/release"
case "$(uname -s)" in
  Darwin) jni="$lib/librustorb_jni.dylib"; ffi="$lib/librustorb_ffi.dylib" ;;
  *) jni="$lib/librustorb_jni.so"; ffi="$lib/librustorb_ffi.so" ;;
esac
# shellcheck disable=SC2086
"$java" ${JAVA_OPTS:-} -Drustorb.jni.library="$jni" -cp "$jar:$out/jni" BenchJni
# shellcheck disable=SC2086
"$java" ${JAVA_OPTS:-} --enable-native-access=ALL-UNNAMED -Drustorb.ffi.library="$ffi" \
  -cp "$out/ffm" BenchFfm
