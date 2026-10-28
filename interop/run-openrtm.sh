#!/usr/bin/env bash
# Runs examples of OpenRTM-aist-Python (data ports and a service port) with
# the RTCs on rustorb, through the omniORBpy compatibility package
# (python/omniorb-compat), and on omniORBpy, in the four combinations
# (openrtm/scenarios.py).
#
#   ./run-openrtm.sh [--local] [--only SCENARIO]...
#
# OpenRTM-aist-Python is cloned at a pinned commit into out/openrtm, unless
# OPENRTM_DIR names a checkout. The rustorb package and the rustorb stubs
# are built on the host (idl_parser must be importable, or IDL_PARSER_DIR
# set); omniidl, omniNames and the RTCs run in the omniORB image.
#
# --local   runs everything on the host instead: omniidl and omniNames on
#           the PATH, OMNIORBPY_DIR the directory of omniORBpy's modules,
#           PYTHON the Python that loads it (default python3)
set -euo pipefail

cd "$(dirname "$0")"
openrtm_repo=https://github.com/OpenRTM/OpenRTM-aist-Python.git
openrtm_ref=b58852f1ef7e515030c4673723406b7d2c749a0c
image=${SERVER_IMAGE:-rustorb-interop-omniorb:latest}

local=0
only=()
while [ $# -gt 0 ]; do
  case "$1" in
    --local) local=1 ;;
    --only) only+=(--only "$2"); shift ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

out=out/openrtm
mkdir -p "$out"
src=${OPENRTM_DIR:-$out/OpenRTM-aist-Python}
if [ ! -d "$src/OpenRTM_aist" ]; then
  rm -rf "$src"
  git init -q "$src"
  git -C "$src" fetch -q --depth 1 "$openrtm_repo" "$openrtm_ref"
  git -C "$src" checkout -q FETCH_HEAD
fi
src=$(cd "$src" && pwd)

# The IDL of OpenRTM-aist-Python's build (RTM_IDL and the observers) and of
# the SimpleService example.
idls=("$src"/OpenRTM_aist/RTM_IDL/*.idl
      "$src"/OpenRTM_aist/ext/sdo/observer/ComponentObserver_OpenRTM.idl
      "$src"/OpenRTM_aist/ext/fsm4rtc_observer/ComponentObserver.idl
      "$src"/OpenRTM_aist/examples/SimpleService/MyService.idl)
idl_include=$src/OpenRTM_aist/RTM_IDL

impls/rustorbpy/build.sh
rm -rf "$out/rustorb-stubs" "$out/omniorbpy-stubs"
mkdir -p "$out/rustorb-stubs" "$out/omniorbpy-stubs"
pythonpath=$PWD/../crates/rustorb-codegen/python
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath=$IDL_PARSER_DIR:$pythonpath
PYTHONPATH=$pythonpath python3 -m rustorb_idl -bpython -C "$out/rustorb-stubs" \
  -I"$idl_include" "${idls[@]}"

if [ $local -eq 1 ]; then
  : "${OMNIORBPY_DIR:?set OMNIORBPY_DIR to the directory of the omniORBpy modules}"
  omniidl -bpython -C "$out/omniorbpy-stubs" -I"$idl_include" "${idls[@]}"
  exec "${PYTHON:-python3}" openrtm/scenarios.py --openrtm "$src" \
    --rustorb impls/rustorbpy/lib --compat ../python/omniorb-compat \
    --rustorb-stubs "$out/rustorb-stubs" --omniorbpy "$OMNIORBPY_DIR" \
    --omniorbpy-stubs "$out/omniorbpy-stubs" --out "$out" --python "${PYTHON:-python3}" \
    "${only[@]}"
fi

# In the image: the IDL paths as mounted there.
cidls=()
for f in "${idls[@]}"; do
  cidls+=("/openrtm/${f#"$src"/}")
done
docker run --rm -u "$(id -u):$(id -g)" \
  -v "$src:/openrtm:ro" -v "$PWD/$out:/out" -v "$PWD/openrtm:/scripts:ro" \
  -v "$PWD/impls/rustorbpy/lib:/rustorb:ro" -v "$PWD/../python/omniorb-compat:/compat:ro" \
  -e HOME=/tmp "$image" bash -c '
    set -e
    omniidl -bpython -C /out/omniorbpy-stubs -I/openrtm/OpenRTM_aist/RTM_IDL "$@" 2>/dev/null
    exec python3 /scripts/scenarios.py --openrtm /openrtm --rustorb /rustorb --compat /compat \
      --rustorb-stubs /out/rustorb-stubs --omniorbpy /opt/omniorb/python \
      --omniorbpy-stubs /out/omniorbpy-stubs --out /out '"${only[*]}"'
  ' bash "${cidls[@]}"
