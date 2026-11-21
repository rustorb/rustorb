#!/usr/bin/env bash
# Builds the rustorb .NET client and server on the host: librustorb_ffi
# and, in out/, the .NET runtime (dotnet/src/Rustorb.Corba), the stubs and
# skeletons of InteropTest.idl from rustorb-idl -bcsharp (in gen/) and the
# sources in src/. run-matrix.sh runs it; rustorb-idl must be importable
# (tools/rustorb-idl.txt), or RUSTORB_IDL_DIR / IDL_PARSER_DIR set to
# checkouts. The interop image runs out/rustorbcs.dll on its .NET 8 runtime.
#
# librustorb_ffi runs in the interop image (Ubuntu 24.04), so the host needs
# a glibc no newer than that image's.
set -euo pipefail

[ -n "${INTEROP_PREBUILT:-}" ] && exit 0

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../../.."

cargo build --release --locked -p rustorb-ffi --manifest-path "$root/Cargo.toml"

rm -rf "$here/gen" "$here/out"
mkdir -p "$here/gen"
pythonpath="${RUSTORB_IDL_DIR:+$RUSTORB_IDL_DIR/python}"
[ -n "${IDL_PARSER_DIR:-}" ] && pythonpath="$IDL_PARSER_DIR${pythonpath:+:$pythonpath}"
PYTHONPATH="$pythonpath" python3 -m rustorb_idl -bcsharp -I "$root/interop/idl" -C "$here/gen" \
  "$root/interop/idl/InteropTest.idl" "$root/interop/idl/PoaProbe.idl" "$root/interop/idl/PiProbe.idl"
dotnet publish "$here/rustorbcs.csproj" -c Release -o "$here/out" --nologo -v quiet
case "$(uname -s)" in
  Darwin) cp "$root/target/release/librustorb_ffi.dylib" "$here/out/" ;;
  *) cp "$root/target/release/librustorb_ffi.so" "$here/out/" ;;
esac
