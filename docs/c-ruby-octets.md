# C と Ruby の数値の列（#212）

Nov 27, 2026 · @ysuga · [速度の改善の計画](perf-plan.md)の段 9（issue [#212](https://github.com/rustorb/rustorb/issues/212)）

C と Ruby のバインディングで、大きな `sequence<octet>`（と数値の列）の受け渡しを速くした。

## 見えていたこと

クラウドの 4 コアの仮想マシンで、1 MiB の echo（`octets:1048576`）は rustorb (Rust) どうしの 2.5 GB/s に対し、C は 296 MB/s、Ruby は 13.5 MB/s だった。サーバーの CPU は 1 回あたり C で 6.1 ms、Ruby で 137 ms。

- **C**：受け取り（`rorb_in_read_array`）と送り（`rorb_out_write_array`）は前から octet の列をまとめてコピーしていた。遅かったのはメモリーの側で、`CORBA_free` と `rustorb_c_copy` が列を 1 要素ずつたどっていた（1 MiB で 100 万回の呼び出し）。それを直すと次は、解放した 1 MiB を glibc がシステムに返し、次の確保でページごとにページフォールトが起きるのが CPU の 4 分の 1 を占めた（`perf record`）。`calloc` のゼロ埋めもそのたび
- **Ruby**：列は `pack`・`unpack` でまとめていたが、送る前に要素ごとに Ruby で型と範囲を調べていた（1 MiB で 100 万回のメソッド呼び出し）。受け取りは Integer の Array になる（マッピングどおり）

## 変えたこと

### C

- 何も持たない値（数値、enum、fixed、それらの struct と配列：`rustorb_c_plain`）は、解放で中をたどらず、コピーは `memcpy` 1 回にする
- 読み込みがすぐ埋めるバッファ（受け取った数値の列、コピー先）はゼロ埋めしない（`rustorb_c_alloc_filled`）。読み込みが失敗したときはゼロにする
- 何も持たない値の 64 KiB〜16 MiB のブロックは、解放したあと 4 つまで（合わせて 32 MiB まで）取っておき、次に同じくらいの大きさ（2 倍未満）を確保するときに使う。`CORBA_sequence_octet_allocbuf` などゼロの値を返す確保では、使い回すときにゼロ埋めする
- `rorb_out_write_array`・`rorb_in_read_array` は、short から double の列も、ホストのバイト順ならまとめてコピーする（違えば要素ごとに並べ替える）。valuetype のチャンクの中では、チャンクの終わりまでずつ読む（`rustorb_cdr::Decoder::read_elements`）

### Ruby

- 数値の列（octet・int8・short〜unsigned long long・float・double）の型と範囲の確認、詰めと取り出しを拡張の中で 1 度に行う（`Out#write_list`・`In#read_list`）。拡張が扱うのは Fixnum と Float の要素で、それ以外（Bignum、Rational など）や範囲外の値があれば今までの Ruby の確認に回す。エラーのメッセージは前と同じ
- octet の列として渡された String は、送る前に複製しない（前は `String#b` で 1 MiB を写していた）
- `Rustorb.octets_as_string = true` で、octet の列と配列をバイナリの String（`Encoding::BINARY`）で受け取る。マッピングの変更なので既定は `false`（[計画](perf-plan.md)の決めること #4）。プロセス全体の設定で、any の中の値にも効く。送るときは前から String も Array も受ける

```ruby
Rustorb.octets_as_string = true
data = target.get_octets(1 << 20)   # => "\a\a\a...", Encoding::BINARY
```

## 測った結果

クラウドの 4 コアの仮想マシン（Intel Xeon 2.3 GHz）、`run-bench.py --modes pairs --pairs same`、各ケース 2 秒。変更前は 1 回、変更後は 3 回の中央値（クラウドのぶれは 10% ほど）。MB/s。

| | `octets:1048576` | `get:1048576` | `put:1048576` |
| --- | ---: | ---: | ---: |
| rustorb (Rust) | 2,473 → 2,680 | 2,430 → 2,589 | 2,590 → 3,203 |
| C | 296 → 1,745 | 524 → 1,743 | 777 → 1,444 |
| Ruby（Array） | 13.5 → 91.2 | 63.5 → 97.3 | 55.6 → 125 |
| Ruby（`octets_as_string`） | — → 1,154 | — → 882 | — → 1,429 |

- C の echo は 5.9 倍で、Rust の 65%。サーバーの CPU は 1 回 6.1 ms から 0.78 ms。残りはほぼコピーで、Rust の生成コードは `-Wbshared_octets` でコピーしないのに対し、C のマッピングは受け取った列を自分のバッファに、サーバントの戻り値をメッセージに写す
- Ruby の既定（Array）は echo で 6.8 倍。残りは 100 万個の Integer の Array を作ることと、その GC
- `octets_as_string` では C の 66%。String の確保と解放のたびのページフォールト（Ruby の GC と glibc の扱い）が残る
- 16 KiB の echo（`octets:16384`、3 回の中央値どうし）は C で 136 → 192 MB/s、Ruby で 12.7 → 48.3 MB/s（String で 142）。struct の列（`samples:100` の p50）は C で 97.8 → 107 µs、Ruby で 1,688 → 1,777 µs で、ぶれの範囲（この経路は変えていない）

issue の終わりの条件（C が Rust の 5 割以上、Ruby の既定で 5 倍以上、String で C と同程度）のうち、String の「C と同程度」は 7 割弱にとどまる。実機の値は段 10（#213）で測る。

## 再現のしかた

```sh
interop/bench/run-bench.py --impls rustorb,rustorbc,rustorbrb --modes pairs --pairs same \
  --cases octets:1048576,get:1048576,put:1048576
RUSTORB_RB_OCTETS_AS_STRING=1 interop/bench/run-bench.py --impls rustorbrb --modes pairs \
  --pairs same --cases octets:1048576,get:1048576,put:1048576
```
