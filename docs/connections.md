# 接続の管理

issue #101。クライアントとサーバーのどちらも、一定時間使われていない GIOP の接続を閉じる。閉じた後の呼び出しは新しい接続で透過的に行う。サーバー側は同時に開いておく接続の数に上限を設けられる。

クライアントは、同じサーバーへの呼び出しが同時に走るとき、接続を 4 本まで増やす（#208、[同じサーバーへの複数の接続](#同じサーバーへの複数の接続)）。

## 使い方

どのバインディングでも ORB 引数で指定する（Java は `ORB.init` のプロパティでもよい）。

```sh
./server -ORBinConScanPeriod 600 -ORBmaxServerConnections 1000
./client -ORBoutConScanPeriod 0     # クライアントの接続を閉じない
```

| オプション | `OrbOptions` | 既定 | 意味 |
| --- | --- | --- | --- |
| `-ORBoutConScanPeriod S` | `client_idle_timeout` | 120 秒 | クライアントの接続（出ていく接続）を、返事待ちの要求がなく S 秒の間なにも送受信しなかったら閉じる。0 で閉じない |
| `-ORBinConScanPeriod S` | `server_idle_timeout` | 180 秒 | サーバーの接続（入ってくる接続）を、実行中の要求がなく S 秒の間要求が来なかったら `CloseConnection` を送って閉じる。0 で閉じない |
| `-ORBmaxServerConnections N` | `max_server_connections` | 0（上限なし） | サーバーが同時に開いておく接続の数 |
| `-ORBmaxConnectionsPerServer N` | `max_connections_per_server` | 4 | クライアントが一つのサーバーのエンドポイントに開く接続の数（1 以上）。omniORB の名前 `-ORBmaxGIOPConnectionPerServer` でも受け付ける |

オプションの名前と既定の秒数は omniORB に合わせた。omniORB の `-ORBoutConScanPeriod` / `-ORBinConScanPeriod` は「調べる間隔」で、`-ORBscanGranularity`（既定 5 秒）の刻みで調べるが、rustorb では「使われなくなってから閉じるまでの時間」とした。実際に閉じるのは、その時間から 4 分の 1（最大 5 秒）遅れるまでの間。`OrbOptions` では秒より細かく指定できる。

閉じたときはトレースのレベル 5 で `closing the connection ...: idle for ...` が出る（[tracing.md](tracing.md)）。

## クライアント側

- ORB に接続が 1 つでもできると、接続を調べるタスクが ORB の実行時環境で動き始める（`-ORBoutConScanPeriod 0` なら動かない）。`shutdown` で止まる
- 返事待ちの要求がなく、最後に要求を送るか返事を受け取ってから `client_idle_timeout` が過ぎた接続を、接続の表から外して閉じる。閉じると決める判定と「新しい要求を受け付けない」印は同じロックの中で付けるので、同時に送ろうとした要求はその接続に載らず、新しい接続で送られる（呼び出し側からは見えない）
- 閉じるときは `CloseConnection` を送らず、TCP の送信側を閉じる（TLS は close_notify）。GIOP 1.0・1.1 ではクライアントが `CloseConnection` を送ることはなく、仕様もクライアントは接続をただ閉じてよいとしている。`Orb::shutdown` / `destroy` は今までどおり `CloseConnection` を送る
- 相手が閉じた接続（`CloseConnection` や切断）も、読み取りタスクが終わった時点で接続の表から外す。それまでは表に残り、次の呼び出しまでソケットが開いたままだった

## サーバー側

- RootPOA を使って待ち受けを始めると、接続を調べるタスクが動き始める
- 実行中・返事待ちの要求がなく、読みかけのメッセージ（Fragment の途中など）もなく、最後に要求が来るか要求が終わってから `server_idle_timeout` が過ぎた接続に `CloseConnection` を送り、送信側を閉じる
- `CloseConnection` を送ると決めた後に読んだ、返事の要る要求は処理しない。クライアントは返事の来ていない要求を `CloseConnection` の後に送り直すので（CORBA 3.x part 2 の GIOP の接続管理にある orderly shutdown）、処理すると二重に実行してしまう。oneway はクライアントが送り直さないので処理する
- `LocateRequest` への返事も `CloseConnection` の後は送らない。クライアントは送り直す
- `CloseConnection` の後、クライアントが接続を閉じるのを 10 秒待つ。閉じなければ読むのをやめて接続を捨てる（送信側だけ閉じてすぐにソケットを閉じると、届いていた要求のせいで RST になり、クライアントが `CloseConnection` を読めないことがある）

## 接続数の上限

`-ORBmaxServerConnections N` を指定すると、サーバーは同時に N 個までしか接続を受け付けない。上限に達したら、要求の実行中でない接続のうち最も長く使われていないものに `CloseConnection` を送って閉じ、それが閉じるのを待って次の接続を受け付ける。どれも実行中なら、新しい接続は待ち受けのキュー（listen backlog）で待ち、1 秒ごとにもう一度閉じられる接続を探す。

クライアント側の上限は、相手のエンドポイントごとの `-ORBmaxConnectionsPerServer`（下）。

## 同じサーバーへの複数の接続

#208（[perf-plan.md](perf-plan.md) の段 5）。0.7 までのクライアントは、同じエンドポイント（ホスト・ポート・トランスポート）への呼び出しをすべて 1 本の接続に載せていた。1 本だと、返事を読むのは 1 か所（I/O タスクか、[read-ownership.md](read-ownership.md) の呼び出したスレッド）で、ほかの呼び出し元はそこから起こされるのを待つ。一つの参照を 4 スレッドから呼ぶと、クラウドで 3.1 万回/s しか出なかった。

今は、ORB はエンドポイントごとに接続の組を持つ。

- 呼び出しは、ほかの呼び出しが載っていない接続を使う。そのスレッドが前に使った接続が空いていれば、それを選ぶ（返事の後も読み続ける #207 の経路にそのまま乗る）
- 空いた接続がなく、開いている接続と開いている途中の接続の合計が `-ORBmaxConnectionsPerServer`（既定 4）より少なければ、新しく開く
- 上限に達していれば、載っている呼び出しが最も少ない接続を一緒に使う（今までと同じ多重化）。上限の分だけ開いている途中のときは、どれかが開き終わるのを待つ（`-ORBclientConnectTimeOutPeriod` か呼び出しの期限まで）
- 1 スレッドが順に呼ぶだけなら、接続は 1 本のまま。`-ORBmaxConnectionsPerServer 1` で、すべての呼び出しを 1 本に載せる今までの動きになる

増えた接続も、ほかの接続と同じく `client_idle_timeout` で閉じる。

上限を超えて接続を一緒に使う呼び出しは、1 本のときと同じ速さになる。クラウド（4 コア）で、上限 4 のまま 8 スレッドから呼ぶと 6.8 万回/s、上限 8 なら 13 万回/s だった。同時に呼ぶスレッドが多いなら上限を上げる。空くのを待つ形にしなかったのは、サーバーからのコールバックの中で同じサーバーを呼ぶような入れ子の呼び出しが、上限の数で行き詰まらないため。

組み合わせ：

- **双方向 GIOP**：`-ORBofferBiDirectionalGIOP 1`（か `BidirectionalPolicy` の `BOTH`）の ORB は 1 本だけ開く。サーバーからのコールバックが来る接続を一つにする。クライアントが差し出した接続（`-ORBacceptBiDirectionalGIOP 1`）を使うエンドポイントへは、その接続だけを使い、新しく開かない
- **TLS**・**giop:unix**：TCP と同じ。接続ごとに TLS のハンドシェイクをする
- **`LocateRequest`・転送**：呼び出しと同じように接続を選ぶ。転送先は別のエンドポイントなので、別の組になる
- **サーバー側**：何も変えていない。接続ごとの要求数の上限（`max_requests_per_connection`）と follow は、接続ごとにそのまま効く。`-ORBmaxServerConnections` は、増えた接続の分も数える

内部の数（`-ORBstatistics 1`）：`connects` は開いた接続、`connection_shares` は上限に達して、ほかの呼び出しが載っている接続を使った呼び出し。

測った値（クラウドの 4 コアの仮想マシン、`run-bench.py --quick`、2 回）：

| ケース | 前 | 後 |
| --- | ---: | ---: |
| threads:4（calls/s） | 30,828・31,819 | 124,121・116,704 |
| threads:4 のクライアントの CPU／呼び出し | 47 µs | 12–13 µs |
| ping p50 | 47.0・45.5 µs | 46.5・45.2 µs |
| oneway（calls/s） | 31.4 万・32.0 万 | 31.0 万・32.1 万 |

## 送り直し

クライアントは、返事の来ないうちに `CloseConnection` を受け取った要求を、新しい接続で送り直す（COMPLETED_NO の扱い）。同じエンドポイントに 3 + `-ORBmaxConnectionsPerServer` 回（既定で 7 回）まで試し、それでも閉じられたら `TRANSIENT`（minor 0、COMPLETED_NO）にする。最初は 2 回だったが、サーバーとクライアントの両方が使っていない接続を閉じると、つなぎ直した直後の接続も閉じられることがあるので 4 回にし、#208 で接続が増えた分を足した。

`CloseConnection` を受け取ったときは、同じエンドポイントへのほかの接続のうち、呼び出しが載っていないものも閉じる。サーバーが使っていない接続を閉じるときは、たいてい全部を閉じているので、送り直した要求がそれに一つずつ当たらないようにする。

返事の来ないうちに接続が切れた（`CloseConnection` なしに切断された）要求は、今までどおり `COMM_FAILURE`（COMPLETED_MAYBE）で、送り直さない。

## 他の ORB との相互接続

`interop/run-idle-probe.sh` で確かめる。各ケースで一方の ORB の idle timeout を 1 秒にし、呼び出し → 4 秒待つ → 接続がなくなったことを `ss` で確かめる → もう一度呼び出す、を行う。omniORBpy は omniORB の C++ のコアの上に載っているので、クライアントとしての omniORB C++ もこれで代表させる。

| クライアント | サーバー | 閉じる側 | 結果 |
| --- | --- | --- | --- |
| rustorbpy | omniORB C++ | クライアント | 通る |
| rustorbpy | omniORB C++ | サーバー（`-ORBinConScanPeriod 1 -ORBscanGranularity 1`） | 通る |
| rustorbpy | omniORBpy | サーバー | 通る |
| rustorbpy | JacORB | クライアント | 通る |
| rustorbpy | JacORB | サーバー（`jacorb.connection.server.timeout=1000`） | 通る |
| omniORBpy | rustorb | クライアント（`-ORBoutConScanPeriod 1 -ORBscanGranularity 1`） | 通る |
| omniORBpy | rustorb | サーバー | 通る |
| JacORB | rustorb | クライアント（`jacorb.connection.client.idle_timeout=1000`） | 通る |
| JacORB | rustorb | サーバー | 通る |
| rustorbpy | rustorb | クライアント・サーバー | 通る |

2026-11-02 に手元で全ケースが通った（omniORB 4.3.4、omniORBpy 4.3.4、JacORB 3.9）。

omniORB のクライアントは、使っていない接続をその次に使うまで読まない。サーバーが `CloseConnection` を送って閉じても、omniORB 側のソケットは次の呼び出しまで CLOSE-WAIT のまま残る（相手が omniORB のサーバーでも同じ）。次の呼び出しで閉じられたことに気づき、新しい接続で送る。rustorb のサーバーは 10 秒後に自分の側を閉じるので、サーバーにソケットは残らない。プローブは omniORB のクライアントについては「接続が ESTABLISHED でなくなったこと」だけを見る。

JacORB の `jacorb.connection.client.idle_timeout`（ミリ秒）はソケットの読み取りのタイムアウトとして使われ、その間なにも来なければ JacORB が接続を閉じる。

## 双方向 GIOP

双方向 GIOP（#103、[bidir-giop.md](bidir-giop.md)）の接続には、サーバーからの要求も載る。その接続は、両方向とも返事待ち・実行中の要求がないときにだけ閉じる。クライアントの判定に「サーバーから来た要求が実行中・読みかけでなく、`client_idle_timeout` の間来ていない」を、サーバーの判定に「クライアントへの呼び出しの返事待ちがなく、`server_idle_timeout` の間送っていない」を加えた。双方向を知らせた接続をクライアントが閉じるときは `CloseConnection` を送る（GIOP 1.2 の双方向の接続では、クライアントも送れる）。
