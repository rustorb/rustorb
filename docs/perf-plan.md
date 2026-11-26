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

- 各段の PR：クラウドで `cargo bench -p rustorb-core`（`--save-baseline` で前後）と、段 1 で足す短い比較の組。1 回あたりのコンテキストスイッチは `perf stat -e context-switches`、システムコールは `strace -c -f`
- 段 10：実機とクラウドで interop/bench を全部回し直す（[比較の再現のしかた](omniorb-comparison.md#再現のしかた)）

プロファイルの手順（クラウド）：

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
