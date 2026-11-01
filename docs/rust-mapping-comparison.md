# OMG IDL → Rust マッピングの比較：rustorb と他の実装・提案

Nov 1, 2026 · @ysuga

rustorb の IDL to Rust マッピング（[rust-mapping.md](rust-mapping.md)、`rustorb-idl -brust`）を、他の実装や標準化の提案と比べる。比較には公開されたドキュメント、README、docs.rs の API ページだけを使い、他の ORB の実装ソースは読んでいない。

## 概要

OMG IDL から Rust への採択済みの標準マッピングはまだなく、2026年6月に OMG が RFP を出した段階にある。公開されている Rust の IDL 生成器の大半は DDS のデータ型だけを対象にしている。インターフェース・例外・any・valuetype まで生成するものは、rustorb の `-brust` のほかに zerodds-corba-rust がある程度。

- **データ型の基本部分（数値、string → `String`、sequence → `Vec<T>`、配列 → `[T; N]`、struct → `struct`、union → `enum`）は、どの実装もほぼ同じ**。rustorb もこの点で独自の選択はしていない
- **違いが出るのは**、名前の変換（rustorb は Rust 流に変換、他の多くは IDL のまま）、上限付きの型と `wstring` の表し方（rustorb と omg-idl-gen は上限のない型と同じ、ROS 2 の rclrs は専用の型）、union の判別子の保持、そしてインターフェースと例外
- **rustorb の特徴**は、操作ごとのエラー `enum` と `?`、`out` をタプルで返す形、valuetype を `Arc` のハンドルで共有・循環まで表す形。他の公開実装に、同等に文書化されたマッピングは見当たらなかった
- OMG の標準化が進んだら、名前の変換や上限付きの型、`char` / `wchar` の扱いが標準とずれる可能性がある。RFP の本文（mars/2026-3-6）は今回取得できなかったので、要件との突き合わせは未確認

## 比較対象

公開情報で確認できたものを、CORBA 寄りのものから並べる。

| 対象 | 種類 | IDL の扱い | インターフェース | 状態 |
| --- | --- | --- | --- | --- |
| rustorb `rustorb-idl -brust` | CORBA ORB | IDL コンパイラ（Python、idl\_parser）、`build.rs` 補助あり | スタブ・サーバント trait・POA 包み | 0.1.0（2026-11-01 公開） |
| [IDL4 to Rust RFP](https://www.omg.org/gettingstarted/rfp-IDL4-RLP.htm)（OMG） | 標準化の提案募集 | IDL4 の全構文が対象 | 対象（全構文） | 2026-06 発行、初回提出 2026-08-17 締切、採択はまだ |
| [zerodds-corba-rust](https://docs.rs/zerodds-corba-rust)（ZeroDDS） | CORBA 向け生成器＋ランタイム型 | ビルド時ツール | `pub trait I` + `IStub` + dispatch、AMI | 1.0.0-rc |
| [omg-idl-gen](https://docs.rs/omg-idl-gen) / [omg-idl-code-gen](https://docs.rs/crate/omg-idl-code-gen/0.2.3)（[rtps-gen](https://lib.rs/crates/rtps-gen) の後継） | データ型の生成器 | CLI / ライブラリ、pest の IDL4 文法 | 対象外（NA） | 0.2.3（2026-05） |
| [dust\_dds](https://docs.rs/dust_dds) / [dust\_dds\_gen](https://docs.rs/dust_dds_gen) | DDS | derive マクロ `DdsType` が主、IDL は `build.rs` から | なし | 0.16 |
| [RustDDS](https://docs.rs/rustdds)（Atostek） | DDS | IDL 生成器なし、serde の型を使う | なし | 0.15 |
| [cyclonedds](https://docs.rs/crate/cyclonedds/2.0.0)（Rust）/ [cargo-cyclonedds](https://docs.rs/crate/cargo-cyclonedds/1.3.0) | DDS | `cyclonedds-idlc` と derive（`DdsType`、`DdsEnum`、`DdsUnion`、`DdsBitmask`） | なし | 3.0 |
| [zerodds-idl-rust](https://docs.rs/zerodds-idl-rust)（→ `zerodds-idlc --lang rust`） | DDS | IDL コンパイラ | なし | rc.6 で取り下げ、後継に移行 |
| ROS 2 [rclrs](https://docs.rs/rosidl_runtime_rs)（rosidl\_generator\_rs） | ROS 2 | rosidl の msg/IDL から生成 | Service / Action の trait（型の組） | rosidl\_runtime\_rs 0.7 |
| ROS 2 [r2r](https://docs.rs/r2r) | ROS 2 | C の introspection から生成 | Client / Server の API | 0.9 |
| [RTI Connector for Rust](https://www.rti.com/developers/rti-labs/connector-rust) | DDS（RTI Labs） | IDL 生成なし、XML と DynamicData、serde | なし | 実験的 |

対象にしなかったもの：eProsima Fast DDS-Gen と OpenDDS に Rust 出力があるかは確認できなかった。rustorb と zerodds-corba-rust 以外の Rust 製 CORBA 実装も、今回の検索では見つかっていない。

## 型の対応

数値・文字列・シーケンス・配列・struct はどの実装も同じで、差は上限付きの型、`wstring`、union、名前に出る。「—」は公開ドキュメントに記載がないもの。

| IDL | rustorb | omg-idl-gen | dust\_dds | ROS 2 rclrs | zerodds-idl-rust（rc.6） |
| --- | --- | --- | --- | --- | --- |
| 整数・`float`・`double`・`boolean`・`octet` | `i16`…`u64`、`f32`/`f64`、`bool`、`u8` | 同じ | 同じ（例で `u8`、`i32`、`f64`） | Rust の対応する基本型 | — |
| `long double` | `LongDouble`（binary128 のビット） | — | — | — | — |
| `char`・`wchar` | どちらも `char` | どちらも `char` | — | — | — |
| `string` | `String`（`in` は `&str`） | `String` | `String` | idiomatic は `String`、RMW は `rosidl_runtime_rs::String` | — |
| `wstring` | `String` | `String` | — | `WString`（UTF-16） | — |
| `string<N>` | `String`（送信時 `BAD_PARAM`、受信時 `MARSHAL`） | — | — | `BoundedString<N>`（作成時に検査） | — |
| `sequence<T>` | `Vec<T>`（`in` は `&[T]`） | `Vec<T>` | — | idiomatic は `Vec<T>`、RMW は `Sequence<T>` | — |
| `sequence<T, N>` | `Vec<T>`（送受信時に検査） | — | — | `BoundedSequence<T, N>` | — |
| 配列 `T x[N][M]` | `[[T; M]; N]` | `[[T; ...]; ...]`（例では次元の順が逆に記載） | — | Rust の配列 | — |
| struct | すべて `pub` の `struct`、derive はメンバー次第 | `pub struct` | `#[derive(DdsType)] struct` | 生成した struct（`Clone + Debug + Default`） | `pub struct` + `impl DdsType` |
| enum | `#[repr(u32)] enum`、範囲外は `MARSHAL` | `enum`（名前は IDL のまま） | `enum`、`bit_bound` と値の指定 | — | `#[repr(i32)] enum` + `from_wire` |
| union | `enum`、複数ラベルと `default` は `Member(D, T)` で判別子を保つ、覆いきらないとき `Default(D)` | `enum`、ラベルごとにバリアント、`switch (long)` のみ | `#[dust_dds(switch(T))] enum`、`case`/`default` | — | `pub enum`、case ごとにバリアント |
| typedef | `pub type` ＋ `alias::X`（`tk_alias` の記述子） | `pub type` | — | — | `pub type` |
| module | snake\_case の `pub mod` | `mod`（名前はそのまま） | — | `<パッケージ>::msg` | `pub mod` |
| const | `pub const`、`fixed` は `pub fn` | `const`（interface 内は NA） | — | — | — |
| `@key`・`@optional`・`@id` など | 対象外（IDL4 の新構文は [#46](https://github.com/rustorb/rustorb/issues/46) で別扱い） | — | `key`、`optional`（`Option<T>`）、`id`、extensibility など | — | `@key`、`@id` |
| `any`・`TypeCode` | `Any`、`TypeCode`、`Any::insert::<T>` / `extract::<T>` | NA | — | — | — |
| `fixed<D,S>` | `Fixed` | NA | — | — | — |
| valuetype | `Arc` のハンドル `V` ＋ 状態 `VState` | 表になし | — | — | — |

名前：rustorb は型を UpperCamelCase、メンバー・操作を snake\_case、定数を SCREAMING\_SNAKE\_CASE に変え、変換後に衝突したら生成器のエラーにする。omg-idl-gen の例では IDL の名前（`VARIANT0`、`LABEL0`）がそのまま。dust\_dds は Rust の型が先にあり、`name = "..."` で IDL 側の名前を付ける。

## インターフェースと操作

IDL の interface を Rust にする公開実装は rustorb と zerodds-corba-rust だけで、他は対象外。zerodds-corba-rust はクレートの説明文にある範囲だけを書いている。

| 観点 | rustorb | zerodds-corba-rust | ROS 2 rclrs（参考） |
| --- | --- | --- | --- |
| クライアント | `ObjectRef` の新しい型 `I`、`narrow` / `unchecked_narrow`、継承は全操作を展開＋`From<Derived> for Base` | `IStub` | `Client` に `Service` の型を渡す |
| サーバント | `trait IServant: Send + Sync`（`&self`）＋`IPoa<T>` が dispatch、継承は親トレイト | `pub trait I` ＋ `dispatch_i` | `Service` は要求と応答の型の組だけ |
| `in` | `Copy` なら値、他は借用（`&str`、`&[T]`、`&T`、`Option<&I>`） | — | — |
| `out` / `inout` | `out` は戻り値のタプル（戻り値が先頭）、`inout` は `&mut T` | — | — |
| 属性 | `x()` / `set_x(v)` | getter `x(&self)`、書き込めれば setter | — |
| ユーザー例外 | 操作ごとの `enum XxxError { Ex(..), System(SystemException) }`、`From` で `?` が使える | ランタイムの `CorbaException`（system / user のバリアント）。操作ごとの型かは記載なし | — |
| `raises` なし | `Result<T, SystemException>` | — | — |
| oneway | `Result<(), SystemException>`、返事を待たない | 返事なしの trait メソッド | — |
| 非同期 | 同期が基本、操作ごとに `async fn xxx_async`（サーバントは同期） | 同期のスタブ＋`@ami` の AMI（`AsyncReply`、コールバック） | — |
| valuetype | ハンドルに操作、`VOperations` を状態に実装、`supports I` ならサーバントになれる | `pub trait V: ValueBase` | — |
| 対象外 | abstract / local interface、`context`、抽象 valuetype | component / home は「phase 2」 | — |
| `unsafe` | 生成コードになし | `forbid(unsafe_code)` | RMW 側は `#[repr(C)]` で C と共有 |

設計の参考：CORBA 以外の Rust の RPC 生成器では、[tonic](https://docs.rs/tonic) がサービスを async のトレイト（戻り値 `Result<Response<T>, Status>`）にしている。例外を操作ごとの型に分ける rustorb の形は、これより型が細かい方向（tonic の記述は公開ページの要約で、詳細は確かめていない）。

## rustorb の位置づけと検討できる点

rustorb のマッピングは、公開されている中では IDL の範囲が最も広く、文書化も最も細かい部類。その分、標準とずれたときの影響も大きいので、次の点は RFP の提出内容が見えてきたら見直す候補。

**他と同じで、変える理由がないもの**

- 基本型、`String`、`Vec<T>`、`[T; N]`、struct → `struct`、union → `enum`、typedef → `pub type`、module → `pub mod`
- `unsafe` なしの生成コード（zerodds-corba-rust も `forbid(unsafe_code)`）
- `build.rs` から生成して `include!` する使い方（dust\_dds\_gen、cyclonedds-build も同じ）

**rustorb 独自の選択（標準とずれるおそれがあるもの）**

| 点 | rustorb | 他の実装 | 見直すときの選択肢 |
| --- | --- | --- | --- |
| 名前の変換 | Rust の命名規約に変換 | omg-idl-gen は変換しない | 標準が IDL の名前を保つ場合に備え、変換しないオプションを足す |
| 上限付きの型 | 上限のない型と同じ Rust の型、送受信で検査 | rclrs は `BoundedString<N>` / `BoundedSequence<T, N>` で作るときに検査 | 今の形を保つ。標準が専用の型を求めたら、記述子の側で吸収できる |
| `wstring` | `String` | rclrs は `WString`（UTF-16） | 同上 |
| `char` / `wchar` | どちらも Rust の `char` | omg-idl-gen も `char` | 標準が別の型（`u8` / `u16` など）を選ぶ可能性を見ておく |
| union | `Member(D, T)` で判別子を保つ、`Default(D)` | omg-idl-gen はラベルごとにバリアントを複製、dust\_dds は `case` 属性 | 判別子を失わない点で rustorb の形が強い |
| 例外 | 操作ごとのエラー `enum` | zerodds-corba-rust は共通の `CorbaException` | 今の形を保つ |
| `out` | 戻り値のタプル | C++ や Java の標準マッピングは参照や Holder | 今の形を保つ |
| 非同期 | 同期＋`_async` 版 | zerodds-corba-rust は AMI | AMI（`sendc_`）との対応を文書に書く |

**取り入れる価値がありそうなもの**

- **DDS の注釈（`@key`、`@optional`、`@id`、extensibility）**：dust\_dds と zerodds は対応済み。`@optional` を `Option<T>` にする形は各実装で一致しているので、#46（IDL4 の新構文）で採るならこの形が自然
- **serde の derive**：RustDDS と RTI Connector は serde の型をそのまま使う。生成する struct に `serde` の derive を付けるオプションがあれば、DDS 系や JSON と型を共有しやすくなる
- **RFP への参加または意見**：初回提出は OMG の Domain / Contributing 会員が対象。rustorb の文書と相互接続の実績は、提出者への意見として使える

未確認の点：RFP 本文の必須要件と、zerodds-corba-rust の型の対応の詳細（仕様書は公開されているが未読）。

## 出典

すべて 2026-11-01 に参照。

- rustorb：[rust-mapping.md](rust-mapping.md)、[async-api.md](async-api.md)
- OMG：[IDL4 to Rust Language Mapping RFP（mars/2026-3-6）](https://www.omg.org/gettingstarted/rfp-IDL4-RLP.htm)、[発表（2026-06-15）](https://edmcouncil.org/announcement/omg-issues-rfp-for-idl4-to-rust-language-mapping/)、[IDL4-CPP 1.0](https://www.omg.org/spec/IDL4-CPP/1.0)、[IDL4-Java 1.0](https://www.omg.org/spec/IDL4-Java/1.0)、[IDL 4.2](https://www.omg.org/spec/IDL/4.2)
- [zerodds-corba-rust](https://docs.rs/zerodds-corba-rust)、[zerodds-idl-rust](https://docs.rs/zerodds-idl-rust)、[zerodds-idl-rust 1.0.0-rc.6](https://docs.rs/zerodds-idl-rust/1.0.0-rc.6)
- [omg-idl-gen](https://docs.rs/omg-idl-gen)、[omg-idl-code-gen 0.2.3](https://docs.rs/crate/omg-idl-code-gen/0.2.3)、[rtps-gen](https://lib.rs/crates/rtps-gen)、[rtps-idl README](https://docs.rs/crate/rtps-idl/latest/source/README.md)
- [dust\_dds](https://docs.rs/dust_dds)、[dust\_dds\_gen](https://docs.rs/dust_dds_gen)
- [RustDDS](https://docs.rs/rustdds)
- [cyclonedds 2.0.0](https://docs.rs/crate/cyclonedds/2.0.0)、[cyclonedds-rs 0.1.1](https://docs.rs/crate/cyclonedds-rs/0.1.1)、[cargo-cyclonedds 1.3.0](https://docs.rs/crate/cargo-cyclonedds/1.3.0)
- ROS 2：[rosidl\_runtime\_rs `Message`](https://docs.rs/rosidl_runtime_rs/latest/rosidl_runtime_rs/trait.Message.html)、[`Sequence`](https://docs.rs/rosidl_runtime_rs/latest/rosidl_runtime_rs/struct.Sequence.html)、[`BoundedString`](https://docs.rs/rosidl_runtime_rs/latest/rosidl_runtime_rs/struct.BoundedString.html)、[`Service`](https://docs.rs/rosidl_runtime_rs/latest/rosidl_runtime_rs/trait.Service.html)、[r2r](https://docs.rs/r2r/latest/r2r/)、[r2r README](https://github.com/sequenceplanner/r2r)
- RTI：[Connector for Rust](https://www.rti.com/developers/rti-labs/connector-rust)、[rticonnextdds-connector-rust](https://github.com/rticommunity/rticonnextdds-connector-rust)
- [tonic](https://docs.rs/tonic)

取得できなかったもの：RFP 本文（omg.org の文書サーバが自動取得を拒否）、ros2\_rust の docs/、dust\_dds の GitHub README。
