# リリースの手順

Oct 31, 2026 · [#83](https://github.com/rustorb/rustorb/issues/83)

rustorb のリリースで何を出すか、どう作るか、公開をどう行うかをまとめる。`.github/workflows/release.yml` がタグから成果物をビルドして確かめ、GitHub Release の下書きに付けたあと、crates.io と PyPI に公開し、Release を公開する。公開は取り消せないので、すべてのビルドと確認が通ったときだけ行い、公開のジョブは `release` 環境で動かす（環境の保護ルールで承認を必須にできる）。

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
3. Release のワークフローが次を行う。ビルドか確認のどれかが失敗したら、何も公開しない
   - タグとパッケージのバージョンが同じで、`CHANGELOG.md` にその版があること
   - `cargo publish --workspace --dry-run`（依存の順にパッケージにし、パッケージどうしでビルドする）と `.crate` の作成
   - wheel（各環境で入れて import する）、sdist、純 Python のパッケージ（`rustorb-idl` は PyPI の idl_parser と入れて動かす）
   - `rustorb-native`（CMake で入れ、`cpp/tests/consumer` を `find_package` でビルドして動かす）、jar
   - Release `vx.y.z` の下書きに成果物、`SHA256SUMS`、`CHANGELOG.md` のその版の節を付ける
   - crates.io に依存の順（cdr → giop → core → codegen → ffi → naming）で公開する（`crates-io` ジョブ）
   - PyPI に `rustorb`（wheel と sdist）、`rustorb-idl`、`rustorb-omniorb-compat` を公開する（`pypi` ジョブ）
   - 両方が終わったら Release を公開する（`publish-release` ジョブ）
4. 公開のジョブが途中で失敗したら、原因を直してから、失敗したジョブを Actions の画面で再実行する。公開済みのクレートとファイルは飛ばす（クレートは crates.io にその版があるか確かめ、PyPI は `skip-existing`）
5. 何も公開する前に失敗し、直すのにワークフローかソースの変更が要るときは（再実行はタグのコミットのワークフローで走るので）、直したものを main に入れてから、Release とタグ `vx.y.z` を消し（Release の画面の Delete と、Tags の画面のタグの Delete）、main の上に作り直す

タグは GitHub のウェブからも打てる。[新しい Release](https://github.com/rustorb/rustorb/releases/new) でタグ `vx.y.z` を main に作り「Publish release」を押すと、タグの push と同じくワークフローが走り、成果物とノートがその Release に付く（その間、Release は成果物なしで見える）。「Save draft」ではタグができないのでワークフローは走らない。

ワークフローは、タグのほか、手で実行したときと、パッケージに関わるファイルを変えたプルリクエストでも走る（ビルドだけで、成果物は実行に残る）。

## 公開の設定

一度だけ、リポジトリの設定で次を用意する。

- **環境 `release`**（Settings → Environments）：crates.io と PyPI の `rustorb` の公開が使う（PyPI の他の2つは下の表の環境）。Required reviewers を付けると、公開の前に承認を求める。Deployment branches and tags でタグ `v*` だけに絞るとよい
- **crates.io**：crates.io の [API Tokens](https://crates.io/settings/tokens) で `publish-new` と `publish-update` のスコープのトークンを作り、環境 `release` のシークレット `CARGO_REGISTRY_TOKEN` に入れる。新しいクレートの最初の公開にはトークンが要る（crates.io の Trusted Publishing は、クレートができた後でしか設定できない）
- **PyPI**：Trusted Publishing を使う。PyPI の [Publishing](https://pypi.org/manage/account/publishing/) で、3つのプロジェクトに pending publisher を登録する。Owner `rustorb`、Repository `rustorb`、Workflow `release.yml` は共通で、Environment だけをプロジェクトごとに変える（PyPI は、同じ設定の pending publisher を1つのプロジェクトにしか登録できないため）。環境は GitHub 側にも作る

  | PyPI のプロジェクト | Environment |
  | --- | --- |
  | `rustorb` | `release` |
  | `rustorb-idl` | `release-rustorb-idl` |
  | `rustorb-omniorb-compat` | `release-rustorb-omniorb-compat` |

  トークンを使うなら、代わりに各環境のシークレット `PYPI_API_TOKEN` に入れる

公開した版は取り消せない（crates.io は yank、PyPI は yank か削除しても同じ版は二度と上げられない）。間違えたら次のパッチ版を出す。

手で公開するときは、Release から成果物を落として（`gh release download vx.y.z -D dist`、`sha256sum -c SHA256SUMS`）、タグのコミットで `cargo publish --workspace --locked --exclude rustorb-python --exclude rustorb-interop` と `python -m twine upload dist/*.whl dist/rustorb-x.y.z.tar.gz dist/rustorb_idl-* dist/rustorb_omniorb_compat-*` を行う。

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
- 2回目以降は crates.io も Trusted Publishing にしてトークンをなくすか
