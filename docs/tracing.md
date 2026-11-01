# ログ・トレース出力

issue #105。接続、要求、例外、転送などで ORB が何をしたかを利用者が見られるようにする。相互接続や設定の問題（つながらない、例外の原因、IOR のアドレス）を調べるためのもの。

## 使い方

どのバインディングでも ORB 引数で指定する。

```sh
./server -ORBtraceLevel 10
./server -ORBtraceLevel 25 -ORBtraceTime 1
```

```text
rustorb: info: listening on 127.0.0.1:2809, published as myhost:2809
rustorb: info: accepted a connection from 127.0.0.1:51432
rustorb: debug: request "echo_long" (id 0) from 127.0.0.1:51432, GIOP 1.2
rustorb: trace: sent Reply (GIOP 1.2, 28 bytes) to 127.0.0.1:51432
```

| オプション | `OrbOptions` | 既定 | 意味 |
| --- | --- | --- | --- |
| `-ORBtraceLevel N` | `trace_level` | 1 | どの出来事を出すか（下の表） |
| `-ORBtraceTime 0\|1` | `trace_time` | 0 | 標準エラーの行の頭に時刻（UTC、マイクロ秒まで）を付ける |

レベルはプロセス全体で 1 つ（omniORB と同じ）。ORB を作ったときに、そのオプションで指定があれば変わる。指定のない ORB は変えない。Rust からは `rustorb_core::trace::set_trace_level`、Python からは `rustorb.set_trace_level` で途中でも変えられる。

| レベル | `tracing` のレベル | 出来事 |
| --- | --- | --- |
| 0 | | 何も出さない |
| 1（既定） | `WARN` | サーバントやサーバントマネージャーの panic、PERSISTENT POA の固定でないポートなど、アプリが知るべきこと |
| 5 | `INFO` | 接続を開いた・受け付けた・閉じた・失った、TLS のハンドシェイクの失敗、待ち受けと公開するアドレス、`LOCATION_FORWARD` に従った、POA の作成と破棄、POA マネージャーの状態 |
| 10 | `DEBUG` | 要求ごと（クライアント・サーバーの両方）：操作名、要求 ID、相手、GIOP のバージョン、返事の状態、システム例外、タイムアウト、同じ ORB 内の呼び出し、オブジェクトの活性化と非活性化 |
| 25 | `TRACE` | GIOP メッセージごと：送った・受けたメッセージの種類、バージョン、大きさ（Fragment もそれぞれ） |

レベルは上のレベルの出来事をすべて含む。数字の区切りは omniORB の `-ORBtraceLevel` の説明（1：重要なエラー、5：接続、10：実行、25：送受信）に合わせた。メッセージの中身のダンプ（omniORB の 30 以上）はまだない。

既定の 1 では、正常に動いている間は何も出ない。これまで標準エラーに直接書いていた警告（サーバントの panic など）は、レベル 1 の出来事になった。

## 仕組み

コアの出来事はすべて [`tracing`](https://docs.rs/tracing) クレートのイベントとして出す。ターゲットは出したモジュールの名前（`rustorb_core::connection`、`rustorb_core::object`、`rustorb_core::server`、`rustorb_core::poa`、`rustorb_core::wire`）。

行き先は次のとおり。

1. **グローバルな `tracing` の subscriber があるとき**：イベントはすべてそこへ行き、絞り込みは subscriber の設定で決まる（`-ORBtraceLevel` は使わない）。Rust のアプリが `tracing-subscriber` などを使っているなら、何もしなくてもそこに rustorb の出来事が混ざる。例：`RUST_LOG=rustorb_core=debug`
2. **ないとき**：トレースレベルが選んだ出来事を、コアが 1 行ずつ標準エラーへ書く（`rustorb: <level>: <message>`）。C++ と Java はこの経路を使う

コアはグローバルな subscriber を自分では設定しない。アプリが ORB の後で `tracing_subscriber::fmt::init()` などを呼んでも失敗しない。

依存に加えたのは `tracing`（`default-features = false`、`std` だけ）だけで、`tracing-subscriber` は使わない。標準エラーへの書き出しはコアの小さな関数で、行の書式と時刻の書式もそこにある。

### 無効なときの重さ

出来事を出す場所では、まずトレースレベル（atomic の読み込み 1 回）を比べる。選ばれていなければ、次に `tracing` 自身の判定（subscriber がなければ最大レベルの比較 1 回）だけで終わり、メッセージの引数は評価しない。`benches/throughput.rs` で、入れる前と後に差がないことを確かめた（下）。

## Python

`CORBA.ORB_init` が、コアの出来事を `logging` へ渡す subscriber を設定する（グローバルな subscriber がほかにない場合）。ロガーは `rustorb` とその下の `rustorb.connection`、`rustorb.object`、`rustorb.server`、`rustorb.poa`、`rustorb.wire`。レベルは WARNING、INFO、DEBUG、TRACE（5、`rustorb._log.TRACE`）。

- `logging` にハンドラーがないとき（`logging.basicConfig()` などをしていない）は、ほかのバインディングと同じ行を標準エラーへ書く。`-ORBtraceLevel` だけで見られる
- ハンドラーがあれば、レコードとして `logging` に渡す。`rustorb` ロガーのレベルをアプリが設定していなければ、トレースレベルに合わせて設定する（`-ORBtraceLevel 25` なら TRACE）。アプリが設定していれば、それに従う

出来事を出すスレッド（I/O スレッドなど）は、キューに積むだけで GIL を取らない。専用のスレッド（`rustorb-log`）が GIL を取って `logging` に渡す。GIL を持った Python のスレッドがコアの処理を待ち、そのコアのスレッドがログのために GIL を待つ、というデッドロックを避けるため。終了時には `atexit` で残りを渡す。そのため、レコードの時刻（`created`）は出来事の時刻より少し後になることがある。

## C++・Java

`CORBA::ORB_init(argc, argv)` / `ORB.init(args, props)` の引数で指定し、標準エラーに出る。コールバックで受け取る API はまだない（要望があれば足す）。

## 性能

`cargo bench -p rustorb-core --bench throughput -- orb`（既定のレベル 1、subscriber なし。4 コアのクラウド環境で、同じマシンで前後を続けて測った）。

前と後を交互に 2 回ずつ測った中央値（1 回目 / 2 回目）。

| ベンチ | 前 | 後 |
| --- | --- | --- |
| `echo_long` | 52.7 / 52.9 µs | 52.3 / 51.5 µs |
| `echo_octets_1MiB` | 1.131 / 1.154 ms | 1.093 / 1.157 ms |
| `echo_octets_shared_1MiB` | 0.902 / 0.878 ms | 0.865 / 0.957 ms |
| `echo_long_connections/16` | 211.7 / 226.6 µs | 222.6 / 223.0 µs |

同じコードを続けて測っても 5〜10% 動く環境で、前後の差はその範囲に収まっている。

## 試験

- `crates/rustorb-core/tests/trace.rs`：subscriber に出来事が届くこと（レベルとターゲット）、subscriber がないとき `-ORBtraceLevel` が標準エラーに書くこと（自分自身を子プロセスとして起動して確かめる）、`-ORBtraceTime`、既定では何も出ないこと
- `python/tests/test_trace.py`：ハンドラーなしで標準エラーへ、`logging.basicConfig` 後はロガーへ、`rustorb.set_trace_level`、既定では何も出ないこと
