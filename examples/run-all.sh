#!/usr/bin/env bash
# Builds the examples and runs every server with every client (check.py).
# CI runs this on Linux; it needs cargo, cmake, a C and C++ compiler,
# Python 3.9+, Java 22+ and Maven (Java), the .NET 8 SDK (C#) and Ruby 3.0+
# with its headers (Ruby).
#
#   examples/run-all.sh                      # all seven
#   EXAMPLES="rust python" examples/run-all.sh
#
# rustorb-idl and the rustorb Python package are installed into a venv
# under build/examples (rustorb-idl takes idl_parser from PyPI).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
build="$root/build/examples"
examples="${EXAMPLES:-rust python cpp java csharp c ruby}"
mkdir -p "$build"
cd "$root"

python3 -m venv "$build/venv"
export PATH="$build/venv/bin:$PATH"
# RUSTORB_IDL_PIP: what to pip install instead of tools/rustorb-idl.txt.
# shellcheck disable=SC2086
python -m pip install -q ${RUSTORB_IDL_PIP:--r tools/rustorb-idl.txt}
idl_dir="$(rustorb-idl --idl-dir)"

cargo build --release --locked -p rustorb-naming --bin rustorb-names
impls=()

for e in $examples; do
  case "$e" in
    rust)
      cargo build --manifest-path examples/rust/Cargo.toml
      impls+=(--impl "rust=examples/rust/target/debug/server|examples/rust/target/debug/client")
      ;;
    python)
      python -m pip install -q ./python
      rm -rf "$build/python" && mkdir -p "$build/python"
      rustorb-idl -bpython -C "$build/python" examples/idl/Echo.idl "$idl_dir/CosNaming.idl"
      py="env PYTHONPATH=$build/python python"
      impls+=(--impl "python=$py examples/python/server.py|$py examples/python/client.py")
      ;;
    cpp)
      cmake -S cpp -B "$build/cpp-install" -DRUSTORB_BUILD_TESTS=OFF -DCMAKE_BUILD_TYPE=Release
      cmake --build "$build/cpp-install"
      cmake --install "$build/cpp-install" --prefix "$build/prefix"
      cmake -S examples/cpp -B "$build/cpp" -DCMAKE_PREFIX_PATH="$build/prefix"
      cmake --build "$build/cpp"
      impls+=(--impl "cpp=$build/cpp/server|$build/cpp/client")
      ;;
    java)
      # The Java job of CI runs the tests; package still compiles them,
      # with rustorb-idl and idl_parser from the venv.
      mvn -B -q -f java/pom.xml -Dpython=python -DskipTests package
      jar="$(ls java/target/rustorb-*.jar | head -n 1)"
      rm -rf "$build/java" && mkdir -p "$build/java/gen" "$build/java/classes"
      rustorb-idl -bjava -C "$build/java/gen" examples/idl/Echo.idl
      # shellcheck disable=SC2046
      javac --release 22 -cp "$jar" -d "$build/java/classes" \
        examples/java/Server.java examples/java/Client.java $(find "$build/java/gen" -name '*.java')
      case "$(uname -s)" in
        Darwin) lib="$root/target/release/librustorb_ffi.dylib" ;;
        *) lib="$root/target/release/librustorb_ffi.so" ;;
      esac
      java="java --enable-native-access=ALL-UNNAMED -Drustorb.ffi.library=$lib -cp $jar:$build/java/classes"
      impls+=(--impl "java=$java Server|$java Client")
      ;;
    csharp)
      cargo build --release --locked -p rustorb-ffi
      case "$(uname -s)" in
        Darwin) ffi="$root/target/release/librustorb_ffi.dylib" ;;
        *) ffi="$root/target/release/librustorb_ffi.so" ;;
      esac
      rm -rf "$build/csharp" && mkdir -p "$build/csharp/gen"
      rustorb-idl -bcsharp -C "$build/csharp/gen" examples/idl/Echo.idl
      for p in server client; do
        dotnet build -c Release -v q -nologo -p:ExampleGen="$build/csharp/gen/" \
          -o "$build/csharp/$p" "examples/csharp/$p/$p.csproj"
      done
      cs="env RUSTORB_FFI_LIBRARY=$ffi dotnet"
      impls+=(--impl "csharp=$cs $build/csharp/server/server.dll|$cs $build/csharp/client/client.dll")
      ;;
    c)
      cmake -S c -B "$build/c-install" -DRUSTORB_BUILD_TESTS=OFF -DCMAKE_BUILD_TYPE=Release
      cmake --build "$build/c-install"
      cmake --install "$build/c-install" --prefix "$build/c-prefix"
      rm -rf "$build/c" && mkdir -p "$build/c/gen"
      rustorb-idl -bc -C "$build/c/gen" examples/idl/Echo.idl
      case "$(uname -s)" in
        Darwin) clibs=(-framework CoreFoundation -framework Security -lpthread -lm) ;;
        *) clibs=(-lpthread -ldl -lm) ;;
      esac
      for p in server client; do
        cc -std=c99 -Wall -Wextra -I"$build/c-prefix/include" -I"$build/c/gen" "examples/c/$p.c" \
          "$build/c/gen/Echo.c" "$build/c-prefix/lib/librustorb_c.a" "$build/c-prefix/lib/librustorb_ffi.a" \
          "${clibs[@]}" -o "$build/c/$p"
      done
      impls+=(--impl "c=$build/c/server|$build/c/client")
      ;;
    ruby)
      (cd ruby && rake compile)
      rm -rf "$build/ruby" && mkdir -p "$build/ruby"
      rustorb-idl -bruby -C "$build/ruby" examples/idl/Echo.idl
      rb="ruby -I$root/ruby/lib -I$build/ruby"
      impls+=(--impl "ruby=$rb examples/ruby/server.rb|$rb examples/ruby/client.rb")
      ;;
    *) echo "unknown example: $e" >&2; exit 2 ;;
  esac
done

python examples/check.py --names target/release/rustorb-names "${impls[@]}"
