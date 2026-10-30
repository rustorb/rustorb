# 負荷・長時間試験とリーク検出

issue #49。計画書のテスト6（[development-plan.md](development-plan.md#テストと相互接続検証)）「負荷・長時間試験：同時接続数、リクエストレート、接続断と再接続、メモリリーク」と、リスク表の「言語間のオブジェクト参照の寿命管理」（各言語のテストでリークを検出）の対策。

どの試験も、毎回の CI では短く、定期実行（`.github/workflows/soak.yml`、毎週月曜と手動）では長く回す。長さは環境変数で変える。

| 試験 | ファイル | 毎回（CI） | 定期実行 |
| --- | --- | --- | --- |
| 負荷（Rust コア） | `crates/rustorb-core/tests/stress.rs` | `cargo test`（ci.yml、3 OS） | `RUSTORB_STRESS_SCALE=10`、release |
| 長時間 | `crates/rustorb-core/tests/soak.rs` | 5 秒 | `RUSTORB_SOAK_SECS=1800`（手動実行では秒数を指定できる） |
| リーク（Python） | `python/tests/test_leak.py` | ci.yml の Python ジョブ | `RUSTORB_LEAK_ROUNDS=20` |
| リーク（C++） | `cpp/tests/test_leak.cc` と全 C++ テスト | ci.yml の `cpp-sanitize`（ASan / LSan） | `RUSTORB_LEAK_ROUNDS=20` |
| リーク（Java） | `java/src/test/java/org/rustorb/test/LeakTest.java` | ci.yml の Java ジョブ | `RUSTORB_LEAK_ROUNDS=20` |
| omniORB との負荷 | `interop/run-load.sh` | interop.yml（各フェーズ 2 秒） | 各フェーズ 20 秒、16 スレッド、64 接続。結果はジョブのサマリーと成果物に残す |

## 負荷試験（Rust コア）

同じプロセスの ORB どうしを TCP（ループバック）でつなぐ。接続断と通信の停止は、テストの中の TCP 中継（`Proxy`）で作る。

| テスト | 確かめること |
| --- | --- |
| `many_clients_each_on_its_own_connection` | 32 個のクライアント ORB（それぞれ自分の接続）から同時に呼ぶ。全部の結果が正しく、サーバントの呼び出し数が合う |
| `many_threads_share_one_connection` | 16 スレッドが一つの接続で `echo_long`・`echo_string`・oneway を混ぜて呼ぶ（多重化）。oneway も失われない |
| `large_messages_in_both_directions` | 0 バイトから 16 MiB までの octet 列を GIOP 1.0・1.1・1.2 で同時に往復させる |
| `oversized_requests_fail_and_the_server_carries_on` | サーバーの `max_message_size` を超える要求は例外になり、サーバーはそのまま次の要求に答える |
| `nested_calls_within_the_pool_size_complete` | 二つの ORB のサーバントが呼び合う入れ子呼び出しが、プールの大きさに収まる限り終わる |
| `an_exhausted_pool_times_out_and_recovers` | 入れ子呼び出しでプールを使い切る（下記）。呼び出しは止まったままにならずタイムアウトし、その後プールは元に戻る |
| `nested_calls_within_one_orb_do_not_use_the_pool` | 同じ ORB の中の入れ子呼び出しは呼び出し元のスレッドで動くので、プールが 2 でも 16 クライアントから深さ 16 の再帰が通る |
| `clients_reconnect_after_the_server_restarts` | 呼び続けている間にサーバーを同じポートで再起動する（`ORB::destroy` で `CloseConnection` を送って閉じる）。再起動のたびに呼び出しが戻る |
| `calls_in_flight_end_when_the_client_shuts_down` | 呼び出し中にクライアント ORB を止めると、どの呼び出しも例外で終わる |
| `dropped_connections_fail_the_calls_on_them_and_the_next_ones_reconnect` | 中継が接続を切る（ネットワーク断）。その接続にあった呼び出しは `COMM_FAILURE`、次の呼び出しは新しい接続で通る |
| `a_stalled_network_times_out_and_the_connection_recovers` | 中継がしばらく何も流さない。呼び出しは `TIMEOUT` になり、流れが戻れば同じ接続で通る（遅れて届いた返事は捨てる） |
| `a_stalled_connection_does_not_hold_up_the_others` | 相手が読まなくなった接続（要求の向きと返事の向きのそれぞれ）が、ほかの接続も、その後の呼び出しも止めない |

各テストは件数と所要時間、毎秒の呼び出し数を標準エラーに出す（`cargo test -- --nocapture` で見える）。

### プールの枯渇

二つの ORB A・B のサーバントが `relay(d)` で互いを呼ぶと、深さ d の呼び出しは A と B のアップコール用スレッドを交互に合わせて d + 1 本、戻るまで占める。プールの上限（`max_server_threads`）を超えると、空きを待つ要求がキューに並び、並んでいる要求の結果を待つスレッドがプールを埋めたままになる。これは設計どおりで（[poa-design.md](poa-design.md) の「枯渇の扱い」）、rustorb はプールを増やしたりはしない。確かめるのは、呼び出しのタイムアウトが設定されていれば全員が `TIMEOUT` で戻り、内側の呼び出しもタイムアウトして解けるので、その後プールが元どおり答えることである。タイムアウトがなければ止まったままになるので、別の ORB と呼び合うサーバーではプールの上限を入れ子の深さに合わせるか、タイムアウトを設定する。

### 試験で見つかって直した不具合

- **送信が詰まるとタイムアウトが効かなかった**：相手が読まなくなって TCP の送信バッファが埋まると、要求の書き込みが終わらず、呼び出しのタイムアウトを過ぎても戻らなかった。書き込み用の錠も持ったままなので、同じ接続のほかの呼び出しも止まった。書き込み（錠を待つ間も）に呼び出しの締め切りをかけた。書き始める前に切れたら `TIMEOUT`（COMPLETED_NO）で接続はそのまま、書きかけで切れたらメッセージの区切りが壊れるので接続を閉じる（[timeouts.md](timeouts.md)）。タイムアウト後の `CancelRequest` と、ORB を止めるときの `CloseConnection` の書き込みにも上限（100 ms、2 秒）をつけた

### 記録しておく挙動

- **再起動中の `OBJ_ADAPTER`**：サーバーの `ORB::shutdown` が始まった後、まだ閉じていない接続に届いた要求は、POA マネージャーが非活性なので `OBJ_ADAPTER`（minor `POA_INACTIVE`）になる。クライアントには再試行してよいか分からない。CORBA の規定では非活性の POA マネージャーは `OBJ_ADAPTER` で断るので今はそのままにし、試験では許す例外に入れている。omniORB が同じ場面で何を返すかは外から測っていない

## 長時間試験

`soak.rs` は1つのテストだけを持つファイルで、プロセスの中でほかのテストと混ざらない。一周ごとに、クライアント ORB（自分の接続と tokio ランタイム）を作り、4 スレッドから呼び、oneway を送り、サーバー側で子 POA を作ってオブジェクトを活性化・呼び出し・非活性化し、POA を壊し、クライアント ORB を `destroy` する。

最初の 2 割（最低 10 周）を慣らしにして、その後の常駐メモリ（`VmRSS`）、ファイル記述子（`/proc/self/fd`）、スレッド数（`/proc/self/status`）を見る。

- ファイル記述子とスレッド数は、慣らしの後と最後でほぼ同じ（それぞれ +2 まで）。プールの最小数を超えたスレッドはアイドルで終わるので、測る前に 0.5 秒待つ
- 常駐メモリは、走行の 6 割の時点から最後までの増加が、その時点の 1 割（最低 8 MiB）まで

/proc がない macOS と Windows では、負荷だけかけて数値は見ない。手元で 150 秒走らせたときは、慣らしの後 6.7 MiB、6 割の時点 6.9 MiB、最後 7.0 MiB、記述子 8・スレッド 6 のまま（ピークは 21・14）だった。

## リーク検出

### Python

`python/tests/test_leak.py`。サーバーとクライアントの ORB を同じプロセスに作り、次を何百回も繰り返す。

- 引数と結果（文字列・バイト列・列・構造体の列）：リモート呼び出しと同一 ORB 内の呼び出しの後で、渡したオブジェクトの参照カウント（`sys.getrefcount`）が元のまま
- 例外：ユーザー例外とシステム例外を受けた後、例外オブジェクトが残っていない
- サーバント：`activate_object` / `servant_to_reference` で活性化して呼び、非活性化したサーバントと、POA ごと壊したサーバントが、弱参照で見て解放されている
- オブジェクト参照：`string_to_object` と narrow と参照の受け渡しを繰り返した後、参照オブジェクトの数が元のまま
- TypeCode と any：実行時に作った再帰型の TypeCode と any の往復を繰り返した後、any が残っていない

ネイティブ側のメモリは常駐メモリで見る（後半の増加が 5%、最低 4 MiB まで。Linux のみ）。

### C++

`cpp/tests/test_leak.cc` は、サーバント（数えて、非活性化と POA の破棄と ORB の破棄で 0 に戻ること）、オブジェクト参照、ユーザー例外とシステム例外、実行時に作る再帰型の TypeCode、AnyTest のすべての any の値を繰り返し作って捨てる。サーバント以外は LeakSanitizer が見る：`-DRUSTORB_SANITIZE=address` で全 C++ テストを AddressSanitizer つきでビルドし、終了時に LSan が解放されていないメモリを報告する。librustorb_ffi（Rust）も同じ malloc を使うので、C++ 側がハンドルを手放し忘れたときのコア側のメモリも見つかる。

`test_value` と `test_dynany` は、循環した valuetype のグラフをわざと作る（共有ポインタの C++ マッピングもコアの `Arc` も循環を解放しない。[valuetype.md](valuetype.md)）ので、この二つはリーク検出を切って（`ASAN_OPTIONS=detect_leaks=0`）メモリ誤用の検出だけ行う。手元で LSan が報告したのはこの二つの循環だけだった。

### Java

ネイティブのハンドルは Java のオブジェクトが持ち、オブジェクトが回収されると `Cleaner` が解放する（`org.rustorb.Owned`）。今いくつ持っているかを `org.rustorb.Diagnostics.liveHandles()` で読めるようにした。`LeakTest` は、オブジェクト参照・ユーザー例外とシステム例外・実行時に作る TypeCode・any の往復を繰り返した後、GC を促してハンドル数が慣らしの後の値まで戻ること、非活性化したサーバントが弱参照で見て回収されることを確かめる。

## omniORB を相手にした性能の目安

`interop/run-load.sh` は、omniORB のイメージのコンテナで Echo サーバー（omniORB C++、omniORBpy、rustorb の Rust と Python）を一つずつ起動し、負荷クライアント（rustorb の Rust 版 `impls/rustorb/src/load_client.rs`、omniORBpy と rustorbpy の `scripts/load-client`）で順に呼ぶ。ネーミングは omniNames。フェーズは次のとおりで、失敗した呼び出しが一つでもあればスクリプトは失敗する。毎秒の呼び出し数は記録するだけで、比べて合否は決めない（マシンで変わる）。

| フェーズ | 内容 |
| --- | --- |
| small | `echo_long` を N スレッドから（一つの接続） |
| string | 100 文字の `echo_string` を同じく |
| connections | `echo_long` を N 個のクライアント ORB から（接続も N 本。Rust 版のみ） |
| large | 1 MiB の `echo_octets` を 4 スレッドから |

手元（4 コア、全コンテナが同じマシン、8 スレッド・16 接続・各フェーズ 3 秒、release ビルド）での毎秒の呼び出し数。12 組すべてエラーなし。

| client | server | small | string | connections | large (1 MiB) |
| --- | --- | ---: | ---: | ---: | ---: |
| rustorb | omniorb-cpp | 32826 | 31671 | 153915 | 1157 |
| omniorbpy | omniorb-cpp | 33424 | 25459 | - | 3161 |
| rustorbpy | omniorb-cpp | 32749 | 33443 | - | 1249 |
| rustorb | omniorbpy | 33186 | 32525 | 14868 | 1445 |
| omniorbpy | omniorbpy | 31255 | 22898 | - | 3581 |
| rustorbpy | omniorbpy | 33403 | 33632 | - | 1444 |
| rustorb | rustorb | 33271 | 33859 | 44342 | 816 |
| omniorbpy | rustorb | 32126 | 31456 | - | 1884 |
| rustorbpy | rustorb | 32180 | 32231 | - | 907 |
| rustorb | rustorbpy | 15346 | 15265 | 15251 | 807 |
| omniorbpy | rustorbpy | 15727 | 15082 | - | 1817 |
| rustorbpy | rustorbpy | 14867 | 14850 | - | 836 |

- small と string はどの組も毎秒 3.3 万回前後で頭打ちになり、ここでは差が見えない（クライアントとサーバーが同じ 4 コアを取り合う）。rustorbpy のサーバーは GIL のため半分ほど
- connections（16 接続）は、omniORB C++ のサーバーが毎秒 15 万回なのに対し rustorb（Rust）のサーバーは 4.4 万回で、3.5 倍の差がある。接続ごとの読み込みからアップコール用プールへの受け渡しが重いと見ている
- large（1 MiB）は、omniORBpy どうしの毎秒 3,600 回に対し、rustorb どうしは 800 回ほど。rustorb がクライアントでもサーバーでも遅く、メッセージの組み立てと読み込みでのコピーが多いと見ている

どちらも正しさの問題ではないので、#49 では記録にとどめ、#64 で詰めた（次の節）。上の表は #64 の前の数字。

## 性能の改善（#64）

### 測り方

Docker を使わず、同じマシンで `rustorb-names`・`rustorb-interop-server`・`rustorb-load-client` をネイティブに動かして測った（4 コアのサンドボックス、8 スレッド・16 接続、各フェーズ 4 秒、release ビルド、3 回の平均）。`rustorb-load-client --phases connections,large` のように、一部のフェーズだけを回せる。どこで時間を使っているかは `perf record --call-graph dwarf` でサーバーとクライアントを別々に取った。

### 見つかったことと直したこと

大きなメッセージ（1 MiB の `echo_octets`）では、サーバーの CPU の 3 割強が `memmove` だった。受け取ったメッセージは、ソケットから読んだ後にさらに 3 回コピーされていた。

1. 読み込み用のバッファをゼロで埋めてから読んでいた
2. `FragmentAssembler::push` が、フラグメントでないメッセージもコピーしていた
3. 復号した `Body` が本体のバイト列をコピーしていた

クライアントでは、返事の要求 ID を見るために読み込みタスクで一度復号し、呼び出し元のスレッドでもう一度復号していたので、`Body` のコピーが 2 回あった。

- 読み込みは、ゼロで埋めないバッファに直接読む（`wire::read_message`）。受信側には 16 KiB の先読みを付け、小さいメッセージは本体と次のヘッダが 1 回の読み込みで入るようにした（それまでは 1 メッセージに 2 回）
- `FragmentAssembler::push_owned` は読み込んだバッファをそのまま持ち、フラグメントは最初のフラグメントのバッファにつなぐ
- `AssembledMessage::into_decoded` で復号すると、`Body` はバッファを共有する（`Arc`）。`Body` の複製も安い
- クライアントの読み込みタスクは、復号した `Message` を呼び出し元に渡す（復号は 1 回）

多接続（16 接続から `echo_long`）では、サーバーの時間の多くがアップコール用プールへの受け渡し（スレッドを起こす futex と切り替え）と、プールの錠の取り合いだった。

- プールは、待っているスレッドを起こすのを錠を放した後にした。起こされたスレッドが錠を待ってまた眠ることがなくなった
- 返事は、作ったスレッドがソケットにノンブロッキング（`try_write`）で直接書く。前に溜まった返事がなく、ソケットが全部受け取れればそれで終わる。受け取りきれなかった分と、その後に来た返事は、接続ごとの書き込みタスクがソケットの空きを待って書く（`server::Outbox`）。それまでは返事ごとに mpsc で書き込みタスクに渡していたので、1 回の呼び出しでスレッドの受け渡しが 1 回減った。書くのは錠の中か、ストリームを持っている書き込みタスクだけなので、メッセージが混ざることはない

### 結果

毎秒の呼び出し数（上の測り方）。「サーバーだけ新」は改善前のクライアントから改善後のサーバーを呼んだもの。

| | small | string | connections | large (1 MiB) |
| --- | ---: | ---: | ---: | ---: |
| 改善前 | 42044 | 42208 | 57376 | 971 |
| 改善後 | 48372 | 47741 | 87606 | 1505 |
| サーバーだけ新 | 44714 | 43184 | 87813 | 1030 |
| クライアントだけ新 | 43659 | 43643 | 56381 | 1275 |

- connections は 1.5 倍。ほぼサーバー側の改善（プールの起こし方と直接の書き込み）
- large は 1.55 倍。クライアントとサーバーの両方でコピーが減った分
- small と string は 1.15 倍ほど。クライアントとサーバーが同じ 4 コアを取り合っているので、この測り方ではあまり差が出ない

### ベンチマーク

`crates/rustorb-core/benches/throughput.rs`（criterion）。`cargo bench -p rustorb-core` で回す。criterion は前回の結果を `target/criterion` に残し、次の実行でその差を出すので、変更の前後で回せば遅くなったかが分かる（名前を付けて比べるには `-- --save-baseline before` と `-- --baseline before`）。CI はビルドだけ行う（`cargo build --all-targets`）。共有のランナーでは数字がぶれるので、合否には使わない。

| ベンチマーク | 内容 | 手元の値（改善後） |
| --- | --- | ---: |
| `giop/assemble_and_decode_1MiB` | 1 MiB の要求を `push_owned` と `into_decoded` で復号し、octet 列を読む（ネットワークなし） | 0.5 µs |
| `orb/echo_long` | 同じプロセスの ORB どうしで 1 回ずつ呼ぶ | 65 µs |
| `orb/echo_octets_1MiB` | 1 MiB の octet 列の往復 | 0.75 ms |
| `orb/echo_long_connections/16` | 16 個のクライアント ORB から同時に呼ぶ | 毎秒 8.6 万回 |

`giop/assemble_and_decode_1MiB` はコピーがないので 1 MiB の大きさによらない。コピーが戻ればここが 100 倍ほど遅くなる。コピーしないことは `rustorb-giop` のテスト（`owned_messages_are_decoded_without_copying`）でも確かめている。

### まだ残っている差

- **多接続**：今いちばん時間を使っているのは、I/O スレッドからアップコール用プールへの受け渡し（サーバーの CPU の 3 割ほど）。サーバントを tokio のスレッドで動かさない設計（[poa-design.md](poa-design.md)）から来るもので、omniORB C++ のように接続ごとのスレッドが読んでそのままアップコールする形と比べると、1 回の呼び出しでスレッドの切り替えが 1 回多い。詰めるなら、アップコールを終えたスレッドが同じ接続の次の要求を自分で読む（leader/follower）形が考えられる
- **大きなメッセージ**：残っているコピーは、カーネルとのやり取りと、言語マッピングの値（`Vec<u8>` など）への復号と、送るメッセージへの書き出しで、どれも今の API では必要なもの。もう一段減らすには、octet 列を借用のまま渡す API が要る
- omniORB C++ / omniORBpy と比べた新しい数字は、Docker が使えるマシンで `interop/run-load.sh` を回して取る（CI の interop.yml の負荷の短い版がジョブのログに出す）

## 残り

- 多接続で、I/O スレッドからアップコール用プールへの受け渡しが omniORB C++ より 1 回多い（上の「まだ残っている差」）
- 再起動中の `OBJ_ADAPTER` について、omniORB が同じ場面で何を返すかを測る
- 循環した valuetype を手放す API（コアの `Value::break_cycles` に当たるもの）を C++ と Java にも出すかどうか
