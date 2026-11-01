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
| 手元のマシン（ネイティブ） | `interop/run-load-local.py` | — | 3 OS で負荷・stress・soak（300 秒）。結果はジョブのサマリーと成果物に残す |

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

- **再起動中の `OBJ_ADAPTER`**：サーバーの `ORB::shutdown` が始まった後、まだ閉じていない接続に届いた要求は、POA マネージャーが非活性なので `OBJ_ADAPTER`（minor `POA_INACTIVE`）になる。クライアントには再試行してよいか分からない。CORBA の規定では非活性の POA マネージャーは `OBJ_ADAPTER` で断るので、そのままにし、試験では許す例外に入れている。omniORBpy 4.3.4 のサーバーも同じ場面で同じ例外を返す（#86 で外から測った）。200 ms かかる操作を 4 スレッドから呼び続け、サーバーが 1 秒後に `orb.shutdown(True)` を呼ぶと、クライアントには `OBJ_ADAPTER`（minor `0x4f4d0001`、つまり `OMG_VMCID | 1` で rustorb の `POA_INACTIVE` と同じ値、COMPLETED_NO）が数回、処理中の呼び出しに `COMM_FAILURE`（COMPLETED_MAYBE）、その後は接続できずに `TRANSIENT`（COMPLETED_NO）が返った

## 長時間試験

`soak.rs` は1つのテストだけを持つファイルで、プロセスの中でほかのテストと混ざらない。一周ごとに、クライアント ORB（自分の接続と tokio ランタイム）を作り、4 スレッドから呼び、oneway を送り、サーバー側で子 POA を作ってオブジェクトを活性化・呼び出し・非活性化し、POA を壊し、クライアント ORB を `destroy` する。

最初の 2 割（最低 10 周）を慣らしにして、その後の常駐メモリ、ファイル記述子、スレッド数を見る。OS ごとの読み方は次のとおり（#63 で macOS と Windows を足した）。

| OS | 常駐メモリ | ファイル記述子 | スレッド数 |
| --- | --- | --- | --- |
| Linux | `/proc/self/status` の `VmRSS` | `/proc/self/fd` | `/proc/self/status` の `Threads` |
| macOS | `proc_pidinfo(PROC_PIDTASKINFO)` の `pti_resident_size` | `proc_pidinfo(PROC_PIDLISTFDS)` | `pti_threadnum` |
| Windows | `K32GetProcessMemoryInfo` の `WorkingSetSize` | ハンドル数（`GetProcessHandleCount`） | Tool Help のスナップショット（`TH32CS_SNAPTHREAD`）でこのプロセスのもの |

- ファイル記述子とスレッド数は、慣らしの後と最後でほぼ同じ（それぞれ +2 まで。Windows はハンドルにイベントなど OS がしばらく持つものも入るので +16 まで）。プールの最小数を超えたスレッドはアイドルで終わるので、測る前に 0.5 秒待つ
- 常駐メモリは、走行の 6 割の時点から最後までの増加が、その時点の 1 割（最低 8 MiB）まで

ほかの OS では、負荷だけかけて数値は見ない。手元で 150 秒走らせたときは、慣らしの後 6.7 MiB、6 割の時点 6.9 MiB、最後 7.0 MiB、記述子 8・スレッド 6 のまま（ピークは 21・14）だった。

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

`test_value` は、サーバントが循環した valuetype のグラフを受け取って返す（共有ポインタの C++ マッピングは、返した結果の循環を解放できない。[valuetype.md](valuetype.md#循環した値の解放)）ので、リーク検出を切って（`ASAN_OPTIONS=detect_leaks=0`）メモリ誤用の検出だけ行う。手元で LSan が報告したのはこの循環だけだった。`test_dynany` は自分で作った循環を切るので、リーク検出をかけて回す（#85 まではコアの Any の中の循環もリークしていた）。

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

- **多接続**：#64 の時点では、I/O スレッドからアップコール用プールへの受け渡しがサーバーの CPU の 3 割ほどを使っていた。#66 で leader/follower にして詰めた（下の「性能の改善（#66〜#69）」）
- **大きなメッセージ**：#64 の時点では、octet 列を言語マッピングの値へ復号するときと、送るメッセージへ書き出すときにコピーがあった。#68 でコアの Rust API と Python からはコピーをなくした（下の節）
- omniORB C++ / omniORBpy と比べた改善後の数字は、PR #65 の CI（interop.yml の負荷の短い版、GitHub Actions のランナー、8 スレッド・16 接続、各フェーズ 2 秒）で次のとおりだった。上の表とはマシンも長さも違うので、比べるのは同じ行の中だけにする。connections は omniORB C++ のサーバー 73,358 回に対し rustorb のサーバー 51,337 回（1.4 倍の差）、large は omniORBpy どうし 4,193 回に対し rustorb どうし 1,406 回（3 倍の差）。手元のマシンで測り直す手段は下の「手元のマシンで回す（#63）」

## 性能の改善（#66〜#69）

#64 の後に残った差を詰めた。測り方は上と同じ `cargo bench -p rustorb-core`（4 コアのサンドボックス、変更前を `--save-baseline before` で保存して比べた）。

| ベンチマーク | 変更前 | 変更後 |
| --- | ---: | ---: |
| `orb/echo_long`（1 回ずつ） | 71 µs | 46 µs |
| `orb/echo_long_connections/16` | 毎秒 7.7 万回 | 毎秒 8.9 万〜9.4 万回 |
| `orb/echo_octets_1MiB`（コピーする API） | 0.79 ms | 0.86 ms |
| `orb/echo_octets_shared_1MiB`（#68 の共有する API） | — | 0.63 ms |

### 読み込みの持ち主の受け渡し（#66、#67）

アップコールを終えたプールのスレッドが同じ接続の次の要求を自分で読み（サーバー、#66）、返事を待つ呼び出し元が自分で返事を読む（クライアント、#67）。設計と守ることは [read-ownership.md](read-ownership.md) にまとめた。Unix だけで効き、Windows では今までどおりタスクが読む。

### octet 列を共有する（#68）

- `rustorb_cdr::Octets`（`rustorb_core::Octets` でも使える）を足した。受け取ったメッセージのバッファを指したまま octet 列を渡し（`Decoder::read_octet_seq_shared`）、送るときは 4 KiB（`SHARE_MIN`）以上の列をメッセージにコピーせず、そのまま書き出す（`Encoder::write_octet_seq_shared`、`OutMessage` と `write_vectored`）。コアの `Value::Octets` もこれを使う
- Python のバインディングは `Octets` から `bytes` を作るときと、その逆の 1 回ずつのコピーだけになった
- C++、Java、`-brust` の生成コードは `Vec<u8>`（や各言語の配列）のままで、復号のときにコピーする。共有する型を各言語のマッピングにどう出すかは別に決める
- 受信バッファの使い回しは入れなかった。大きなメッセージをタスクで読むようにした（[read-ownership.md](read-ownership.md)）後は、確保のコストが計測に出なかった
- コピーしないことは `rustorb-giop` のテスト（`shared_octet_sequences_are_not_copied`）で、共有した列の往復は `tests/stress.rs` の大きなメッセージの試験で確かめている

### 要求ごとの小さなコスト（#69）

- サーバーの返事も `OutMessage` で書き、書き込みタスクは溜まった返事をまとめて `write_vectored` で書く
- 要求を組み立てるバッファを最初に 256 バイト確保し、要求ごとの再確保を減らした。計測ではぶれの範囲で、はっきりした差は出なかった
- ソケットの送受信バッファの大きさを `OrbOptions::socket_send_buffer`、`socket_receive_buffer` で設定できるようにした（既定は OS のまま）。ループバックでは、既定、256 KiB、4 MiB のどれでも 64 KiB で約 154 µs、1 MiB で約 630 µs、8 MiB で約 3.7 ms と差がなかった。実際のネットワークで効くかは「手元のマシンで回す（#63）」の別マシンの測り方で見る
- 多接続のベンチマークを perf で見ると、時間の多くはスレッドの切り替えで、malloc と free は 5% ほどだった。I/O スレッドが CPU の 2 割ほどを使っているのは、スレッドが読んでいる間もソケットが epoll に登録されたままで、届いたデータでタスクが起こされるため（[read-ownership.md](read-ownership.md) の「残り」）

## 手元のマシンで回す（#63）

CI のランナーも開発用のサンドボックスも共有のマシンで、Docker の中でクライアントとサーバーとネーミングが同じコアを取り合うので、上の数字は実機の目安にならない。`interop/run-load-local.py` は、同じ試験を Docker を使わずに自分のマシン（Linux、macOS、Windows）で回し、結果をマシンの情報と一緒に残す。

```sh
# 負荷（全組）と stress と soak（60 秒）
interop/run-load-local.py --idl-parser ../idl_parser
# 長く・大きく。leak は Python・C++（ASan/LSan）・Java のリーク検出
interop/run-load-local.py --tests all --seconds 20 --threads 16 --connections 64 \
  --stress-scale 10 --soak-secs 1800 --leak-rounds 20 --idl-parser ../idl_parser
# 一部だけ
interop/run-load-local.py --tests load --servers rustorb,omniorb-cpp --clients rustorb
```

| 引数 | 内容 |
| --- | --- |
| `--tests` | `load`、`stress`、`soak`、`leak-python`、`leak-cpp`、`leak-java`（`leak` で三つとも、`all` で全部）。既定は `load,stress,soak` |
| `--seconds` `--threads` `--connections` | 負荷の各フェーズの長さ、スレッド数、`connections` の接続数（既定 5 秒、8、16） |
| `--servers` `--clients` | 負荷で使う実装（既定は使えるもの全部） |
| `--stress-scale` `--soak-secs` `--leak-rounds` | `RUSTORB_STRESS_SCALE`（既定 1）、`RUSTORB_SOAK_SECS`（既定 60）、`RUSTORB_LEAK_ROUNDS`（既定 1） |
| `--idl-parser DIR` | idl_parser のチェックアウト（`IDL_PARSER_DIR` でもよい）。C++・Java・Python の実装の生成に使う |
| `--no-build` | ビルド済みのものをそのまま使う |
| `--out DIR` | 結果の置き場所（既定 `interop/out/load-local/日時/`） |

### 負荷の組

ネーミングは `rustorb-names`（`-memory`）。サーバーを一つずつ起動し、そのサーバーに各クライアントを順に当てる。フェーズは上の「omniORB を相手にした性能の目安」と同じで、`connections` は Rust のクライアントだけが回す。

| 実装 | サーバー | 負荷クライアント | 要るもの |
| --- | --- | --- | --- |
| rustorb（Rust） | `rustorb-interop-server` | `rustorb-load-client` | cargo |
| rustorbpy | `impls/rustorbpy/server.py` | `scripts/load-client` | bash、idl_parser |
| rustorbcpp | `impls/rustorbcpp/bin/server` | `bin/load_client`（`load_client.cc`） | bash、C++ コンパイラ、idl_parser |
| rustorbjava | `interop.Server` | `interop.LoadClient`（`impls/java/interop/LoadClient.java`） | bash、JDK、idl_parser |
| omniorb-cpp | `impls/omniorb-cpp/server`（`make` でビルド） | — | `omniidl` が PATH にある omniORB |
| omniorbpy | `impls/omniorbpy/server.py` | `scripts/load-client` | 上に加えて omniORBpy |

omniORB は `omniidl` の場所から接頭辞を求め、その `lib` を `LD_LIBRARY_PATH`（macOS は `DYLD_LIBRARY_PATH`）に、`python`・`lib/pythonX.Y/site-packages` を `PYTHONPATH` に足す。道具が足りない実装は飛ばし、理由を結果に書く。Windows では C++・Java・Python の生成スクリプト（bash）を使わないので、Rust の実装だけを回す。

### 別のマシンに置く

サーバーのマシンで `--serve` を付けると、ネーミング（2809 番）と全サーバー（2810 番から）を、クライアントのマシンから届くアドレスで起動して待つ（Ctrl-C で止める）。

```sh
# サーバーのマシン（192.168.1.10）
interop/run-load-local.py --serve --host 192.168.1.10
# クライアントのマシン
interop/run-load-local.py --server-host 192.168.1.10
```

クライアントのマシンでは負荷だけを回す。サーバーのマシンで束縛されていないサーバー（道具がなくて飛ばしたもの）は、最初のクライアントが 10 秒で見つけられなければ飛ばす。負荷クライアントは Echo が見つからないとき終了コード 2 を返す（Rust・Python・C++・Java とも `--timeout` 秒、既定 30）。

### 出力

`result.json`（マシン・引数・組ごとのフェーズの回数と失敗数と毎秒の回数・試験ごとの結果と時間と標準エラーの要点・飛ばした実装）と、下の欄に貼れる `result.md`、各組と各試験のログを書く。マシンの情報は CPU の名前、論理 CPU 数、メモリ、OS、rustc・Python・Java の版、rustorb のコミット（作業ツリーに変更があれば `-dirty`）。失敗した呼び出しか試験が一つでもあれば終了コードは 1。

soak.yml の `local` ジョブが 3 OS でこのスクリプトを回す（負荷 5 秒・soak 300 秒）。

### 実機の結果

自分のマシンで回したら、`result.md` をここに足す。比べるのは同じ表の中だけにする。

#### 開発用のサンドボックス（参考）

実機ではない。仕組みを確かめるために回したもの。4 コアの共有マシン（Intel Xeon 2.80GHz、16 GiB、Linux）、全プロセスが同じマシン、8 スレッド・16 接続・各フェーズ 1 秒、release ビルド、rustorb b036e4c に #63 の変更を足したもの。omniORB 4.3.4 / omniORBpy 4.3.4 は配布物からビルドして `/opt/omniorb` に入れた。30 組すべてエラーなし。

| client | server | small | string | connections | large (1 MiB) |
| --- | --- | ---: | ---: | ---: | ---: |
| rustorb | rustorb | 16466 | 16113 | 31212 | 451 |
| rustorbpy | rustorb | 15553 | 15816 | - | 481 |
| rustorbcpp | rustorb | 15383 | 16901 | - | 481 |
| rustorbjava | rustorb | 10188 | 13680 | - | 274 |
| omniorbpy | rustorb | 14809 | 13109 | - | 921 |
| rustorb | rustorbpy | 6665 | 6458 | 6842 | 610 |
| rustorbpy | rustorbpy | 6503 | 6201 | - | 597 |
| rustorbcpp | rustorbpy | 7090 | 6372 | - | 599 |
| rustorbjava | rustorbpy | 6614 | 6644 | - | 500 |
| omniorbpy | rustorbpy | 7226 | 7385 | - | 890 |
| rustorb | rustorbcpp | 15300 | 16160 | 30442 | 438 |
| rustorbpy | rustorbcpp | 15677 | 15102 | - | 452 |
| rustorbcpp | rustorbcpp | 15752 | 14906 | - | 432 |
| rustorbjava | rustorbcpp | 14386 | 15237 | - | 363 |
| omniorbpy | rustorbcpp | 15920 | 15248 | - | 1003 |
| rustorb | rustorbjava | 15495 | 15849 | 27880 | 556 |
| rustorbpy | rustorbjava | 14541 | 15210 | - | 572 |
| rustorbcpp | rustorbjava | 15490 | 15518 | - | 629 |
| rustorbjava | rustorbjava | 14668 | 14340 | - | 485 |
| omniorbpy | rustorbjava | 15911 | 14279 | - | 886 |
| rustorb | omniorb-cpp | 18106 | 17601 | 45374 | 730 |
| rustorbpy | omniorb-cpp | 16356 | 18048 | - | 718 |
| rustorbcpp | omniorb-cpp | 18242 | 17974 | - | 766 |
| rustorbjava | omniorb-cpp | 18244 | 17619 | - | 604 |
| omniorbpy | omniorb-cpp | 18121 | 14870 | - | 2317 |
| rustorb | omniorbpy | 16236 | 18084 | 8013 | 922 |
| rustorbpy | omniorbpy | 17476 | 15789 | - | 809 |
| rustorbcpp | omniorbpy | 16596 | 18061 | - | 943 |
| rustorbjava | omniorbpy | 16316 | 15913 | - | 733 |
| omniorbpy | omniorbpy | 16209 | 13502 | - | 2667 |

- small と string は、ここでもどの組も毎秒 1.5 万〜1.8 万回で頭打ちになる（4 コアを全プロセスで取り合う）。rustorbpy のサーバーは GIL のため半分ほど
- connections は omniORB C++ のサーバー 4.5 万回に対し rustorb（Rust・C++・Java）のサーバーは 2.8 万〜3.1 万回
- large は omniORBpy どうしの 2,700 回に対し、rustorb どうしは 300〜600 回。片方を omniORBpy にするとどちら向きでも 750〜1,000 回なので、クライアントとサーバーの両方に差がある

## 残り

- 実機での結果を「手元のマシンで回す」の欄に足す（#63 の仕組みで、開発者のマシンで）

#86 で片づけたもの：

- スレッドが読んでいる間の I/O タスクの起床は、このままにする（理由は [read-ownership.md](read-ownership.md) の「しないこと」）。follow の待ち時間と上限は設定できるようにした（同じく「設定」）
- C++、Java、`-brust` の生成コードでは octet 列を共有する型を出さない。C++ の `std::vector<CORBA::Octet>` と Java の `byte[]` は標準のマッピングで決まっている。`-brust` は `Vec<u8>` のままにし、コピーを避けたいときはコアの `Value::Octets`（`rustorb_core::Octets`）を DII や `Any` で使う
- 再起動中の `OBJ_ADAPTER` は、omniORBpy のサーバーも同じものを返すと分かったので、そのままにする（上の「記録しておく挙動」）
- Windows では、スレッドがソケットを読む形にはしない（[read-ownership.md](read-ownership.md) の「しないこと」）
