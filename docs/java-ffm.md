# Java バインディングの FFM 版の試作と JNI 版との比較

[#51](https://github.com/rustorb/rustorb/issues/51)。Java バインディングは JNI のライブラリ（`crates/rustorb-jni`）を通して `rustorb-ffi` の C ABI を呼んでいる（[java-binding.md](java-binding.md)）。Java 22 で正式になった FFM API（`java.lang.foreign`）なら、Java から C ABI を直接呼べて JNI のライブラリが要らなくなる。置き換えるかどうかを決めるために、FFM で最小のクライアントとサーバントを試作し、JNI 版と同じ呼び出しで比べた。

## 試作（`java/ffm-prototype`）

- `src/org/rustorb/ffm`（約690行）：C ABI の関数の `MethodHandle`（`Ffi`）、ORB・RootPOA・参照（`Orb`）、`rorb_object_invoke` とコールバック（`Stubs`・`Call`・`Operation`）、`rorb_servant_vtable` のサーバント（`Servant`・`Reply`）、ストリームの一部（`Out`・`In`：`long` と `sequence<octet>`）
- `bench`：同じ計測（`Harness`）を、JNI 版（`java/` のランタイムと `rustorb-idl -bjava` のスタブ・スケルトン）と FFM 版（生成器が出す形を手で書いたスタブとサーバント）で回す
- `run-bench.sh`：両方をビルドし、同じ JVM で続けて動かす（Java 22 以降）

試作の作りで、本実装でもそのまま使えるもの：

- **コールバックの context**：`rorb_object_invoke` のコールバックは呼び出したスレッドで呼ばれるので、context はスレッドごとの呼び出しのスタックの深さにした（入れ子の呼び出しにも対応）。JNI 版は context に `JNIEnv` とローカル参照を入れている
- **サーバントの self**：番号で、Java 側の表がサーバントを持つ。`add_ref` / `remove_ref` で表から外す。JNI 版の peer（Rust の `Box` と GlobalRef）に当たるものが Java だけで書ける
- **アップコールのスレッド**：FFM は JVM の外で作られたスレッドからのアップコールで自動的にアタッチする。2万回の呼び出しで見えた `Thread` は2つで、スレッドは一度アタッチされたらそのまま（デーモン）だった。JNI 版が自前でしているアタッチとデタッチのコードが要らない
- **例外**：アップコールから例外を投げると JVM が終わるので、すべて捕まえて `RORB_CALLBACK_FAILED` か `UNKNOWN` の応答にする（JNI 版も同じ考え方）
- **配列**：`Linker.Option.critical(true)` で Java の `byte[]` をそのまま C に渡せる（コピーなし）。短く、コールバックしない関数（ストリームの読み書き）は `critical` で呼ぶ
- **演算名**：生成コードが演算ごとに一度だけネイティブの文字列を作れる。JNI 版は呼び出しのたびに `String` を変換する

## 計測

Intel Xeon 2.1 GHz の4コア（クラウドの VM）、OpenJDK 25.0.4、Linux。同じプロセスに2つの ORB（ループバックの TCP）と、同一 ORB 内のオブジェクト。1秒の慣らしの後、1秒ずつ5回の中央値で、2回走らせた平均。1回の呼び出しあたりの時間（8スレッドは全体のスループットを1呼び出しあたりにしたもの）。

| ケース | JNI | FFM | 差 |
| --- | ---: | ---: | ---: |
| 同一 ORB 内 `ping` | 4.98 µs | 3.98 µs | −20% |
| 同一 ORB 内 `echo_long` | 5.31 µs | 3.97 µs | −25% |
| 同一 ORB 内 `echo_record`（`long` 16個の構造体） | 6.58 µs | 4.84 µs | −26% |
| 同一 ORB 内 `echo_octets` 64 KiB | 46.9 µs | 32.1 µs | −32% |
| TCP `ping` | 55.9 µs | 50.7 µs | −9% |
| TCP `echo_long` | 57.1 µs | 52.0 µs | −9% |
| TCP `echo_record` | 56.8 µs | 54.9 µs | −3% |
| TCP `echo_long` 8スレッド | 31.5 µs | 31.9 µs | ±0 |
| TCP `echo_octets` 64 KiB | 201 µs | 179 µs | −11% |
| TCP `echo_octets` 1 MiB | 1.76 ms | 1.34 ms | −24% |

- ストリームの読み書き1回あたりは、`echo_record` と `echo_long` の差（60回分）から JNI が約21 ns、FFM が約15 ns
- TCP 越しの呼び出しは通信とスレッドの受け渡しが大半で、境界をまたぐ費用の差はほとんど見えない（TCP の小さい呼び出しの数 µs の差は回ごとのぶれと同じくらい）
- 大きな配列は FFM の方が速い。JNI 版は `Get/SetByteArrayRegion` で一度コピーするが、FFM は Java の配列をそのまま読み書きする
- 試作は生成コードの全機能（Any、valuetype、例外の種類の対応など）を持たないので、本実装では差が少し縮むかもしれない

## 保守性

| | JNI 版 | FFM 版 |
| --- | --- | --- |
| ネイティブのコード | `crates/rustorb-jni`（Rust 約5400行、196関数）と `Native.java`（372行） | なし。`rustorb-ffi` の `cdylib` をそのまま使う |
| 配布 | `librustorb_jni` を OS ごとにビルドして同梱 | `librustorb_ffi` を OS ごとにビルドして同梱（C++ と同じもの） |
| C ABI を変えたとき | Rust の JNI の関数と `Native.java` の両方を直す | Java の `MethodHandle` の宣言だけ。構造体の配置は `MemoryLayout` で書くので、C のヘッダーとずれると実行時に壊れる（`jextract` で生成すればずれない） |
| スレッド | アタッチ・デタッチとローカル参照のフレームを自前で管理 | JVM に任せる |
| Java の下限 | 11 | 22（正式版の FFM） |
| 起動時の警告 | Java 24 以降、`System.load` に `--enable-native-access` を付けないと警告が出る（JEP 472。今回の計測でも出た） | 同じく `--enable-native-access` が要る |

## 結論と提案

- 性能：同一 ORB 内の呼び出しと大きな配列では FFM が 20〜30% 速い。通常の TCP 越しの呼び出しでは差がない
- 保守性：JNI の Rust のクレート（約5400行）がなくなり、ネイティブのビルドは C++ と共通の `librustorb_ffi` だけになる。これが一番大きい利点
- 欠点：Java の下限が 11 から 22 に上がる。Java 11 / 17 / 21 の利用者（OpenRTM-aist-Java など、LTS の古い版で動くもの）が使えなくなる。両方持つと Java 側の実装が二重になる

提案：今は JNI 版を続け、Java の下限を 22 以上に上げてよい時期（Java 21 の利用者が十分減ったとき）に FFM へ移る。そのとき、試作の作り（コールバックの context、サーバントの表、`critical` の配列）をそのまま使い、構造体の配置は `jextract` で `rustorb.h` から生成する。試作と計測は `java/ffm-prototype` に残し、移るときに `run-bench.sh` で測り直す。
