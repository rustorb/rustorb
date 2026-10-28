#!/usr/bin/env bash
# Runs the CosNaming operation sequence against the naming service and
# compares the output with the expected result for omniNames.
#
#   ./run-naming-check.sh [--update]
#
# --update   rewrites expected/naming-check.txt with the current output
#
# NAMING_IMAGE / NAMING_CMD select the naming service as in compose.yaml, so
# the same check can later be pointed at the rustorb Naming Service.
set -euo pipefail

cd "$(dirname "$0")"
expected=expected/naming-check.txt
update=0
[ "${1:-}" = "--update" ] && update=1

mkdir -p out
project=interop-naming-check
compose=(docker compose -p "$project" -f compose.yaml)
trap '"${compose[@]}" down -t 2 >/dev/null 2>&1' EXIT

"${compose[@]}" up -d naming
"${compose[@]}" run --rm -T --no-deps client \
  impls/omniorbpy/naming-check --naming corbaloc::naming:2809/NameService \
  > out/naming-check.txt
"${compose[@]}" logs --no-color naming > out/naming-check-naming.log 2>&1

if [ $update -eq 1 ]; then
  mkdir -p "$(dirname "$expected")"
  cp out/naming-check.txt "$expected"
  echo "updated $expected"
elif diff -u "$expected" out/naming-check.txt; then
  echo "naming check: output matches $expected"
else
  echo "naming check: output differs from $expected" >&2
  exit 1
fi
