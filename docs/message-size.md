# GIOP メッセージの最大長

issue #102。ORB が送る・受け取る GIOP メッセージの大きさの上限を決める。

## 使い方

どのバインディングでも ORB 引数で指定する（Java は `ORB.init` のプロパティ `ORBgiopMaxMsgSize` でもよい）。

```sh
./server -ORBgiopMaxMsgSize 2097152
```

| オプション | `OrbOptions` | 既定 | 意味 |
| --- | --- | --- | --- |
| `-ORBgiopMaxMsgSize N` | `max_message_size` | 67108864（64 MiB） | 送る・受け取る GIOP メッセージの最大の長さ（バイト、12 バイトのヘッダーを含み、Fragment はつないだ全体）。1024 未満は `BAD_PARAM` |

## 上限を超えたとき

| 場面 | 結果 |
| --- | --- |
| 送ろうとした要求が上限を超える | 送らずに `MARSHAL`（minor 0、COMPLETED_NO）。接続はそのまま使える |
| サーバーの返事が上限を超える | 返事の代わりに `MARSHAL`（minor 0、COMPLETED_YES）を返す。オペレーションは実行済みなので YES |
| 上限を超えるメッセージを受け取った | ヘッダーで分かった時点で `MessageError` を送って接続を閉じる。待っていた呼び出しは `COMM_FAILURE` |

送る前の検査には、相手の上限ではなく自分の上限を使う（相手の上限は IOR などで知らされない）。相手の上限が小さいと分かっているときは、クライアントの `-ORBgiopMaxMsgSize` をそれに合わせると、相手に送って接続を切られる前に `MARSHAL` で分かる。

上限を超えたときは、トレースのレベル 1（既定）で警告が出る（[tracing.md](tracing.md)）。

同じプロセスのオブジェクトへの呼び出し（colocated）はネットワークを通らないので、上限を見ない。
