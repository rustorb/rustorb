# rustorb
Rust製のCORBA実装

## ドキュメント

- [開発計画](docs/development-plan.md)

## クレート構成

| クレート | 役割 | ライセンス |
| --- | --- | --- |
| `rustorb-cdr` | CDRのエンコード・デコード | MIT OR Apache-2.0 |
| `rustorb-giop` | GIOP/IIOPメッセージ型、IOR・corbaloc | MIT OR Apache-2.0 |
| `rustorb-core` | ORB本体（接続管理、リクエスト配送、POA）。内部でtokioを使い、公開APIは同期 | MIT OR Apache-2.0 |
| `rustorb-naming` | CosNaming（Naming Service） | MIT OR Apache-2.0 |
| `rustorb-ffi` | C++・Javaバインディング向けの安定C ABI層 | MIT OR Apache-2.0 |
| `rustorb-codegen` | IDLコード生成器（idl_parserのASTを入力） | GPL-3.0-or-later |

ビルドとテスト：

```sh
cargo build --workspace
cargo test --workspace
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
