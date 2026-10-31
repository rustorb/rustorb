# SSLIOP（IIOP over TLS）

[#78](https://github.com/rustorb/rustorb/issues/78)。GIOP の接続を TLS の上に張る。CORBA Security の SSLIOP の範囲で、IOR の `TAG_SSL_SEC_TRANS` コンポーネントで TLS の待ち受けを知らせる。CSIv2（`TAG_CSI_SEC_MECH_LIST`、`TAG_TLS_SEC_TRANS`）とセキュリティサービスの API は扱わない（[development-plan.md](development-plan.md#目的とスコープ)）。

omniORB C++ と omniORBpy の SSL トランスポートとは、両方向でつながることを確かめた。確かめ方は omniORB のマニュアルと、実際に動かした結果だけ（catior、相互接続のマトリクス）。

## TLS の実装

[rustls](https://github.com/rustls/rustls) 0.23（暗号は `ring`、TLS 1.2 と 1.3）を使う。ライセンスは rustls が Apache-2.0 / ISC / MIT、ring が Apache-2.0 AND ISC、webpki が ISC で、コアの MIT OR Apache-2.0 と両立する。OpenSSL には依存しない。

接続の読み書きは今までの作りのまま変えない（[read-ownership.md](read-ownership.md)）。rustls は sans-I/O なので、ソケットとの間に挟む層を `crates/rustorb-core/src/tls.rs` に書いた。

- 読む：接続の読み手（reader task、またはスレッド）がソケットから読む関数を `Tls::read` に渡す。`Tls::read` は rustls に平文が残っていればそれを返し、なければ暗号文を読んでレコードを解く。`WouldBlock` を返すのは、平文が残っておらず、ソケットにも何もないときだけ。だから、これまでどおり `WouldBlock` の後でソケットを待てばよい
- 書く：メッセージは `Tls::encrypt` で丸ごと暗号化し、レコードは rustls の中に順に溜まる。`Tls::flush` は、ソケットが受け取るだけ書く。どのスレッドが書き出しても、レコードは作られた順にソケットへ出る。サーバーの `Outbox` は、書き切れなかったレコードを writer task に任せる
- 握手：接続（クライアント）と accept（サーバー）の直後、読み書きを分ける前に行う。クライアントでは接続のタイムアウトに含め、サーバーは 30 秒で打ち切る
- 閉じる：`CloseConnection` の後に `close_notify` を送る。相手が `close_notify` なしで TCP を閉じたときは、平文の EOF と同じように扱う
- 期限：TLS のクライアントが送る途中で期限が来たら、接続を閉じる。暗号化したレコードは取り消せないので、平文のときのように「まだ何も書いていない」状態に戻れないため

## IOR

`TAG_SSL_SEC_TRANS`（タグ 20）のデータは `SSLIOP::SSL`、つまり `target_supports`・`target_requires`（どちらも `Security::AssociationOptions`、`unsigned short`）と `port`。TLS の待ち受けは、IIOP プロファイルのホストのこのポートにある。`rustorb_giop::SslSecTrans` で読み書きする。

omniORB 4.3.4 の IOR（catior）：

- TLS だけ：プロファイルのポートが 0、`TAG_SSL_SEC_TRANS port = N supports 102 requires 102`
- TCP と TLS の両方：プロファイルのポートは TCP のもの。コンポーネントは同じ値（`requires` は TLS で接続したときの条件で、TCP を断る意味ではない）

102 は Integrity・Confidentiality・EstablishTrustInTarget・EstablishTrustInClient。rustorb が出す IOR：

| | `target_supports` | `target_requires` |
| --- | --- | --- |
| TLS だけ | Integrity・Confidentiality・TrustInTarget・TrustInClient（102） | Integrity・Confidentiality・TrustInTarget（38） |
| TCP もある | 上に NoProtection を足す（103） | 同じ |
| クライアント認証が必須 | | TrustInClient を足す（102） |

- 最初のプロファイルは、最初の TCP の待ち受け（なければ最初の TLS の待ち受けのホストと、ポート 0）。最初の TLS の待ち受けは、ホストが同じならこのプロファイルのコンポーネントにする
- 2つ目以降の TCP の待ち受けは今までどおり `TAG_ALTERNATE_IIOP_ADDRESS`。そこに TLS は付けられないので、2つ目以降の TLS の待ち受け（とホストの違う最初の TLS の待ち受け）は、それぞれポート 0 のプロファイルにする

## クライアントが選ぶアドレス

プロファイルごとに：

- `TAG_SSL_SEC_TRANS` があり、ORB に CA（`-ORBsslCAFile`）が設定されている：TLS の待ち受けだけを使う。TLS がつながらなくても、平文には落とさない（落とすと、TLS を妨げるだけで平文にさせられる）
- それ以外：今までどおり、プロファイルのアドレスと代替アドレス。ポート 0 は使わない

TLS だけのサーバーを CA のないクライアントが呼ぶと、使えるアドレスがないので `TRANSIENT`。接続はホスト・ポート・TLS かどうかで使い回す。自分のサーバーの TLS の待ち受けへの呼び出しは、平文と同じく同一 ORB 内の呼び出しになる。

## 設定

`OrbOptions` と ORB の引数。Python（`CORBA.ORB_init(argv)`）・C++（`CORBA::ORB_init(argc, argv)`）・Java（`ORB.init(args, props)`、プロパティは `ORBsslCAFile` などの名前）は、どれもこの引数をコアに渡すので、バインディングに追加の API はない。

| 引数 | `OrbOptions` | 意味 |
| --- | --- | --- |
| `-ORBendPoint giop:ssl:HOST:PORT` | `tls_endpoints` | TLS の待ち受け（繰り返せる）。HOST と PORT は TCP と同じく省ける |
| `-ORBendPointPublish giop:ssl:HOST:PORT` | `tls_endpoint_publish` | 最初の TLS の待ち受けとして IOR に出すアドレス |
| `-ORBsslCAFile PATH` | `tls.ca_file` | 相手の証明書を確かめる CA（PEM）。クライアントはこれがないと TLS を使わない |
| `-ORBsslCertFile PATH` | `tls.cert_file` | 自分の証明書と中間 CA（PEM）。なければ鍵のファイルから読む |
| `-ORBsslKeyFile PATH` | `tls.key_file` | 自分の秘密鍵（PEM、暗号化なし）。証明書も一緒に入れてよい（omniORB の鍵ファイルと同じ形） |
| `-ORBsslVerifyClient none\|optional\|required` | `tls.client_auth` | サーバーがクライアントの証明書を求めるか（既定 `none`） |
| `-ORBsslVerifyHost 0\|1` | `tls.verify_host` | クライアントが、サーバーの証明書が IOR のホスト名のものか確かめるか（既定 1。チェーンはどちらでも確かめる） |

- TCP の待ち受け（`-ORBendPoint giop:tcp:`）がなく TLS の待ち受けだけなら、平文では待ち受けない。どちらもなければ、今までどおり TCP の待ち受けを1つ
- TLS の待ち受けに鍵がない、ファイルが読めない、などは RootPOA を使うときに `INITIALIZE`。クライアントの設定が読めないときは、TLS の呼び出しが `TRANSIENT`
- クライアントは自分の証明書（鍵のファイル）があれば、求められたときに送る。omniORB は既定で、相手に証明書を求める（両方向）
- SNI は送らない。暗号化した鍵（omniORB の `key_file_password`）、CRL、OCSP は扱わない
- omniORB の `corbaloc:ssliop:` は扱わない

## テスト

- `crates/rustorb-core/tests/tls.rs`：TLS だけのサーバーの IOR（ポート 0 とコンポーネント）、1 MiB のメッセージと 8 スレッドからの呼び出し、同一 ORB 内の呼び出し、CA のないクライアントは `TRANSIENT`、TCP と TLS の両方、クライアント認証（必須・任意）、別の CA の証明書は断る、ホスト名の検査とその無効化、鍵のない TLS の待ち受けは `INITIALIZE`、ORB の引数
- `crates/rustorb-giop/tests/ior.rs`：omniORBpy の IOR の `TAG_SSL_SEC_TRANS`
- `python/tests/test_tls.py`：Python の `ORB_init` の引数で、クライアント証明書あり・なし、TLS なし、別の CA
- 証明書：`crates/rustorb-core/tests/tls`（`make-certs.sh` で作り直せる。2つの CA、サーバー・クライアント・別名・別の CA のサーバー。100 年有効。テスト用の鍵も入っている）
- 相互接続：`interop/run-matrix.sh --ssl`。サーバーとクライアントのコールバックは TLS だけで待ち受け、証明書は両方向で必須。omniORB C++・omniORBpy・rustorb（Rust・Python・C++・Java）の組すべて。失敗は omniORBpy の wchar の既知のものだけ。CI で回す。omniORB のイメージは OpenSSL 付きでビルドする（`--with-openssl`）
