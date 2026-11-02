# 未対応の CORBA 機能

Nov 1, 2026

0.1.0 の後（PR #98 の時点）に、rustorb がまだ対応していない CORBA の機能を洗い出し、優先度を付けた。CORBA 3.x と関連仕様（Messaging、Portable Interceptors、CSIv2、COS サービスなど）の機能を、[development-plan.md](development-plan.md) と各設計書の「残り」「対象外」、生成器（`rustorb_idl` の各 backend）がエラーにする構文、コア（`rustorb-core`・`rustorb-giop`）のコードと突き合わせた。他の ORB のソースは読んでいない。

優先度は「omniORB / omniORBpy の置き換え」と OpenRTM での利用を基準にした目安。issue の欄が空のものは起票していない（需要が出たら起票する）。

## P1：置き換えの障害になりやすいもの

| 機能 | 現状 | 影響 | issue |
| --- | --- | --- | --- |
| Portable Interceptors（`ORBInitializer`、Client / ServerRequestInterceptor、IORInterceptor、`PICurrent`） | コア（Rust）は実装済み。Python・C ABI / C++・Java は未実装 | 認証トークンやトレース ID をサービスコンテキストで運ぶ用途、フレームワークのフックが使えない。CSIv2・FT CORBA などの土台にもなる | [#100](https://github.com/rustorb/rustorb/issues/100)、設計：[portable-interceptors.md](portable-interceptors.md) |
| 接続の管理（使っていない接続を閉じる、接続数の上限） | アップコールスレッドの idle 終了はあるが、接続を閉じる仕組みはない | 相手の多い長寿命のサーバーで接続が溜まる | [#101](https://github.com/rustorb/rustorb/issues/101) |
| GIOP メッセージの最大長の設定と、送信側の Fragment | 受信の上限は `OrbOptions::max_message_size`（既定 64 MiB）だが ORB 引数がない。送るメッセージは分割しない | 大きな値を送ると相手の上限に当たって失敗する。JacORB は GIOP 1.1 の Fragment を受けない（`known-failures.txt`） | [#102](https://github.com/rustorb/rustorb/issues/102) |

## P2：需要が見込めるもの

| 機能 | 現状 | メモ | issue |
| --- | --- | --- | --- |
| 双方向 GIOP（BiDir、`BiDirPolicy`） | [poa-design.md](poa-design.md) で MVP の対象外 | NAT・ファイアウォール越しのコールバックに要る | [#103](https://github.com/rustorb/rustorb/issues/103) |
| `-brust` の abstract interface・local interface・抽象 valuetype・`context` 句 | 生成器がエラーにする（[rust-mapping.md](rust-mapping.md)）。コアと他の言語は abstract interface に対応済み | Rust の利用者がローカルオブジェクトを IDL で書けない | [#104](https://github.com/rustorb/rustorb/issues/104) |
| ログ・トレースの出力（`-ORBtraceLevel` 相当） | コアにログの仕組みがない | 相互接続や設定の問題を利用者が調べにくい | [#105](https://github.com/rustorb/rustorb/issues/105) |
| Messaging AMI（IDL の `sendc_` と `ReplyHandler`、ポーリング） | Rust は async API（[async-api.md](async-api.md)）と DII の deferred で代替。標準の AMI の生成コードはない | C++・Java・Python に非同期呼び出しの標準 API がない | |
| Messaging のその他のポリシー（SyncScope、Rebind、RequestPriority など） | タイムアウト（RelativeRoundtripTimeout）だけ | oneway の `SYNC_WITH_*` は内部で持っているので、ポリシーとして公開するのは小さい | |
| C++・Java の DII / DSI | コアと Python だけ（[dii-dsi.md](dii-dsi.md)、需要待ちと決定済み） | ブリッジ・ゲートウェイ・汎用ツール向け | |
| `POAManagerFactory`（CORBA 3） | なし（POA ごとに暗黙の POAManager） | 複数の POA で POAManager を作成・共有する API | |

## P3：仕様にはあるが需要が薄いもの

| 機能 | 現状 | issue |
| --- | --- | --- |
| Interface Repository、`_interface` / `get_interface`、`create_operation_list` | 対象外。`_interface` は `INTF_REPOS` | |
| `CORBA::Context` と IDL の `context` 句 | DII / DSI でも `NO_IMPLEMENT`、生成器も扱わない | |
| Implementation Repository（PERSISTENT な POA の起動と転送） | なし。PERSISTENT の POA は固定ポートで動かす前提 | |
| CSIv2（認証・権限の伝搬）、セキュリティサービスの API | 対象外（SSLIOP のみ）。SSLIOP も SNI・暗号化した鍵・CRL・OCSP、`corbaloc:ssliop:` は扱わない（[ssliop.md](ssliop.md)） | |
| DynAny の抽象インターフェース（`insert_abstract` / `get_abstract`） | `NO_IMPLEMENT`（[dynany.md](dynany.md)） | |
| ネイティブのコードセットの設定（char / wchar） | ネゴシエーションはあるが、ネイティブのコードセットは固定 | |
| 他のトランスポート：DIOP（UDP）、MIOP（マルチキャスト）、ZIOP（圧縮）、Unix ドメインソケット | TCP と TLS だけ。Unix ソケットは omniORB 独自（`giop:unix`） | |
| omniORBpy 互換の追加：`omniORB.URI`、`setClientThreadCallDeadline`、`installTimeoutExceptionHandler`、`sslTP` | 使うアプリケーションが出たら（[omniorb-compat.md](omniorb-compat.md)、[timeouts.md](timeouts.md)） | |

## 対象外のまま（計画で決定済み）

- CORBA Component Model（CCM、Lightweight CCM）
- COS サービス：Event / Notification、Trading、Property、Life Cycle、Time、Transaction（OTS）、Concurrency など。Naming 以外は実装しない
- RT-CORBA、Fault Tolerant CORBA（サービスコンテキスト ID の定数だけある）
- IDL to C++98 マッピング
- IDL4 の `map` と `@optional`（GIOP に形がない）
- Naming の KVS 実装、omniNames のログの移行
