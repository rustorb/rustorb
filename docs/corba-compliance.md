# CORBA 仕様への対応状況

Nov 21, 2026 · @ysuga · 0.6.0 の後（PR #174 の時点）

rustorb が CORBA 3.x と関連仕様のどこまでに対応しているかを、仕様の章ごとにまとめる。各機能の詳しい挙動は、表の「文書」の欄の設計書を見る。未対応のものの優先度と経緯は [unsupported-features.md](unsupported-features.md) にある。他の ORB のソースは読んでいない。既存の ORB との違いは、仕様と、外から動かして測った結果だけを根拠にしている。

記号：✅ 対応済み、🟡 一部対応、❌ 未対応（対象外と決めたものを含む）

## Core と GIOP / IIOP

| 機能 | 状況 | メモ | 文書 |
| --- | --- | --- | --- |
| CDR（基本型、string / wstring、struct、union、enum、sequence、array） | ✅ | | [development-plan.md](development-plan.md) |
| `fixed` | ✅ | | [any-typecode.md](any-typecode.md) |
| `long double` | ✅ | 仕様どおり IEEE binary128 で送る（omniORB C++ は x87 の形式で送るので値が合わない） | [long-double.md](long-double.md) |
| GIOP 1.0〜1.2 | ✅ | 受信は 1.0〜1.2。送信の上限は `-ORBmaxGIOPVersion` | |
| Fragment | ✅ | 受信は 1.1・1.2。送信は GIOP 1.2 だけ（`-ORBgiopFragmentSize`） | [message-size.md](message-size.md) |
| メッセージの最大長 | ✅ | `-ORBgiopMaxMsgSize`（既定 64 MiB） | [message-size.md](message-size.md) |
| `LocateRequest`、`LOCATION_FORWARD`、`CloseConnection`、`MessageError`、`CancelRequest` | ✅ | | |
| IOR、`corbaloc:`、`corbaname:` | ✅ | | |
| コードセットのネゴシエーション | 🟡 | ネゴシエーションはするが、ネイティブのコードセット（char / wchar）は固定 | |
| `CORBA::ORB` / `CORBA::Object` の操作 | ✅ | `register_initial_reference`、`create_policy`、PolicyManager / PolicyCurrent、`validate_connection` など | [orb-operations.md](orb-operations.md) |
| Interface Repository、`_interface` / `get_interface`、`create_operation_list` | ❌ | `_interface` は `INTF_REPOS` | |
| `CORBA::Context`、IDL の `context` 句 | ❌ | DII / DSI でも `NO_IMPLEMENT`。生成器はすべての backend でエラーにする | [rust-mapping.md](rust-mapping.md) |
| `Object::get_domain_managers` | ❌ | ドメインの仕組みがない | [orb-operations.md](orb-operations.md) |

## トランスポートと接続

| 機能 | 状況 | メモ | 文書 |
| --- | --- | --- | --- |
| IIOP（TCP） | ✅ | | |
| SSLIOP（TLS、rustls） | 🟡 | SNI、暗号化した鍵、CRL / OCSP、`corbaloc:ssliop:` は扱わない | [ssliop.md](ssliop.md) |
| omniORB の `giop:unix`（Unix ドメインソケット） | ✅ | Linux と macOS。Windows は `BAD_PARAM` | [unix-transport.md](unix-transport.md) |
| 双方向 GIOP（`BiDirPolicy`） | ✅ | | [bidir-giop.md](bidir-giop.md) |
| 使っていない接続を閉じる、接続数の上限 | ✅ | `-ORBoutConScanPeriod`、`-ORBinConScanPeriod`、`-ORBmaxServerConnections`、`-ORBmaxConnectionsPerServer` | [connections.md](connections.md) |
| ログ・トレース | ✅ | `-ORBtraceLevel`（omniORB に合わせたレベル） | [tracing.md](tracing.md) |
| CSIv2、Security Service の API | ❌ | 対象外 | |
| DIOP、MIOP、ZIOP | ❌ | | |

## POA

| 機能 | 状況 | メモ | 文書 |
| --- | --- | --- | --- |
| 7 種のポリシー、`create_POA` / `find_POA` / `destroy` | ✅ | | [poa-design.md](poa-design.md) |
| 活性化と id・参照・サーバントの変換 | ✅ | `activate_object(_with_id)`、`servant_to_id` など | |
| servant manager（`ServantActivator`・`ServantLocator`）、デフォルトサーバント、`AdapterActivator` | ✅ | | |
| `POAManager` の 4 状態、`POAManagerFactory`（CORBA 3） | ✅ | | |
| `PortableServer::Current` | ✅ | | |
| スレッドモデル（ORB_CTRL、SINGLE_THREAD、MAIN_THREAD） | ✅ | | |
| Implementation Repository | ❌ | PERSISTENT の POA は固定ポートで動かす前提 | |

## 型：Any、TypeCode、DynAny、valuetype

| 機能 | 状況 | メモ | 文書 |
| --- | --- | --- | --- |
| Any、TypeCode（`equal` / `equivalent` / `get_compact_typecode`、再帰型） | ✅ | | [any-typecode.md](any-typecode.md) |
| DynAny | 🟡 | `insert_abstract` / `get_abstract` だけ `NO_IMPLEMENT` | [dynany.md](dynany.md) |
| valuetype（状態メンバー、継承、truncatable、null、共有と循環、value box） | ✅ | | [valuetype.md](valuetype.md) |
| custom valuetype、`supports`、操作と初期化子 | ✅ | コードベース URL は読み飛ばすだけ | [valuetype.md](valuetype.md) |
| 抽象インターフェース | ✅ | | |

## DII / DSI

| 機能 | 状況 | メモ | 文書 |
| --- | --- | --- | --- |
| `Request`、`NVList`、deferred、`send_multiple_requests_*` | ✅ | Rust、Python、C++、Java | [dii-dsi.md](dii-dsi.md) |
| `ServerRequest`（DSI） | ✅ | 同上 | |
| コンテキスト（`ctx`、`ctxlist`）、`get_default_context` | ❌ | `NO_IMPLEMENT` | |

## Portable Interceptors

| 機能 | 状況 | メモ | 文書 |
| --- | --- | --- | --- |
| `ORBInitializer`、`ORBInitInfo` | ✅ | | [portable-interceptors.md](portable-interceptors.md) |
| `ClientRequestInterceptor`、`ServerRequestInterceptor`、`ForwardRequest` | ✅ | 同一 ORB 内の呼び出しでも両側を呼ぶ | |
| `IORInterceptor`、`PICurrent`、`IOP::CodecFactory` / `Codec` | ✅ | | |
| `arguments`・`result`・`exceptions` | 🟡 | DII / DSI の要求だけ。静的なスタブ・スケルトンでは `NO_RESOURCES` | |
| `register_policy_factory`、`IORInterceptor_3_0`、`ObjectReferenceTemplate` | ❌ | `NO_IMPLEMENT`。需要が出たら | |
| `send_poll` | ❌ | TII がないので呼ばない | |

## Messaging

| 機能 | 状況 | メモ | 文書 |
| --- | --- | --- | --- |
| コールバック型の AMI（`sendc_`、`ReplyHandler`、`ExceptionHolder`） | ✅ | Python、C++、Java（`rustorb-idl -Wbami`）。Rust は async API | [ami.md](ami.md)、[async-api.md](async-api.md) |
| ポーリング型の AMI（`sendp_`、Poller） | ❌ | 入れないと決めた | [ami.md](ami.md) |
| `RelativeRoundtripTimeoutPolicy` | ✅ | omniORB 互換の呼び出しタイムアウトも | [timeouts.md](timeouts.md) |
| `SyncScopePolicy` | ✅ | | [sync-scope.md](sync-scope.md) |
| Rebind、RequestPriority など他のポリシー、TII、ルーター | ❌ | | |

## COS サービス

| 機能 | 状況 | メモ |
| --- | --- | --- |
| CosNaming（`NamingContextExt` まで） | ✅ | クライアントとサーバー、`rustorb-names`（omniNames の代わり、永続化あり） |
| Event / Notification、Trading、Property、Life Cycle、Time、Transaction、Concurrency など | ❌ | 対象外 |

## 対象外と決めたもの

CORBA Component Model（CCM、Lightweight CCM）、RT-CORBA、Fault Tolerant CORBA、IDL to C++98 マッピング、IDL4 の `map` と `@optional`（GIOP に形がない）。

## 言語マッピング

| 言語 | 基準 | 状況 | 文書 |
| --- | --- | --- | --- |
| Rust | OMG のマッピングがまだないので独自（IDL4 to Rust の RFP を追う） | `-brust`。abstract interface、local interface、抽象 valuetype まで。`supports` する抽象 valuetype と `context` 句はエラー | [rust-mapping.md](rust-mapping.md) |
| Python | 標準の IDL to Python マッピング | omniidl `-bpython` と同じ API（`run-pymapping-check.sh` で比べる）。omniORBpy 互換パッケージで OpenRTM-aist-Python がそのまま動く | [python-binding.md](python-binding.md)、[omniorb-compat.md](omniorb-compat.md) |
| C++ | IDL to C++11（v1.7、実装は C++17） | IDL4 の `int8` / `uint8`、`bitmask`、`bitset` も | [cpp-binding.md](cpp-binding.md) |
| Java | 標準の IDL to Java マッピング | FFM を使うので Java 22 以上。JacORB の生成コード（portable stub）も動く | [java-binding.md](java-binding.md) |

## 相互接続の検証

相手は omniORB C++ 4.3.4、omniORBpy 4.3.4、JacORB 3.9。rustorb の Rust・Python・C++・Java と組み合わせ、CI（`.github/workflows/interop.yml`）で毎回回す。詳しくは [interop/README.md](../interop/README.md)。

| 検証 | 相手 | メモ |
| --- | --- | --- |
| 基本、any、valuetype、custom、abstract の各スイート | omniORB C++、omniORBpy、JacORB | クライアントとサーバーのすべての組。基本・any・value は GIOP 1.2 と 1.1。custom は omniORB にないので rustorb と JacORB |
| Fragment（`--fragment 1024`） | 同上 | rustorb が送る側の組 |
| SSLIOP（`--ssl`）、`giop:unix`（`--unix`） | omniORB C++、omniORBpy | JacORB には TLS の設定と `giop:unix` がない。`giop:unix` は 36 組すべて通る |
| ネーミング | omniNames、rustorb-names | 操作の列、`corbaname:`、rustorb-names の再起動 |
| Portable Interceptors | omniORB C++（クライアント）、omniORBpy、JacORB | 42 組（GIOP 1.2）と 36 組（1.1・1.0）。omniORB は独自のインターセプター |
| AMI | omniORB C++、omniORBpy、JacORB のサーバー | |
| 双方向 GIOP | omniORB C++、omniORBpy | JacORB とは確かめていない |
| POA、DynAny、ORB 操作、タイムアウト、idle 接続、手組みの GIOP | omniORB C++、omniORBpy（idle は JacORB も） | 外から測った挙動と比べる |
| OpenRTM-aist（Python、C++）と rtshell | omniORBpy、omniORB C++ | omniNames と rustorb-names の両方 |

期待どおりの失敗は `interop/known-failures.txt` に記録したものだけで、どれも相手の ORB 側の原因による（omniORBpy の wchar の読み込み、omniORB の GIOP 1.1 wchar と long double の形式、JacORB の GIOP 1.1 Fragment、Java 17 での値の書き込み、union と valuetype の Helper）。
