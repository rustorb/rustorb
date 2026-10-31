# rustorb
Rust製のCORBA実装

## ドキュメント

- [開発計画](docs/development-plan.md)
- [Python生成器（rustorb-idl -bpython）](docs/python-codegen.md)
- [IDL to Rust マッピング（rustorb-idl -brust）](docs/rust-mapping.md)
- [Python バインディング（PyO3）](docs/python-binding.md)
- [C++ バインディング（C ABI + IDL to C++11）](docs/cpp-binding.md)
- [Java バインディング（FFM + IDL to Java）](docs/java-binding.md)

## クレート構成

| クレート | 役割 | ライセンス |
| --- | --- | --- |
| `rustorb-cdr` | CDRのエンコード・デコード | MIT OR Apache-2.0 |
| `rustorb-giop` | GIOP/IIOPメッセージ型、IOR・corbaloc | MIT OR Apache-2.0 |
| `rustorb-core` | ORB本体（接続管理、リクエスト配送、POA）。内部でtokioを使い、公開APIは同期 | MIT OR Apache-2.0 |
| `rustorb-naming` | CosNaming（Naming Service） | MIT OR Apache-2.0 |
| `rustorb-python` | Python パッケージ `rustorb` のネイティブモジュール `rustorb._native`（PyO3、[docs/python-binding.md](docs/python-binding.md)） | MIT OR Apache-2.0 |
| `rustorb-ffi` | C++・Javaバインディング向けの安定C ABI層 | MIT OR Apache-2.0 |
| `rustorb-codegen` | IDLコード生成器（idl_parserのASTを入力）。`python/` に `rustorb-idl` コマンド（Rust・Python・C++11・Java のバックエンド、[docs/python-codegen.md](docs/python-codegen.md)、[docs/rust-mapping.md](docs/rust-mapping.md)）と、`build.rs` から `-brust` を呼ぶ `rustorb_codegen::Config` | GPL-3.0-or-later |

Python パッケージ `rustorb`（`python/rustorb`）は、Python から使う rustorb です。IDL から `rustorb-idl -bpython` で生成したコードと、`from rustorb import CORBA, PortableServer` で使います。`pip install ./python` で maturin がネイティブモジュール（`crates/rustorb-python`）をビルドしてインストールします。

ビルドとテスト：

```sh
cargo build --workspace
cargo test --workspace
```

Python のテスト（ORB と生成器のテストには [idl_parser](https://github.com/sugarsweetrobotics/idl_parser) が必要。ORB のテストはネイティブモジュールをソースツリーに置いてから）：

```sh
python/build-native.sh
PYTHONPATH=path/to/idl_parser python3 -m unittest discover -s python/tests
PYTHONPATH=path/to/idl_parser python3 -m unittest discover -s crates/rustorb-codegen/python/tests
```

チェックインしている Rust の生成コード（`rustorb-naming`、`interop/impls/rustorb`、生成器のテスト）は、IDL か生成器を変えたら `IDL_PARSER_DIR=path/to/idl_parser crates/rustorb-codegen/generate-rust.sh` で作り直します（CI が差分を確かめます）。

C++ と Java のテスト（CMake、Maven。どちらも cargo でネイティブライブラリをビルドし、rustorb-idl でテスト用の IDL からコードを生成する）：

```sh
cmake -S cpp -B build/cpp -DIDL_PARSER_DIR=path/to/idl_parser && cmake --build build/cpp && ctest --test-dir build/cpp
mvn -f java/pom.xml -Didl.parser.dir=path/to/idl_parser verify
```

ファジング（nightlyと [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) が必要）：

```sh
cargo +nightly fuzz list
cargo +nightly fuzz run giop_message
```

## ライセンス

rustorb は、以下のいずれかを利用者が選べるデュアルライセンスで提供します。

- Apache License, Version 2.0（[LICENSE-APACHE](LICENSE-APACHE)）
- MIT License（[LICENSE-MIT](LICENSE-MIT)）

### IDLコード生成器の例外

IDLコード生成器は GPL-3.0-or-later の [idl_parser](https://github.com/sugarsweetrobotics/idl_parser) を利用するため、生成器だけは GPL-3.0-or-later で提供します。生成器が出力したコードは生成器のライセンスの対象外で、入力したIDLの権利者がライセンスを自由に決められます。出力に含まれる rustorb 由来の定型部分は MIT OR Apache-2.0 で提供します。

### コントリビューション

明示的な断りがない限り、rustorb に提出されたコントリビューションは、Apache-2.0 の定義に従い、追加の条件なしで上記のデュアルライセンス（生成器への提出は GPL-3.0-or-later）で提供されるものとします。
