# long double

Oct 31, 2026 · @ysuga · issue #41

IDL の `long double` を、コア・Python・C++・Java・Rust のバインディングと rustorb-idl の生成器で扱えるようにした。ここでは送受信の形式、言語ごとの表現、既存の ORB との違いをまとめる。

## 送受信の形式

CORBA 仕様（CDR の浮動小数点型）どおり、IEEE 754 の 4 倍精度（binary128）で送る。符号 1 ビット、指数 15 ビット（バイアス 16383）、仮数 112 ビットの 16 オクテットで、8 オクテット境界に揃える。リトルエンディアンのストリームでは 16 オクテット全体を逆順にする。

```text
1.5 （ビッグエンディアン）  3f ff 80 00 00 00 00 00 00 00 00 00 00 00 00 00
```

既存の ORB はこの形式で送らない（[既存の ORB との違い](#既存の-orb-との違い)）。合わせる設定は置かず、仕様の形式だけにした（2026-10-31 に決定）。

## 言語ごとの表現

| | 型 | 変換 |
| --- | --- | --- |
| コア（Rust） | `rustorb_cdr::LongDouble`（`rustorb_core::LongDouble` でも使える） | binary128 のビットをそのまま持つ。`from_f64` は正確、`to_f64` は最も近い `f64`（偶数丸め） |
| Rust（`-brust`） | `LongDouble`（`Copy`、ビットで比べるので `Eq`） | 定数は `LongDouble::from_f64(..)` |
| Python | `float` | 受け取った値は最も近い `float`。`int` も渡せる |
| C++ | `long double` | 下を参照 |
| Java | `double` | 標準の Java マッピングに `long double` がないので、最も近い `double`。型は `tk_longdouble` のまま送る |

受け取った値はコアではビットのまま残るので、Rust で受けてそのまま返せば精度は落ちない。Python と Java は `double` の精度になる。

### C++

`long double` の形式は処理系で違うので、`rustorb/cdr.h` の `rustorb::cdr::long_double::to_bits` / `from_bits` が変換する。

- binary128 の処理系（aarch64 の Linux など）はビットを写すだけ
- それ以外（x86_64 の x87 80 ビット形式、`long double` が `double` の MSVC など）は `frexp` / `ldexp` で変換する。binary128 への変換は正確で、binary128 からの変換は仮数を整数のまま `long double` の桁数に偶数丸めしてから組み立てる（非正規化数の範囲では桁数を減らす）。範囲を超えれば無限大か 0

g++ の `-mlong-double-64` / `-mlong-double-128` と、`double` での変換を Python の `fractions` による正確な丸めと 10 万件比べて確かめた。

### C ABI

```c
void rorb_out_write_longdouble(rorb_out* out, uint64_t hi, uint64_t lo);
void rorb_in_read_longdouble(rorb_in* in, uint64_t* hi, uint64_t* lo);
void rorb_longdouble_from_double(double v, uint64_t* hi, uint64_t* lo);
double rorb_longdouble_to_double(uint64_t hi, uint64_t lo);
```

`hi` / `lo` は binary128 のビットの上位と下位の 64 ビット。

### Java

rustorb の `org.omg.CORBA.portable.InputStream` / `OutputStream` に `read_longdouble()` / `write_longdouble(double)`、`org.omg.CORBA.Any` に `insert_longdouble(double)` / `extract_longdouble()` を足した（標準にはないので、他の ORB のストリームでは使えない）。基底クラスの既定の実装は `NO_IMPLEMENT`。`-bjava` は `long double` を `double` にし、`out` / `inout` の Holder には `DoubleHolder` を使う（値を入れるだけで、`_read` / `_write` は `double` の形式なので使わない）。

## DynAny

コアに `insert_longdouble` / `get_longdouble` と `_seq` を足した。Python と C++ も同じ名前で持つ。Java の `DynAny` には型がないので操作を置かず、`from_any` / `to_any` で扱う。`create_dyn_any_from_type_code` は `tk_longdouble` から 0 を作る（omniORB と同じ）。

## 既存の ORB との違い

ブラックボックスで確かめた（[clean-room 方針](development-plan.md)どおりソースは読んでいない）。

| ORB | 挙動 |
| --- | --- |
| omniORB 4.3.4（C++、x86_64） | binary128 ではなく、x87 の 80 ビット形式のメモリ内容を 16 オクテットで送る（1.5 はビッグエンディアンで `00 00 00 00 00 00 3f ff c0 00 00 00 00 00 00 00`）。binary128 を受け取ると 0 や NaN になる |
| omniORBpy 4.3.4 | `long double` に対応していない。omniidl -bpython は「omniORBpy does not support the long double type」でエラーになり、`CORBA.Any(CORBA.TC_longdouble, ..)` は送るときに `NO_IMPLEMENT` |
| JacORB 3.9 | IDL コンパイラが `long double` を `double`（`tk_double`、8 オクテット）に置き換える。`Any` に `long double` を入れる操作もない |

このため相互接続テスト（`run-matrix.sh --suite any`）の `longdouble` の値（`interop/idl/AnyTest.idl`）は rustorb のバインディング（Rust・Python・C++・Java）の間で通り、omniORB C++・omniORBpy との組は `interop/known-failures.txt` に既知の失敗として載せている。omniORB C++ とは TypeCode だけのケース（`make_tc` など）と、rustorb の Rust・C++・Java のサーバーを通す `echo`（ビットをそのまま返す）は通る。omniORBpy は `tk_longdouble` の TypeCode も扱えない（`BAD_TYPECODE`）。JacORB は any のスイートに入っていない。
