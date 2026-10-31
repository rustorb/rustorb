# tokio ランタイムの共有と async API

[#50](https://github.com/rustorb/rustorb/issues/50)。#3 の決定（[development-plan.md](development-plan.md#非同期ランタイムとスレッドモデル)）では、tokio は ORB の内部に閉じ込めて公開 API は同期にしつつ、既存の tokio ランタイムを共有する手段と async API を用意するとしていた。Rust のコア（`rustorb-core`）にその2つを加えた。

## ランタイムの共有

`Orb::with_runtime(options, handle)` は、ORB 自身のランタイムを作らず、アプリケーションのランタイムの `Handle` の上で通信 I/O（待ち受け、接続、GIOP の送受信）を動かす。`OrbOptions::io_threads` は使わない。`Orb::runtime_handle()` は ORB が使うランタイムのハンドルを返す（自分のものでも、渡されたものでも）。

- ランタイムは I/O と time を有効にしておく
- サーバントは今までどおり ORB のアップコール用スレッドプールで動く。ランタイムのワーカーでは動かない
- マルチスレッドのランタイムには制約がない。`current_thread` のランタイムは、どこかのスレッドがその `block_on` を実行している間しか I/O が進まないので、ブロックする呼び出し（サーバントの中からの呼び出しなど）はそれまで待つ
- **終了の順序**：ランタイムより先に ORB を `destroy` する。`destroy` は接続を閉じて待ち受けのタスクを止めるので、ランタイムに ORB のタスクは残らない（テストで `num_alive_tasks()` が 0 になることを確かめる）。ランタイムが先に終わった場合、呼び出しは `COMM_FAILURE` か `TRANSIENT` になり、`destroy` は返る
- `current_thread` のランタイムでは、`shutdown(true)` と `destroy` は外から呼ぶとランタイムが回るまで待ち、中から呼ぶとランタイムを止めてしまう。中から `shutdown(false)` と `run_async().await` で終了させてから `destroy` する

## async API

機能フラグは設けない。追加の依存がなく（tokio はもともと使っている）、同期 API の振る舞いも変わらないため。

| 同期 | async |
| --- | --- |
| `ObjectRef::request(op)` → `Request` | `ObjectRef::request_async(op)` → `AsyncRequest` |
| `Request::invoke` / `send_oneway` | `AsyncRequest::invoke` / `send_oneway`（`Future`） |
| `ObjectRef::is_a` / `non_existent` / `locate` | `is_a_async` / `non_existent_async` / `locate_async` |
| `Orb::run` | `Orb::run_async`（シャットダウンが済むと完了する） |

- 返す `Future` は `Send` で、`tokio::spawn` できる。そのため `AsyncRequest::args` の関数は `Send + Sync` を要求する（`Request::args` は今までどおり要求しない）
- `Future` はどのエグゼキュータで poll してもよい。poll の間は ORB のランタイムのコンテキストに入るので、ソケットとタイマーは ORB のランタイムのものになる（I/O も time も無効の `current_thread` ランタイムから呼べることをテストで確かめる）
- 呼び出しを poll するスレッドはブロックしない。同期 API と違い、応答を呼び出し元のスレッドで直接読む最適化（[#67](https://github.com/rustorb/rustorb/issues/67)）は使わず、接続の読み取りタスクから受け取る
- 同じ ORB のオブジェクトへの呼び出し（同一 ORB 内呼び出し）は、サーバントをアップコール用スレッドプールで動かし、その完了を待つ。同期 API では呼び出し元のスレッドで直接動かすが、async では ワーカーを塞がないため、またサーバントが中で同期 API を使えるようにするためにプールに渡す
- タイムアウトは `invoke` を呼んだ時点で決まる。スレッドのタイムアウト（`set_thread_call_timeout`）もその時点のスレッドのものを使う
- `run_async` は MAIN_THREAD_MODEL の要求を処理しない。それには `run` か `perform_work` をスレッドで呼ぶ。async のコードから ORB を止めて待つには `shutdown(false)` の後に `run_async().await`

## tokio のワーカーからの同期 API

今までどおり `BAD_INV_ORDER`（minor 0）にする。ブロックしてよいことにすると、ORB のランタイムを共有した場合に I/O を進めるワーカーを塞いでデッドロックしうるため。`spawn_blocking` のスレッドもランタイムのコンテキストに入っているので同じく `BAD_INV_ORDER` になる。async のコードからは async API を使い、どうしても同期 API を使うなら `std::thread` で別スレッドを立てる。

例外は `shutdown(true)` と `destroy` で、ランタイムの中から呼ぶと別のスレッドで終了処理をして待つ（ワーカーは塞ぐ）。

## 実装

- 呼び出しの手順（アドレスの順の試行、転送、アドレッシングモードの切り替え、元の参照への戻り）を `Walk` にまとめ、同期の `invoke` と `invoke_async` で共有する
- 引数を書く関数は `WriteArgs` トレイトで同期版（`dyn Fn`）と async 版（`dyn Fn + Send + Sync`）の両方を受ける。`Kind` と送信の関数はこれについて総称的
- 同一 ORB 内呼び出しは、要求の組み立て（`local_call`）、実行（`LocalCall::run`）、応答の解釈（`local_outcome`）に分け、async では実行だけをプールに渡す
- `Orb::in_runtime` は poll のたびに `Handle::enter` する `Future` の包み

## テスト

`crates/rustorb-core/tests/async_api.rs`：共有したマルチスレッドのランタイムでの TCP 越しの呼び出し（多数のタスクから並行に、`is_a_async` など、oneway、ワーカーからの同期 API が `BAD_INV_ORDER`、別スレッドからの同期 API）と、`destroy` 後にタスクが残らないこと。ORB 自身のランタイムの呼び出しを I/O のないランタイムで poll すること（タイムアウトを含む）。同一 ORB 内呼び出し（サーバントの中の同期呼び出しを含む）。`current_thread` のランタイム。`run_async`。ランタイムが先に終わった場合。

## 残り

- Rust の IDL 生成器（`-brust`）が async 版のスタブを出すこと（[#77](https://github.com/rustorb/rustorb/issues/77)）。今は `request_async` を直接使う
