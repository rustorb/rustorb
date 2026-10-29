# valuetype

Oct 29, 2026 · @ysuga · M6 の後

rustorb の valuetype（値型）の実装をまとめる。この段階の範囲は CDR、Rust コア、Python バインディングと `rustorb-idl -bpython`。完了条件は「状態メンバー・継承・truncatable・null・共有参照・value box を使う操作が omniORB C++・omniORBpy と相互接続できること」で、`interop/run-matrix.sh --suite value` で判定する。C++ と Java のバインディングは次の段階で加える（それまで `-bc++11` / `-bjava` は valuetype を含む IDL をエラーにする）。

## 構成

| 場所 | 内容 |
| --- | --- |
| `crates/rustorb-cdr/src/value.rs` | 値のタグ、null、間接参照、リポジトリ ID とその共有、チャンク、終了タグ（`Encoder::begin_value` / `end_value`、`Decoder::read_value_header` / `end_value`） |
| `crates/rustorb-core/src/value.rs` | `Value::Valuetype`、`ValueRef`、valuetype の登録、`write_valuetype` / `read_valuetype` |
| `crates/rustorb-core/tests/valuetype.rs` | 共有、循環、切り捨て、未知の型、value box、Any の中の値 |
| `crates/rustorb-python/src/value.rs` | Python のインスタンスとコアの `ValueRef` の変換 |
| `python/rustorb/CORBA.py`、`_rt.py` | `CORBA.ValueBase`、`ValueMember`、`ORB.create_value_tc`、値ファクトリ |
| `crates/rustorb-codegen/python/rustorb_idl/python_backend.py` | valuetype・value box・前方宣言の生成 |
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

## 相互接続

`interop/idl/ValueTest.idl` の `ValueEcho`（23ケース）を rustorb（Rust）・rustorbpy・omniORBpy・omniORB C++ のクライアントとサーバーで実装し、`run-matrix.sh --suite value` で回す。rustorb か rustorbpy を含む16組は GIOP 1.2 / 1.1 ですべて通る。

ケース：プレーンな値、null、truncatable の派生型、struct の中の共有と別インスタンス、同一性（`same`）、20個のリストと2個の循環、sequence と他の値をメンバーに持つ値、sequence の中の共有と null、value box（long・string・struct）、Any の中の循環・派生型・value box。

### 実測で分かったこと

ソースは読まず、生成コードのコンパイル結果と通信内容だけを見た。

- omniORB C++ の `OBV_` クラスは、操作のない valuetype なら具象クラス（参照カウントと `_copy_value` を持つ）で、そのままファクトリに使える。private のメンバーのアクセサは protected
- omniORBpy はチャンク付きの値をリポジトリ ID なし（`0x7fffff08`）で書くことがある。omniORB C++ は Any の中の値を型情報なし（`0x7fffff00`）で書く。どちらも宣言された型（Any なら TypeCode）で読める
- valuetype の TypeCode のメンバーの可視性を、omniORB C++ は public でも 0（private）で書き、omniORBpy は 1 で書く。このため omniORB C++ が送った Any を omniORBpy が受け取ると TypeCode が `equal` にならず、omniorb-cpp → omniorbpy の `echo_any[ring]` と `echo_any[derived]` は `known-failures.txt` に載せた（rustorb はどちらの書き方も受け取って、そのまま返す）

## 残り

- C++ と Java のバインディング（次の段階）
- `custom` の valuetype（`CustomMarshal`）、abstract interface、`supports` するインターフェース、valuetype の操作と初期化子のサーバント側の扱い
- コードベース URL の利用
