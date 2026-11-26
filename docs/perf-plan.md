# 速度の改善の計画（omniORB との差を縮める）

Nov 26, 2026 · @ysuga · tracking issue [#203](https://github.com/rustorb/rustorb/issues/203)

[omniORB との比較](omniorb-comparison.md)（#201、実機の結果は #202）で見えた速度の差を縮める計画。差の見立てを、クラウドでの予備のプロファイルで確かめ直し、段に分けた。各段は sub-issue（#204〜#213）にしてある。

omniORB はバイナリとしてだけ使い、ソースは読まない（[clean-room 方針](clean-room-policy.md)）。設計の根拠は自分たちの計測と、OS・tokio の公開の文書だけにする。

## 目標

実機（MacBook Pro、Apple M5 Max、[比較の環境](omniorb-comparison.md#環境実機)）での値。

| 項目 | rustorb（今） | omniORB C++ | 目標 | 段 |
| --- | ---: | ---: | ---: | --- |
| ping 往復 p50 | 21.4 µs | 12.5 µs | 15 µs 以下 | 2・4 |
| CPU / ping（クライアント・サーバー） | 12.5・12.2 µs | 4.1・4.3 µs | それぞれ 6 µs 以下 | 2・4 |
| サーバーの oneway | 19.2 万回/s（CPU 17 µs） | 65.4 万回/s（2 µs） | 40 万回/s | 3 |
| 一つの参照を 4 スレッドから | 9.4 万回/s | 20.5 万回/s | 18 万回/s | 5 |
| 同一プロセスの ping | 1.29 µs | 0.04 µs | 0.3 µs（core）、0.1 µs（生成コード） | 6・7 |
| クライアントの 1 MiB 受信（`get`、omniORBpy のサーバー） | 7.7 GB/s | 11.8 GB/s | 10 GB/s | 8 |

rustorb が速いところ（起動、最初の呼び出し、起動時のメモリ、大きなデータの echo）は落とさない。各段の PR で前後を測る。

## 予備のプロファイル

クラウドの 4 コアの仮想マシンで、`rustorb-bench serve` と `rustorb-bench client`（release、`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`）を `perf record --call-graph dwarf`、`perf stat`、`strace -c`、`perf trace -s` で見た。数字は仮想マシンのもので、カーネルの割合は実機より大きく出ている見込み。

### 基準の値（クラウド、今の main）

| ケース | 値 |
| --- | ---: |
| ping p50 | 51 µs |
| oneway | 5.1 万回/s |
| threads:4 | 2.2 万回/s |
| 同一プロセスの ping | 2.98 µs |

### 分かったこと

1. **コストの大半は起床とシステムコール。** ping 1 回でクライアント・サーバーとも約 2.2 回のコンテキストスイッチ（`perf stat`：2 秒で 3.6 万回の呼び出しに 7.9 万回と 8.3 万回）。CPU の約 8 割がカーネルで、ORB 自身のユーザー空間のコード（CDR、ディスパッチ）は 15% ほど。CDR の組み立てを速くしても差はあまり縮まない
2. **I/O スレッドの空振りの起床。** スレッドが読む経路（[read-ownership.md](read-ownership.md)、#66・#67）でも、ソケットは tokio の epoll に登録されたままで、データが届くたびに `rustorb-io` が起こされる。`rustorb-io` が CPU の約 2 割（両側）。カーネルの `sock_def_readable` も、起こす相手が 2 つ（`poll` と epoll）あるので重い。read-ownership.md の「しないこと」で手間に見合わないとした所だが、1 回の往復の大きな部分なので取り直す（段 2）
3. **`recvfrom` の 3 分の 2 が `EAGAIN`。** 読めるか試してから待つ順番の所がある（段 2）
4. **follow は続けて届く要求で逆効果。** サーバーを `-ORBfollowLinger 0` で動かすと、ping は 51→65 µs に遅くなるが、oneway は 5.1 万→7.9 万回/s に速くなる（`-ORBfollowLinger 5000` では 4.9 万回/s）。follow しているスレッドと I/O タスクが同じ接続を取り合い、受け渡しが増えていると見ている。サーバーでは要求が複数の upcall スレッドに散っていた（段 3）
5. **同一プロセスは GIOP を往復している。** `object::local_call` は要求ヘッダとサービスコンテキストを含むメッセージ全体を組み立てて解き、返事も組み立てて解き、`std::sync::mpsc` で待つ（段 6）
6. **ユーザー空間の上位**は `malloc`／`free`、`memmove`、tokio の `CachedParkThread::block_on` とタイマー（`clock_gettime`）、CDR の `align`・`read_octets`、`object::invoke`、`Connection::forget`・`expect_reply`（段 4）

## 段

| 段 | issue | 内容 | 依存 |
| --- | --- | --- | --- |
| 1 | #204 | 計測の土台：短い比較の組、前後の差の表、起床・システムコール・follow などの内部の数 | なし |
| 2 | #205 | 1 回の呼び出しの起床を減らす：スレッドが読む間は epoll から外す、`EAGAIN` を減らす | 1 |
| 3 | #206 | サーバーの oneway と続けて届く要求：follow とタスクの取り合い、プールにまとめて渡す | 2 |
| 4 | #207 | 小さな呼び出しの残りのコスト：確保、`block_on`、要求ごとの組み立て | 2 |
| 5 | #208 | 一つの参照を複数スレッドから：同じサーバーへの接続を増やす | 2 |
| 6 | #209 | 同一プロセス：GIOP を通さない近道（core） | 1 |
| 7 | #210 | 同一プロセス：生成コードの型どうしの直接呼び出し（rustorb-idl） | 6 |
| 8 | #211 | 大きなデータのクライアント側：受信のコピーとバッファ | 1 |
| 9 | #212 | C・Ruby の octet 列 | なし |
| 10 | #213 | 実機で測り直し、比較の文書を更新、リリース | 2〜9 |

段 2 がいちばん効くと見ている（見えている 2.2 回の起床のうち 1 回分）。段 3〜5 は段 2 の後の経路の上で作る。段 6・8・9 は段 2 と独立で、並べて進められる。

## 決めること（推奨で進める）

1. **段 2 の作り方**：スレッドが持ち主になるときに接続を tokio から外し（`TcpStream::into_std` は登録を外す）、返すときに `from_std` で登録し直す形を先に試作する。`epoll_ctl` は呼び出しごとではなく受け渡しごとになる。うまくいかなければ mio を直接使う小さなリアクター（`EPOLLONESHOT`）にする。接続ごとに読むスレッドを張りつける形は、接続数に比例してスレッドが増えるので選ばない
2. **段 5 の既定**：同じサーバーへの接続は、同時の呼び出しの数に応じて 4 本まで増やす。`-ORBmaxConnectionsPerServer 1` で今と同じにできる
3. **段 6・7 の近道は自動で使う**。ポータブルインターセプターが登録されている、POA が ORB_CTRL_MODEL でない、POA マネージャーが active でない、のどれかなら今の経路に戻す
4. **Ruby の octet 列**をバイナリの String で受け渡すのはマッピングの変更なので、選べる形にして既定は今のまま
5. **Windows**（スレッドが読む経路がない）は今回の対象外。段 2 の後も Windows は今の経路

## 測り方

段 1（#204）で用意した物差し。段 2 以降の PR は、変更の前と後でこれを回し、差を PR に書く。

- 各段の PR：クラウドで下の「短い比較の組」と `cargo bench -p rustorb-core`（`--save-baseline` で前後）。起床・システムコールの話をする段では「1 回あたりのコンテキストスイッチとシステムコール」も
- 段 10：実機とクラウドで interop/bench を全部回し直す（[比較の再現のしかた](omniorb-comparison.md#再現のしかた)）

### 短い比較の組

`interop/bench/run-bench.py --quick` は、rustorb どうしと omniORB C++ どうし（ビルドできるもの）で ping・any・oneway・threads:4・get:1048576 と、同一プロセスの ping だけを回す。既定（各ケース 2 秒、3 回の中央値）で 1 分ほど。`compare-bench.py` が 2 回の結果の差を表にする。

```sh
git switch main
interop/bench/run-bench.py --quick --stats --out /tmp/before
git switch my-branch
interop/bench/run-bench.py --quick --stats --out /tmp/after
interop/bench/compare-bench.py /tmp/before /tmp/after --counters
```

`compare-bench.py` は p50・calls/s のほか、CPU 時間とコンテキストスイッチのうち `--threshold`（既定 5%）より動いたものを、良くなった（+）・悪くなった（−）の印つきで出す。`--all` ですべて出す。omniORB C++ の行は、マシンのぶれを見る目安にもなる（omniORB は変わらないので、その行が動いた分はぶれ）。

**ぶれ**：クラウドの 4 コアの仮想マシンでは、同じコードを続けて 2 回回しても（各 0.5 秒、2 回の中央値）、oneway・threads:4・get で 10% ほど違った。既定の長さで回し、10% より小さい差は、コンテキストスイッチや下の内部の数が同じ向きに動いているときだけ効果と見る。`--stats` で数えるのを入れた回と入れない回の差も、この範囲で向きがそろわなかった（ping p50 は数えた回が 10% 速く、any は 7% 遅い）。数えること自体の重さは見えない。

run-bench.py が記録するもの（`--quick` でなくても）：

- **1 回あたりのコンテキストスイッチ**（自発・非自発の合計）。クライアントは `wait4` の `ru_nvcsw + ru_nivcsw`（Linux・macOS。何もしないクライアントの分を引く）、サーバーは `/proc/PID/task/*/status`（Linux のみ）。サーバーはスレッドの名前ごとにも分ける（`server.csw[rustorb-io]`、`server.csw[rustorb-upcall]` など）。終わったスレッドの分は数えない
- **`--stats`**：rustorb のクライアントとサーバーの内部の数（下）の 1 回あたり。クライアントは `rustorb-bench client --stats` が各ケースの後に出す行、サーバーは `rustorb-bench serve --stats-file PATH` が 50 ms ごとに書くファイルを、ケースの前後で読む

### ORB の内部の数

`rustorb_core::stats` に、プロセス全体の数を足した。既定では数えない（数えないときは 1 回の relaxed な読み込みだけ）。`-ORBstatistics 1` か `stats::set_enabled(true)` で数え始め、`stats::snapshot()` の差（`Snapshot::since`）を取る。`Snapshot` の `Display` は JSON の 1 行。

| 数 | 意味 |
| --- | --- |
| `task_wakeups` | 接続の I/O タスクが読むために起きた（読めるようになった、またはスレッドが読む役を返した） |
| `task_wakeups_thread_reads` | そのうち、スレッドが読んでいる最中だった（空振り） |
| `task_wakeups_busy_empty` | アップコールのスレッドが要求を処理中に起き、読むものがなかった |
| `task_messages` | I/O タスクが読んだメッセージ |
| `recv_calls`・`recv_would_block` | ソケットの `recv`（タスクとスレッドの両方）と、そのうち何もなかった（`EAGAIN`）もの |
| `thread_polls` | スレッドがソケットを `poll` で待った |
| `claims`・`claim_failures` | スレッドが読む役を取れた・取れなかった（I/O タスクが読んでいた） |
| `thread_messages`・`thread_read_timeouts` | スレッドが自分で読んだメッセージ、待ち時間が尽きた回数 |
| `hand_backs` | スレッドが、読み残しがあって読む役をタスクに返した（タスクを起こす） |
| `follows`・`follow_requests` | サーバー：アップコールのスレッドが要求の後に接続を読み始めた回数、そうして読んだ要求 |
| `follow_stops_queued`・`follow_stops_limit` | サーバー：follow を、プールに仕事が待っていてやめた・`-ORBfollowLimit` に達してやめた |
| `follow_pauses`・`follow_overdue` | サーバー：I/O タスクが要求を渡したスレッドに読みを譲った（#206）・そのスレッドの処理が `-ORBfollowLinger` より長引いてタスクに戻した |
| `replies_read_here`・`replies_from_task` | クライアント：返事を呼び出したスレッドが自分で読んだ・I/O タスクから受け取った |
| `pool_jobs`・`pool_wakes`・`pool_spawns`・`pool_queued`・`pool_sleeps` | アップコールのプール：渡した仕事、それで起こしたスレッド、新しく作ったスレッド、空きがなく待たせた仕事、仕事がなくて眠ったスレッド |
| `outbox_direct`・`outbox_queued` | サーバー：返事を作ったスレッドがそのまま書けた・書き込みタスクに回した |

I/O タスクの `task_wakeups` は、tokio のタスクが起こされた回数で、I/O スレッド（`rustorb-io`）が epoll から起きた回数ではない。後者はサーバーの `server.csw[rustorb-io]` で見る。

### 1 回あたりのコンテキストスイッチとシステムコール

Linux（クラウド）。`perf` の入れ方は下の「プロファイルの手順」。回数を ping の総数（クライアントの JSON 行の `total_calls`）で割る。

```sh
target/release/rustorb-bench serve -ORBendPoint giop:tcp:127.0.0.1: > ior.txt &
SRV=$(pgrep -x rustorb-bench)
# サーバー：クライアントが走る 3 秒の間
perf stat -e context-switches,cpu-migrations,raw_syscalls:sys_enter -p "$SRV" -- sleep 3 &
# クライアント：全体（warm-up なし）
perf stat -e context-switches,cpu-migrations,raw_syscalls:sys_enter -- \
  target/release/rustorb-bench client --ior "$(cat ior.txt)" --cases ping --seconds 2.5 --warmup 0
# システムコールの内訳（strace は遅くするので、比べるのは回数の比だけ）
strace -c -f -o cli.strace target/release/rustorb-bench client --ior "$(cat ior.txt)" \
  --cases ping --seconds 1 --warmup 0
strace -c -f -p "$SRV" -o srv.strace &   # 終わったら kill -INT
perf trace -s -p "$SRV" -- sleep 3       # strace より軽い内訳
```

スレッドごとの起床は `perf stat --per-thread -e context-switches -p "$SRV"`、または run-bench.py の `server.csw[...]`。

macOS（実機）。`perf` も `/proc` もない。run-bench.py はクライアントのコンテキストスイッチだけ記録する。サーバーとシステムコールは DTrace か Instruments で見る（SIP が DTrace を止めているときは Instruments）。この手順は実機ではまだ確かめていない。段 10 で確かめて直す。

```sh
# システムコールの数（サーバー、3 秒）
sudo dtrace -p "$SRV" -n 'syscall:::entry /pid == $target/ { @[probefunc] = count(); } tick-3s { exit(0); }'
# スレッドが CPU を離れた回数（コンテキストスイッチ）
sudo dtrace -p "$SRV" -n 'sched:::off-cpu /pid == $target/ { @[tid] = count(); } tick-3s { exit(0); }'
# Instruments（System Trace）
xcrun xctrace record --template 'System Trace' --attach "$SRV" --time-limit 3s --output srv.trace
```

### cargo bench

`crates/rustorb-core/benches/throughput.rs` に、`orb/oneway`（クライアントが送れるだけ送り、サーバントが全部を処理するまで）、`orb/echo_long_threads_one_ref/4`（一つの参照を 4 スレッドから）、`orb/echo_long_in_process`（同じ ORB の中）を足した。

```sh
cargo bench -p rustorb-core --bench throughput -- --save-baseline before 'orb/'
# 変更の後
cargo bench -p rustorb-core --bench throughput -- --baseline before 'orb/'
```

### 基準の値（段 1 の時点の main）

予備のプロファイルの値（ping 51 µs、oneway 5.1 万回/s、threads:4 2.2 万回/s、同一プロセス 2.98 µs）が、段 2 以降が比べる基準。同じ種類のクラウドの仮想マシン（4 コア）で、段 1 の物差しで測り直した値も残す（`run-bench.py --quick --stats`、各 2 秒、3 回の中央値。omniORB は入れていない）。仮想マシンは毎回違うので、各段の PR では基準をその場で測り直して比べる。

| ケース | 値 | クライアントの CPU／コンテキストスイッチ（1 回あたり） | サーバーの CPU／コンテキストスイッチ（1 回あたり） |
| --- | ---: | ---: | ---: |
| ping p50 | 54.6 µs | 41.0 µs／1.99 | 49.2 µs／1.79 |
| any p50 | 79.1 µs | 54.5 µs／2.00 | 60.5 µs／2.00 |
| oneway | 48,305 回/s | 2.85 µs／0.00 | 33.2 µs／2.00 |
| threads:4 | 23,917 回/s | 52.7 µs／2.60 | 43.6 µs／2.15 |
| get:1048576 | 1,533 MB/s | 392 µs／8.54 | 485 µs／2.05 |
| 同一プロセスの ping p50 | 3.41 µs | | |

内部の数で、段 2・3 に効くもの（1 回あたり）：

- ping：サーバーの `rustorb-io` が 1 回あたり 0.99 回眠って起きる（空振りの起床）。tokio のタスクまで起こされるのは 0.26 回で、残りは I/O スレッドが epoll から起きるだけ。`recv` は両側とも 3 回で、うち 2 回が `EAGAIN`（読む前の試しと、読んだ後の残りの確認）
- oneway：サーバーで要求ごとにプールへの受け渡し（`pool_jobs` 1.0）とスレッドの起床（`pool_wakes` 1.0）。follow は要求ごとに始まるが、`claim` の 0.89 回が失敗し（I/O タスクが読んでいる）、follow で読めた要求はない。`rustorb-upcall` が 1 回あたり 1.88 回のコンテキストスイッチ
- threads:4：クライアントの返事はほぼすべて I/O タスク経由（`replies_from_task` 1.00）
- get:1048576：クライアントで I/O タスクが 1 回あたり 5.5 回起き、`recv` の 5.6 回が `EAGAIN`

### プロファイルの手順（クラウド）

```sh
apt-get install linux-tools-6.8.0-71-generic
ln -sf /usr/lib/linux-tools/6.8.0-71-generic/perf /usr/local/bin/perf
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only cargo build --release -p rustorb-interop --bin rustorb-bench
target/release/rustorb-bench serve -ORBendPoint giop:tcp:127.0.0.1: > ior.txt &
perf record -F 4999 --call-graph dwarf,16384 -p "$(pgrep -x rustorb-bench)" -o srv.data -- sleep 4 &
perf record -F 4999 --call-graph dwarf,16384 -o cli.data -- \
  target/release/rustorb-bench client --ior "$(cat ior.txt)" --cases ping --seconds 3
perf report -i srv.data --no-children --sort comm,sym --stdio -g none
```
