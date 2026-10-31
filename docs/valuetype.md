# valuetype

Oct 29, 2026 · @ysuga · M6 の後

rustorb の valuetype（値型）の実装をまとめる。範囲は CDR、Rust コア、C ABI、Python・C++・Java のバインディングと `rustorb-idl -bpython` / `-bc++11` / `-bjava`。完了条件は「状態メンバー・継承・truncatable・null・共有参照・value box を使う操作が omniORB C++・omniORBpy（と JacORB）と相互接続できること」で、`interop/run-matrix.sh --suite value` で判定する。最初の段階（PR #35）で CDR・コア・Python を、次の段階（#36）で C ABI・C++・Java を加え、#42 で `custom` の valuetype、`supports`、valuetype の操作と初期化子（と Rust の生成コード `-brust`）を加えた。

## 構成

| 場所 | 内容 |
| --- | --- |
| `crates/rustorb-cdr/src/value.rs` | 値のタグ、null、間接参照、リポジトリ ID とその共有、チャンク、終了タグ（`Encoder::begin_value` / `end_value`、`Decoder::read_value_header` / `end_value`） |
| `crates/rustorb-core/src/value.rs` | `Value::Valuetype`、`ValueRef`、valuetype の登録、`write_valuetype` / `read_valuetype` |
| `crates/rustorb-core/tests/valuetype.rs` | 共有、循環、切り捨て、未知の型、value box、Any の中の値、custom の値、コードベース URL |
| `crates/rustorb-core/src/idl/value.rs` | Rust の生成コードの `ValueInfo`、`CustomMarshal`、`DataOutputStream` / `DataInputStream` |
| `crates/rustorb-python/src/value.rs` | Python のインスタンスとコアの `ValueRef` の変換 |
| `python/rustorb/CORBA.py`、`_rt.py` | `CORBA.ValueBase`、`ValueMember`、`ORB.create_value_tc`、値ファクトリ |
| `crates/rustorb-codegen/python/rustorb_idl/python_backend.py` | valuetype・value box・前方宣言の生成 |
| `crates/rustorb-ffi/src/value.rs`、`include/rustorb/rustorb.h` | C ABI：値のヘッダー・終了・インスタンスの表、valuetype の登録と型の解決の関数（`rorb_tc_value` は `any.rs`） |
| `cpp/include/rustorb/corba.h`、`cdr.h` | C++：`CORBA::ValueBase`、`ValueFactoryBase`、値ファクトリの表、`ORB::create_value_tc`、値の読み書き |
| `crates/rustorb-codegen/python/rustorb_idl/cxx_backend.py` | `-bc++11` の valuetype・`OBV_` クラス・value box |
| `cpp/tests/test_value.cc` | C++ の相互接続ケース、ファクトリ、切り捨て、ファクトリなし、`create_value_tc`、`_copy_value` |
| `java/src/main/java/org/rustorb/Native.java`（`*Value*`）、`java/src/main/java/org/omg/CORBA/portable`、`org/omg/CORBA_2_3`、`org/rustorb/Values.java` | Java：`StreamableValue`、`ValueFactory`、`BoxedValueHelper`、CORBA_2_3 のストリームと ORB、値ファクトリの表 |
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
- コードベース URL は書かない。読み込みでは読み飛ばす（間接参照の URL も）

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
- `abstract` の valuetype の具象でない値は書き出しで `BAD_PARAM`、読み込みで `MARSHAL`
- 宣言された型が `ValueBase`（`IDL:omg.org/CORBA/ValueBase:1.0`）なら、登録されたどの型の値も受け取る
- `custom` の valuetype は「custom の値」の節

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
- 生成器：`valuetype`、`valuetype X <型>;`（value box）、前方宣言、`custom`、`supports`、操作と初期化子（「custom の値」「supports・操作・初期化子」の節）

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
- 生成器：`valuetype`、value box、前方宣言、`custom`、`supports`（abstract interface も。[abstract interface](#abstract-interface)）、操作と初期化子。local interface を `supports` するとエラー

```cpp
auto p = CORBA::make_reference<OBV_ValueTest::Point>(5, 6);
ValueTest::Pair got = echo->echo_pair(ValueTest::Pair(p, p));
assert(got.first() == got.second());
```

## Java

IDL to Java のマッピング（CORBA_2_3 のストリームと ORB）に従う。

- valuetype は `org.omg.CORBA.portable.StreamableValue` を実装したクラスで、状態メンバーの公開フィールド（private のメンバーは protected）、`_read` / `_write`（基底の分は `super` で）、`_truncatable_ids`、`_type` を持つ。操作も `supports` もない valuetype は具象クラスにし、`<名前>DefaultFactory`（`ValueFactory`）も生成する
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

## custom の値

`custom valuetype` の状態は、アプリケーションの `marshal` / `unmarshal`（`CustomMarshal`）が `DataOutputStream` / `DataInputStream` で書き・読む。CDR では常にチャンク付きで、リポジトリ ID 1つ（`0x7fffff0a`）で書く（仕様の 9.3.4.5 と 9.3.4.6）。TypeCode の型修飾子は `VM_CUSTOM`。

- コア：custom の値の `ValueRef` の状態は、`marshal` が書いた順の `Value::Any` の列。書き出しはそれぞれを型どおりに書く（Any でないものは `BAD_PARAM`）。読み込みは状態の形が分からないので、リポジトリ ID ごとに `register_custom_reader` で登録した読み込み関数（`CustomReader`）に任せる。登録がなければ `MARSHAL`。読み込み関数は読んだ値を `ValueRef::set_native` でインスタンスに付けられる
- Python：`CORBA.CustomMarshal` を継承したクラス（生成コード）を、アプリケーションが `marshal(os)` と `unmarshal(is)` を実装して値ファクトリに登録する。`DataOutputStream` は `write_*` の呼び出しを (TypeCode, 値) の列に記録し、コアに Any の列として渡す。読み込みはコアの読み込み関数として登録した `unmarshal` を、メッセージを読みながら呼ぶ（GIL を取り、`DataInputStream` がコアのストリームから直接読む）。入れ子の値（`write_Value` / `read_Value`）もメッセージの中の共有を保つ
- C++：`CORBA::CustomMarshal` を継承した抽象クラスを生成し、アプリケーションが `marshal(CORBA::DataOutputStream&) const` と `unmarshal(CORBA::DataInputStream&)`、`_copy_value` を実装してファクトリを登録する。ストリームは C ABI のストリームに直接書き・読む
- Java：`org.omg.CORBA.portable.CustomValue` を実装した抽象クラスを生成し、`marshal(org.omg.CORBA.DataOutputStream)` / `unmarshal(DataInputStream)` を実装したクラスのファクトリを登録する。`DefaultFactory` は作らない
- Rust：状態の構造体 `<名前>State` にアプリケーションが `CustomMarshal`（`rustorb_core::idl`）を実装する。`DataOutputStream::write::<T>` / `write_array`、`DataInputStream::read::<T>` / `read_array`。Rust の custom の値は Any に入らない（コアが状態を読めないので `MARSHAL`）

## supports・操作・初期化子

| | 操作 | `supports I` | 初期化子 `factory init(..)` |
| --- | --- | --- | --- |
| Python | クラスのメソッド（アプリケーションがサブクラスで実装し、ファクトリに登録する） | クラスが `<モジュール>__POA.I` も継承し、インスタンスがそのままサーバント（`_this()`）。`_objref_<名前>` を `I` のオブジェクト参照として登録する | 生成しない（omniORBpy と同じく、実装クラスのコンストラクタで作る） |
| C++ | 純粋仮想関数（`supports` するインターフェースの操作も） | `POA_<モジュール>::<名前>` が `POA_..::I` と値のクラスを継承する。実装は `POA_..::<名前>` と `OBV_..::<名前>` を継承し、`CORBA::make_reference` で作る（サーバントの参照カウントは最後の参照で手放す） | `<名前>_init`（`ValueFactoryBase` の派生）に純粋仮想の初期化子を生成し、アプリケーションが実装して登録する |
| Java | 抽象メソッド | クラスが `IOperations` を実装し、`IPOATie` でサーバントにする | `<名前>ValueFactory`（`ValueFactory` の派生インターフェース）に初期化子を生成する |
| Rust | トレイト `<名前>Operations` をアプリケーションが状態の構造体に実装する。ハンドルにも同名のメソッド（状態をロックして呼ぶ） | ハンドルが `IServant` を実装し、`IPoa(値).activate(&poa)` でサーバントにする | 生成しない（`<名前>::new(状態)` で作る） |

操作や `supports` のある valuetype は、Python・C++・Java では具象クラスがないので既定のファクトリを登録しない。受け取るにはアプリケーションの実装クラスのファクトリが要る。Rust は状態の型が実装を持つので、そのまま受け取れる。Rust では例外を投げる valuetype の操作と、ハンドルのメソッド（`new`・`get` など）と同名の操作は生成器のエラーにした。

## 相互接続

`interop/idl/ValueTest.idl` の `ValueEcho`（26ケース）を rustorb（Rust）・rustorbpy・rustorbcpp・rustorbjava・omniORBpy・omniORB C++・JacORB のクライアントとサーバーで実装し、`run-matrix.sh --suite value` で回す。49組すべてが通る（失敗は `known-failures.txt` の範囲だけ）。Java の2つ（rustorbjava と JacORB）は `interop/impls/java/interop/ValueTests.java` を共有し、valuetype の実装クラスとファクトリを自分で登録する（JacORB の生成するクラスは抽象クラスで、既定のファクトリがないため）。

ケース：プレーンな値、null、truncatable の派生型、struct の中の共有と別インスタンス、同一性（`same`）、20個のリストと2個の循環、sequence と他の値をメンバーに持つ値、sequence の中の共有と null、value box（long・string・struct）、Any の中の循環・派生型・value box、`supports` する値（`echo_tally`）とそのサーバントの呼び返し（`call_count`）、操作（`account_doubled`）、初期化子のファクトリ（`make_account`）。

custom の値は `interop/idl/CustomTest.idl` の `CustomEcho`（7ケース：プレーン・null・空・派生の値を含む Blob、struct の中の共有と別インスタンス、sequence）で、`run-matrix.sh --suite custom` で回す。omniORB は custom の valuetype に対応していないので、rustorb の4つと JacORB で回す。rustorb の4つの間の16組はすべて通る。

### 実測で分かったこと

ソースは読まず、生成コードのコンパイル結果と通信内容だけを見た。

- omniORB C++ の `OBV_` クラスは、操作のない valuetype なら具象クラス（参照カウントと `_copy_value` を持つ）で、そのままファクトリに使える。private のメンバーのアクセサは protected
- omniORBpy はチャンク付きの値をリポジトリ ID なし（`0x7fffff08`）で書くことがある。omniORB C++ は Any の中の値を型情報なし（`0x7fffff00`）で書く。どちらも宣言された型（Any なら TypeCode）で読める
- valuetype の TypeCode のメンバーの可視性を、omniORB C++ は public でも 0（private）で書き、omniORBpy は 1 で書く。このため omniORB C++ が送った Any を omniORBpy が受け取ると TypeCode が `equal` にならず、omniorb-cpp → omniorbpy の `echo_any[ring]` と `echo_any[derived]` は `known-failures.txt` に載せた（rustorb はどちらの書き方も受け取って、そのまま返す）
- JacORB 3.9 の IDL コンパイラは、操作のない valuetype も抽象クラスにし、`DefaultFactory` を生成しない。value box の Helper は rustorb と同じ形（基本型の箱はクラス、それ以外は中身の型）
- JacORB 3.9 は Any の中の値を書くときに `javax.rmi.CORBA.Util` を使い、その実装（JBoss の jar）が Java 17 にない JDK の内部クラスを要るため、JacORB のクライアントは値の入った Any を送れず、JacORB のサーバーも返せない。`echo_any` の3ケースを JacORB の組で `known-failures.txt` に載せた（イメージには javax.transaction の API の jar を足し、`--add-opens java.base/java.io=ALL-UNNAMED` を付けたが、その先で止まる）。Any の外の値は JacORB とすべて通る
- omniORB 4.3 の `omniidl -bcxx` は custom の valuetype を受け付けず（"Unsupported IDL construct"）、omniORBpy は custom の値の送信で `NO_IMPLEMENT` になる。このため custom の相互接続は別のスイートにした
- `supports` と操作・初期化子の生成コードの形は、omniidl（`-bpython`、`-bcxx`）と JacORB の IDL コンパイラの出力に合わせた：omniORBpy は値のクラスが POA のスケルトンも継承し、操作のある値は自身のファクトリにならない。omniORB C++ は `POA_<名前>` が POA のクラスと値のクラスを継承し、`<名前>_init` に純粋仮想の初期化子、`OBV_` クラスは抽象。実装は `PortableServer::ValueRefCountBase` で二つの参照カウントをまとめる。JacORB は値のクラスが `IOperations` を実装した抽象クラスで、`<名前>ValueFactory` に初期化子
- JacORB の `DataOutputStream.write_Value` も、Any と同じく `javax.rmi.CORBA` で RMI のリポジトリ ID を作ろうとして Java 17 で止まる（`INTEROP_TRACE=1` で Java のクライアントがスタックトレースを出す）。Blob は中に Point を `write_Value` で書くので、JacORB の組は null と空以外の5ケースを `known-failures.txt` に載せた
- IDL の識別子は大文字小文字だけが違うと衝突する（omniidl も JacORB も、メンバー `blob` と型 `Blob` をエラーにする）

## abstract interface

`abstract interface A` の値は、オブジェクト参照か valuetype の値のどちらか。CDR では boolean の判別子のあとに、TRUE ならオブジェクト参照（IOR）、FALSE なら値を書く。TypeCode は `tk_abstract_interface`。コアの `Value` は `Objref` か `Valuetype` で、ほかの値は `BAD_PARAM` になる。rustorb は nil をオブジェクト参照の nil（TRUE と空の IOR）で書き、読むときは null の値（FALSE と `0`）も受け付ける。

| | 型 | オブジェクト | 値（`valuetype V supports A`） |
| --- | --- | --- | --- |
| Python | `A`（普通のクラス）と `_objref_A`。`A` の型の引数にはオブジェクト参照・値・`None` を渡す | `interface I : A` のオブジェクト参照。受け取った参照は `_objref_A` | 値のクラスが `A` を継承する |
| C++ | `class A : public virtual CORBA::AbstractBase`（純粋仮想の操作）。`IDL::traits<A>::ref_type` に参照も値も入る。`CORBA::Object` と `CORBA::ValueBase` は `AbstractBase` を仮想継承する | `interface I : A` のスタブは `A` と `CORBA::Object` を継承し、`A` の操作も持つ。`A` の型で受け取った参照は入れ子のスタブ `A::_rorb_stub`。`IDL::traits<I>::narrow` で `I` に、`dynamic_pointer_cast` で値か参照かを見分ける | 値のクラスが `A` を継承する |
| Java | `interface A extends IDLEntity`（操作を直接持ち、`AOperations` はない）、`_AStub`、`AHelper`（`narrow(java.lang.Object)`）、`AHolder` | `IOperations` が `A` を継承する。`A` の型で受け取った参照は `_AStub` | 値のクラスが `A` を実装する |

abstract interface にはスケルトンがなく、それを継承する最初の具象インターフェースのスケルトン（C++ の `POA_I`、Java の `IPOA`）が `A` の操作を受ける。`supports` できる具象インターフェースは一つだけで、abstract interface はいくつでもよい。Java のストリームは `write_abstract_interface` / `read_abstract_interface`、C++ と Java の `DataOutputStream` / `DataInputStream` は `write_Abstract` / `read_Abstract` を持つ。Rust の生成器は abstract interface を扱わない（[rust-mapping.md](rust-mapping.md)）。

相互接続は `interop/idl/AbstractTest.idl` の `AbstractEcho`（13ケース：参照・値・nil の往復、参照と値それぞれへの操作の呼び出し、参照か値かの判定、struct と sequence の中の共有と混在、`interface Person : Named` を返す操作、Any の中の値と参照）で、`run-matrix.sh --suite abstract` で回す。Rust を除く6つの実装の36組が通る（失敗は `known-failures.txt` の範囲だけ）。

### 実測で分かったこと（abstract interface）

- 生成コードの形は omniidl（`-bpython`、`-bcxx`）と JacORB の IDL コンパイラの出力に合わせた。omniORBpy は `Named` が普通のクラスで `_objref_Named` を登録し、値のクラスが `Named` を継承する。omniORB C++ は `Named` が `CORBA::AbstractBase` を仮想継承し、値のクラスが `ValueBase` と `Named` を継承する。JacORB は `Named` が操作を直接持つインターフェースで、`PersonOperations` が `Named` を継承し、Helper は `read_abstract_interface` / `write_abstract_interface` を使う
- omniORB C++ の生成コードは nil を null の値（FALSE と `0`）で書き、受け取った nil の参照は nil のオブジェクト参照（null ポインタではない）になる。rustorb は nil の参照で書くが、どちらの実装も両方の書き方を受け付けるので、すべての組で nil が往復する
- JacORB 3.9 は abstract interface の値も `javax.rmi.CORBA` で書こうとして Java 17 で止まる（サーバーのログに `sun.reflect.ReflectionFactory`）。JacORB は値を受け取れるが送れないので、値を送るケースを JacORB の組で `known-failures.txt` に載せた。参照はすべて通る
- JacORB の `NamedHelper.insert` は、オブジェクトを参照の型の TypeCode（`tk_objref`）で Any に入れる。このため JacORB のクライアントの `echo_any[object]` は TypeCode が `Named` にならない

## 残り

- コードベース URL の利用（読み飛ばすだけ）
- Rust の custom の値を Any に入れること
