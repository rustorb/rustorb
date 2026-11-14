# DII と DSI

[#44](https://github.com/rustorb/rustorb/issues/44)。生成コードなしで要求を組み立てて送る DII（Dynamic Invocation Interface）と、生成コードなしで要求を受ける DSI（Dynamic Skeleton Interface）を、Rust のコア（`rustorb-core`）と Python のバインディングに加えた。C ABI（`rustorb-ffi`）と C++ は [#155](https://github.com/rustorb/rustorb/issues/155) で加えた。Java は [#156](https://github.com/rustorb/rustorb/issues/156)。

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

## C ABI（`rustorb.h`）

[#155](https://github.com/rustorb/rustorb/issues/155)。コアの `DiiRequest` と `ServerRequest` をそのままハンドルにした。値はすべて `rorb_any`、向きは `RORB_ARG_IN`・`RORB_ARG_OUT`・`RORB_ARG_INOUT`（1・2・3、CORBA の `ARG_*` と同じ値）。

DII：

- `rorb_request_new(target, op, len, &req, exc)` / `rorb_request_release`
- `rorb_request_add_arg(req, name, len, flags, any, exc)`：`out` は Any の TypeCode だけを使う。`rorb_any_typed(tc)` が値のない、型だけの Any を作る
- `rorb_request_set_return_type`、`rorb_request_add_exception`、`rorb_request_set_timeout`（マイクロ秒。負ならポリシーどおり、0 なら無制限）
- `rorb_request_invoke`・`rorb_request_send_oneway`・`rorb_request_send_deferred`・`rorb_request_poll_response(req, &ready, exc)`・`rorb_request_get_response`
- 応答の後：`rorb_request_result`、`rorb_request_argument(req, i, &any, exc)`（`inout` と `out` は応答の値）。ユーザー例外のときは送る関数が `RORB_USER_EXCEPTION` を返し、`rorb_request_exception` が例外リストの TypeCode で読んだ本体の Any を返す。リストにない例外は `UNKNOWN`（minor `UNLISTED_USER_EXCEPTION`、completed yes）のシステム例外になる
- oneway は他の呼び出しと同じく SyncScope のポリシーに従う（[sync-scope.md](sync-scope.md)）

DSI：DSI のサーバントは、他のサーバントと同じ vtable のサーバント。`dispatch` の中で次の順に呼ぶ。

1. `rorb_server_request_new(in, op, len, &sr, exc)`：受けた本体（`rorb_in`）から `ServerRequest` を作る。本体を読む前に呼ぶ
2. `rorb_server_request_add_param(sr, name, len, flags, tc, exc)` をパラメーターの数だけ呼び、`rorb_server_request_arguments` で `in` と `inout` を読む（2回目は `BAD_INV_ORDER`）。`rorb_server_request_argument(sr, i, &any, exc)` が読んだ値
3. `rorb_server_request_set_argument(sr, i, any, exc)` で `out` と `inout` を書く（`in` のパラメーターや型の違う Any は `BAD_PARAM`）
4. `rorb_server_request_set_result` か `rorb_server_request_set_exception`
5. `rorb_server_request_reply(sr, reply, exc)` が結果か例外を `dispatch` の `rorb_server_reply` に書く

PI の `arguments`・`result`・`exceptions` は、コアと同じく DII と DSI の要求で使える（[portable-interceptors.md](portable-interceptors.md)）。

## C++（`<rustorb/dii.h>`、`<rustorb/cdr.h>` が読み込む）

IDL to C++11 マッピングの DII / DSI。

- `obj->_request(op)` と `obj->_create_request(ctx, op, arg_list, result, [exclist, ctxlist,] request, flags)`、`orb->create_list` / `create_named_value` / `create_exception_list`
- `CORBA::Request`：`add_in_arg(name)` / `add_inout_arg` / `add_out_arg` が `Any&` を返す。`out` には型だけを入れる（`rustorb::detail::typed_any(tc)` か、その型の値）。`set_return_type`、`exceptions()->add(tc)`、`invoke`・`send_oneway`・`send_deferred`・`poll_response`・`get_response`、`return_value()`、`arguments()`（`NVList`）、`target()`・`operation()`・`result()`・`exceptions()`
- `NVList`（`add`・`add_item`・`add_value`・`item`・`remove`・`count`、範囲外は `CORBA::Bounds`）、`NamedValue`、`ExceptionList`
- `orb->send_multiple_requests_oneway` / `_deferred`、`poll_next_response`、`get_next_response`
- `PortableServer::DynamicImplementation` を継承して `invoke(object_reference<ServerRequest>)` と `_primary_interface(oid, poa)` を書く。`ServerRequest` は `operation()`・`arguments(nvlist)`・`set_result`・`set_exception`

決めたこと：

- ユーザー例外は `CORBA::UnknownUserException` で返る。`exception()` が例外リストの TypeCode で読んだ本体の Any（標準の C++ マッピングと同じ）。サーバントの `invoke` が `UnknownUserException` を投げたときは、その Any を `set_exception` したのと同じに扱う
- リストにないユーザー例外は `CORBA::UNKNOWN`（minor `0x4f4d0001`、completed yes）。Python と同じ
- `Request` は送るたびに、そのときの引数・戻り値の型・例外リストから C ABI の要求を作る。同じ `Request` を何度も送れる
- ローカルなオブジェクト（`LocalObject`）への `_request` は `NO_IMPLEMENT`
- `ServerRequest::arguments` は `NVList` の Any の TypeCode をパラメーターの型にする。読んだ値を同じ `NVList` に入れ、`invoke` から戻るときに `out` と `inout` の値を応答に書く
- POA に登録するときのリポジトリ ID は `_primary_interface({}, nullptr)` から1回だけ取る
- `get_next_response` は ORB ごとの deferred の要求を1ミリ秒ごとに見て回る。何もないときは `BAD_INV_ORDER`
- コンテキスト（`ctx`・`ctxlist`・`Request::ctx()`・`ServerRequest::ctx()`）と `get_default_context` は `NO_IMPLEMENT`
- 制限：C++ の Any は valuetype を写して持つ。別々の Any に入れた同じ valuetype は2つになり、DII / DSI の引数どうしでは共有が保たれない（静的なスタブ・スケルトンでは保たれる）。value の `same[shared]` が既知の失敗

## テスト

- `crates/rustorb-core/tests/dii.rs`：DSI のサーバントと静的なサーバントを、DII（同期・async・deferred・oneway）で TCP 越しと同一 ORB 内で呼ぶ。`ServerRequest` の呼び出し順の検査
- `python/tests/test_dii.py`：Python の DII から静的なサーバントと DSI のサーバントを呼ぶ。戻り値・構造体・`inout` と `out`・ユーザー例外・システム例外・oneway・deferred と `get_next_response`・`ServerRequest` の呼び出し順・並行する要求
- 相互接続：`interop/impls/rustorbpy-dii` は `rustorbpy` のクライアントとサーバーを、スタブの呼び出しを DII に、サーバントを DSI に差し替えて動かす（`dii_adapters.py`）。basic・any・value・abstract で、omniORB C++・omniORBpy・rustorb（Rust・Python）との組が両方向で `rustorbpy` と同じ結果になる（失敗は `long double` と omniORBpy の wchar の既知のものだけ）。CI では basic（GIOP 1.2 / 1.1）・any・value を回す。valuetype の共有はこの value の組で確かめる
- `cpp/tests/test_dii.cc`：basic・any・value の相互接続の項目を、C++ の DII → DSI、DII → 静的なサーバント、静的なスタブ → DSI の3通りで回す。API の項目：アクセサー、`_create_request`、システム例外、deferred と `get_next_response`、`send_multiple_requests_*`、`NVList` と `Bounds`、ローカルなオブジェクト、`ServerRequest` の呼び出し順、SyncScope に従う oneway、インターセプターの `arguments`・`result`・`exceptions`
- 相互接続：`interop/impls/rustorbcpp-dii` は `rustorbcpp` のクライアントとサーバーを、スタブの呼び出しを DII に、サーバントを DSI に差し替えて動かす（`dii_adapters.h`、環境変数 `RUSTORB_DII=1`）。basic（GIOP 1.2 / 1.1）・any・value で、omniORB C++・omniORBpy・`rustorbpy-dii` との組が両方向で `rustorbcpp` と同じ結果になる（失敗は `long double`、omniORBpy の wchar、上の `same[shared]` の既知のものだけ）。CI でも回す
