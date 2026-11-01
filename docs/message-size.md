# GIOP メッセージの最大長と Fragment

issue #102。ORB が送る・受け取る GIOP メッセージの大きさの上限を決める。また、大きなメッセージを GIOP 1.2 の Fragment に分けて送れるようにする。

## 使い方

どのバインディングでも ORB 引数で指定する（Java は `ORB.init` のプロパティ `ORBgiopMaxMsgSize` でもよい）。

```sh
./server -ORBgiopMaxMsgSize 2097152
```

| オプション | `OrbOptions` | 既定 | 意味 |
| --- | --- | --- | --- |
| `-ORBgiopMaxMsgSize N` | `max_message_size` | 67108864（64 MiB） | 送る・受け取る GIOP メッセージの最大の長さ（バイト、12 バイトのヘッダーを含み、Fragment はつないだ全体）。1024 未満は `BAD_PARAM` |
| `-ORBgiopFragmentSize N` | `fragment_size` | 0（分割しない） | GIOP 1.2 の要求・返事が N バイトを超えたら、N バイト以下の Fragment に分けて送る。0 か 1024 以上 |

## 上限を超えたとき

| 場面 | 結果 |
| --- | --- |
| 送ろうとした要求が上限を超える | 送らずに `MARSHAL`（minor 0、COMPLETED_NO）。接続はそのまま使える |
| サーバーの返事が上限を超える | 返事の代わりに `MARSHAL`（minor 0、COMPLETED_YES）を返す。オペレーションは実行済みなので YES |
| 上限を超えるメッセージを受け取った | ヘッダーで分かった時点で `MessageError` を送って接続を閉じる。待っていた呼び出しは `COMM_FAILURE` |

送る前の検査には、相手の上限ではなく自分の上限を使う（相手の上限は IOR などで知らされない）。相手の上限が小さいと分かっているときは、クライアントの `-ORBgiopMaxMsgSize` をそれに合わせると、相手に送って接続を切られる前に `MARSHAL` で分かる。

上限を超えたときは、トレースのレベル 1（既定）で警告が出る（[tracing.md](tracing.md)）。

同じプロセスのオブジェクトへの呼び出し（colocated）はネットワークを通らないので、上限を見ない。

## 送信側の Fragment

`-ORBgiopFragmentSize N` を指定すると、GIOP 1.2 の `Request`・`Reply`・`LocateReply` で N バイトを超えるものを、最初のメッセージ（`more fragments` のフラグ付き）と続く `Fragment` に分けて送る。既定は今までどおり分割しない。

- 分けるのは GIOP 1.2 だけ。GIOP 1.1 の Fragment は送らない（request id がなく、同じ接続の要求が混ざると相手が組み立てられない。JacORB 3.9 は受け付けない）
- 最初のメッセージは要求・返事のヘッダー（対象、オペレーション名、サービスコンテキスト）を全部含む。N がヘッダーより小さいときは最初のメッセージだけ N を超える
- 最後以外のメッセージの長さは 8 の倍数（N は 8 の倍数に切り下げる）。`Fragment` のヘッダーは 16 バイトなので、本体のアラインメントは「メッセージ全体の先頭から」でも「各 Fragment の先頭から」でも同じになり、どちらの読み方をする相手でも読める
- 1 つのメッセージの Fragment は続けて送り、他のメッセージを間に挟まない
- 上限（`-ORBgiopMaxMsgSize`）は分ける前の全体の長さで見る。受け取る側も Fragment をつないだ全体で見る
- 分けるときは、大きな octet 列もコピーしてつなぐ（`write_octet_seq_shared` のコピーなしの送信は効かない）

### 既定で分割しない理由

相手の上限は Fragment をつないだ全体にかかる（rustorb もそう数える）ので、分けても相手の上限を超える問題は解けない。分けると相手が組み立てる分の手間が増え、相手による違いに当たる危険だけが増える。分けたいのは、相手が 1 つのメッセージの大きさを別に制限しているときや、途中の機器が大きなメッセージを嫌うときなどに限られる。

### 相互接続

`interop/run-matrix.sh --fragment 1024` で、rustorb の各クライアント・サーバーが 1024 バイトの Fragment で送り、omniORB C++・omniORBpy・JacORB と組み合わせて確かめる（CI の interop ワークフロー）。rustorb のクライアントには 300000 バイトの `echo_octets[large]` を足した。
