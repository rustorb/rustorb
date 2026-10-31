# リリースの手順

Oct 31, 2026 · [#83](https://github.com/rustorb/rustorb/issues/83)

rustorb のリリースで何を出すか、どう作るか、公開をどう行うかをまとめる。成果物は `.github/workflows/release.yml` がタグからビルドして GitHub Release の下書きに付ける。crates.io と PyPI への公開は取り消せないので、ワークフローでは行わず、メンテナーが下書きを確かめてから手で行う。

## 出すもの

| 成果物 | 中身 | 公開先 |
| --- | --- | --- |
| クレート | `rustorb-cdr`、`rustorb-giop`、`rustorb-core`、`rustorb-naming`（`rustorb-names` を含む）、`rustorb-ffi`、`rustorb-codegen` | crates.io |
| `rustorb` の wheel | ネイティブモジュール込みの abi3 wheel（CPython 3.9 以降）。Linux x86_64 / aarch64（manylinux 2_28）、macOS x86_64 / arm64、Windows x64。sdist も | PyPI |
| `rustorb-idl` | IDL コンパイラ（純 Python。idl_parser 0.6.2 以降に依存） | PyPI |
| `rustorb-omniorb-compat` | omniORBpy 互換パッケージ（純 Python） | PyPI |
| `rustorb-native-<版>-<環境>` | `librustorb_ffi`（静的と共有）、C++ のヘッダ、CMake のパッケージ（`find_package(rustorb)`）。Linux x86_64 / aarch64、macOS arm64、Windows x86_64 | GitHub Release |
| `rustorb-<版>.jar` | Java のマッピング。共有ライブラリは上の `rustorb-native` から使う | GitHub Release |

`rustorb-python`（wheel の中身）と `rustorb-interop`（相互接続テスト）は crates.io に出さない（`publish = false`）。

## バージョン

バージョンは Cargo のワークスペース、3つの `pyproject.toml`、`rustorb_idl.__version__`、`java/pom.xml`、`cpp/CMakeLists.txt` に書いてあり、すべて同じにする。

```sh
tools/set-version.py 0.2.0          # まとめて書き換える
cargo update --workspace            # Cargo.lock を合わせる（--locked のビルドのため）
tools/set-version.py --check        # そろっているか（CI とリリースのワークフローでも確かめる）
```

1.0 までは、マイナーバージョンを上げたときに API が変わりうる（C++ の CMake パッケージも `SameMinorVersion` で照合する）。

## リリースの流れ

1. `CHANGELOG.md` の `## [x.y.z] - unreleased` を確かめ、日付を入れる。新しい版なら `tools/set-version.py x.y.z` と `cargo update --workspace` も行う
2. main に入れたら、タグを打って push する：`git tag vx.y.z && git push origin vx.y.z`
3. Release のワークフローが次を行う。どれかが失敗したら下書きは作られない
   - タグとパッケージのバージョンが同じで、`CHANGELOG.md` にその版があること
   - `cargo publish --workspace --dry-run`（依存の順にパッケージにし、パッケージどうしでビルドする）と `.crate` の作成
   - wheel（各環境で入れて import する）、sdist、純 Python のパッケージ（`rustorb-idl` は PyPI の idl_parser と入れて動かす）
   - `rustorb-native`（CMake で入れ、`cpp/tests/consumer` を `find_package` でビルドして動かす）、jar
   - Release `vx.y.z` の下書きに成果物、`SHA256SUMS`、`CHANGELOG.md` のその版の節を付ける
4. 下書きを確かめ、下の「公開」の手順で crates.io と PyPI に上げる
5. 下書きの Release を公開する

ワークフローは、タグのほか、手で実行したときと、パッケージに関わるファイルを変えたプルリクエストでも走る（ビルドだけで、成果物は実行に残る）。

## 公開

下書きから成果物を落として、メンテナーの手元で行う。

```sh
gh release download vx.y.z -D dist
cd dist && sha256sum -c SHA256SUMS
```

**crates.io**：依存の順に上げる必要があるので、ワークスペースから `cargo publish` する（cargo が順を決め、それぞれが索引に載るのを待つ）。タグのコミットで行う。

```sh
git checkout vx.y.z
cargo publish --workspace --locked --exclude rustorb-python --exclude rustorb-interop
```

**PyPI**：`rustorb`（wheel と sdist）、`rustorb-idl`、`rustorb-omniorb-compat` を上げる。`rustorb-omniorb-compat` は `rustorb` に、`rustorb-idl` は idl_parser に依存するので、`rustorb` を先に上げる。

```sh
python -m twine upload dist/rustorb-x.y.z*.whl dist/rustorb-x.y.z.tar.gz
python -m twine upload dist/rustorb_idl-* dist/rustorb_omniorb_compat-*
```

公開した版は取り消せない（crates.io は yank、PyPI は yank か削除しても同じ版は二度と上げられない）。間違えたら次のパッチ版を出す。

## 成果物の使い方

**C++**：`rustorb-native` を展開した場所を `CMAKE_PREFIX_PATH` に入れる。IDL からのコード生成には `pip install rustorb-idl` で入る `rustorb-idl` を使う（PATH にあるか、`RUSTORB_IDL` で指定する）。

```cmake
find_package(rustorb 0.1 REQUIRED)
rustorb_idl_generate(SOURCES Echo.idl)   # build/idl/Echo.hh、Echo.cc
add_executable(app main.cc ${SOURCES})
target_include_directories(app PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/idl")
target_link_libraries(app PRIVATE rustorb::rustorb)
```

ソースツリーからも入れられる：`cmake -S cpp -B build -DRUSTORB_BUILD_TESTS=OFF && cmake --build build && cmake --install build --prefix PREFIX`（cargo で `librustorb_ffi` をビルドする）。例は `cpp/tests/consumer`。

**Java**：jar をクラスパスに入れ、`rustorb-native` の共有ライブラリ（`lib/librustorb_ffi.so`、`lib/librustorb_ffi.dylib`、`bin/rustorb_ffi.dll`）を `-Drustorb.ffi.library=PATH` か `java.library.path` で渡す（[java-binding.md](java-binding.md)）。Java 22 以降で `--enable-native-access=ALL-UNNAMED` を付ける。

## 残り

- Java を Maven Central に出すか（名前空間の確認と署名鍵が要る）、共有ライブラリを jar に同梱するか
- PyPI の Trusted Publishing と crates.io のトークンを使って、公開もワークフローから行うか
