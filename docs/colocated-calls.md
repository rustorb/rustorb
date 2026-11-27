# 同一プロセスの呼び出し（#209）

Nov 27, 2026 · @ysuga · [速度の改善の計画](perf-plan.md)の段 6（issue [#209](https://github.com/rustorb/rustorb/issues/209)）

同じ ORB のオブジェクトへの呼び出し（colocated）を、生成コードを変えずに core だけで軽くした。

## 前の形

呼び出しはネットワークを通らないが、GIOP のメッセージを往復していた（`object.rs` の `LocalCall`、`Server::call_local`）。

1. クライアントが、要求ヘッダとコードセットのサービスコンテキストを含む `Request` のメッセージ全体を組み立て、すぐに解いて `RequestHeader` と本体にする
2. サーバーが、POA マネージャーの判断を `std::sync::mpsc` で受け取り（active ならその場で）、返事の宛先にも `mpsc` を作り、アップコールを呼び出し元のスレッドで走らせる
3. サーバントの返事を `Reply` のメッセージ全体に組み立てて送り、クライアントがそれを解く

プロファイル（クラウドの 4 コアの仮想マシン、`perf record`、`strace -c`）で見えたのは、次の 2 つ。

- **1 回の呼び出しで futex が 3 回。** 要求の終わりに、POA と POA マネージャーの「処理中の数」（`Activity`）と、サーバー全体の処理中の数（`Ticket`）がそれぞれ 0 に戻るたびに `Condvar::notify_all` を呼んでいた。待つ人がいなくても futex の wake のシステムコールになる。TCP 越しの要求も同じ
- **GIOP の組み立てと解きが 2 往復**、`mpsc` のチャネルが 2 本、確保（ヘッダの複製、`ServiceContext`、`Box` の閉包）

## 変えたこと

### GIOP を通さない近道

GIOP 1.2 以上（rustorb の IOR は 1.2）で、引数は CDR の本体だけを組み立てる。GIOP 1.2 の本体は 8 バイトの境界から始まるので、本体だけをストリームの位置 0 から書けば揃えは同じになる（`Body::from_vec`）。

サーバー側（`Server::call_direct`）は、オブジェクトキーから POA とオブジェクト ID を探し、次の両方が成り立てば、呼び出し元のスレッドでサーバントを呼ぶ。

- サーバーのポータブルインターセプターが登録されていない
- POA が ORB_CTRL_MODEL で、POA マネージャーが active

返事は `ServerReply` の本体だけ（`ServerReply::body_only`）で、状態（`NO_EXCEPTION`・`USER_EXCEPTION`）と一緒に返す。システム例外と `LOCATION_FORWARD`（サーバントマネージャーの `ForwardRequest`）はエンコードせず、値のまま返す。クライアントはそれを TCP 越しの返事と同じ所（`reply_outcome`、転送なら次の IOR）に渡す。

サーバントを探して呼ぶ所（`Server::run_servant`：サーバントマネージャー、`PoaCurrent`、パニックを `UNKNOWN` にする所、`_is_a` などの ORB の操作）は、TCP 越しの要求と同じ関数を使う。違うのは返事の行き先だけ。

### 今の経路に戻るとき

近道の条件が成り立たないときは、前と同じく `Server::call_local` を通る（決めること #3）。holding のマネージャーで待つ、SINGLE_THREAD_MODEL で順番を待つ、MAIN_THREAD_MODEL でメインのスレッドに回す、サーバーのインターセプターが要求ヘッダとサービスコンテキストを見る、のどれも前のまま。

この場合も、GIOP 1.2 以上なら要求のメッセージを組み立てて解くことはせず、`RequestHeader`（コードセットのサービスコンテキストとクライアントのインターセプターのものを含む）を直接作って渡す。GIOP 1.2 より前は、本体の揃えが要求ヘッダの長さに依るので、前と同じくメッセージ全体を組み立てて解く。

| 場合 | 経路 |
| --- | --- |
| ORB_CTRL_MODEL、マネージャーが active、サーバーのインターセプターなし | 近道 |
| マネージャーが holding・discarding・inactive | 今の経路（holding は待ち、ほかは `TRANSIENT`・`OBJ_ADAPTER`） |
| SINGLE_THREAD_MODEL・MAIN_THREAD_MODEL | 今の経路 |
| サーバーのインターセプターがある | 今の経路（両側のインターセプターを呼ぶ） |
| クライアントのインターセプターだけがある | 近道（クライアントのインターセプターは今までどおり呼ぶ） |
| `-ORBmaxGIOPVersion 1.0`・`1.1`、または IOR が 1.2 より前 | 今の経路（メッセージ全体） |
| 対象のアドレスが `ProfileAddr`・`ReferenceAddr`（`NEEDS_ADDRESSING_MODE` の後） | 今の経路 |

`-ORBstatistics 1` の `local_direct` が近道を通った呼び出し、`local_general` が今の経路を通った呼び出し。

前と同じ所：

- **スレッド**：ORB_CTRL_MODEL の呼び出しは前から呼び出し元のスレッドで走っていた。async の呼び出し（ORB のランタイムの中）は、前と同じくアップコールのプールで走る
- **oneway と `SyncScope`**：サーバントは呼び出し元のスレッドで走り終えてから戻る（前と同じ）。`SYNC_NONE`・`SYNC_WITH_TRANSPORT` は何も返さない。`SYNC_WITH_SERVER` はサーバントが見つかるまでの例外（`OBJECT_NOT_EXIST` など）だけを返す。`SYNC_WITH_TARGET` はサーバントが上げた例外も返す
- **タイムアウト**：近道はその場で走るので待つ所がない（前もマネージャーが active なら同じ）。holding で待つ間のタイムアウトは今の経路で前のとおり
- **サービスコンテキスト**：サーバーのインターセプターがなければ、誰も読まない。返事のサービスコンテキストは前も空だった
- **ORB の停止**：近道の呼び出しも `Ticket` を持つので、`shutdown(true)` は走っている呼び出しを待つ

### 要求の終わりに futex を呼ばない

`poa::Activity`（POA と POA マネージャーの処理中の数）を、数えるのは原子的な操作、待つ人がいるときだけロックと条件変数を使う形にした。サーバー全体の処理中の数（`Ticket`）も同じ `Activity` にした。数を減らす側は 0 になった後で待つ人の数を見て、いればロックを取って起こす。待つ側は待つ人の数を増やしてからロックの下で数を見る。どちらも sequentially consistent なので、待つ側が 0 を見るか、減らす側が待つ人を見るかのどちらかは必ず起きる。

TCP 越しの要求でも、要求の終わりの futex が 3 回減る。

## 結果

クラウドの 4 コアの仮想マシン。`run-bench.py --quick --stats --impls rustorb`（各 2 秒、3 回の中央値）を、main と交互に 2 回ずつ回した。

| ケース | 値 | 前 | 後 |
| --- | --- | ---: | ---: |
| 同一プロセスの ping | p50 | 2.44・2.52 µs | 0.88・0.89 µs |
| 同一プロセスの ping | p99 | 5.02・6.26 µs | 1.43・1.37 µs |
| 同一プロセスの ping | CPU / 回 | 2.67・2.81 µs | 0.99・0.99 µs |
| oneway（TCP） | 回/s | 30.0 万・30.5 万 | 36.9 万・35.3 万 |
| oneway（TCP） | サーバーの CPU / 回 | 3.35・3.28 µs | 2.75・2.87 µs |

`cargo bench -p rustorb-core --bench throughput -- orb/echo_long_in_process`：2.56 µs → 0.92 µs。

同一プロセスの ping は、目安（クラウドで 1 µs 以下）に届いた。実機の値は段 10（#213）で測る。

TCP 越しの oneway が速くなったのは、要求の終わりの futex がなくなったため（サーバーのアップコールのスレッドが 1 回あたり 3 回のシステムコールを省く）。ping・any は 10% のぶれの中で動かなかった。threads:4 と get:1048576 は run-bench.py の 2 回とも 5〜10% 低く出たが、サーバーとクライアントを手で交互に 3 回ずつ回すと threads:4 は後のほうが高く（前 18.5 万〜19.9 万回/s、後 19.8 万〜20.8 万回/s）、ぶれと見ている。

残りの 0.9 µs のうち大きいのは、オブジェクトキーから POA とオブジェクトを探す所（`Server::find`、`PoaInner::resolve`）、`PoaCurrent` の出入り、参照のアドレスの一覧の取り出し（`ObjectRef::current_target`）、本体の確保と解放。

## しないこと

- **生成コードの型どうしの直接呼び出し**：CDR を通さずにスタブからサーバントの型の関数を呼ぶのは段 7（#210、rustorb-idl）
- **大きな octet 列のコピー**：近道の本体は共有の octet 列（`write_octet_seq_shared`）もコピーする。同一プロセスで大きなデータを渡す場面が見えたら考える
