# 双方向 GIOP（BiDir GIOP）

issue #103。クライアントが開いた GIOP の接続を、サーバーからクライアントへの要求（コールバック）にも使う。NAT やファイアウォールの内側にいて外から接続できないクライアントも、コールバックを受けられる。CORBA 3.x part 2 の GIOP の章にある Bi-Directional GIOP と、`BiDirPolicy` モジュールの `BidirectionalPolicy` の実装。

## 使い方

クライアントとサーバーの両方で有効にする。既定ではどちらも無効（omniORB と同じ）。

```sh
./server -ORBacceptBiDirectionalGIOP 1
./client -ORBofferBiDirectionalGIOP 1
```

| オプション | `OrbOptions` | 既定 | 意味 |
| --- | --- | --- | --- |
| `-ORBofferBiDirectionalGIOP 0\|1` | `offer_bidir` | 0 | 開く接続を双方向に使ってよいと相手に知らせる（クライアント側） |
| `-ORBacceptBiDirectionalGIOP 0\|1` | `accept_bidir` | 0 | 相手が知らせた接続を、その相手のオブジェクトの呼び出しに使う（サーバー側） |

ORB の `ORBPolicyManager`（Rust では `Orb::set_policy_overrides`）に `BidirectionalPolicy` の `BOTH` を置くと、両方のオプションを 1 にしたのと同じになる（`NORMAL` なら両方 0）。オブジェクト参照とスレッドの上書きには置けない（`NO_PERMISSION`）。

クライアントのコールバックのオブジェクトは、`BidirectionalPolicy` が `BOTH` の POA に置く。

```rust
let poa = root.create_poa(
    "callbacks",
    Some(&root.the_poa_manager()),
    &[Policy::Bidirectional(Bidirectional::Both)],
)?;
```

```python
from rustorb import BiDirPolicy, CORBA   # omniORBpy の互換パッケージでは from omniORB import BiDirPolicy

both = orb.create_policy(
    BiDirPolicy.BIDIRECTIONAL_POLICY_TYPE,
    CORBA.Any(BiDirPolicy._tc_BidirectionalPolicyValue, BiDirPolicy.BOTH),
)
poa = root_poa.create_POA("callbacks", root_poa._get_the_POAManager(), [both])
```

`create_policy` の値は `BiDirPolicy::BidirectionalPolicyValue`（`unsigned short` の別名）で、`NORMAL` が 0、`BOTH` が 1。ポリシーの型は 37（`BIDIRECTIONAL_POLICY_TYPE`）。C ABI では `rorb_poa_create_poa` に `{RORB_BIDIRECTIONAL_POLICY_TYPE, 1}` を渡す。C++ と Java の `BiDirPolicy` の API はまだない（ORB オプションは使える）。

## クライアント側

- 接続を開いたとき、ORB が双方向を知らせる設定で、しかもサーバー側が動いている（RootPOA を使ったことがある）なら、その接続を双方向に使えるようにする。設定や待ち受けはその時点のもので決まり、後から変えても開いている接続には効かない
- その接続で送る最初の GIOP 1.2 の `Request` に、`BI_DIR_IIOP` サービスコンテキスト（`IIOP::BiDirIIOPServiceContext`）を付ける。中身は ORB の待ち受けの host と port を、IOR に書くもの（`-ORBendPointPublish` で変えたもの）のまま並べたもので、TLS の待ち受けも含む。送れなかったときは次の要求に付ける。GIOP 1.0・1.1 の要求と `LocateRequest` には付けない
- その接続でサーバーから来た `Request` はサーバー側に渡して処理し、返事も同じ接続で返す。`LocateRequest` と `CancelRequest` も扱う。双方向を知らせていない接続にサーバーから来た要求は、今までどおり捨てる
- サーバーから来た要求を受けるのは `BidirectionalPolicy` が `BOTH` の POA のオブジェクトだけ。そうでない POA のオブジェクトへの要求には `NO_PERMISSION`（minor 0、COMPLETED_NO）を返す。仕様にこの場合の例外の定めは見当たらず、rustorb で決めた
- request id は、接続を開いた側が偶数、受け付けた側が奇数を使う（GIOP 1.2 の双方向の規則）。rustorb のクライアントは双方向でない接続でも偶数（0, 2, 4, …）を使うようになった

## サーバー側

- 双方向を受け入れる設定なら、GIOP 1.2 の要求に付いた `BI_DIR_IIOP` コンテキストの待ち受けアドレスを、その接続に結び付ける。以後、IOR のアドレス（host と port）がそれと一致するオブジェクトの呼び出しは、新しい接続を開かずにその接続で送る。TLS の接続は、同じ host と port の平文のアドレスの呼び出しにも使う
- 受け入れない設定なら、コンテキストは読まずに無視する（トレースのレベル 10 で出る）。オブジェクトの呼び出しは今までどおり IOR のアドレスに接続する
- 接続が閉じたら結び付きも外す。以後の呼び出しは IOR のアドレスに接続しようとする（NAT の内側なら `TRANSIENT` になる）
- 結び付けたときはトレースのレベル 5 で `calls to HOST:PORT go over the connection from ...`、知らせた側にはレベル 5 で `offered the connection to ... for bidirectional GIOP` が出る

## 待ち受けアドレスを信じる条件

`BI_DIR_IIOP` コンテキストの待ち受けアドレスは、クライアントが自分で書いたもので、確かめようがない。双方向の目的は NAT やファイアウォールの内側のクライアントなので、アドレスを接続元の IP と照らし合わせることもできない（内側のアドレスと外から見えるアドレスは違う）。悪意のあるクライアントが他人のアドレスを知らせると、そのアドレスのオブジェクトへの呼び出しを横取りできる。rustorb のサーバーは次の条件で信じる。

1. `-ORBacceptBiDirectionalGIOP 1`（か ORB の `BidirectionalPolicy` `BOTH`）のときだけ。既定は信じない
2. 先着順。そのアドレスへの使える接続がもうあるとき（こちらから開いた接続も、別のクライアントが先に知らせた接続も）は、それを使い続け、新しく知らせた接続には結び付けない（トレースのレベル 5 で `not taken` が出る）。先に知らせた接続が閉じるまで、後からの横取りはできない
3. 結び付けるのは、知らせた接続が開いている間だけ

これで防げないのは、まだ誰も接続していないアドレスを先に名乗ることで、信頼できないクライアントがつなぐサーバーでは双方向を有効にしない。クライアントの身元を確かめたいときは SSLIOP でクライアント証明書を求める（`-ORBsslVerifyClient required`、[ssliop.md](ssliop.md)）。CORBA の仕様も、待ち受けアドレスを信じるかどうかは ORB の方針に任せている。

## 接続の寿命

[connections.md](connections.md) の「使っていない接続を閉じる」に、両方向の要求を加えた。

- クライアント：双方向に使える接続は、こちらからの要求の返事待ちがないことに加えて、サーバーから来た要求が実行中でも読みかけでもなく、`client_idle_timeout` の間来ていないときに閉じる。閉じると決めた後に来た、返事の要るサーバーからの要求は処理しない。閉じるときは（GIOP 1.2 で双方向を知らせた接続なので）`CloseConnection` を送り、サーバーはそれを見て、返事の来ていない要求を新しい接続で送り直せる
- サーバー：双方向の接続は、実行中の要求がないことに加えて、クライアントへの呼び出しの返事待ちがなく、`server_idle_timeout` の間どちらの向きにも使われていないときに閉じる。閉じると決めた時点で結び付きを外すので、その後のクライアントへの呼び出しは別の接続を探す。`-ORBmaxServerConnections` で閉じる接続を選ぶときも同じ
- サーバーに `CloseConnection` を送られた、あるいは送ったときに返事の来ていないクライアントへの呼び出しは、送り直しの対象（COMPLETED_NO）。切断なら `COMM_FAILURE`（COMPLETED_MAYBE）
- 双方向の接続では、読み取りタスクは `max_requests_per_connection` の空きを待たない。待つと、空きを作るはずの返事が同じ接続の後ろにあって読めず、行き詰まるため。空きがなければ要求ごとにタスクを作って待つ

## 他の ORB との相互接続

`interop/run-bidir-probe.sh` で確かめる。クライアントは双方向を知らせ、コールバックのオブジェクトの IOR に誰も待ち受けていないアドレス（`127.0.0.1:1`）を書く。サーバーの `Echo::call_back` がコールバックを呼べるのは、クライアントの開いた接続を使ったときだけ。2 回呼んで答えと、クライアントからサーバーへの接続が 1 本であることを見る。omniORBpy は omniORB の C++ のコアの上に載っているので、クライアントとしての omniORB C++ もこれで代表させる。

| クライアント | サーバー | 結果 |
| --- | --- | --- |
| rustorbpy | omniORB C++ | 通る |
| rustorbpy | omniORBpy | 通る |
| omniORBpy | rustorb | 通る |
| omniORBpy | rustorbpy | 通る |
| rustorbpy | rustorb | 通る |
| omniORBpy | omniORB C++ | 通る（比較のため） |

2026-11-02 に手元で全ケースが通った（omniORB 4.3.4、omniORBpy 4.3.4）。

動かして分かった omniORB の挙動（トレースと結果から。ソースは読んでいない）：

- omniORB は、トランスポートの規則に `bidir` がないと双方向を受け入れない。サーバーは `-ORBserverTransportRule "* unix,ssl,tcp,bidir"`、クライアントは `-ORBclientTransportRule "* unix,ssl,tcp,bidir"` が要る。ないとサーバーはトレースに `... to switch to bidirectional is rejected because "bidir" is not found in the matched rule` と出す
- omniORB のサーバーは、コールバックの参照を受け取ったオブジェクトの POA が `BidirectionalPolicy` `BOTH` でないと、受け入れた双方向の接続をコールバックに使わず、IOR のアドレスに接続しにいく。rustorb のサーバーは ORB の設定だけで決め、POA のポリシーは見ない
- omniORB のクライアントは `BI_DIR_IIOP` コンテキストに、実際に待ち受けているアドレスと `-ORBendPointPublish` のアドレスの両方を並べる。rustorb は IOR に書くアドレスだけを並べる
- omniORBpy の `BiDirPolicy` モジュールでは、TypeCode の名前が `_tc_BidrectionalPolicyValue`（綴りが違う）。rustorb の `BiDirPolicy` は正しい綴りと、omniORBpy 向けのアプリケーションのためにこの綴りの両方を持つ

JacORB とは確かめていない（相互接続の優先順位の後のほう）。
