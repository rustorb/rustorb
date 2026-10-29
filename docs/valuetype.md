# valuetype

Oct 29, 2026 · @ysuga · M6 の後

rustorb の valuetype（値型）の実装をまとめる。範囲は CDR、Rust コア、C ABI、Python・C++・Java のバインディングと `rustorb-idl -bpython` / `-bc++11` / `-bjava`。完了条件は「状態メンバー・継承・truncatable・null・共有参照・value box を使う操作が omniORB C++・omniORBpy（と JacORB）と相互接続できること」で、`interop/run-matrix.sh --suite value` で判定する。最初の段階（PR #35）で CDR・コア・Python を、次の段階（#36）で C ABI・C++・Java を加えた。

## 構成

| 場所 | 内容 |
| --- | --- |
| `crates/rustorb-cdr/src/value.rs` | 値のタグ、null、間接参照、リポジトリ ID とその共有、チャンク、終了タグ（`Encoder::begin_value` / `end_value`、`Decoder::read_value_header` / `end_value`） |
| `crates/rustorb-core/src/value.rs` | `Value::Valuetype`、`ValueRef`、valuetype の登録、`write_valuetype` / `read_valuetype` |
| `crates/rustorb-core/tests/valuetype.rs` | 共有、循環、切り捨て、未知の型、value box、Any の中の値 |
| `crates/rustorb-python/src/value.rs` | Python のインスタンスとコアの `ValueRef` の変換 |
| `python/rustorb/CORBA.py`、`_rt.py` | `CORBA.ValueBase`、`ValueMember`、`ORB.create_value_tc`、値ファクトリ |
| `crates/rustorb-codegen/python/rustorb_idl/python_backend.py` | valuetype・value box・前方宣言の生成 |
| `crates/rustorb-ffi/src/value.rs`、`include/rustorb/rustorb.h` | C ABI：値のヘッダー・終了・インスタンスの表、valuetype の登録と型の解決の関数（`rorb_tc_value` は `any.rs`） |
| `cpp/include/rustorb/corba.h`、`cdr.h` | C++：`CORBA::ValueBase`、`ValueFactoryBase`、値ファクトリの表、`ORB::create_value_tc`、値の読み書き |
| `crates/rustorb-codegen/python/rustorb_idl/cxx_backend.py` | `-bc++11` の valuetype・`OBV_` クラス・value box |
| `cpp/tests/test_value.cc` | C++ の相互接続ケース、ファクトリ、切り捨て、ファクトリなし、`create_value_tc`、`_copy_value` |
| `crates/rustorb-jni/src/value.rs`、`java/src/main/java/org/omg/CORBA/portable`、`org/omg/CORBA_2_3`、`org/rustorb/Values.java` | Java：`StreamableValue`、`ValueFactory`、`BoxedValueHelper`、CORBA_2_3 のストリームと ORB、値ファクトリの表 |
| `crates/rustorb-codegen/python/rustorb_idl/java_backend.py` | `-bjava` の valuetype・`DefaultFactory`・Helper・value box |
| `java/src/test/java/org/rustorb/test/ValuetypeTest.java` | Java の相互接続ケース、既定のファクトリ、切り捨て、ファクトリなし、`create_value_tc` |
| `interop/idl/ValueTest.idl` | 相互接続の IDL |

## CDR

CORBA 3.4 Part 2 の 9.3.4 に従う。

- 値のタグは `0x7fffff00` にフラグを足したもの：1 はコードベース URL、2 はリポジトリ ID 1つ、6 はリポジトリ ID の列、8 はチャンク。予約されたビット（`0xf0`）が立ったタグは `MARSHAL`
- null は long の 0。間接参照は `0xffffffff` と、オフセットの long の位置からの相対オフセット（負）。同じインスタンスの2回目からと、同じリポジトリ ID の文字列・ID の列の2回目からに使う。位置はメッセージ（カプセル化の中ならカプセル化）の中のもの
- チャンクは正の長さ（`0x7fffff00` 未満）とその後のデータ。終了タグはチャンクの外の負の long で、値の入れ子の深さ（一番外が -1）を負にしたもの。読み込みではそれより深い終了タグも受け付け、外側の値もまとめて閉じる（仕様の許すとおり）

書き出しの選択：

- リポジトリ ID は常に書く。宣言された型と同じ型のときは ID 1つ（2）。truncatable の型は自分と truncatable で辿れる基底の ID の列（6）をチャンク付きで書く。それ以外の派生型も ID 1つで書く
- チャンク付きの値の中の値はすべてチャンク付きにする（仕様の要求）
- チャンクは値のヘッダーの直後に開き、入れ子の値のタグの前で閉じ、その値の後で開き直す。空のチャンクは書かない。null と間接参照はチャンクの中に書く
- コードベース URL は書かない

読み込み：

- チャンクの中では、`align` と `read_octets` がチャンクの境界で次のチャンクの長さを読む。値のタグはチャンクの中でも外でも受け付ける
- 読み終えた値の残り（切り捨てた派生部分や、知らない入れ子の値）は `end_value` が終了タグまで読み飛ばす
- 値の入れ子は256まで（超えると `MARSHAL`）

## Rust コア

`Value::Valuetype(Option<ValueRef>)` が valuetype と value box の値で、`None` が null。`ValueRef` は `Arc` で共有するインスタンスで、TypeCode（実際の型）と状態メンバーの列を持つ。

- 状態メンバーは基底の分を先に並べた平らな列。value box は中身1つの列
- 同一性は `Arc` のポインタで決まる。書き出しでは同じ `ValueRef` が2回目に出てきたら間接参照になり、読み込みでは同じ位置を指す間接参照が同じ `ValueRef` になる。循環した値も書ける・読める
- `ValueRef` は `Mutex` でメンバーを持つので、読み込みで作ってから中身を埋める（循環のため）。比較（`PartialEq`）は循環で止まる。循環した値を手放すときは `Value::break_cycles` で切る（`Arc` の循環はリークする）

型の解決：

- 受け取った ID の列のうち、最初に知っている型を使う。知っている型は、宣言された型（とその TypeCode の中の型）と、`register_value_type` で登録した型のうち宣言された型に代入できるもの
- どれも知らなければ `MARSHAL`（ファクトリなし）。truncatable の派生型は、ID の列の中の知っている基底で受け取り、残りを読み飛ばす
- `custom` の valuetype は `NO_IMPLEMENT`、`abstract` の valuetype の具象でない値は書き出しで `BAD_PARAM`、読み込みで `MARSHAL`

```rust
let point = TypeCode::value_tc(TCKind::Value, "IDL:ValueTest/Point:1.0", "Point", 0, None,
    &[("x", long.clone(), PUBLIC_MEMBER), ("y", long, PUBLIC_MEMBER)])?;
let p = ValueRef::new(point.clone(), vec![Value::Long(1), Value::Long(2)]);
out.write_value(&point, &Value::Valuetype(Some(p)))?;
```

Any の中の valuetype も同じ書き方になる（`Any::new(tc, Value::Valuetype(..))`）。

## Python

omniORBpy と同じ形の標準マッピングにした。

- valuetype は `CORBA.ValueBase` を継承したクラスで、コンストラクタは全状態メンバー（位置か名前）か、引数なし。`_NP_members` に基底を含む状態メンバーの名前を並べる。private のメンバーも属性
- value box は中身の型の値そのもの（`LongBox` の値は `int`）で、null は `None`
- 受け取った値は、そのリポジトリ ID に登録された値ファクトリ（引数なしの呼び出し可能オブジェクト）で作り、属性を設定する。生成コードは、操作も初期化子もない valuetype のクラスを自身のファクトリとして登録する。ファクトリがなくクラスが分かっていればクラスの `__new__` で作る
- `ORB.register_value_factory` / `unregister_value_factory` / `lookup_value_factory`、`ORB.create_value_tc`、`create_value_box_tc`、`CORBA.ValueMember`、`VM_*` と `PRIVATE_MEMBER` / `PUBLIC_MEMBER`、TypeCode の `member_visibility` / `type_modifier` / `concrete_base_type`
- 1回の呼び出し（引数の変換から応答の変換まで）の中で同じ Python オブジェクトは同じ `ValueRef` になり、逆も同じ。変換は再帰でなく作業リストで行うので、長いリストでもスタックを使い切らない（アップコールのスレッドのスタックは小さい）
- 生成器：`valuetype`、`valuetype X <型>;`（value box）、前方宣言。`supports` は未対応でエラー

```python
p = ValueTest.Point3(1, 2, 3, "three")
got = echo.echo_pair(ValueTest.Pair(p, p))
assert got.first is got.second
```

## C ABI

C++ と Java のバインディングは、状態の読み書きを自分で行い、その周り（値のタグ、リポジトリ ID、チャンク、終了タグ）と共有の管理を C ABI に任せる。

- 書き出し：`rorb_out_write_null_value`、`rorb_out_begin_value(out, key, count, ids, id_lens, chunked)`、`rorb_out_end_value`。`key` は呼び出し側のインスタンスの名前（C++ はオブジェクトのアドレス、Java は `IdentityHashMap` で振った `1 << 62` 以上の番号）で、同じストリームで2回目の `key` は間接参照になり 0 を返す。1 を返したときだけ状態を書いて `end_value` を呼ぶ
- 読み込み：`rorb_in_read_value_header(in, &slot, &id_count)` が `RORB_VALUE_NULL` / `RORB_VALUE_INDIRECTION` / `RORB_VALUE_NEW` を返す。新しい値なら `rorb_in_value_id(in, i, &len)` で受け取った ID を見てファクトリを選び、インスタンスを作ったらすぐ（状態を読む前に。循環のため）`rorb_in_set_value(in, slot, ptr, release)` で置き、状態を読んで `rorb_in_end_value`（切り捨てた残りを読み飛ばす）。間接参照は `rorb_in_get_value(in, slot)` で置いたインスタンスを得る。置いたインスタンスはストリームの終わりに `release(ptr)` で手放す
- `rorb_tc_value` が valuetype の TypeCode を作る（`ORB::create_value_tc`）。`rorb_register_value_type(tc)` で、Any の中の値をコアが読むときに使う型を登録する
- `rorb_set_value_type_resolver(resolve, ctx)`：登録されていない ID の型をコアが問い合わせる関数。Java は、まだ読み込まれていないクラスの型（Any の中の派生型など）を、この関数から `<名前>Helper.type()` をリフレクションで呼んで返す。見つかった型はコアが登録する

## C++

IDL to C++11 のマッピングに従う。

- valuetype は `CORBA::ValueBase` を仮想継承した抽象クラスで、状態メンバーごとに純粋仮想のアクセサとモディファイア（private のメンバーは protected）を持つ。`OBV_<モジュール>::<名前>` が具象クラスで、全状態メンバーのコンストラクタと `_copy_value` を持つ。参照は `IDL::traits<T>::ref_type`（`CORBA::object_reference<T>`、`shared_ptr` 相当）で、null は `nullptr`、`IDL::traits<T>::narrow` で派生型に変換する
- value box は `ValueBase` の派生クラスで、`_value()` で中身を取得・設定する
- 値ファクトリは `CORBA::ValueFactoryBase` の派生クラスで、`create_for_unmarshal()` が空のインスタンスを返す。`ORB::register_value_factory`（前のファクトリを返す）・`unregister_value_factory`・`lookup_value_factory` で、表はプロセスに1つ。生成コードは、操作も初期化子もない valuetype の `OBV_` クラスと value box の既定のファクトリ（`rustorb::detail::DefaultValueFactory<T>`）を、静的初期化で登録する（コアにも型を登録する）
- 受け取った値は、受け取った ID の順に最初にファクトリのある型で作る（ID がなければ宣言された型）。ファクトリがなければ `MARSHAL`
- Any には `<<=` / `>>=` で入る。`ORB::create_value_tc`、`CORBA::ValueMember`・`ValueMemberSeq`、`VM_*`、`PRIVATE_MEMBER` / `PUBLIC_MEMBER`
- 生成器：`valuetype`、value box、前方宣言。`custom`、`supports`、valuetype の操作・属性・初期化子はエラー

```cpp
auto p = CORBA::make_reference<OBV_ValueTest::Point>(5, 6);
ValueTest::Pair got = echo->echo_pair(ValueTest::Pair(p, p));
assert(got.first() == got.second());
```

## Java

IDL to Java のマッピング（CORBA_2_3 のストリームと ORB）に従う。

- valuetype は `org.omg.CORBA.portable.StreamableValue` を実装したクラスで、状態メンバーの公開フィールド（private のメンバーは protected）、`_read` / `_write`（基底の分は `super` で）、`_truncatable_ids`、`_type` を持つ。操作のない valuetype なので具象クラスにし、`<名前>DefaultFactory`（`ValueFactory`）も生成する
- value box：中身が基本型なら `value` フィールドを持つクラス（`LongBox`）、それ以外は中身の Java の型そのもの（`StringBox` は `String`、`PairBox` は `Pair`）。`<名前>Helper` は `BoxedValueHelper` を実装する
- 値ファクトリは `org.omg.CORBA_2_3.ORB` の `register_value_factory` / `unregister_value_factory` / `lookup_value_factory` で、表はプロセスに1つ。登録がなければ、リポジトリ ID から `<名前>DefaultFactory` のクラスを探す（`<名前>Package` と `#pragma prefix` の逆順のパッケージも）
- ストリームは `org.omg.CORBA_2_3.portable.InputStream` / `OutputStream` の `read_value` / `write_value` を持つ。同じインスタンスは同一性（`==`）で共有を決める
- 生成器のエラーは C++ と同じ

```java
Point p = new Point();
p.x = 5;
Pair got = echo.echo_pair(new Pair(p, p));
assert got.first == got.second;
```

## 相互接続

`interop/idl/ValueTest.idl` の `ValueEcho`（23ケース）を rustorb（Rust）・rustorbpy・rustorbcpp・rustorbjava・omniORBpy・omniORB C++・JacORB のクライアントとサーバーで実装し、`run-matrix.sh --suite value` で回す。49組すべてが GIOP 1.2 / 1.1 で通る（失敗は `known-failures.txt` の範囲だけ）。Java の2つ（rustorbjava と JacORB）は `interop/impls/java/interop/ValueTests.java` を共有し、valuetype の実装クラスとファクトリを自分で登録する（JacORB の生成するクラスは抽象クラスで、既定のファクトリがないため）。

ケース：プレーンな値、null、truncatable の派生型、struct の中の共有と別インスタンス、同一性（`same`）、20個のリストと2個の循環、sequence と他の値をメンバーに持つ値、sequence の中の共有と null、value box（long・string・struct）、Any の中の循環・派生型・value box。

### 実測で分かったこと

ソースは読まず、生成コードのコンパイル結果と通信内容だけを見た。

- omniORB C++ の `OBV_` クラスは、操作のない valuetype なら具象クラス（参照カウントと `_copy_value` を持つ）で、そのままファクトリに使える。private のメンバーのアクセサは protected
- omniORBpy はチャンク付きの値をリポジトリ ID なし（`0x7fffff08`）で書くことがある。omniORB C++ は Any の中の値を型情報なし（`0x7fffff00`）で書く。どちらも宣言された型（Any なら TypeCode）で読める
- valuetype の TypeCode のメンバーの可視性を、omniORB C++ は public でも 0（private）で書き、omniORBpy は 1 で書く。このため omniORB C++ が送った Any を omniORBpy が受け取ると TypeCode が `equal` にならず、omniorb-cpp → omniorbpy の `echo_any[ring]` と `echo_any[derived]` は `known-failures.txt` に載せた（rustorb はどちらの書き方も受け取って、そのまま返す）
- JacORB 3.9 の IDL コンパイラは、操作のない valuetype も抽象クラスにし、`DefaultFactory` を生成しない。value box の Helper は rustorb と同じ形（基本型の箱はクラス、それ以外は中身の型）
- JacORB 3.9 は Any の中の値を書くときに `javax.rmi.CORBA.Util` を使い、その実装（JBoss の jar）が Java 17 にない JDK の内部クラスを要るため、JacORB のクライアントは値の入った Any を送れず、JacORB のサーバーも返せない。`echo_any` の3ケースを JacORB の組で `known-failures.txt` に載せた（イメージには javax.transaction の API の jar を足し、`--add-opens java.base/java.io=ALL-UNNAMED` を付けたが、その先で止まる）。Any の外の値は JacORB とすべて通る

## 残り

- `custom` の valuetype（`CustomMarshal`）、abstract interface、`supports` するインターフェース、valuetype の操作と初期化子のサーバント側の扱い
- コードベース URL の利用
