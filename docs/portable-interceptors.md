# Portable Interceptors 設計書

[#100](https://github.com/rustorb/rustorb/issues/100)（[#107](https://github.com/rustorb/rustorb/issues/107) の 4・5 段目）。CORBA 3.x の `PortableInterceptor` モジュール（`ORBInitializer`、要求インターセプター、IOR インターセプター、`PICurrent`）と、それが使う `IOP::Codec` を、コア（`rustorb-core`）と Python・C ABI・C++・Java のバインディングに加える。この文書は実装の前の設計で、実装の PR はこれをマージしてから出す。

根拠は CORBA 3.x の仕様（Portable Interceptors の章と `PortableInterceptor`・`IOP` の IDL）だけで、他の ORB のソースは読まない（[development-plan.md](development-plan.md) の方針）。相互接続での挙動は実際に動かして確かめる。

## 範囲

| 機能 | 初回 | 備考 |
| --- | --- | --- |
| `ORBInitializer`（`pre_init`・`post_init`）、`ORBInitInfo`、`register_orb_initializer` | 入れる | |
| `ClientRequestInterceptor` と `ClientRequestInfo` | 入れる | `send_poll` は呼ばない（下記） |
| `ServerRequestInterceptor` と `ServerRequestInfo` | 入れる | |
| `IORInterceptor`（`establish_components`）と `IORInfo` | 入れる | |
| `PICurrent`（スロット、スレッドと要求のスコープ） | 入れる | |
| `ForwardRequest`、システム例外での中断 | 入れる | |
| `IOP::CodecFactory` と `Codec`（`ENCODING_CDR_ENCAPS` 1.0〜1.2） | 入れる | サービスコンテキストとタグ付きコンポーネントの中身を作るのに要る |
| 同一 ORB 内の呼び出し（colocated） | 入れる | 両側のインターセプターを呼ぶ |
| `arguments`・`result`・`exceptions`（`Dynamic::ParameterList`） | DII・DSI のときだけ | 静的なスタブ・スケルトンでは `NO_RESOURCES`（下記） |
| `ORBInitInfo::register_policy_factory` と独自のポリシー | 後 | 初回は `NO_IMPLEMENT`。需要が出たら issue にする |
| `IORInterceptor_3_0`（`components_established`、状態の通知）と `ObjectReferenceTemplate` | 後 | 同上 |
| `send_poll` | 呼ばない | TII（時間に依存しない呼び出し）だけの点で、rustorb に TII はない。AMI（#100 の範囲外）でも使わない |
| omniORBpy 互換シムの `omniORB.interceptors` | 後 | 需要が出たら（[omniorb-compat.md](omniorb-compat.md)） |

LocateRequest にはインターセプターを呼ばない（要求インターセプターは Request メッセージのためのもの）。`_is_a`・`_non_existent` などの ORB の操作は普通の要求なので呼ぶ。

## 全体の形

```
Orb::init / with_options / with_runtime
  └─ 登録された ORBInitializer の pre_init → ORB の中身を作る → post_init
       └─ ORBInitInfo に足されたインターセプターとスロット数を Interceptors に固める
           （ORB ができた後は増えない）

OrbInner.pi: Option<Arc<Interceptors>>
  ├─ client: Vec<Arc<dyn ClientRequestInterceptor>>   （登録順）
  ├─ server: Vec<Arc<dyn ServerRequestInterceptor>>
  ├─ ior:    Vec<Arc<dyn IorInterceptor>>
  └─ slots:  u32
```

インターセプターが一つもない ORB は `pi` が `None` で、要求ごとの差はこの `Option` を 1 回見るだけにする。ベンチマーク（[load-testing.md](load-testing.md)）で、インターセプターなしのときに今と差がない（ノイズの範囲）ことを確かめる。何もしないインターセプターを 1 つずつ入れたときの増え方も測って書き残す。

新しいモジュールは `rustorb_core::pi`（`PortableInterceptor`）と `rustorb_core::iop`（`IOP::Codec`・`CodecFactory`、`ServiceContext`・`TaggedComponent` の再公開）。

## ORB の初期化

### 登録

- `rustorb_core::pi::register_orb_initializer(Arc<dyn OrbInitializer>)`：プロセス全体の登録。仕様の `PortableInterceptor::register_orb_initializer` と同じく、それより後に作る ORB すべてに効く
- `OrbOptions::initializers: Vec<Arc<dyn OrbInitializer>>`：その ORB だけのもの。Rust ではこちらを勧める（テストで ORB ごとに変えられる）
- 呼ぶ順は、プロセス全体のもの（登録順）、`OrbOptions` のもの（並び順）

`OrbOptions` は `Clone` と `Debug` を持つので、`initializers` は `Arc` の列にし、`Debug` は数だけを出す。

### 流れ

1. `OrbOptions` を `-ORB` 引数から作る（今と同じ）
2. `ORBInitInfo` を作り、各 initializer の `pre_init` を呼ぶ
3. ORB の中身（ランタイム、接続の表、初期参照）を作る
4. 各 initializer の `post_init` を呼ぶ。ここでは `resolve_initial_references` が使える
5. インターセプターとスロット数を固め、`ORBInitInfo` を無効にする（後から使うと `OBJECT_NOT_EXIST`）

`pre_init` / `post_init` が例外を返したときは、それまでに足されたインターセプターの `destroy` を呼んで ORB の作成を失敗させ、その例外を `ORB_init` の失敗として返す（バインディングでの言語の例外は `INITIALIZE`）。仕様が例外を無視するよう求めているかは実装の前に章の文言で確かめ、違えば仕様に合わせてこの節を直す。

`Orb::destroy` は、要求が終わった後に各インターセプターの `destroy` を 1 回ずつ呼ぶ。

### `ORBInitInfo`

| 操作 | rustorb |
| --- | --- |
| `arguments` | `ORB_init` に渡された引数（`-ORB` の引数を取り除く前のもの） |
| `orb_id` | `-ORBid`（新しく受け付ける。既定は `""`） |
| `codec_factory` | `CodecFactory` |
| `register_initial_reference` | 今の `Orb::register_initial_reference` と同じ表に入れる。`pre_init` でも使える |
| `resolve_initial_references` | `post_init` だけ。`pre_init` では `BAD_INV_ORDER`（minor 14） |
| `add_client_request_interceptor` ほか 2 つ | 名前の重なりは `DuplicateName`。名前が `""` のもの（匿名）はいくつでもよい |
| `allocate_slot_id` | 0 から順に払い出す |
| `register_policy_factory` | 初回は `NO_IMPLEMENT` |

`resolve_initial_references("PICurrent")` と `("CodecFactory")` は ORB のローカルな初期参照として返す（今の `register_local_initial_reference` の仕組み）。

## 要求インターセプター

### インターセプトの単位

1 回のインターセプト（starting point から ending point まで）は、1 つのアドレスへの 1 回の送信に対応する。rustorb の呼び出しの流れ（`object.rs` の `Walk`）に当てはめると次のとおり。

| 起きたこと | 今の動き | インターセプター |
| --- | --- | --- |
| 試すアドレスが決まった | 接続して送る | `send_request`（接続より前。`effective_profile` はそのアドレスのプロファイル） |
| 接続できない、送れない（completed no） | 次のアドレスを試す | `receive_exception`（`TRANSIENT` / `COMM_FAILURE`、completed no）。次のアドレスで新しく `send_request` |
| 返事の前の `CloseConnection` | 新しい接続で送り直す | `receive_other`（`TRANSPORT_RETRY`）、送り直しで新しく `send_request` |
| `NEEDS_ADDRESSING_MODE` | 別の形のターゲットで送り直す | `receive_other`（`TRANSPORT_RETRY`） |
| `LOCATION_FORWARD` | 転送先に送る | `receive_other`（`LOCATION_FORWARD`、`forward_reference`） |
| 返事 | 呼び出しの結果 | `receive_reply` / `receive_exception` |
| タイムアウト | `TIMEOUT` | `receive_exception`（`TIMEOUT`） |
| oneway を送った | すぐ戻る | `receive_other`（`SUCCESSFUL`、`response_expected` は false） |

どのアドレスにもつながらないときも、試したアドレスごとにインターセプターが例外を見る。送り直しを ORB の中で隠さず、毎回別のインターセプトとして見せるのは、仕様の `TRANSPORT_RETRY` がそのためにあり、送られる要求とインターセプトが 1 対 1 になって分かりやすいからである。

`ClientRequestInfo::request_id` は、GIOP の要求 ID ではなく ORB ごとのインターセプト用の通し番号にする（接続を開く前に決まっている必要があるため）。

### 呼ぶ順と例外（flow rules）

仕様の規則どおりにする。

- starting point（`send_request`）は登録順、ending point（`receive_*`）は逆順
- starting point でインターセプター k が例外を返すと、k より後の `send_request` は呼ばず、要求も送らない。`send_request` を済ませた 0〜k−1 にだけ、逆順で `receive_exception`（その例外）を呼ぶ
- ending point でインターセプターが例外を返すと、残りのインターセプターには `receive_exception` でその例外を渡す。呼び出し元に返る結果は最後の例外
- `ForwardRequest`：`send_request`・`receive_exception`・`receive_other` で返せる。済ませたインターセプターに `receive_other`（`LOCATION_FORWARD`）を呼んでから、転送先への新しい試行を `Walk` に渡す（今の `LOCATION_FORWARD` と同じく `max_forwards` で数える）。`receive_reply` での `ForwardRequest` は仕様で許されないので `BAD_INV_ORDER` にする

### サービスコンテキスト

- `add_request_service_context(sc, replace)` は `send_request` の中だけ。同じ ID があって `replace` が false なら `BAD_INV_ORDER`（minor 15）
- コアが自分で書くもの（CodeSets、#103 の後は BI_DIR_IIOP）は、マーシャルのときにインターセプターが足したものの前に置く。クライアントのインターセプターからは見えない（コード系の交渉は接続の後で決まるため）。インターセプターがその ID を足そうとすると `BAD_INV_ORDER`（minor 15）
- 返事のサービスコンテキストは、今は `Reply` を作るときに捨てているので、`receive_*` まで持ち回る

### スレッド

- 同期の呼び出し（`Request::invoke`）：インターセプターは呼び出し元のスレッドで呼ぶ
- async の呼び出し（[async-api.md](async-api.md)）：`Future` を poll している ORB のランタイムのスレッドで呼ぶ。インターセプターは短く、ブロックしない前提とし、そう書いておく。`PICurrent` の要求スコープは、`request_async` を呼んだスレッドで写しておく（下記）
- DII の `send_deferred`：送るときに `send_request`、`get_response` / `poll_response` で結果が分かったときに `receive_*`

## サーバーのインターセプター

### 呼ぶ場所

ユーザーのコードを I/O タスクで動かさない決まり（[poa-design.md](poa-design.md)）を守り、すべてアップコールのスレッド（そのサーバントを動かすスレッド。SINGLE_THREAD_MODEL / MAIN_THREAD_MODEL ならその決まりのスレッド）で呼ぶ。`server.rs` の `upcall` の中の順は次のとおり。

1. `receive_request_service_contexts`（servant manager より前。仕様の求めるとおり）
2. `poa.resolve`（active object map、servant manager の `incarnate` / `preinvoke`）
3. `receive_request`
4. サーバントの呼び出し
5. `postinvoke`（今と同じ位置）
6. `send_reply` / `send_exception` / `send_other`
7. 返事を送る

アップコールより前に断られる要求（POA が見つからない、POA manager が discarding / inactive、終了中）も、インターセプターがあれば、アップコールのスレッドで `receive_request_service_contexts` と `send_exception` を通してから返事を送る。アクセス制御や記録のインターセプターが、断られた要求を見落とさないようにするためである。インターセプターがないときは今のまま I/O タスクで返す。POA manager が holding のあいだの要求は、解放されてからインターセプターを通る。

### 呼ぶ順と例外

クライアントと同じ規則。

- `receive_request_service_contexts` は登録順、`send_*` は逆順。`receive_request` は登録順
- `receive_request_service_contexts` か `receive_request` が例外を返すと、サーバントは呼ばず、その例外を返事にする。済ませたインターセプターには `send_exception`
- `send_reply` が例外を返すと、返事をその例外（completed yes）に替え、残りには `send_exception`
- `ForwardRequest`：`receive_request_service_contexts`・`receive_request`・`send_exception`・`send_other` で返せる。返事は `LOCATION_FORWARD`、残りには `send_other`

### 返事のサービスコンテキストと GIOP の版

`add_reply_service_context` はサーバーのどの点でも使える。返事の本体（サーバントが書く）は返事のヘッダーより後ろにあるのに、ヘッダーのサービスコンテキストは `send_*` まで決まらない。そこで、インターセプターがあるときの `ServerReply` は、本体を別の encoder に書き、送る直前にヘッダーを書いて本体をつなぐ。

- GIOP 1.2：本体はヘッダーの後で 8 バイトに揃えて始まるので、本体のバイト列はヘッダーの長さによらない。どの点で足したコンテキストも送れる
- GIOP 1.0 / 1.1：本体はヘッダーの直後から始まり、CDR の揃えはメッセージの先頭からなので、ヘッダーの長さが 8 の倍数でずれると本体を書き直す必要がある。ヘッダーの長さは常に 4 の倍数なので、ずれるのは 4 バイトのときだけである。本体は、サーバントを呼ぶ前（`receive_request` まで）に足されたコンテキストでのヘッダーの長さを前提に書く。`send_*` で足したコンテキストがヘッダーの長さを 4 バイトずらすときは、そのコンテキストを送らず、レベル 1 の警告をトレースに出す。ずれないとき（多くの場合）は送る

1.0 / 1.1 で、詰め物のサービスコンテキストを足して長さを合わせる方法も考えたが、使ってよい ID が rustorb にはない（OMG の VSCID を持っていない）ので採らない。相互接続の相手（omniORB・omniORBpy・JacORB）はどれも 1.2 で話すので、実際にはこの制限に当たりにくい。

インターセプターがないときの返事の作り方は今のまま変えない。

### 同一 ORB 内の呼び出し

今の colocated の呼び出しは、要求をマーシャルしてサーバー側に渡し、返事のメッセージを受け取る（`object.rs` の `LocalCall`）。この形のまま、クライアントのインターセプターを呼び出し元で、サーバーのインターセプターをアップコールのスレッド（ORB_CTRL_MODEL なら呼び出し元のスレッド）で呼ぶ。サービスコンテキストはマーシャルされたメッセージに乗るので、TCP 越しと同じものが届く。

## `ClientRequestInfo` / `ServerRequestInfo`

情報のオブジェクトはその点の呼び出しの間だけ有効にする（Rust では `&ClientRequestInfo<'_>` を渡し、バインディングでは呼び出しが終わると無効になる薄い手を渡す）。どの点で何が使えるかは仕様の表に従い、使えない点では `BAD_INV_ORDER`（minor 14）。

rustorb で中身が決まるもの：

| 属性・操作 | rustorb |
| --- | --- |
| `arguments`・`result` | DII（クライアント）と DSI（サーバー。サーバントが `arguments` を呼んだ後）だけ。静的なスタブ・スケルトンは TypeCode を持たないので `NO_RESOURCES`（minor 1）。仕様が言語の対応がないときに認める答え |
| `exceptions` | DII の例外リスト。他は `NO_RESOURCES` |
| `contexts`・`operation_context` | `NO_RESOURCES`（rustorb は `context` 句を扱わない） |
| `received_exception` / `sending_exception` | システム例外は Any に入れて返す。ユーザー例外は TypeCode が分かるとき（DII の例外リスト、DSI の `set_exception`）だけ。分からないときは仕様どおり `UNKNOWN` を入れた Any。リポジトリ ID は `received_exception_id`、サーバー側は rustorb 独自の `sending_exception_id` で取れる |
| `sync_scope` | `SYNC_WITH_TARGET`（応答あり）か `SYNC_NONE`（oneway） |
| `target`・`effective_target`・`effective_profile`・`get_effective_component(s)` | 今の `Walk` の参照と、試しているアドレスのプロファイル |
| `get_request_policy` | クライアントの実効ポリシー（今ある `RelativeRoundtripTimeout`） |
| `object_id`・`adapter_id`・`adapter_name`・`orb_id`・`server_id` | オブジェクトキーから。`adapter_id` はオブジェクトキーの POA の部分、`adapter_name` は RootPOA からの名前の列、`server_id` は `-ORBServerId`（新しく受け付ける。既定は `""`） |
| `target_most_derived_interface`・`target_is_a` | `receive_request` から。サーバントの `repository_id` と `is_a` |
| `get_server_policy` | POA のポリシー |

## `PICurrent`

スロットの値は Any（初めは `tk_null`）。スロットは ORB ごとで、`allocate_slot_id` で払い出した数だけある。範囲外の ID は `InvalidSlot`。

- スレッドのスコープ（TSC）：スレッドごと・ORB ごとの値の列。アプリケーションとサーバントは `PICurrent::get_slot` / `set_slot` でここを読み書きする
- 要求のスコープ（RSC）：インターセプトごとの値の列

| 場所 | 動き |
| --- | --- |
| クライアント：インターセプトを始めるとき | 呼び出したスレッドの TSC を写して RSC にする。インターセプターは `ri.get_slot` で RSC を読む。async の呼び出しは `request_async` のときに写す |
| クライアント：インターセプターの中 | `PICurrent::set_slot` はインターセプターを動かしているスレッドの TSC を変える（RSC は変わらない）。インターセプターの中からの入れ子の呼び出しは、その TSC から新しい RSC を作る |
| サーバー：`receive_request_service_contexts` | 空の RSC を作る。`ri.set_slot` で RSC に書く |
| サーバー：`receive_request` の前 | RSC を、アップコールのスレッドの TSC に写す（元の TSC は取っておく）。サーバントとそこからの入れ子の呼び出しはこの TSC を見る |
| サーバー：`send_*` | RSC を見る。サーバントが TSC に書いたものは RSC に戻らない |
| サーバー：返事を送った後 | アップコールのスレッドの TSC を元に戻す（同一 ORB 内の呼び出しで、呼び出し元の TSC を壊さないため） |

インターセプターの中からの入れ子の呼び出し（認証サーバーへの問い合わせなど）にも、またインターセプターが呼ばれる。無限に入れ子にならないよう、スロットに印を付けて自分の呼び出しを見分けるのはアプリケーションの役目とし、例をドキュメントに載せる。

## IOR インターセプター

- POA を作るとき（RootPOA を含む）に、各 `IorInterceptor` の `establish_components(IORInfo)` を登録順に呼ぶ。呼ぶのは `create_POA` を呼んだスレッド（RootPOA は `root_poa` を最初に呼んだスレッド）
- `IORInfo::add_ior_component(c)` は POA のすべての IIOP プロファイルに、`add_ior_component_to_profile(c, TAG_INTERNET_IOP)` は IIOP プロファイルに足す。他のプロファイル ID は `BAD_PARAM`（rustorb の IOR は IIOP プロファイルだけ）
- 足したコンポーネントは `PoaInner` に持ち、`server.rs` の `Server::ior` がコード系・SSL・代替アドレスのコンポーネントの後ろに付ける
- `IORInfo::get_effective_policy(type)` は POA のポリシー（初回は 7 つの POA ポリシーだけ。独自のポリシーは `register_policy_factory` と一緒に後で）
- `establish_components` の例外は無視し、警告をトレースに出す（仕様の扱い）

## `IOP::Codec`

`CodecFactory::create_codec(Encoding)`。`ENCODING_CDR_ENCAPS` の 1.0〜1.2 だけで、他は `UnknownEncoding`。

- `encode(any)` / `decode(octets)`：TypeCode と値を CDR のカプセルにする
- `encode_value(any)` / `decode_value(octets, tc)`：値だけ
- バイト順は ORB の `byte_order`。読むときはカプセルの先頭のバイトに従う
- コード系は rustorb のネイティブ（char は UTF-8、wchar は UTF-16）。1.0 の wchar・wstring は `MARSHAL`。読めないときは `FormatMismatch`、TypeCode と合わないときは `TypeMismatch`

今の `Any`・`TypeCode`・CDR のエンコーダーの上に作る。

## Rust の API（案）

```rust
pub trait OrbInitializer: Send + Sync {
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> { Ok(()) }
    fn post_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> { Ok(()) }
}

pub trait Interceptor: Send + Sync {
    fn name(&self) -> &str;
    fn destroy(&self) {}
}

pub enum InterceptError {
    System(SystemException),
    ForwardRequest(ObjectRef),
}

pub trait ClientRequestInterceptor: Interceptor {
    fn send_request(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> { Ok(()) }
    fn receive_reply(&self, ri: &ClientRequestInfo<'_>) -> Result<(), SystemException> { Ok(()) }
    fn receive_exception(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> { Ok(()) }
    fn receive_other(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> { Ok(()) }
}

pub trait ServerRequestInterceptor: Interceptor {
    fn receive_request_service_contexts(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> { Ok(()) }
    fn receive_request(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> { Ok(()) }
    fn send_reply(&self, ri: &ServerRequestInfo<'_>) -> Result<(), SystemException> { Ok(()) }
    fn send_exception(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> { Ok(()) }
    fn send_other(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> { Ok(()) }
}

pub trait IorInterceptor: Interceptor {
    fn establish_components(&self, info: &IorInfo<'_>);
}
```

- 戻り値の型で、仕様がその点で許す例外だけを返せるようにする（`receive_reply` と `send_reply` は `ForwardRequest` を返せない）
- 情報のオブジェクトは `&self` のメソッドだけを持ち、サービスコンテキストやスロットの書き込みは中の `RefCell` で行う（1 つのスレッドの中だけで使われる）
- `send_poll` は、IDL との対応のために既定の実装だけを置き、呼ばない

## バインディング

### Python（`rustorb.PortableInterceptor`・`rustorb.IOP`）

- 標準の Python マッピングに従い、ローカルインターフェースのクラスを継承して書く：`PortableInterceptor.ORBInitializer`、`ClientRequestInterceptor`、`ServerRequestInterceptor`、`IORInterceptor`。属性は `_get_name()` のようなメソッド（`ri._get_operation()`、`ri._get_request_id()` など）
- 登録は `PortableInterceptor.register_orb_initializer(init)` を `CORBA.ORB_init` の前に呼ぶ
- 例外は `PortableInterceptor.ForwardRequest(obj)`、`ORBInitInfo.DuplicateName`、`InvalidSlot`、`IOP.Codec.FormatMismatch` など
- `IOP.ServiceContext(context_id, context_data)`、`IOP.TaggedComponent(tag, component_data)` は IDL の構造体どおり
- インターセプターは Rust からアップコールのスレッドや呼び出し元のスレッドで GIL を取って呼ぶ。Python から呼び出したときは呼び出しの間 GIL を放しているので、同じスレッドで取り直せる
- `PICurrent` の TSC は OS のスレッドごとなので、Python のスレッドとそのまま対応する

### C ABI（`rustorb-ffi`）と C++

- servant manager と同じ形（[cpp-binding.md](cpp-binding.md) の「POA の範囲」）：`rorb_orb_initializer`・`rorb_client_interceptor`・`rorb_server_interceptor`・`rorb_ior_interceptor` は関数表（各点、`name`、`destroy`、`add_ref`、`remove_ref`）と `self` で、`rorb_register_orb_initializer` で登録する
- 情報のオブジェクト（`rorb_client_request_info*` など）は呼び出しの間だけ有効な手。属性ごとのアクセサー（`rorb_cri_operation`、`rorb_ri_get_request_service_context`、`rorb_cri_add_request_service_context` など）を置く
- `ForwardRequest`・`DuplicateName`・`InvalidSlot` などは今の `LOCAL_EXCEPTION`（例外の番号と、`ForwardRequest` なら転送先の参照）で返す
- `ABI_VERSION` を上げる
- C++：`PortableInterceptor::register_orb_initializer`、`ORBInitializer` などのクラスは `CORBA::LocalObject` から派生し、`CORBA::make_reference<T>()` で作る（IDL to C++11 のローカルインターフェース）。C++ の例外（`PortableInterceptor::ForwardRequest` など）は C ABI の状態に直す

### Java（FFM、`org.omg.PortableInterceptor`・`org.omg.IOP`）

- 標準の Java マッピングの型（`ORBInitializer`、`ClientRequestInterceptor`、`ClientRequestInfo` などの `*Operations` とインターフェース、`ForwardRequest`、`ORBInitInfoPackage.DuplicateName`、`org.omg.IOP.Codec` など）を `java/src/main/java/org/omg` に置く
- 登録は Java の標準の方法：`ORB.init` のプロパティか system property の `org.omg.PortableInterceptor.ORBInitializerClass.<クラス名>`。そのクラスを引数なしのコンストラクターで作り、C ABI の `rorb_register_orb_initializer` に相当する ORB ごとの登録で渡す
- 呼び出しは今の servant manager と同じ FFM の upcall スタブで、情報のオブジェクトは呼び出しの間だけ有効

各バインディングの `PICurrent` は `resolve_initial_references("PICurrent")` で取り、`get_slot` / `set_slot` の値はそれぞれの Any。

## 試験

- `crates/rustorb-core/tests/pi.rs`：各点の呼ばれる順（登録順と逆順）、starting point の例外で済ませたものにだけ ending point が呼ばれること、各点の `ForwardRequest`、サービスコンテキストの往復と `replace`、PICurrent の TSC と RSC（クライアント・サーバー・入れ子・同一 ORB 内）、oneway、タイムアウト、`CloseConnection` での送り直し、断られる要求（POA なし・discarding）、DII / DSI の `arguments`、使えない点での `BAD_INV_ORDER`。TCP 越しと同一 ORB 内の両方。GIOP 1.0 / 1.1 / 1.2 の返事のサービスコンテキスト（1.0 / 1.1 の 4 バイトずれの警告を含む）
- IOR インターセプター：足したコンポーネントが IOR に入ること（omniORB の `catior` で読む）、`get_effective_policy`
- `Codec`：TypeCode ごとの往復、1.0 の wchar、壊れたカプセル
- Python・C++・Java：それぞれのテストで、各点と例外の写像、`ForwardRequest`、PICurrent、登録の方法（Java はプロパティ）
- ベンチマーク：インターセプターなしで差がないこと、何もしないインターセプターでの増え方

## 相互接続

確かめたいのは、サービスコンテキストとタグ付きコンポーネントが ORB の間で届くことである。

- JacORB：標準の Portable Interceptors を持つので、試験用の Java のサーバーとクライアントに標準の API でインターセプターを入れる。rustorb（Rust・Python・C++・Java）のクライアントが足した要求のコンテキストを JacORB のサーバーが読み、同じ中身を返事のコンテキストで返す。逆向きも同じ
- omniORB C++・omniORBpy：それぞれの公開されたマニュアルにあるインターセプターの API を試験用のプログラムで使い、同じことをする（使い方はマニュアルだけを見て、ソースは読まない）
- 相手が自分で送るコンテキスト（CodeSets など）が rustorb のサーバーのインターセプターから `get_request_service_context` で読めること
- IOR：rustorb が足したコンポーネントが相手の ORB の IOR の解析で壊れないこと（`catior`、相手からの呼び出しが通ること）
- `interop/run-pi-probe.sh` にまとめ、CI では JacORB と omniORBpy の組を回す

## 実装の順（#107 の PR）

1. この設計書
2. コア：`pi`・`iop`、初期化、クライアントとサーバーの要求インターセプター、PICurrent、IOR インターセプター、Codec、`-ORBid` / `-ORBServerId`、Rust のテストと JacORB との相互接続
3. Python：`rustorb.PortableInterceptor`・`rustorb.IOP`、テスト、omniORBpy・JacORB との相互接続
4. C ABI・C++・Java：テスト、omniORB C++・JacORB との相互接続

`register_policy_factory`・`IORInterceptor_3_0`・omniORBpy 互換の `omniORB.interceptors` は、需要が出たら issue にする。

## 決めたこと（レビューで変えてよい）

- インターセプトは 1 つのアドレスへの 1 回の送信ごと。ORB の送り直しは `TRANSPORT_RETRY` で見せる
- サーバーの点はすべてアップコールのスレッドで呼ぶ。アップコールの前に断られる要求も、インターセプターがあれば通す
- 静的なスタブ・スケルトンの `arguments`・`result`・`exceptions` は `NO_RESOURCES`。生成器に TypeCode を出させるのは、需要が出たら
- GIOP 1.0 / 1.1 で `send_*` が足した返事のコンテキストが揃えをずらすときは送らずに警告する
- コアが書くサービスコンテキストはクライアントのインターセプターから見えず、同じ ID は足せない
