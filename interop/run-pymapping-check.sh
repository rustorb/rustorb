#!/usr/bin/env bash
# Compares the Python code that rustorb-idl generates with the code of
# omniidl -bpython, through the public API that each makes visible
# (scripts/pymapping-dump).
#
#   ./run-pymapping-check.sh [--update]
#
# --update   rewrites the expected files with the current output
#
# omniidl runs in the omniORB image and its code is dumped under omniORBpy;
# rustorb-idl runs on the host (idl_parser must be importable, or set
# IDL_PARSER_DIR to a checkout of it) and its code is dumped under the
# rustorb Python package (../python). The outputs are compared with
# expected/pymapping-omniorbpy.txt and expected/pymapping-rustorb.txt; the
# differences between those two files are listed in README.md.
set -euo pipefail

cd "$(dirname "$0")"
update=0
if [ "${1:-}" = "--update" ]; then
  update=1
fi
image=${SERVER_IMAGE:-rustorb-interop-omniorb:latest}
codegen_idl=../crates/rustorb-codegen/tests/idl
modules=(Mapping _GlobalIDL Mapping__POA _GlobalIDL__POA CosNaming CosNaming__POA
         InteropTest InteropTest__POA PoaProbe PoaProbe__POA)

rm -rf out/pymapping
mkdir -p out/pymapping/omniorbpy out/pymapping/rustorb

docker run --rm -u "$(id -u):$(id -g)" \
  -v "$PWD/out:/out" -v "$PWD/idl:/idl:ro" -v "$PWD/$codegen_idl:/codegen-idl:ro" \
  -v "$PWD/scripts:/scripts:ro" -w /out/pymapping/omniorbpy "$image" bash -c '
    set -e
    for f in /codegen-idl/Mapping.idl /codegen-idl/CosNaming.idl /idl/InteropTest.idl /idl/PoaProbe.idl; do
      omniidl -bpython "$f" 2>/dev/null
    done
    /scripts/pymapping-dump --runtime omniorbpy "$@"' bash "${modules[@]}" \
  > out/pymapping-omniorbpy.txt

pythonpath=$PWD/../crates/rustorb-codegen/python
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath=$IDL_PARSER_DIR:$pythonpath
for f in "$codegen_idl/Mapping.idl" "$codegen_idl/CosNaming.idl" idl/InteropTest.idl idl/PoaProbe.idl; do
  PYTHONPATH=$pythonpath python3 -m rustorb_idl -bpython -C out/pymapping/rustorb "$f"
done
(cd out/pymapping/rustorb && PYTHONPATH=$PWD/../../../../python \
  ../../../scripts/pymapping-dump --runtime rustorb "${modules[@]}") > out/pymapping-rustorb.txt

failed=0
for impl in omniorbpy rustorb; do
  actual=out/pymapping-$impl.txt
  expected=expected/pymapping-$impl.txt
  if [ $update -eq 1 ]; then
    cp "$actual" "$expected"
    echo "updated $expected"
  elif diff -u "$expected" "$actual"; then
    echo "python mapping $impl: output matches $expected"
  else
    echo "python mapping $impl: output differs from $expected" >&2
    failed=1
  fi
done
exit $failed
