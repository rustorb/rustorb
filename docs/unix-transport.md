# Unix ドメインソケット（`giop:unix`）

[#157](https://github.com/rustorb/rustorb/issues/157)（計画 [#150](https://github.com/rustorb/rustorb/issues/150)）。omniORB 独自のトランスポート `giop:unix` を入れる。同じホストのプロセス同士を、TCP を通さずに Unix ドメインソケットでつなぐ。GIOP のメッセージはそのまま流れ、変わるのは下のストリームだけ。

対応 OS は Linux と macOS（Unix 系）。Windows の AF_UNIX は tokio が扱わないので対象外で、Windows では `-ORBendPoint giop:unix:...` が `BAD_PARAM` になる。

omniORB との互換は、既存 ORB のソースを読まない方針（[development-plan.md](development-plan.md)）に従い、omniORB C++ / omniORBpy 4.3.4 を動かして外から見た結果（catior、トレース、作られたファイル）と相互接続だけを根拠にした。ソースは読んでいない。

## omniORB の振る舞い（観察した結果）

omniORBpy 4.3.4 のサーバーとクライアントを Docker の中で動かして確かめた。

### IOR

`-ORBendPoint giop:unix:/sock/a.sock` だけのサーバーの IOR（catior）：

```
1. IIOP 1.2 9edd55fb91d8 0 POA(root) ...
      TAG_ORB_TYPE omniORB (ATT\x00)
      TAG_CODE_SETS ...
      TAG_OMNIORB_UNIX_TRANS 9edd55fb91d8 /sock/a.sock
```

- プロファイルは IIOP 1.2 が1つ。ホストはホスト名（`gethostname`）、ポートは 0
- ソケットはタグ `0x41545402`（`TAG_OMNIORB_UNIX_TRANS`）のコンポーネント。中身はカプセル化（バイト順の1オクテットの後）に CDR の `string` が2つ：ホスト名、ソケットのパス
- TCP の待ち受けもあるときは、プロファイルのホストとポートが TCP のもの（`-ORBendPoint giop:tcp::` なら IP アドレス）になり、`TAG_OMNIORB_UNIX_TRANS` のホスト名はやはり `gethostname` のもの
- 同じパスの待ち受けを2回書いても、ソケットとコンポーネントは1つ

### ソケットのファイル

- `giop:unix:PATH`：PATH にソケットを作る。PATH の親ディレクトリがないと `INITIALIZE`（ディレクトリは作らない）
- `giop:unix:`（パスなし）：`/tmp/omni-USER/` にソケットを作る。USER はパスワードデータベースのユーザー名で、`$USER` も `$TMPDIR` も見ない。ディレクトリがなければ作る（モードは 0755 を umask で削ったもの）。ファイル名は `PID-SECONDS`（プロセス ID を9桁のゼロ埋め、UNIX 時刻の秒）。例：`/tmp/omni-root/000000007-1794697815`
- `giop:unix:DIR/`（`/` で終わる）：DIR に `PID-SECONDS` のソケットを作る
- ソケットのモードは umask にかかわらず 0777。誰が届くかはディレクトリで決まる
- ORB を壊す（`destroy`）とソケットのファイルを消す。ディレクトリは残す。プロセスが落ちたときはファイルが残る
- PATH に何かあると、それを消してから作る。前のプロセスが残したソケットだけでなく、普通のファイルも消された

### クライアントがどれを使うか

- 既定の `clientTransportRule` は `* unix,ssl,tcp`。`TAG_OMNIORB_UNIX_TRANS` のホスト名が自分のホスト名と同じときは、Unix ソケットを最初に使う（トレース：`Client attempt to connect to giop:unix:/tmp/s.sock`）
- ホスト名が違えば（別のコンテナから同じソケットのファイルが見えていても）使わず、TCP にする
- ソケットにつながらない（ファイルがない）ときは、TCP のアドレスに切り替える（`Switch rope to use address giop:tcp:...`）
- `-ORBclientTransportRule "* tcp"` にすると Unix ソケットを使わない

## rustorb の作り

### 待ち受け（サーバー）

`crates/rustorb-core/src/unix.rs`。パスの決め方は omniORB に合わせ、危ないところだけ変えた。

- `giop:unix:`、`giop:unix:PATH`、`giop:unix:DIR/` の意味は omniORB と同じ。既定の名前は `PID-SECONDS` で、同じプロセスの2つ目からは `PID-SECONDS-N` を付けて重ならないようにする
- PATH にソケットがあり、つないでも誰も応えない（前のプロセスが残した）ときは消して作り直す
- **omniORB と違うところ**：PATH にあるのがソケットでない（普通のファイルなど）か、別のプロセスが待ち受けているソケットのときは、消さずに `INITIALIZE` にする。omniORB のように消すと、パスを書き間違えただけでファイルを失い、動いているサーバーを黙って乗っ取ってしまうため
- ソケットのモードは omniORB と同じ 0777
- ORB を止める（`shutdown`）とき、待ち受けを止めてすぐにソケットのファイルを消す。消すのは自分が作ったファイル（デバイスと i ノードが同じ）のときだけで、後から別のプロセスが同じパスに作り直していれば残す
- 同じパスを2回書いても待ち受けは1つ
- `-ORBendPoint` が `giop:unix` だけなら、TCP の待ち受けは作らない（既定の TCP の待ち受けを足すのは、待ち受けが1つも書かれていないときだけ）
- `-ORBendPointPublish` は TCP と TLS のためのもので、Unix ソケットには関係しない

### IOR

最初の IIOP 1.2 プロファイルに、`giop:unix` の待ち受けごとに `TAG_OMNIORB_UNIX_TRANS` を1つ足す（`rustorb_giop::UnixTrans`）。ホスト名は `gethostname` のもの。コンポーネントの順は、コードセット、`TAG_SSL_SEC_TRANS`、`TAG_OMNIORB_UNIX_TRANS`、代替アドレス、IOR インターセプタの分。TCP も TLS もない ORB では、プロファイルのホストはホスト名、ポートは 0（omniORB と同じ）。

### クライアントが選ぶアドレス

プロファイルごとに、使う順に：

1. `TAG_OMNIORB_UNIX_TRANS` のうち、ホスト名が自分の `gethostname` と同じもの（大文字小文字は区別しない）
2. 今までどおり：TLS の待ち受け（[ssliop.md](ssliop.md)）、または TCP のアドレスと代替アドレス。ポート 0 は使わない

Unix ソケットにつながらなければ次のアドレスを試すので、ソケットが消えていても TCP で届く。別のホストの Unix ソケットしかない IOR は、使えるアドレスがないので `TRANSIENT`。omniORB の `clientTransportRule` は入れていないので、Unix ソケットを使わせない設定はない（同じホストなら TCP より速く、困る場面が見当たらないため）。

接続はパスごとに1本を使い回し、TCP と同じく `-ORBoutConScanPeriod` / `-ORBinConScanPeriod` で閉じる（[connections.md](connections.md)）。自分のサーバーの Unix ソケットを指す呼び出しは、TCP と同じく同一 ORB 内の呼び出しになる。

### 接続の層

TCP と Unix ソケットの違いは `crates/rustorb-core/src/sock.rs` に閉じ込めた。`Stream`・`ReadHalf`・`WriteHalf` が tokio の `TcpStream` / `UnixStream` とその半分を包み、接続の読み書き（[read-ownership.md](read-ownership.md) のスレッドによる読み取りと `poll` を含む）、`Outbox`、idle close、`-ORBmaxServerConnections` は、どちらかを気にせずに動く。

- TLS は TCP の上だけ。`giop:ssl` と `giop:unix` は同じ ORB に並べられ、クライアントは同じホストなら Unix ソケットを先に使う（omniORB の既定の規則 `unix,ssl,tcp` と同じ順）
- 双方向 GIOP（[bidir-giop.md](bidir-giop.md)）は Unix ソケットの接続でも使える。`BI_DIR_IIOP` の listen point は IIOP（ホストとポート）なので、クライアントは TCP の待ち受けを持ち、サーバーはそのアドレスへの呼び出しを、クライアントが開いた Unix ソケットの接続に流す
- トレースでは相手を `unix:PATH` と書く。サーバー側では相手のアドレスがないので `an unknown peer`
- `SO_SNDBUF` / `SO_RCVBUF`（`socket_send_buffer` / `socket_receive_buffer`）は Unix ソケットにも効く。`TCP_NODELAY` は TCP だけ

## 設定

| 引数 | `OrbOptions` | 意味 |
| --- | --- | --- |
| `-ORBendPoint giop:unix:PATH` | `unix_endpoints` | PATH に Unix ソケットを作って待ち受ける（繰り返せる）。PATH が空なら `/tmp/omni-USER/` に、`/` で終わればその中に新しい名前で作る |

`Orb::unix_endpoints()` は待ち受けているソケットのパス。Python（`CORBA.ORB_init(argv)`）・C++（`CORBA::ORB_init(argc, argv)`）・Java（`ORB.init(args, props)`）は、どれも引数をコアに渡すので、バインディングに追加の API はない。

## テスト

- `crates/rustorb-core/tests/unix.rs`：IOR の形、ソケットのモードと後始末、既定のパスと `DIR/`、残ったソケット・使われているソケット・普通のファイル・ないディレクトリ、大きなメッセージ、同一 ORB 内の呼び出し、クライアントの選び方（自分のホスト・他のホスト・消えたソケット、テストが作った待ち受けで、どちらに接続が来たかを見る）、双方向 GIOP
- `crates/rustorb-giop/tests/ior.rs`：omniORBpy 4.3.4 が出した IOR の `TAG_OMNIORB_UNIX_TRANS` を読み、同じバイト列に書き戻せること
- `python/tests/test_unix.py`：Python の ORB 引数
- 相互接続：`interop/run-matrix.sh --unix`。サーバーとクライアントのコールバックの待ち受けを `giop:unix` だけにし、2つのコンテナに同じホスト名と、ソケットを置くボリュームを共有させる（ネームサービスは TCP のまま）。JacORB は `giop:unix` がないので外す。omniORB C++ / omniORBpy と rustorb（Rust・Python・C++・Java）の全組み合わせが、TCP のときと同じ既知の失敗（omniORBpy の `wchar` など）を除いて通る。CI（`interop.yml`）で回している

## 扱わないこと

- Windows の AF_UNIX
- `corbaloc:omniunix:...`（omniORB の corbaloc の書き方）。今までどおり読めるが、使えるアドレスとしては扱わない
- omniORB の `clientTransportRule` / `serverTransportRule`
- Linux の抽象名前空間のソケット
