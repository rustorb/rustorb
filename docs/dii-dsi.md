# DII と DSI

[#44](https://github.com/rustorb/rustorb/issues/44)。生成コードなしで要求を組み立てて送る DII（Dynamic Invocation Interface）と、生成コードなしで要求を受ける DSI（Dynamic Skeleton Interface）を、Rust のコア（`rustorb-core`）と Python のバインディングに加えた。C++ と Java は需要が出たら加える（[development-plan.md](development-plan.md#目的とスコープ)）。

値はすべて Any で受け渡す。valuetype の共有とキャッシュの扱いは静的なスタブ・スケルトンと同じ。

## コア（`rustorb_core::dii`）

### DII

`ObjectRef::create_request(op)` が `DiiRequest` を返す。

- 引数：`add_in_arg(name, any)`・`add_inout_arg(name, any)`・`add_out_arg(name, tc)`。`arguments_mut()` で `NamedValue`（名前・値・`ArgMode`）の列を直接いじってもよい。`out` の値は応答が来るまで `Value::Void`
- `set_return_type(tc)`（既定は `void`）、`add_exception(tc)`（受け取るユーザー例外の TypeCode）、`set_timeout`
- 送り方：`invoke`（同期）、`invoke_async`（`Send` な `Future`、[async-api.md](async-api.md)）、`send_oneway`、`send_deferred` の後に `poll_response` / `get_response`。deferred の要求は ORB のランタイムで送り、応答はチャネルで受ける
- 応答の後は `result()` が戻り値、`arguments()` の `inout` と `out` が応答の値
- ユーザー例外は `Exception::User` で返る。`exception_value(&e)` が例外リストの TypeCode でその本体を Any にする。リストにない例外は `UNKNOWN`（completed yes）

### DSI

`DynamicImplementation`（`primary_interface`・`is_a`・`invoke`）を実装し、`DynamicServant(impl)` を POA に登録する。`invoke` が受け取る `ServerRequest` は次の順に使う。

1. `arguments(params)`：パラメーターの名前・TypeCode・向きの列を渡すと、`in` と `inout` の値を読んで返す。1回だけ呼べる（2回目は `BAD_INV_ORDER`）
2. `parameters_mut()` で `out` と `inout` の値を書く
3. `set_result(any)` か `set_exception(any)`。`set_exception` は `tk_except` の Any でなければ `BAD_PARAM`。`arguments` より前の `set_result` は `BAD_INV_ORDER`

`invoke` が返ると、ORB が結果か例外を応答に書く。`invoke` が `Err` を返せばそのシステム例外が応答になる。

`ServerRequest` は要求の本体を所有する（`DetachedInput`：本体のバッファの共有と読み始めの位置）ので、`invoke` の外に持ち出す実装（Python のバインディング）でも本体を写さない。

## Python（`rustorb.CORBA`・`rustorb.PortableServer`）

標準の Python マッピングの API。

- `obj._request(op)` と `obj._create_request(ctx, op, arg_list, result, exceptions)`、`orb.create_list` / `create_named_value` / `create_exception_list`
- `Request`：`add_in_arg` / `add_inout_arg` / `add_out_arg`、`set_return_type`、`add_exception`、`invoke`・`send_oneway`・`send_deferred`・`poll_response`・`get_response`、`return_value()`、`arguments()`（`NVList`）
- `orb.send_multiple_requests_oneway` / `_deferred`、`poll_next_response`、`get_next_response`
- `PortableServer.DynamicImplementation` を継承して `invoke(request)` と `_primary_interface(oid, poa)` を書く。`ServerRequest` は `operation()`・`arguments(nvlist)`・`set_result`・`set_exception`

決めたこと：

- Python の値と Any の変換と呼び出しは1つの valuetype のスコープの中で行う。引数と戻り値の間で共有した valuetype は共有のまま届く。`send_deferred` はスコープの外で送るので、送るまで値を保つ
- 応答の値は `invoke` の後で Python の値に戻す。`in` の引数は送ったときのものをそのまま使い、もう一度は変換しない
- サーバントがリストにないユーザー例外を投げたときは、クライアントで `UNKNOWN`（minor `UNLISTED_USER_EXCEPTION`）にする
- POA に登録するときのリポジトリ ID は `_primary_interface(None, None)` から取る
- `get_next_response` は deferred の要求を1ミリ秒ごとに見て回る（応答の届いた順の通知は持たない）
- コンテキスト（`ctx`・`ctxlist`）は扱わない。`ServerRequest.ctx()` と `orb.create_operation_list`（インターフェースリポジトリが要る）は `NO_IMPLEMENT`

## テスト

- `crates/rustorb-core/tests/dii.rs`：DSI のサーバントと静的なサーバントを、DII（同期・async・deferred・oneway）で TCP 越しと同一 ORB 内で呼ぶ。`ServerRequest` の呼び出し順の検査
- `python/tests/test_dii.py`：Python の DII から静的なサーバントと DSI のサーバントを呼ぶ。戻り値・構造体・`inout` と `out`・ユーザー例外・システム例外・oneway・deferred と `get_next_response`・`ServerRequest` の呼び出し順・並行する要求
- 相互接続：`interop/impls/rustorbpy-dii` は `rustorbpy` のクライアントとサーバーを、スタブの呼び出しを DII に、サーバントを DSI に差し替えて動かす（`dii_adapters.py`）。basic・any・value・abstract で、omniORB C++・omniORBpy・rustorb（Rust・Python）との組が両方向で `rustorbpy` と同じ結果になる（失敗は `long double` と omniORBpy の wchar の既知のものだけ）。CI では basic（GIOP 1.2 / 1.1）・any・value を回す。valuetype の共有はこの value の組で確かめる
