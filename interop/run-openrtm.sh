#!/usr/bin/env bash
# Runs examples of OpenRTM-aist-Python (data ports and a service port) with
# the RTCs on rustorb, through the omniORBpy compatibility package
# (python/omniorb-compat), on omniORBpy, and in C++ (OpenRTM-aist on
# omniORB) (openrtm/scenarios.py), and rtshell on rustorb and on omniORBpy
# against those RTCs (openrtm/rtshell.py).
#
#   ./run-openrtm.sh [--local] [--naming IMPL] [--only SCENARIO]...
#
# OpenRTM-aist-Python, rtshell and rtctree are cloned at pinned commits into
# out/openrtm, unless OPENRTM_DIR, RTSHELL_DIR or RTCTREE_DIR name
# checkouts. The rustorb package, the rustorb stubs (rtctree's with
# -Wbpackage=rtctree.rtc) and rustorb-names are built on the host
# (idl_parser must be importable, or IDL_PARSER_DIR set); omniidl, the
# naming service and the RTCs run in the omniORB image, which has the C++
# examples of OpenRTM-aist.
#
# --naming IMPL  the naming service: omninames (default) or rustorb
#                (rustorb-names); the results go to out/openrtm/IMPL
# --only S       run the scenario S of scenarios.py only, and not rtshell.py
# --local        runs everything on the host instead: omniidl and omniNames
#                on the PATH, OMNIORBPY_DIR the directory of omniORBpy's
#                modules, PYTHON the Python that loads it (default python3),
#                OPENRTM_CXX_EXAMPLES the directory of the C++ examples
#                (ConsoleInComp...; without it the C++ RTCs are skipped)
set -euo pipefail

cd "$(dirname "$0")"
openrtm_repo=https://github.com/OpenRTM/OpenRTM-aist-Python.git
openrtm_ref=b58852f1ef7e515030c4673723406b7d2c749a0c
rtshell_repo=https://github.com/OpenRTM/rtshell.git
rtshell_ref=d35459c947f1e2e1fecbfacde2e912184b869adb
rtctree_repo=https://github.com/OpenRTM/rtctree.git
rtctree_ref=9803b6b52d3c006bc30d45f07ae40cbe1bd615b4
image=${SERVER_IMAGE:-rustorb-interop-omniorb:latest}

local=0
naming=omninames
only=()
while [ $# -gt 0 ]; do
  case "$1" in
    --local) local=1 ;;
    --naming) naming="$2"; shift ;;
    --only) only+=(--only "$2"); shift ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

out=out/openrtm
mkdir -p "$out"

# clone URL REF DIR [FILE]: a shallow checkout of REF in DIR, unless DIR
# already has FILE
clone() {
  if [ ! -e "$3/$4" ]; then
    rm -rf "$3"
    git init -q "$3"
    git -C "$3" fetch -q --depth 1 "$1" "$2"
    git -C "$3" checkout -q FETCH_HEAD
  fi
  (cd "$3" && pwd)
}
src=$(clone "$openrtm_repo" "$openrtm_ref" "${OPENRTM_DIR:-$out/OpenRTM-aist-Python}" OpenRTM_aist)
rtshell=$(clone "$rtshell_repo" "$rtshell_ref" "${RTSHELL_DIR:-$out/rtshell}" rtshell)
rtctree=$(clone "$rtctree_repo" "$rtctree_ref" "${RTCTREE_DIR:-$out/rtctree}" rtctree)

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

# rtctree imports its stubs from the package rtctree.rtc: a copy of rtctree
# for each ORB, with the stubs of its IDL generated into it.
for impl in rustorb omniorbpy; do
  rm -rf "$out/rtctree-$impl"
  mkdir -p "$out/rtctree-$impl"
  cp -r "$rtctree/rtctree" "$out/rtctree-$impl/"
done
PYTHONPATH=$pythonpath python3 -m rustorb_idl -bpython -Wbpackage=rtctree.rtc \
  -C "$out/rtctree-rustorb" -I"$rtctree/idl" "$rtctree"/idl/*.idl

results=$out/$naming
mkdir -p "$results"
case "$naming" in
  omninames) naming_cmd="omniNames -start {port} -logdir {dir}" ;;
  rustorb)
    cargo build --release --locked -p rustorb-naming --bin rustorb-names \
      --manifest-path ../Cargo.toml
    mkdir -p "$out/bin"
    cp ../target/release/rustorb-names "$out/bin/"
    naming_cmd="BIN/rustorb-names -start {port} -logdir {dir}"
    ;;
  *) echo "unknown naming service: $naming" >&2; exit 2 ;;
esac

if [ $local -eq 1 ]; then
  : "${OMNIORBPY_DIR:?set OMNIORBPY_DIR to the directory of the omniORBpy modules}"
  omniidl -bpython -C "$out/omniorbpy-stubs" -I"$idl_include" "${idls[@]}"
  omniidl -bpython -Wbpackage=rtctree.rtc -C "$out/rtctree-omniorbpy" -I"$rtctree/idl" \
    "$rtctree"/idl/*.idl
  py=${PYTHON:-python3}
  common=(--openrtm "$src" --rustorb impls/rustorbpy/lib --compat ../python/omniorb-compat
          --rustorb-stubs "$out/rustorb-stubs" --omniorbpy "$OMNIORBPY_DIR"
          --omniorbpy-stubs "$out/omniorbpy-stubs" --out "$results" --python "$py"
          --naming "${naming_cmd//BIN/$PWD/$out/bin}")
  [ -n "${OPENRTM_CXX_EXAMPLES:-}" ] && common+=(--cxx-examples "$OPENRTM_CXX_EXAMPLES")
  status=0
  "$py" openrtm/scenarios.py "${common[@]}" "${only[@]}" || status=1
  if [ ${#only[@]} -eq 0 ]; then
    "$py" openrtm/rtshell.py "${common[@]}" --rtshell "$rtshell" \
      --rtctree-rustorb "$out/rtctree-rustorb" \
      --rtctree-omniorbpy "$out/rtctree-omniorbpy" || status=1
  fi
  exit $status
fi

# In the image: the IDL paths as mounted there.
cidls=()
for f in "${idls[@]}"; do
  cidls+=("/openrtm/${f#"$src"/}")
done
docker run --rm -u "$(id -u):$(id -g)" \
  -v "$src:/openrtm:ro" -v "$rtshell:/rtshell:ro" -v "$rtctree/idl:/rtctree-idl:ro" \
  -v "$PWD/$out:/out" -v "$PWD/openrtm:/scripts:ro" \
  -v "$PWD/impls/rustorbpy/lib:/rustorb:ro" -v "$PWD/../python/omniorb-compat:/compat:ro" \
  -e HOME=/tmp -e NAMING_CMD="${naming_cmd//BIN//out/bin}" -e RESULTS="/out/$naming" \
  -e ONLY="${only[*]}" "$image" bash -c '
    set -e
    omniidl -bpython -C /out/omniorbpy-stubs -I/openrtm/OpenRTM_aist/RTM_IDL "$@" 2>/dev/null
    omniidl -bpython -Wbpackage=rtctree.rtc -C /out/rtctree-omniorbpy -I/rtctree-idl \
      /rtctree-idl/*.idl 2>/dev/null
    common=(--openrtm /openrtm --rustorb /rustorb --compat /compat
            --rustorb-stubs /out/rustorb-stubs --omniorbpy /opt/omniorb/python
            --omniorbpy-stubs /out/omniorbpy-stubs --out "$RESULTS"
            --cxx-examples /opt/openrtm/examples --naming "$NAMING_CMD")
    status=0
    # shellcheck disable=SC2086 # $ONLY is empty or --only options
    python3 /scripts/scenarios.py "${common[@]}" $ONLY || status=1
    if [ -z "$ONLY" ]; then
      python3 /scripts/rtshell.py "${common[@]}" --rtshell /rtshell \
        --rtctree-rustorb /out/rtctree-rustorb --rtctree-omniorbpy /out/rtctree-omniorbpy \
        || status=1
    fi
    exit $status
  ' bash "${cidls[@]}"
