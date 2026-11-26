# omniORB との比較（機能と速度）

Nov 26, 2026 · @ysuga · 0.7.1 の後（33fe439 に `interop/bench` を足した時点）。実機（macOS、Apple M5 Max）の結果は 23ae7e3 で測って足した

rustorb と omniORB（C++ と omniORBpy）を、機能と速度で比べる。速度は、同じ IDL と同じ呼び方のベンチマークを両方の ORB と rustorb の全言語で書き（[interop/bench](../interop/bench/README.md)）、同一プロセス、別プロセス、rustorb と omniORB の相互接続、omniORB どうし、言語ごと、データの大きさ、速い周期、oneway、並行、接続の確立、CPU とメモリを測った。

omniORB はバイナリとしてだけ使った（[clean-room 方針](clean-room-policy.md)）。下の omniORB の機能の欄は、omniORB の公開の文書と、動かして確かめた結果（初期参照の一覧など）による。ソースは読んでいない。

## まとめ

- **小さな呼び出しの往復**（ループバックの TCP、別プロセス）は、omniORB どうし 30 µs に対し rustorb どうし 47 µs（1.6 倍）。rustorb と omniORB を組むと 39 µs 前後で、クライアントとサーバーのどちらの側も rustorb だと 9 µs ほど延びる。p99 も同じ比
- **同一プロセス**では omniORB は呼び出しを直接サーバントに渡し（0.08 µs）、rustorb は同じプロセスでも CDR に組み立てて渡すので 3.3 µs。40 倍の差で、いちばん大きい
- **大きなデータ**（1 MiB の echo）は、omniORBpy どうし 6.3 GB/s、omniORB C++ どうし 1.8 GB/s に対し rustorb どうし 1.0 GB/s。相手を omniORB にすると、rustorb がクライアントのとき 1.3〜3.2 GB/s、サーバーのとき 2.8〜3.0 GB/s で、クライアント側の差が大きい
- **oneway**は、サーバーが rustorb だと毎秒 5.8 万回前後（相手を問わず）、omniORB C++ だと 13〜23 万回。サーバーの 1 回あたりの CPU が 28 µs と 7〜10 µs
- **一つの参照を 4 スレッドから**呼ぶと、omniORB C++ どうしは毎秒 20 万回、rustorb どうしは 2.8 万回（7 倍）。rustorb のクライアントはすべての呼び出しを 1 本の接続に多重化し、omniORB のクライアントは同時の呼び出しごとに接続を開く（omniORB の文書の `oneCallPerConnection`・`maxGIOPConnectionPerServer`）。4 プロセスからでは 13.6 万回と 4.9 万回
- **CPU**は 1 回の ping でクライアント 35 µs・サーバー 41 µs（omniORB は 15 µs・15 µs）。上の差はおもに 1 回あたりのコスト
- **rustorb が速いところ**：プロセスの起動から 1 回目の返事まで（Rust 2.6 ms、omniORB C++ 6.4 ms）、最初の呼び出し（接続の確立込みで 289 µs と 437 µs）、起動時のメモリ（4.6 MiB と 8.2 MiB）。rustorb のクライアントから omniORB C++ に oneway を送ると、omniORB C++ のクライアントより速く送れる（18〜23 万回と 13 万回）
- **言語ごと**：rustorb の C++・Java・C#・C・Python は小さな呼び出しでは Rust とほぼ同じ（Python のサーバーは GIL で遅い）。大きな octet 列は C（1 MiB 185 MB/s）と Ruby（10 MB/s、受け取りが Integer の Array）が遅い。Java はプロセスの起動と最初の呼び出しが遅い（417 ms、6.6 ms）
- **giop:unix** は omniORB C++ では TCP より速い（23 µs）が、rustorb では TCP より遅い（60 µs）

ここまではクラウドの 4 コアの仮想マシンでの数字。実機（MacBook Pro、Apple M5 Max、macOS）で同じベンチを回した結果は「[実機での結果](#実機での結果macosapple-m5-max)」にある。小さな呼び出しの比（rustorb どうしが omniORB C++ どうしの 1.7 倍）と CPU の比はほぼ同じで、大きなデータは順位が入れ替わり（rustorb どうし 8.6 GB/s、omniORB C++ どうし 3.0 GB/s）、4 スレッドの差は 7 倍から 2.2 倍に縮んだ。

差の原因の見立てと改善の候補は「[差の見立てと改善の候補](#差の見立てと改善の候補)」にまとめた。どれもまだ確かめていない見立てで、直すかどうかは別に決める。

## 機能の比較

記号：✅ あり、🟡 一部、❌ なし、？ 確かめていない

| 機能 | rustorb | omniORB C++ | omniORBpy | メモ |
| --- | --- | --- | --- | --- |
| 言語 | Rust、C++、Python、Java、C#、C、Ruby | C++ | Python | rustorb の C++ は IDL to C++11、omniORB は従来の C++ マッピング |
| GIOP 1.0〜1.2、IIOP | ✅ | ✅ | ✅ | |
| SSLIOP | 🟡（rustls、CRL・OCSP なし） | ✅（OpenSSL） | ✅ | |
| `giop:unix` | ✅（Linux・macOS） | ✅ | ✅ | rustorb は omniORB の形を外から測って合わせた |
| HTTP のトランスポート | ❌ | ✅（`libomnihttpTP4` がある） | ？ | |
| ZIOP（圧縮） | ❌ | ✅（`libomniZIOP4` がある） | ？ | |
| 双方向 GIOP | ✅ | ✅ | ✅ | 相互接続を確認済み |
| 同一プロセスの呼び出し | CDR に組み立てて渡す | サーバントを直接呼ぶ | サーバントを直接呼ぶ | 下の「同一プロセス」 |
| クライアントの接続 | 相手ごとに 1 本に多重化 | 同時の呼び出しごとに 1 本（上限あり、文書による） | 同左 | 下の「並行」 |
| サーバーのスレッド | I/O タスク＋アップコールのプール（leader/follower） | 接続ごとのスレッドとプールの切り替え（文書による） | 同左 | |
| メッセージの最大長の既定 | 64 MiB | 2 MB（文書による） | 同左 | ベンチでは両方 64 MiB にした |
| POA（7 種のポリシー、servant manager など） | ✅ | ✅ | ✅ | |
| `POAManagerFactory`（CORBA 3） | ✅ | ❌（初期参照にない） | ❌ | |
| Any・TypeCode | ✅ | ✅ | ✅ | |
| DynAny | ✅（抽象インターフェースを除く） | ✅ | ❌（[dynany.md](dynany.md)） | |
| valuetype、抽象インターフェース | ✅ | ✅ | ✅ | |
| custom valuetype | ✅ | ❌ | ❌ | interop の custom スイートは rustorb と JacORB だけ |
| `long double` | ✅（binary128） | 🟡（x87 の形式で送る） | ？ | [long-double.md](long-double.md) |
| DII / DSI | ✅（全言語） | ✅ | ❌（`_request`・`create_list` がない） | |
| Portable Interceptors（標準の API） | ✅ | ❌（独自のインターセプター） | ❌（独自の `omniORB.interceptors`） | `PICurrent`・`CodecFactory` は初期参照にない（4.3.4 で確認） |
| コールバック型の AMI | ✅（C++、Java、Python、C#、C。Rust は async） | ✅（文書による） | ？ | [ami.md](ami.md) |
| ポーリング型の AMI | ❌ | ✅（文書による） | ？ | |
| 呼び出しのタイムアウト | ✅（標準の `RelativeRoundtripTimeoutPolicy` と omniORB 互換の API） | ✅（独自の API） | ✅（独自の API） | [timeouts.md](timeouts.md) |
| PolicyManager・PolicyCurrent、`SyncScopePolicy` | ✅ | ❌（初期参照にない） | ❌ | |
| ネーミング | ✅（`rustorb-names`、永続化あり） | ✅（omniNames） | ✅ | 相互に置き換えて動くことを確認済み |
| Event / Notification | ❌ | 別のプロジェクト（omniEvents、omniNotify） | 同左 | |
| Interface Repository | ❌ | ❌（`NO_RESOURCES`） | ❌ | |
| omniORBpy の API（`omniORB` パッケージ） | ✅（互換パッケージ、[omniorb-compat.md](omniorb-compat.md)） | — | ✅ | OpenRTM-aist-Python がそのまま動く |
| ライセンス | MIT OR Apache-2.0（生成器は GPL） | LGPL（ツールは GPL） | 同左 | |

rustorb の対応状況の詳細は [corba-compliance.md](corba-compliance.md)。

## 測り方

### 環境

開発用のクラウドのサンドボックス（共有の仮想マシン）。実機の結果ではない（実機は「[環境（実機）](#環境実機)」）。

- Intel Xeon Processor @ 2.10GHz、論理 CPU 4、メモリ 16 GiB、Linux 6.18（Ubuntu 24.04）
- rustc 1.97.0（release ビルド）、Python 3.13.18、OpenJDK 25、.NET 8、Ruby 3.3.6、g++ 13
- omniORB 4.3.4 / omniORBpy 4.3.4 を配布物（tar.bz2、sha256 を確認）からビルドして `/opt/omniorb` に入れた
- クライアントとサーバーは同じマシンで、ループバックの TCP（`giop:tcp:127.0.0.1:`）。4 コアを取り合うので、スレッドやプロセスを増やす測り方はコア数で頭打ちになる
- すべての ORB に `-ORBgiopMaxMsgSize 67108864`

各ケースは 0.3 秒の慣らしのあと 2 秒、すべての呼び出しの時間を測る。別プロセスでは、組ごとにサーバーを起動し直してケースごとに新しいクライアントのプロセスで測る。これを 3 回くり返し、表は 3 回の中央値。3 回の幅（最大と最小の差）は、相互接続の 25 組で中央値の 9%（全ケースの中央）で、9 割のケースが 25% 以内だった。4 プロセスと 1 MiB 以上の octet 列はばらつきが大きく、2 倍近く違う回もあったので、1〜2 割の差は差と見ない。CPU 時間はクライアント（何もしないクライアントのプロセスの分を引く）とサーバーの、ケースの間の分を、慣らしを含む呼び出しの数で割った。

### プログラム

全実装が同じ `Bench::Target`（[Bench.idl](../interop/bench/Bench.idl)）を提供し、同じケースを同じ引数と出力で回す。C++ は omniORB 用（従来のマッピング）と rustorb 用（C++11 マッピング）で中身が同じプログラム、Python は一つのスクリプトを両方の ORB で動かす。

| ケース | 1 回の呼び出し |
| --- | --- |
| `ping` | `void ping()` |
| `long` / `string` | `echo_long`、100 文字の `echo_string` |
| `octets:N` | N バイトの `sequence<octet>` を送って同じものを返す（MB/s は往復の合計） |
| `put:N` / `get:N` | N バイトを送って `unsigned long` を返す / N バイトを返す |
| `samples:100` | 100 個の struct（`long`、`double` 3 つ、短い文字列）の列を echo |
| `any` | その struct 1 つを入れた any を echo |
| `oneway` | `oneway post(long)` を 2 秒送り続け、全部届くまで（`take_posted()` で確かめる） |
| `threads:4` | 一つの参照に 4 スレッドから `echo_long` |
| `procs:4` | 4 つのクライアントのプロセスから同時に `echo_long` |
| `first` | 新しいプロセスでの最初の `ping`（接続の確立を含む）。10 プロセスの中央値 |

## 結果

### 同一プロセス

サーバントと呼び出し側が同じ ORB にいるとき（`inproc`）。数字は 1 回の時間の中央値（µs）か毎秒の回数。

| | rustorb (Rust) | rustorb C++ | rustorbpy | rustorb Java | rustorb C# | rustorb C | rustorb Ruby | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| ping（µs） | 3.33 | 3.51 | 5.54 | 3.92 | 3.83 | 3.60 | 6.13 | 0.08 | 0.48 |
| echo_long（µs） | 3.50 | 3.58 | 6.26 | 4.16 | 4.07 | 3.85 | 9.65 | 0.09 | 0.59 |
| any（µs） | 9.45 | 10.5 | 66.8 | 13.6 | 11.8 | 9.26 | 35.7 | 0.28 | 1.92 |
| struct 100 個（µs） | 54.3 | 46.4 | 233 | 167 | 56.4 | 71.2 | 2,283 | 4.97 | 46.8 |
| octet 1 KiB（µs） | 3.94 | 4.20 | 7.45 | 5.42 | 5.44 | 13.5 | 241 | 0.48 | 0.64 |
| octet 1 MiB（µs） | 1,361 | 1,439 | 1,140 | 612 | 730 | 10,122 | 206,896 | 744 | 0.64 |
| octet 8 MiB（MB/s） | 1,241 | 1,274 | 1,895 | 2,527 | 1,303 | 188 | 10.2 | 2,833 | — |
| oneway（回/s） | 301,672 | 290,649 | 144,470 | 252,322 | 243,511 | 263,122 | 108,214 | 15,647,345 | 1,305,489 |
| 4 スレッド（回/s） | 558,404 | 561,465 | 17,836 | 516,761 | 481,318 | 516,414 | 13,004 | 1,437,965 | 35,165 |
| CPU / ping（µs） | 3.70 | 3.99 | 6.33 | 7.82 | 4.83 | 4.11 | 7.88 | 0.15 | 0.79 |

- omniORB はサーバントの関数を直接呼ぶ。C++ では `in` の引数を参照で渡すだけで、返り値の octet 列のコピー（1 MiB で 744 µs）だけが残る。omniORBpy は `bytes` をそのまま返すので大きさに関係なく 0.64 µs（MB/s は意味がないので —）
- rustorb は同じプロセスでもリクエストを CDR に組み立て、サーバー側の経路（POA の検索、アップコール）を通すので、最小で 3.3 µs かかる。ソケットは使わない
- rustorb の Rust・C++ では、octet 列が 16 KiB の 6 µs から 256 KiB で 386 µs に跳ぶ。大きな領域の確保（ページの割り当て）が毎回起きていると見ている（未確認）
- C と Ruby の octet 列は大きさに比例して遅い（下の「言語ごと」）

### 別プロセス：同じ実装どうし

| 組 | ping p50（µs） | ping p99（µs） | any（µs） | struct 100（µs） | 1 MiB（MB/s） | 8 MiB（MB/s） | oneway（回/s） | 4 スレッド（回/s） | 4 プロセス（回/s） |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 47.3 | 88.2 | 62.7 | 123 | 994 | 844 | 58,518 | 27,698 | 48,949 |
| rustorb C++ | 49.8 | 99.3 | 68.1 | 103 | 946 | 855 | 58,355 | 28,670 | 51,837 |
| rustorbpy | 75.2 | 135 | 173 | 348 | 1,133 | 1,013 | 14,016 | 12,923 | 12,144 |
| rustorb Java | 52.3 | 102 | 68.3 | 250 | 1,423 | 1,428 | 59,847 | 26,586 | 48,326 |
| rustorb C# | 48.5 | 86.4 | 65.5 | 138 | 1,209 | 1,361 | 58,914 | 27,042 | 48,363 |
| rustorb C | 48.0 | 83.2 | 59.6 | 129 | 185 | 176 | 60,434 | 28,074 | 49,223 |
| rustorb Ruby | 80.1 | 150 | 144 | 2,652 | 10.0 | 10.1 | 22,488 | 14,510 | 17,921 |
| omniORB C++ | 30.0 | 62.8 | 39.3 | 57.0 | 1,799 | 1,416 | 134,201 | 201,937 | 135,951 |
| omniORBpy | 35.9 | 76.6 | 53.4 | 167 | 6,323 | 2,999 | 71,287 | 34,574 | 42,571 |

### 相互接続：rustorb と omniORB の組み合わせ

行がクライアント、列がサーバー。対角が同じ実装どうし（omniORB どうしは右下の 4 つ）。

ping の往復（p50、µs）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 47.3 | 47.1 | 69.9 | 39.5 | 44.2 |
| rustorb C++ | 48.3 | 49.8 | 68.3 | 41.6 | 42.4 |
| rustorbpy | 52.3 | 56.6 | 75.2 | 43.4 | 45.3 |
| omniORB C++ | 39.0 | 38.6 | 54.7 | 30.0 | 34.6 |
| omniORBpy | 39.6 | 40.5 | 60.5 | 33.3 | 35.9 |

ping の往復（p99、µs）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 88.2 | 87.3 | 125 | 72.2 | 90.3 |
| rustorb C++ | 108 | 99.3 | 121 | 79.9 | 85.5 |
| rustorbpy | 108 | 104 | 135 | 94.6 | 90.3 |
| omniORB C++ | 76.8 | 77.2 | 108 | 62.8 | 78.7 |
| omniORBpy | 74.0 | 81.8 | 114 | 76.4 | 76.6 |

any の往復（p50、µs）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 62.7 | 62.8 | 117 | 52.1 | 57.9 |
| rustorb C++ | 63.0 | 68.1 | 122 | 53.4 | 58.3 |
| rustorbpy | 107 | 110 | 173 | 96.3 | 105 |
| omniORB C++ | 53.3 | 51.1 | 102 | 39.3 | 48.9 |
| omniORBpy | 54.7 | 57.8 | 114 | 45.9 | 53.4 |

1 MiB の octet 列の echo（MB/s、往復の合計）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 994 | 1,045 | 1,842 | 1,322 | 3,223 |
| rustorb C++ | 957 | 946 | 1,821 | 1,329 | 2,917 |
| rustorbpy | 701 | 667 | 1,133 | 1,282 | 2,756 |
| omniORB C++ | 2,793 | 2,567 | 2,934 | 1,799 | 6,684 |
| omniORBpy | 3,020 | 2,636 | 3,035 | 1,825 | 6,323 |

1 MiB を送るだけ（`put`）／受け取るだけ（`get`）（MB/s）

| クライアント＼サーバー | rustorb (Rust) | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 1,875 ／ 1,800 | 1,662 ／ 1,772 | 2,530 ／ 2,825 | 2,540 ／ 2,809 |
| rustorbpy | 1,960 ／ 1,736 | 1,623 ／ 1,675 | 2,451 ／ 2,232 | 2,393 ／ 2,253 |
| omniORB C++ | 3,300 ／ 2,339 | 3,044 ／ 2,171 | 6,459 ／ 5,649 | 6,027 ／ 5,137 |
| omniORBpy | 3,322 ／ 2,346 | 2,751 ／ 2,157 | 6,363 ／ 5,385 | 5,898 ／ 5,230 |

oneway（毎秒の回数、全部届くまで）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 58,518 | 60,094 | 14,045 | 180,599 | 66,870 |
| rustorb C++ | 58,451 | 58,355 | 14,154 | 211,578 | 78,107 |
| rustorbpy | 60,541 | 61,420 | 14,016 | 234,296 | 74,600 |
| omniORB C++ | 55,838 | 56,995 | 14,184 | 134,201 | 57,965 |
| omniORBpy | 62,297 | 61,992 | 14,249 | 138,908 | 71,287 |

一つの参照を 4 スレッドから（毎秒の回数）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 27,698 | 27,889 | 12,554 | 36,916 | 35,121 |
| rustorb C++ | 28,552 | 28,670 | 12,662 | 36,831 | 37,098 |
| rustorbpy | 25,684 | 25,760 | 12,923 | 34,003 | 34,361 |
| omniORB C++ | 65,236 | 68,412 | 13,659 | 201,937 | 32,604 |
| omniORBpy | 50,422 | 50,128 | 13,419 | 52,538 | 34,574 |

4 プロセスから同時に（毎秒の回数の合計）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 48,949 | 49,941 | 12,805 | 85,205 | 43,596 |
| rustorb C++ | 48,277 | 51,837 | 12,652 | 89,046 | 42,200 |
| rustorbpy | 47,435 | 46,926 | 12,144 | 75,095 | 41,156 |
| omniORB C++ | 81,468 | 76,349 | 13,148 | 135,951 | 36,535 |
| omniORBpy | 74,886 | 75,791 | 12,948 | 104,728 | 42,571 |

- 小さな呼び出しでは、rustorb をクライアントにしてもサーバーにしても omniORB どうしより 9 µs ほど遅く、両側が rustorb だとその合計（17 µs）になる
- rustorbpy のクライアントは rustorb (Rust) のクライアントと同じくらいの速さ（Python の部分は 5 µs ほど）。rustorbpy のサーバーは GIL のためアップコールが 1 本ずつになり、oneway や並行の呼び出しでは毎秒 1.4 万回で頭打ちになる
- 大きなデータは、どのサーバーに対しても rustorb のクライアントが omniORB のクライアントの半分以下。サーバーとしては rustorbpy が rustorb (Rust) より速い（1 MiB で 1.8 GB/s と 1.0 GB/s）ので、Rust のサーバーのスタブかディスパッチに余分なコピーがあると見ている
- omniORB C++ のサーバーは大きな echo では omniORBpy のサーバーより遅い（1.8 GB/s と 6.3 GB/s）。`put` と `get` は速い（6 GB/s）ので、受け取った列を返す側（C++ のサーバントでの `new Octets(v)` のコピー）の分と見ている
- oneway は、rustorb のクライアントから omniORB C++ に送ると 18〜23 万回で、omniORB C++ のクライアント（13 万回）より速い。サーバーが rustorb だと相手を問わず 5.6〜6.2 万回
- 4 スレッドでは、omniORB C++ のクライアントは rustorb のサーバーにも 6.5 万回（rustorb のクライアントの 2.4 倍）出る。クライアントの接続の多重化とサーバーの処理の両方に差がある

### 大きさごと

octet 列の echo の 1 回の時間（p50、µs）と速さ（MB/s）。

| クライアント → サーバー（µs） | 64 B | 1 KiB | 16 KiB | 256 KiB | 1 MiB | 8 MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) → rustorb (Rust) | 48.5 | 49.7 | 132 | 552 | 2,044 | 19,468 |
| omniORB C++ → omniORB C++ | 31.7 | 32.6 | 54.9 | 286 | 1,122 | 11,768 |
| omniORBpy → omniORBpy | 39.3 | 39.2 | 50.3 | 107 | 302 | 5,607 |
| rustorb (Rust) → omniORB C++ | 41.8 | 44.6 | 102 | 405 | 1,482 | 13,436 |
| omniORB C++ → rustorb (Rust) | 39.0 | 40.4 | 89.6 | 197 | 712 | 7,923 |
| rustorb (Rust) → omniORBpy | 43.0 | 45.1 | 88.6 | 177 | 634 | 7,338 |
| omniORBpy → rustorb (Rust) | 41.4 | 42.5 | 89.4 | 174 | 658 | 7,802 |

| クライアント → サーバー（MB/s） | 64 B | 1 KiB | 16 KiB | 256 KiB | 1 MiB | 8 MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) → rustorb (Rust) | 2.50 | 38.6 | 245 | 918 | 994 | 844 |
| omniORB C++ → omniORB C++ | 3.72 | 57.7 | 557 | 1,746 | 1,799 | 1,416 |
| omniORBpy → omniORBpy | 3.01 | 49.3 | 582 | 4,660 | 6,323 | 2,999 |
| rustorb (Rust) → omniORB C++ | 2.86 | 42.9 | 305 | 1,231 | 1,322 | 1,232 |
| omniORB C++ → rustorb (Rust) | 3.23 | 47.5 | 347 | 2,568 | 2,793 | 2,085 |
| rustorb (Rust) → omniORBpy | 2.76 | 41.1 | 351 | 2,863 | 3,223 | 2,249 |
| omniORBpy → rustorb (Rust) | 2.82 | 43.2 | 347 | 2,818 | 3,020 | 2,139 |

- 16 KiB で差が開き始める（rustorb どうし 132 µs、omniORB どうし 55 µs）。16 KiB は 1 回の読み書きでは収まらない大きさで、rustorb の読み込みのバッファ（16 KiB の先読み）と関係がありそう
- 8 MiB では、どの組も 1 MiB より遅くなる

### 言語ごと

rustorb の各言語を、同じ言語どうし、rustorb (Rust) のサーバー、omniORB C++ のサーバーと組んだもの（ping は p50 の µs）。

| 言語 | 同一プロセス ping | 同じ言語どうし ping | → rustorb (Rust) ping | → omniORB C++ ping | omniORB C++ → ping | → rustorb (Rust) 1 MiB（MB/s） | → omniORB C++ 1 MiB（MB/s） | omniORB C++ → 1 MiB（MB/s） | → omniORB C++ 4 スレッド（回/s） |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 3.33 | 47.3 | 47.3 | 39.5 | 39.0 | 994 | 1,322 | 2,793 | 36,916 |
| rustorb C++ | 3.51 | 49.8 | 48.3 | 41.6 | 38.6 | 957 | 1,329 | 2,567 | 36,831 |
| rustorbpy | 5.54 | 75.2 | 52.3 | 43.4 | 54.7 | 701 | 1,282 | 2,934 | 34,003 |
| rustorb Java | 3.92 | 52.3 | 49.2 | 42.6 | 35.7 | 865 | 1,170 | 2,520 | 33,285 |
| rustorb C# | 3.83 | 48.5 | 48.4 | 44.5 | 37.9 | 832 | 1,146 | 1,851 | 35,024 |
| rustorb C | 3.60 | 48.0 | 47.1 | 42.3 | 35.2 | 556 | 818 | 278 | 35,344 |
| rustorb Ruby | 6.13 | 80.1 | 52.1 | 43.1 | 71.7 | 79.0 | 81.3 | 11.2 | 18,901 |
| omniORB C++ | 0.08 | 30.0 | 39.0 | 30.0 | 30.0 | 2,793 | 1,799 | 1,799 | 201,937 |
| omniORBpy | 0.48 | 35.9 | 39.6 | 33.3 | 34.6 | 3,020 | 1,825 | 6,684 | 52,538 |

- 小さな呼び出しでは、どの言語もコア（Rust）の速さでほぼ決まる。言語の層の分は Java・C#・C で 1 µs 未満、Python・Ruby で数 µs
- C の octet 列は、受け取り（omniORB C++ → C で 278 MB/s）と同一プロセス（1 MiB で 10 ms）が遅い。生成コードの型記述をたどって 1 要素ずつ読み書きしていると見ている
- Ruby は octet 列を Integer の Array として受け取る（[ruby-binding.md](ruby-binding.md) のマッピング）ので、1 MiB で 0.2 秒かかる。struct の列も遅い（100 個で 2.3 ms）
- Java・C# の大きなデータは、Rust の生成コードより速い場面がある（同一プロセスの 1 MiB で 612 µs と 1,361 µs）。理由は調べていない

### 接続の確立と起動

新しいクライアントのプロセスでの最初の `ping`（接続の確立を含む、10 回の中央値）と、そのプロセスの起動から終わりまで。

| 組 | 最初の呼び出し（µs） | クライアントのプロセス全体（ms） |
| --- | ---: | ---: |
| rustorb (Rust) → rustorb (Rust) | 289 | 2.6 |
| rustorb C++ → rustorb C++ | 372 | 3.1 |
| rustorbpy → rustorbpy | 515 | 45.5 |
| rustorb Java → rustorb Java | 6,650 | 417.1 |
| rustorb C# → rustorb C# | 1,505 | 52.3 |
| rustorb C → rustorb C | 357 | 3.0 |
| rustorb Ruby → rustorb Ruby | 468 | 94.4 |
| omniORB C++ → omniORB C++ | 437 | 6.4 |
| omniORBpy → omniORBpy | 502 | 46.6 |
| rustorb (Rust) → omniORB C++ | 244 | 2.5 |
| omniORB C++ → rustorb (Rust) | 482 | 6.6 |
| rustorbpy → omniORBpy | 374 | 43.9 |
| omniORBpy → rustorbpy | 636 | 45.3 |

- Python のプロセスの 45 ms はほとんどがインタープリターとモジュールの読み込みで、rustorbpy と omniORBpy で同じくらい
- Java の最初の呼び出しの 6.6 ms は、FFM の呼び出しの準備と JIT の前の実行と見ている

### giop:unix

同じ実装どうしの TCP（ループバック）と Unix ドメインソケット。

| 組 | ping tcp（µs） | ping unix | 1 MiB tcp（MB/s） | 1 MiB unix | oneway tcp（回/s） | oneway unix | 4 スレッド tcp | 4 スレッド unix |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 47.3 | 60.4 | 994 | 808 | 58,518 | 69,504 | 27,698 | 32,962 |
| rustorb C++ | 49.8 | 61.1 | 946 | 813 | 58,355 | 70,037 | 28,670 | 31,342 |
| rustorbpy | 75.2 | 90.1 | 1,133 | 998 | 14,016 | 13,431 | 12,923 | 12,527 |
| omniORB C++ | 30.0 | 23.3 | 1,799 | 1,729 | 134,201 | 93,732 | 201,937 | 185,850 |
| omniORBpy | 35.9 | 42.8 | 6,323 | 5,127 | 71,287 | 56,184 | 34,574 | 36,233 |

omniORB C++ の往復は Unix ソケットで TCP より 2 割速くなるが、rustorb は逆に 2〜3 割遅くなる。rustorb の Unix ソケットの経路では、返事をスレッドの上で読む仕組み（[read-ownership.md](read-ownership.md)）が働いていない可能性がある（未確認）。

### CPU とメモリ

1 回あたりの CPU 時間（µs）と、サーバーの常駐メモリ（起動して IOR を出した時点と、全ケースを通しての最大。最大は 8 MiB のケースで決まる）。

| 組 | クライアント CPU / ping | サーバー CPU / ping | クライアント CPU / 1 MiB | サーバー CPU / 1 MiB | クライアント CPU / oneway | サーバー CPU / oneway | サーバー起動時（MiB） | サーバー最大（MiB） |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) → rustorb (Rust) | 34.6 | 41.2 | 714 | 1,800 | 2.45 | 28.5 | 4.6 | 44.7 |
| rustorb C++ → rustorb C++ | 37.5 | 44.7 | 781 | 1,893 | 2.49 | 28.5 | 6.9 | 47.0 |
| rustorbpy → rustorbpy | 43.6 | 66.9 | 1,441 | 754 | 2.90 | 125 | 19.8 | 49.4 |
| rustorb Java → rustorb Java | 45.9 | 47.5 | 1,155 | 902 | 5.09 | 28.2 | 103.5 | 583.7 |
| rustorb C# → rustorb C# | 38.0 | 43.6 | 996 | 1,146 | 3.10 | 29.4 | 28.9 | 231.9 |
| rustorb C → rustorb C | 35.3 | 41.1 | 2,168 | 9,512 | 2.36 | 27.3 | 5.9 | 40.2 |
| rustorb Ruby → rustorb Ruby | 40.5 | 68.3 | 25,279 | 180,833 | 5.24 | 78.8 | 25.0 | 236.9 |
| omniORB C++ → omniORB C++ | 14.9 | 15.4 | 321 | 1,056 | 0.60 | 9.78 | 8.2 | 26.2 |
| omniORBpy → omniORBpy | 17.5 | 19.0 | 278 | 262 | 1.03 | 20.1 | 19.8 | 29.2 |
| rustorb (Rust) → omniORB C++ | 28.3 | 23.3 | 657 | 1,140 | 2.54 | 6.91 | 8.2 | 24.3 |
| omniORB C++ → rustorb (Rust) | 20.6 | 35.7 | 305 | 695 | 0.69 | 29.4 | 4.5 | 49.1 |

- ping 1 回の CPU は、rustorb がクライアントで 2.3 倍、サーバーで 2.7 倍。往復の時間の差（17 µs）より CPU の差（46 µs）の方が大きく、スレッドの起こし合い（I/O タスクと呼び出し側・アップコールのスレッドの受け渡し）にかかっていると見ている
- rustorb のサーバーの oneway 1 回は 28 µs で、omniORB C++ の 3 倍。oneway の上限（毎秒 5.8 万回）はこれで決まる
- メモリは、起動時は rustorb (Rust) の方が小さいが、大きなデータを扱ったあとの最大は omniORB の 1.7 倍。Java・C#・Ruby の最大はランタイムのヒープによる

## 実機での結果（macOS、Apple M5 Max）

上と同じプログラムと同じ測り方を、ふだん使っている Mac で回した結果。クラウドの数字と同じ見方で並べ、ちがいを最後にまとめた。Java・C#・C・Ruby は今回は測っていない（下の「環境」）。

### まとめ（実機）

- **小さな呼び出しの往復**は、omniORB どうし 12.5 µs に対し rustorb どうし 21.4 µs（1.7 倍）。rustorb と omniORB を組むと 18 µs 前後で、rustorb の側ごとに 5 µs 前後延びる。どちらもクラウドの 2.2〜2.4 倍速く、比はクラウド（1.6 倍）とほぼ同じ
- **同一プロセス**は rustorb 1.29 µs、omniORB C++ 0.04 µs（30 倍）
- **大きなデータ**（1 MiB の echo）は、クラウドと逆に rustorb どうし（8.6 GB/s）が omniORB C++ どうし（3.0 GB/s）より速い。omniORB C++ のサーバーは 1 MiB の echo 1 回に CPU 490 µs かかり、相手が誰でも 3 GB/s で頭打ち。omniORBpy どうしは 14.3 GB/s でいちばん速く、omniORBpy のサーバーに対しても rustorb のクライアントは 10.1 GB/s と omniORB のクライアント（14.3 GB/s）の 7 割
- **oneway**は、サーバーが rustorb だと毎秒 19 万回前後（相手を問わず）、omniORB C++ だと 29〜65 万回。サーバーの 1 回あたりの CPU が 17 µs と 2〜3 µs
- **一つの参照を 4 スレッドから**は rustorb どうし 9.4 万回、omniORB C++ どうし 20.5 万回（2.2 倍）。コアが多いのでクラウド（7 倍）より差が小さい。4 プロセスからでは 10.0 万回と 21.0 万回
- **CPU**は 1 回の ping でクライアント 12.5 µs・サーバー 12.2 µs（omniORB C++ は 4.1 µs・4.3 µs、3 倍）
- **rustorb が速いところ**はクラウドと同じ：最初の呼び出し（136 µs と 378 µs）、プロセスの起動から終わり（2.7 ms と 5.1 ms）、起動時のメモリ（2.8 MiB と 3.5 MiB）。これに大きなデータ（上）が加わる。一方、クラウドで rustorb のクライアントの方が速かった omniORB C++ への oneway は、この Mac では omniORB C++ のクライアントの半分（33 万回と 65 万回）
- **giop:unix** は、rustorb も omniORB も小さな呼び出しで TCP より速い（rustorb 15.8 µs、omniORB C++ 6.1 µs）。クラウドで rustorb だけ遅くなったのは、この Mac では起きなかった。代わりに 1 MiB の octet 列はどれも TCP の 1〜4 割に落ちる

### 環境（実機）

- MacBook Pro（Mac17,7）、Apple M5 Max（18 コア：Super 6・Performance 12）、メモリ 128 GiB、macOS 26.5.2、AC 電源
- rustc 1.97.0（Homebrew、release ビルド）、Python 3.14.6（Homebrew）、Apple clang 21
- omniORB 4.3.4 / omniORBpy 4.3.4 をクラウドと同じ配布物（tar.bz2、SourceForge の MD5 と一致）からビルドして `~/.local/omniorb-4.3.4` に入れた。omniORBpy は Python 3.14 向け
- クライアントとサーバーは同じマシンで、ループバックの TCP（`giop:tcp:127.0.0.1:`）。ほかの作業はできるだけ止めて回した
- 測ったのは rustorb (Rust)・rustorb C++・rustorbpy・omniORB C++・omniORBpy。Java と .NET はこの Mac に入っておらず、C は CMake がなく、Ruby は macOS 付属の 2.6 で bench.rb が動かない（3.x の文法）ので測っていない
- 各ケース 0.3 秒の慣らしのあと 2 秒、3 回の中央値（クラウドと同じ）。3 回の幅は相互接続の 25 組で中央値の 1.5%（全ケースの中央）、99% のケースが 25% 以内で、クラウドよりずっと揃っている。外れたのは rustorb どうしの最初の呼び出し（幅 57%）と rustorb C++ どうしの 8 MiB（35%）
- CPU 時間とサーバーのメモリは、macOS には `/proc` がないので `ps` で取った（CPU は 10 ms 単位、メモリは起動時の常駐だけで最大は取れない）。CPU の表はケースを ping・1 MiB・any・oneway に絞って全組を回し直した値

### 同一プロセス（実機）

| | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| ping（µs） | 1.29 | 1.29 | 2.00 | 0.04 | 0.17 |
| echo_long（µs） | 1.33 | 1.33 | 2.17 | 0.04 | 0.21 |
| any（µs） | 3.71 | 4.08 | 22.7 | 0.12 | 0.92 |
| struct 100 個（µs） | 18.8 | 18.8 | 86.4 | 1.71 | 25.4 |
| octet 1 KiB（µs） | 1.58 | 1.67 | 2.63 | 0.38 | 0.25 |
| octet 1 MiB（µs） | 106 | 126 | 115 | 326 | 0.25 |
| octet 8 MiB（MB/s） | 22,239 | 20,268 | 19,308 | 6,433 | — |
| oneway（回/s） | 821,803 | 827,315 | 494,652 | 66,334,581 | 4,572,551 |
| 4 スレッド（回/s） | 375,542 | 412,305 | 83,822 | 3,618,251 | 362,238 |
| CPU / ping（µs） | 1.31 | 1.34 | 2.13 | 0.04 | 0.23 |

- omniORB C++ の 1 MiB の echo（326 µs）は、返り値の列のコピーの分。この Mac ではこのコピーが rustorb の 1 回（106 µs）より遅い。rustorb の 256 KiB 以上の跳ね上がり（クラウドで 386 µs）は、この Mac では見えない（256 KiB で 26 µs）

### 別プロセス：同じ実装どうし（実機）

| 組 | ping p50（µs） | ping p99（µs） | any（µs） | struct 100（µs） | 1 MiB（MB/s） | 8 MiB（MB/s） | oneway（回/s） | 4 スレッド（回/s） | 4 プロセス（回/s） |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 21.4 | 28.4 | 26.5 | 47.6 | 8,635 | 7,007 | 192,050 | 93,912 | 100,175 |
| rustorb C++ | 21.9 | 30.3 | 27.4 | 48.0 | 8,336 | 6,079 | 187,972 | 91,836 | 99,480 |
| rustorbpy | 29.8 | 36.7 | 56.2 | 123 | 8,792 | 8,652 | 82,858 | 71,133 | 69,483 |
| omniORB C++ | 12.5 | 18.8 | 14.9 | 22.3 | 3,026 | 2,974 | 654,499 | 204,560 | 209,580 |
| omniORBpy | 13.5 | 19.3 | 16.5 | 87.4 | 14,320 | 15,814 | 571,422 | 145,407 | 151,101 |

### 相互接続（実機）

行がクライアント、列がサーバー。

ping の往復（p50、µs）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 21.4 | 21.2 | 29.1 | 17.7 | 18.7 |
| rustorb C++ | 21.5 | 21.9 | 29.1 | 18.0 | 18.9 |
| rustorbpy | 22.4 | 22.7 | 29.8 | 19.4 | 20.2 |
| omniORB C++ | 18.1 | 18.0 | 25.7 | 12.5 | 13.2 |
| omniORBpy | 18.4 | 18.5 | 25.4 | 13.3 | 13.5 |

ping の往復（p99、µs）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 28.4 | 28.1 | 35.6 | 26.8 | 26.4 |
| rustorb C++ | 28.2 | 30.3 | 35.5 | 26.2 | 26.5 |
| rustorbpy | 28.9 | 29.4 | 36.7 | 26.8 | 27.5 |
| omniORB C++ | 24.6 | 24.5 | 35.7 | 18.8 | 19.3 |
| omniORBpy | 24.9 | 25.0 | 34.7 | 19.1 | 19.3 |

any の往復（p50、µs）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 26.5 | 26.8 | 43.1 | 22.2 | 23.5 |
| rustorb C++ | 26.7 | 27.4 | 43.0 | 22.5 | 23.8 |
| rustorbpy | 46.3 | 46.2 | 56.2 | 39.3 | 41.2 |
| omniORB C++ | 20.8 | 21.3 | 39.7 | 14.9 | 16.5 |
| omniORBpy | 22.5 | 22.5 | 39.6 | 17.0 | 16.5 |

1 MiB の octet 列の echo（MB/s、往復の合計）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 8,635 | 8,379 | 8,813 | 3,055 | 10,119 |
| rustorb C++ | 8,518 | 8,336 | 8,640 | 3,062 | 9,935 |
| rustorbpy | 8,555 | 8,306 | 8,792 | 2,967 | 10,052 |
| omniORB C++ | 9,497 | 9,102 | 9,800 | 3,026 | 14,374 |
| omniORBpy | 9,441 | 8,939 | 9,798 | 3,342 | 14,320 |

1 MiB を送るだけ（`put`）／受け取るだけ（`get`）（MB/s）

| クライアント＼サーバー | rustorb (Rust) | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 8,312 ／ 7,709 | 7,904 ／ 7,296 | 10,181 ／ 7,726 | 10,129 ／ 7,808 |
| rustorbpy | 8,145 ／ 7,686 | 7,703 ／ 7,114 | 10,094 ／ 7,712 | 10,119 ／ 7,739 |
| omniORB C++ | 8,386 ／ 9,723 | 7,942 ／ 8,994 | 13,726 ／ 11,889 | 13,544 ／ 11,833 |
| omniORBpy | 8,220 ／ 9,631 | 7,850 ／ 8,929 | 13,270 ／ 11,825 | 12,989 ／ 11,625 |

oneway（毎秒の回数、全部届くまで）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 192,050 | 191,639 | 84,582 | 327,536 | 398,386 |
| rustorb C++ | 193,840 | 187,972 | 84,192 | 321,054 | 394,874 |
| rustorbpy | 189,592 | 187,786 | 82,858 | 288,935 | 304,009 |
| omniORB C++ | 194,789 | 191,938 | 85,004 | 654,499 | 561,015 |
| omniORBpy | 191,428 | 190,780 | 85,492 | 579,985 | 571,422 |

一つの参照を 4 スレッドから（毎秒の回数）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 93,912 | 91,342 | 72,382 | 114,952 | 110,136 |
| rustorb C++ | 93,342 | 91,836 | 73,610 | 115,148 | 111,531 |
| rustorbpy | 89,995 | 88,800 | 71,133 | 110,078 | 106,610 |
| omniORB C++ | 146,510 | 146,864 | 79,374 | 204,560 | 147,081 |
| omniORBpy | 138,533 | 137,550 | 81,885 | 186,533 | 145,407 |

4 プロセスから同時に（毎秒の回数の合計）

| クライアント＼サーバー | rustorb (Rust) | rustorb C++ | rustorbpy | omniORB C++ | omniORBpy |
| --- | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 100,175 | 99,923 | 70,723 | 143,723 | 120,004 |
| rustorb C++ | 100,718 | 99,480 | 71,089 | 143,258 | 121,291 |
| rustorbpy | 98,346 | 96,860 | 69,483 | 137,198 | 117,824 |
| omniORB C++ | 146,622 | 146,454 | 81,030 | 209,580 | 149,168 |
| omniORBpy | 143,359 | 144,420 | 84,097 | 201,030 | 151,101 |

- 小さな呼び出しでは、rustorb の側ごとに 5 µs 前後延びる（クラウドでは 9 µs）。比はクラウドとほぼ同じ
- 大きなデータは、サーバーで速さが決まる。omniORB C++ のサーバーは相手を問わず 3 GB/s、rustorb と rustorbpy のサーバーは 8〜10 GB/s、omniORBpy のサーバーは 10〜14 GB/s。`put` は omniORB C++ のサーバーでも速い（10〜14 GB/s）ので、omniORB C++ の遅さは echo で列を返すときのコピー（同一プロセスの 326 µs と同じもの）と見ている
- クライアントとしては、rustorb は omniORB の 6〜7 割（omniORBpy のサーバーに 10.1 GB/s と 14.3 GB/s、`get` で 7.7 GB/s と 11.8 GB/s）。クラウドの「半分以下」より差は小さい
- oneway は、rustorb のサーバーが相手を問わず 19 万回で頭打ち。rustorb のクライアントから omniORB C++ へは 33 万回で、omniORB C++ のクライアント（65 万回）の半分。クラウドでは rustorb のクライアントの方が速かったが、この Mac では逆になった
- 4 スレッド・4 プロセスでは、omniORB C++ のクライアントは rustorb のサーバーに 14.7 万回出る。rustorb のクライアントは omniORB C++ のサーバーに 11.5 万回（4 スレッド）と 14.4 万回（4 プロセス）で、1 本の接続への多重化の差はクラウドより小さい

### 大きさごと（実機）

| クライアント → サーバー（µs） | 64 B | 1 KiB | 16 KiB | 256 KiB | 1 MiB | 8 MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) → rustorb (Rust) | 22.0 | 22.6 | 46.3 | 102 | 242 | 1,818 |
| omniORB C++ → omniORB C++ | 13.0 | 13.5 | 25.5 | 160 | 699 | 5,636 |
| omniORBpy → omniORBpy | 13.8 | 14.3 | 20.9 | 59.4 | 146 | 1,059 |
| rustorb (Rust) → omniORB C++ | 18.7 | 19.8 | 40.8 | 168 | 702 | 5,433 |
| omniORB C++ → rustorb (Rust) | 18.2 | 19.0 | 35.4 | 82.5 | 220 | 1,533 |
| rustorb (Rust) → omniORBpy | 19.6 | 20.3 | 37.1 | 82.4 | 207 | 1,318 |
| omniORBpy → rustorb (Rust) | 18.7 | 18.9 | 35.6 | 81.9 | 222 | 1,552 |

| クライアント → サーバー（MB/s） | 64 B | 1 KiB | 16 KiB | 256 KiB | 1 MiB | 8 MiB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) → rustorb (Rust) | 5.79 | 90.2 | 700 | 5,118 | 8,635 | 7,007 |
| omniORB C++ → omniORB C++ | 9.59 | 151 | 1,261 | 3,262 | 3,026 | 2,974 |
| omniORBpy → omniORBpy | 9.18 | 142 | 1,503 | 8,822 | 14,320 | 15,814 |
| rustorb (Rust) → omniORB C++ | 6.64 | 102 | 795 | 3,122 | 3,055 | 3,085 |
| omniORB C++ → rustorb (Rust) | 6.95 | 107 | 903 | 6,370 | 9,497 | 8,711 |
| rustorb (Rust) → omniORBpy | 6.43 | 98.8 | 873 | 6,449 | 10,119 | 12,727 |
| omniORBpy → rustorb (Rust) | 6.78 | 107 | 896 | 6,376 | 9,441 | 8,532 |

- 小さい側の差（64 B で 22 µs と 13 µs）は ping と同じ。256 KiB 以上では rustorb どうしの方が omniORB C++ どうしより速い（omniORB C++ のサーバーのコピーのため）
- クラウドで見えた「16 KiB で差が開き始める」は、この Mac では 16 KiB で 46 µs と 26 µs（1.8 倍）で、小さな呼び出しの比と変わらない

### 接続の確立と起動（実機）

新しいクライアントのプロセスでの最初の `ping`（接続の確立を含む、10 回の中央値）と、そのプロセスの起動から終わりまで（相互接続の実行の値）。

| 組 | 最初の呼び出し（µs） | クライアントのプロセス全体（ms） |
| --- | ---: | ---: |
| rustorb (Rust) → rustorb (Rust) | 136 | 2.7 |
| rustorb C++ → rustorb C++ | 154 | 4.1 |
| rustorbpy → rustorbpy | 295 | 38.7 |
| omniORB C++ → omniORB C++ | 378 | 5.1 |
| omniORBpy → omniORBpy | 387 | 29.1 |
| rustorb (Rust) → omniORB C++ | 123 | 2.7 |
| omniORB C++ → rustorb (Rust) | 396 | 5.2 |
| rustorbpy → omniORBpy | 270 | 38.4 |
| omniORBpy → rustorbpy | 460 | 29.3 |

- 同じ組だけを別に回した実行では rustorb どうしの最初の呼び出しが 218 µs で、ばらつきが大きい（ほかの組は 1 割以内）。どちらでも omniORB どうし（378〜462 µs）より速い
- Python のプロセスは rustorbpy が 39 ms、omniORBpy が 29 ms

### giop:unix（実機）

| 組 | ping tcp（µs） | ping unix | 1 MiB tcp（MB/s） | 1 MiB unix | oneway tcp（回/s） | oneway unix | 4 スレッド tcp | 4 スレッド unix |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) | 21.4 | 15.8 | 8,635 | 862 | 192,050 | 188,480 | 93,912 | 118,684 |
| rustorb C++ | 21.9 | 16.0 | 8,336 | 871 | 187,972 | 190,723 | 91,836 | 118,471 |
| rustorbpy | 29.8 | 21.0 | 8,792 | 859 | 82,858 | 83,834 | 71,133 | 77,869 |
| omniORB C++ | 12.5 | 6.12 | 3,026 | 1,332 | 654,499 | 1,949,592 | 204,560 | 281,241 |
| omniORBpy | 13.5 | 7.37 | 14,320 | 1,642 | 571,422 | 567,547 | 145,407 | 129,169 |

- 小さな呼び出しは、どの実装も Unix ソケットで TCP より 2〜5 割短い。クラウドで rustorb だけが遅くなったのは、仮想マシンの事情だった可能性がある（未確認）
- 1 MiB の octet 列は、どの実装も Unix ソケットで大きく落ちる（rustorb 0.86 GB/s、omniORB C++ 1.3 GB/s）。macOS の Unix ソケットの送受信のバッファが小さいためと見ている（未確認）
- omniORB C++ の oneway は Unix ソケットで毎秒 195 万回まで上がる

### CPU とメモリ（実機）

1 回あたりの CPU 時間（µs）と、サーバーの起動時の常駐メモリ（`ps` の RSS）。

| 組 | クライアント CPU / ping | サーバー CPU / ping | クライアント CPU / 1 MiB | サーバー CPU / 1 MiB | クライアント CPU / oneway | サーバー CPU / oneway | サーバー起動時（MiB） |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rustorb (Rust) → rustorb (Rust) | 12.5 | 12.2 | 204 | 215 | 1.81 | 17.2 | 2.8 |
| rustorb C++ → rustorb C++ | 12.8 | 12.5 | 206 | 218 | 1.38 | 17.1 | 7.7 |
| rustorbpy → rustorbpy | 15.7 | 18.5 | 206 | 211 | 2.30 | 29.4 | 26.9 |
| omniORB C++ → omniORB C++ | 4.08 | 4.27 | 171 | 493 | 1.52 | 2.05 | 3.5 |
| omniORBpy → omniORBpy | 4.95 | 4.88 | 123 | 120 | 1.03 | 3.24 | 21.4 |
| rustorb (Rust) → omniORB C++ | 11.9 | 4.55 | 287 | 490 | 4.25 | 3.27 | 3.5 |
| omniORB C++ → rustorb (Rust) | 4.68 | 10.6 | 118 | 234 | 1.46 | 17.7 | 2.8 |

- ping 1 回の CPU は、rustorb がクライアントで 3.1 倍、サーバーで 2.9 倍（クラウドでは 2.3 倍と 2.7 倍）。往復の時間の差（9 µs）より CPU の差（16 µs）の方が大きいのはクラウドと同じ
- rustorb のサーバーの oneway 1 回は 17 µs で、omniORB C++ の 8 倍。oneway の上限（毎秒 19 万回）はこれで決まる
- omniORB C++ のサーバーの 1 MiB の echo は 490 µs で、rustorb のサーバー（215 µs）の 2.3 倍

### クラウドとのちがい

| 項目 | クラウド（Xeon 4 コア、Linux） | 実機（M5 Max、macOS） |
| --- | --- | --- |
| ping p50、rustorb どうし／omniORB C++ どうし | 47.3 µs ／ 30.0 µs（1.6 倍） | 21.4 µs ／ 12.5 µs（1.7 倍） |
| 同一プロセスの ping | 3.33 µs ／ 0.08 µs | 1.29 µs ／ 0.04 µs |
| 1 MiB の echo、rustorb どうし／omniORB C++ どうし | 994 ／ 1,799 MB/s（omniORB が速い） | 8,635 ／ 3,026 MB/s（rustorb が速い） |
| oneway、rustorb どうし／omniORB C++ どうし | 5.9 万／13.4 万回 | 19.2 万／65.4 万回 |
| 4 スレッド、rustorb どうし／omniORB C++ どうし | 2.8 万／20.2 万回（7 倍） | 9.4 万／20.5 万回（2.2 倍） |
| CPU / ping（クライアント・サーバー）、rustorb／omniORB C++ | 35・41 ／ 15・15 µs | 12.5・12.2 ／ 4.1・4.3 µs |
| giop:unix の ping、rustorb | TCP より遅い（60 µs） | TCP より速い（15.8 µs） |

- 小さな呼び出しの比（1.6〜1.7 倍）と CPU の比（2〜3 倍）は、マシンが変わってもほぼ同じで、rustorb の 1 回あたりのコストの差と見てよさそう
- 大きなデータの順位は入れ替わった。rustorb のクライアント・サーバーの大きなデータの扱い（クラウドで半分以下）は、この Mac ではメモリの帯域が十分にあって目立たないと見ている（未確認）。omniORB C++ の echo のコピーはこの Mac の方が相対的に重い
- 4 スレッドの差は、クラウドではコア数（4）で頭打ちになっていた分が大きかった。「差の見立て」の表の 1 本の接続への多重化は、この Mac でも 2.2 倍の差として残る

## 差の見立てと改善の候補

測った結果からの見立てで、プロファイルでの確認はまだしていない。直すなら、それぞれ issue にしてから取りかかる。

改善の計画と予備のプロファイルは [perf-plan.md](perf-plan.md)（tracking issue #203）にある。

| 差 | 大きさ | 見立て | 候補 |
| --- | --- | --- | --- |
| 同一プロセスの呼び出し | 3.3 µs と 0.08 µs | 同じプロセスでも CDR に組み立て、サーバーの経路を通す | 生成コードの型どうしでサーバントを直接呼ぶ近道（PI・POA の状態・スレッドのポリシーを守れる場合に限る） |
| 小さな呼び出しの往復と CPU | +9 µs／側、CPU 2.3〜2.7 倍 | I/O タスクとスレッドの間の受け渡しと起床 | perf でクライアントとサーバーの 1 回の経路を見る。`-ORBfollowLinger` の効果も測る |
| 一つの参照を複数スレッドから | 2.8 万回と 20 万回 | 1 本の接続に多重化し、読み書きがその接続で直列になる | 同時の呼び出しの数に応じて接続を増やす設定（omniORB の `maxGIOPConnectionPerServer` に当たるもの） |
| サーバーの oneway・並行の処理 | 5.8 万回と 13 万回 | 1 要求あたりのディスパッチ（プールへの受け渡し）が重い | サーバーの 1 要求の経路のプロファイル |
| 大きなデータ（クライアント） | omniORB の半分以下 | 送るときの組み立てと受け取ったあとのコピー、大きな領域の確保 | Rust の生成コードで `Octets`（共有）を返す・受ける API、バッファの再利用 |
| 大きなデータ（Rust のサーバー） | rustorbpy のサーバーより遅い | `&[u8]` を渡す前と返り値の `Vec<u8>` のコピー | 同上 |
| giop:unix | TCP より遅い | スレッドで読む経路が Unix ソケットで働いていない？ | 経路を確かめる |
| C の octet 列 | 1 MiB で 10 ms | 型記述をたどって 1 要素ずつ処理 | octet・基本型の列はまとめてコピー |
| Ruby の octet 列 | 1 MiB で 0.2 秒 | Integer の Array として受け取るマッピング | バイナリの String で受け取る（マッピングの変更なので互換に注意） |

速いところ（起動、最初の呼び出し、起動時のメモリ、oneway の送信）は、そのままでよい。

## 再現のしかた

```sh
pip install -r tools/rustorb-idl.txt
# omniORB / omniORBpy（バイナリとして使う。下は配布物をビルドして /opt/omniorb に入れた場合）
export OMNIIDL=/opt/omniorb/bin/omniidl OMNIORB_PREFIX=/opt/omniorb
interop/bench/run-bench.py                                   # 全実装、全組（時間がかかる）
interop/bench/run-bench.py --impls rustorb,omniorbcpp,omniorbpy --repeat 3
interop/bench/run-bench.py --modes pairs --pairs same --transports tcp,unix
```

このページの表は、次の 8 回の実行の `result.json` を合わせたもの（`interop/out/bench/` に残る）。

| 実行 | 内容 |
| --- | --- |
| `--modes inproc`（rustorb の全言語）、`--modes inproc --impls omniorbcpp,omniorbpy` | 同一プロセス |
| `--modes pairs --pairs all --impls rustorb,rustorbcpp,rustorbpy,omniorbcpp,omniorbpy` | 相互接続の 25 組 |
| `--modes pairs --pairs L:L,L:rustorb,L:omniorbcpp,omniorbcpp:L`（L は Java・C#・C・Ruby） | 言語ごと |
| `--modes pairs --pairs same --transports unix` | giop:unix |
| `--modes pairs --pairs same --cases first` | 接続の確立と起動 |

実機（macOS）の表は、次の 5 回の実行から（omniORB は `OMNIIDL=~/.local/omniorb-4.3.4/bin/omniidl OMNIORB_PREFIX=~/.local/omniorb-4.3.4`、Java・C#・C・Ruby は除く）。

| 実行 | 内容 |
| --- | --- |
| `--impls rustorb,rustorbcpp,rustorbpy --modes inproc`、`--impls omniorbcpp,omniorbpy --modes inproc` | 同一プロセス |
| `--impls rustorb,rustorbcpp,rustorbpy,omniorbcpp,omniorbpy --modes pairs --pairs all` | 相互接続の 25 組、接続の確立と起動 |
| 同じ 5 実装で `--modes pairs --pairs same --transports unix` | giop:unix |
| 同じ 5 実装で `--modes pairs --pairs all --cases ping,octets:1048576,any,oneway` | CPU とメモリ（`ps` で取るように直したあと） |

同じマシンで測った値どうしだけを比べる。別のマシンに分けたクライアントとサーバーでの結果は、取れたらここに足す。

1 回だけ、rustorb Ruby のクライアントから rustorb (Rust) のサーバーへの oneway で、クライアントが何も出力せずに終わった（3 回のうち 1 回）。同じ組をあと 4 回回しても再現しなかった。
