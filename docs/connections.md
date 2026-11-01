# 使っていない接続を閉じる

issue #101。クライアントとサーバーのどちらも、一定時間使われていない GIOP の接続を閉じる。閉じた後の呼び出しは新しい接続で透過的に行う。サーバー側は同時に開いておく接続の数に上限を設けられる。

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

クライアント側の上限はない（相手のエンドポイントごとに接続は 1 つ）。

## 送り直し

クライアントは、返事の来ないうちに `CloseConnection` を受け取った要求を、新しい接続で送り直す（COMPLETED_NO の扱い）。同じエンドポイントに 4 回まで試し、それでも閉じられたら `TRANSIENT`（minor 0、COMPLETED_NO）にする。今までは 2 回だったが、サーバーとクライアントの両方が使っていない接続を閉じると、つなぎ直した直後の接続も閉じられることがあるので増やした。

返事の来ないうちに接続が切れた（`CloseConnection` なしに切断された）要求は、今までどおり `COMM_FAILURE`（COMPLETED_MAYBE）で、送り直さない。

## 他の ORB との相互接続

`interop/run-idle-probe.sh` で確かめる。各ケースで一方の ORB の idle timeout を 1 秒にし、呼び出し → 4 秒待つ → 接続がなくなったことを `ss` で確かめる → もう一度呼び出す、を行う。omniORBpy は omniORB の C++ のコアの上に載っているので、クライアントとしての omniORB C++ もこれで代表させる。

| クライアント | サーバー | 閉じる側 | 結果 |
| --- | --- | --- | --- |
| rustorbpy | omniORB C++ | クライアント | |
| rustorbpy | omniORB C++ | サーバー（`-ORBinConScanPeriod 1 -ORBscanGranularity 1`） | |
| rustorbpy | omniORBpy | サーバー | |
| rustorbpy | JacORB | クライアント | |
| rustorbpy | JacORB | サーバー（`jacorb.connection.server.timeout=1000`） | |
| omniORBpy | rustorb | クライアント（`-ORBoutConScanPeriod 1 -ORBscanGranularity 1`） | |
| omniORBpy | rustorb | サーバー | |
| JacORB | rustorb | クライアント（`jacorb.connection.client.idle_timeout=1000`） | |
| JacORB | rustorb | サーバー | |
| rustorbpy | rustorb | クライアント・サーバー | |

## 双方向 GIOP（#103）への引き継ぎ

BiDir ではクライアントの開いた接続にサーバーからの要求も載る。その接続を閉じてよいのは、両方向とも返事待ち・実行中の要求がないときに限る（クライアント側の判定に「サーバーからの要求を実行中でない」を、サーバー側の判定に「こちらからの要求の返事待ちでない」を加える）。GIOP 1.2 の BiDir の接続では、クライアントも `CloseConnection` を送れる。
