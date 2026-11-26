<p align="center">
  <a href="https://rustorb.github.io/">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="docs/images/rustorb-wordmark-dark.svg">
      <img alt="rustorb" src="docs/images/rustorb-wordmark-light.svg" width="420">
    </picture>
  </a>
</p>

# rustorb
Rust製のCORBA実装（[English](README.md)）

**公式ページ: <https://rustorb.github.io/ja/>**（クイックスタート、インストール、各言語のチュートリアル）

rustorb は IIOP、SSLIOP、omniORB の Unix ドメインソケットの上で GIOP 1.0〜1.2 を話し、CI で omniORB、omniORBpy、JacORB とクライアント・サーバー・ネーミングサービスのすべての組み合わせで試しています。既存のシステムの omniORB / omniORBpy を置き換えることを目指しており、Python バインディングは omniidl -bpython と同じ API を生成し、互換パッケージで OpenRTM-aist-Python などの omniORBpy のアプリケーションがそのまま動きます。

## 機能

- **プロトコル**：CDR、フラグメントを含む GIOP 1.0〜1.2、IOR、`corbaloc:` と `corbaname:`、コードセットの交渉、双方向 GIOP、SSLIOP（rustls の TLS）、omniORB の `giop:unix` エンドポイント（Unix ドメインソケット。Linux と macOS）
- **サーバー側**：標準のポリシーを持つ POA、サーバントマネージャー、デフォルトサーバント、アダプターアクティベーター、3つのスレッドモデル、`POAManagerFactory`
- **型**：Any、TypeCode、DynAny、`fixed`、`long double`、valuetype（custom、truncatable、boxed、`supports`）、abstract interface
- **ネーミングサービス**：CosNaming のクライアントとサーバー、`rustorb-names` デーモン（omniNames の代わり。永続化つき）
- **非同期呼び出し**：Python、C++、Java の callback AMI（`rustorb-idl -Wbami` の `sendc_` と応答ハンドラ）、Rust の非同期 API（任意）
- **その他**：DII / DSI（Rust、Python、C++、Java）、Portable Interceptors、ポリシー、呼び出しのタイムアウトと oneway の `SyncScope`、omniORBpy の `omniORB.URI`、トレース

## ドキュメント

- [公式ページ](https://rustorb.github.io/ja/)（クイックスタート、インストール、各言語のチュートリアル）
- [入門ガイド（英語）](docs/getting-started.md)と[例](examples/)
- [開発計画](docs/development-plan.md)
- [CORBA 仕様への対応状況](docs/corba-compliance.md)
- [omniORB との比較（機能と速度）](docs/omniorb-comparison.md)
- [未対応の CORBA 機能と対応する issue](docs/unsupported-features.md)
- [Python生成器（rustorb-idl -bpython）](https://github.com/rustorb/rustorb-idl/blob/main/docs/python-codegen.md)
- [IDL to Rust マッピング（rustorb-idl -brust）](docs/rust-mapping.md)
- [Python バインディング（PyO3）](docs/python-binding.md)
- [C++ バインディング（C ABI + IDL to C++11）](docs/cpp-binding.md)
- [Java バインディング（FFM + IDL to Java）](docs/java-binding.md)
- [omniORBpy 互換](docs/omniorb-compat.md)
- [POA](docs/poa-design.md)、[Any と TypeCode](docs/any-typecode.md)、[DynAny](docs/dynany.md)、[valuetype](docs/valuetype.md)、[DII / DSI](docs/dii-dsi.md)、[callback AMI](docs/ami.md)、[Portable Interceptors](docs/portable-interceptors.md)
- [SSLIOP](docs/ssliop.md)、[Unix ドメインソケット](docs/unix-transport.md)、[双方向 GIOP](docs/bidir-giop.md)、[接続](docs/connections.md)、[タイムアウト](docs/timeouts.md)、[SyncScope](docs/sync-scope.md)、[ORB の操作](docs/orb-operations.md)、[トレース](docs/tracing.md)、[非同期 API](docs/async-api.md)、[負荷試験](docs/load-testing.md)
- [相互接続テスト](interop/README.md)
- [リリースの手順](docs/release.md)、[変更履歴](CHANGELOG.md)

## クレート構成

| クレート | 役割 | ライセンス |
| --- | --- | --- |
| `rustorb-cdr` | CDRのエンコード・デコード | MIT OR Apache-2.0 |
| `rustorb-typesupport` | DDS の型サポート（XCDR1/XCDR2、キーハッシュ）。`rustorb-idl -brust -Wbdds` の生成コードが使う（[docs/dds-type-support.md](docs/dds-type-support.md)） | MIT OR Apache-2.0 |
| `rustorb-giop` | GIOP/IIOPメッセージ型、IOR・corbaloc | MIT OR Apache-2.0 |
| `rustorb-core` | ORB本体（接続管理、リクエスト配送、POA）。内部でtokioを使い、公開APIは同期 | MIT OR Apache-2.0 |
| `rustorb-naming` | CosNaming（Naming Service） | MIT OR Apache-2.0 |
| `rustorb-python` | Python パッケージ `rustorb` のネイティブモジュール `rustorb._native`（PyO3、[docs/python-binding.md](docs/python-binding.md)） | MIT OR Apache-2.0 |
| `rustorb-ffi` | C++・Javaバインディング向けの安定C ABI層 | MIT OR Apache-2.0 |

IDL コード生成器（`rustorb-idl` コマンドと、`build.rs` から使う `rustorb-codegen` クレート。GPL-3.0-or-later）は別のリポジトリ [rustorb/rustorb-idl](https://github.com/rustorb/rustorb-idl) にあり、別の版でリリースします。このリポジトリのテストと例は `tools/rustorb-idl.txt` で固定した版を使います（`pip install -r tools/rustorb-idl.txt`。手元のチェックアウトを試すときは `RUSTORB_IDL_DIR` に指定）。

Python パッケージ `rustorb`（`python/rustorb`）は、Python から使う rustorb です。IDL から `rustorb-idl -bpython` で生成したコードと、`from rustorb import CORBA, PortableServer` で使います。`pip install ./python` で maturin がネイティブモジュール（`crates/rustorb-python`）をビルドしてインストールします。

ビルドとテスト：

```sh
cargo build --workspace
cargo test --workspace
```

Python のテスト（固定した版の rustorb-idl が必要。ORB のテストはネイティブモジュールをソースツリーに置いてから）：

```sh
pip install -r tools/rustorb-idl.txt
python/build-native.sh
python3 -m unittest discover -s python/tests
```

チェックインしている Rust の生成コード（`rustorb-naming`、`interop/impls/rustorb`、`tests/mapping` のマッピングのテスト）は、IDL か固定した生成器の版を変えたら `tools/generate-rust.sh` で作り直します（CI が差分を確かめます）。

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
