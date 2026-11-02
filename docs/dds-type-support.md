# DDS の型サポート（rustorb-idl -brust -Wbdds）

Nov 2, 2026 · [#113](https://github.com/rustorb/rustorb/issues/113)（計画は [#111](https://github.com/rustorb/rustorb/issues/111)）

IDL の struct・union・enum・bitmask・typedef から、DDS（[rustorb-dds](https://github.com/rustorb/rustorb-dds)）で送るための型サポートを Rust の生成コードに出す。読み書きは DDS-XTypes 1.3 の Extended CDR（XCDR1 と XCDR2、7.4 節）、キーとキーハッシュは 7.6.8 節に従う。他の実装のソースは見ず、仕様だけを根拠にした（clean-room）。

## 使い方

```sh
rustorb-idl -brust -Wbdds_only -C src/generated Shapes.idl   # DDS だけ
rustorb-idl -brust -Wbdds      -C src/generated Shapes.idl   # CORBA と DDS の両方
```

- `-Wbdds`：今までの CORBA の出力（[rust-mapping.md](rust-mapping.md)）に加えて、struct・union・enum・bitmask に DDS の impl を出す。同じ Rust の型を CORBA と DDS の両方で使える。GIOP に形のない `@optional`・`@external`・`@value` は、今までどおりエラー
- `-Wbdds_only`：DDS の impl だけを出す。生成コードは `rustorb-core` に依存しない。`@optional`・`@external`・`@value`、`int8`、`bitmask` が使える。interface・valuetype・exception・native はエラー
- `-Wbtypesupport=PATH`：ランタイムのクレートのパス（既定 `::rustorb_typesupport`）
- `-Wbextensibility=KIND`：拡張性の注釈がない型の拡張性（`final`・`appendable`・`mutable`、既定は仕様どおり `appendable`、DDS-XTypes 7.3.1.2.1.8）

`build.rs` からは `rustorb_codegen::Config::new().dds_only()`（または `.dds()`）で同じことができる。

## ランタイム（`rustorb-typesupport`）

生成コードが使うトレイトと補助は、rustorb の新しいクレート `rustorb-typesupport`（MIT OR Apache-2.0）に置いた。依存するのは `rustorb-cdr` だけで、生成コードもこのクレートだけに依存する。生成器（GPL-3.0-or-later、#118 で別リポジトリに分ける予定）とは関係なく、生成されたコードは rustorb の中でテストでき、rustorb-dds はこのクレートを使う。

```rust
pub trait TypeSupport {
    type Rust;                       // 値の Rust の型
    const PRIMITIVE: bool = false;   // DDS-XTypes 7.2.2.2 の基本型か
    fn write(v: &Self::Rust, enc: &mut Encoder) -> Result<()>;
    fn read(dec: &mut Decoder<'_>) -> Result<Self::Rust>;
    fn write_key(v: &Self::Rust, enc: &mut Encoder) -> Result<()>;  // キーホルダー（7.6.8）
    fn max_key_end(offset: usize) -> Option<usize>;               // キーホルダーの最大の大きさ
}

pub trait TopicType: TypeSupport<Rust = Self> {
    const TYPE_NAME: &'static str;          // "Module::Type"
    const EXTENSIBILITY: Extensibility;
    const KEYED: bool;
    fn to_payload(&self, encoding: Encoding, order: ByteOrder) -> Result<Vec<u8>>;
    fn from_payload(buf: &[u8]) -> Result<Self>;
    fn key_holder(&self) -> Result<Vec<u8>>;
    fn key_hash(&self) -> Result<KeyHash>;
}
```

- `TypeSupport` は IDL の型ごとの記述子のトレイトで、CORBA の `rustorb_core::idl::IdlType` と同じ考え方。Rust の型が同じになる IDL の型（`char` と `wchar`、`string` と `wstring`、上限つきの形）は記述子の型（`Char8`、`Char16`、`Str`、`WStr`、`BoundedStr<N>`、`BoundedWStr<N>`、`Seq<D>`、`BoundedSeq<D, N>`、`Array<D, N>`、`External<D>`）で区別する。基本型と生成した型は自分が記述子
- エンコーディングの版は `Encoder`/`Decoder` の `Encoding`（`Xcdr1` か `Xcdr2`）で決まる
- `TopicType` は入れ子でない struct と union に出す。`to_payload` は RTPS の SerializedPayload（カプセル化ヘッダー、値、4 オクテットへの詰め物）
- 集成型のヘッダー（DHEADER、EMHEADER、パラメーターリスト、`@optional`）は `rustorb_typesupport::aggregate` の関数にまとめ、生成コードはそれを呼ぶ
- キーハッシュとメンバー名のハッシュに使う MD5 は RFC 1321 から実装した（依存を `rustorb-cdr` だけにするため）

## マッピング

| IDL | Rust の型 | 記述子 |
| --- | --- | --- |
| `boolean`, `octet`, `int8`, `short` … `unsigned long long`, `float`, `double` | `bool`, `u8`, `i8`, `i16` … `u64`, `f32`, `f64` | 同じ型 |
| `long double` | `LongDouble` | 同じ型 |
| `char` / `wchar` | `char` | `Char8`（U+00FF まで）/ `Char16`（BMP） |
| `string` / `string<N>` | `String` | `Str` / `BoundedStr<N>`（UTF-8 のオクテット数） |
| `wstring` / `wstring<N>` | `String` | `WStr` / `BoundedWStr<N>`（UTF-16 の単位数） |
| `sequence<T>` / `sequence<T, N>` | `Vec<T>` | `Seq<D>` / `BoundedSeq<D, N>` |
| `T a[N][M]` | `[[T; M]; N]` | `Array<Array<D, M>, N>` |
| `@optional T` | `Option<T>` | （struct の生成コードが扱う） |
| `@external T` | `Box<T>` | `External<D>` |
| `enum` | `enum`（`value()`、`from_value()`） | 生成した型 |
| `bitmask` | `struct B(pub u8 / u16 / u32 / u64)`、フラグは定数 | 生成した型 |
| `typedef` | `type` | 元の型の記述子 |

struct・union・enum の Rust の型は CORBA のマッピングと同じ（`-Wbdds` で両方の impl が付く）。`-Wbdds_only` のときだけ `@optional` と `@external` のメンバーが `Option` と `Box` になる。

## 注釈

| 注釈 | 扱い |
| --- | --- |
| `@final` `@appendable` `@mutable` `@extensibility(K)` | 拡張性。なければ `appendable`（`-Wbextensibility` で変えられる） |
| `@key` | キーのメンバー。キーハッシュと `KEYED`。`@optional` と一緒はエラー。must understand になる |
| `@id(n)` `@hashid` `@hashid("名前")` `@autoid(SEQUENTIAL / HASH)` | メンバー ID（7.3.1.2.1.1）。`@autoid` は型か、それを囲むいちばん内側のモジュールのもの。値なしの `@autoid` は HASH（IDL 4.2 8.3.1.2）。ID の重複はエラー |
| `@optional` | final / appendable では XCDR1 はパラメーターヘッダー（なければ長さ 0）、XCDR2 は有無の boolean。mutable ではないときメンバーごと書かない |
| `@external` | `Box`。書き方は中身と同じ |
| `@must_understand` | EMHEADER の M フラグとパラメーター ID の FLAG_MUST_UNDERSTAND。読む側が知らない ID でこのフラグがあればエラー |
| `@nested` `@default_nested` `@topic` | 入れ子の型には `TopicType` を出さない。`@topic` は他の二つより優先（7.3.1.2.1.16） |
| `@bit_bound` | enum は 8 以下で `int8`、16 以下で `int16`、それ以外は `int32` として書く。bitmask は `uint8`〜`uint64` |
| `@value` `@default_literal` | enum の値と既定の列挙子 |
| `@position` | bitmask のビットの位置 |

## 読み書きの規則と、仕様の解釈

DDS-XTypes 1.3 の 7.4.3.5.3 の規則（1）〜（30）に従う。規則がはっきりしないところや、仕様の中で食い違っているところは、次のように決めた。相互接続の試験（rustorb/rustorb-dds#8）で確かめる。

- **列挙型は基本型ではない**：7.2.2.2 の基本型に列挙型と bitmask は入っていないので、XCDR2 では enum・bitmask・文字列・struct などの列や配列に DHEADER を付ける（規則 9、12）
- **配列の配列は一つの配列**：表 40 の ARRAY\_TYPE のとおり、多次元配列には DHEADER を一つだけ付ける
- **キーホルダーの再帰**：7.6.8 の手順 3 は「メンバーが集成型なら」としか書いていないが、列と配列の要素の struct もキーホルダーにして書く
- **7.6.8 の例 3**：`Foo` では `string<16> label`、`FooKeyHolder` では `string<12>` で、最大の大きさ 21 は 12 で計算している。どちらでも 16 オクテットを超えるので MD5 になり、ハッシュは同じ
- **mutable の欠けたメンバー**：受け取らなかったメンバーは、optional なら `None`、キーならエラー、それ以外は既定値（7.4.1.2.3）。既定値のない型（union など）ならエラー
- **appendable の古い版**：XCDR2 では DHEADER の終わりより後のメンバーを既定値にする。XCDR1 には長さがないので分からない。新しい版のメンバーは XCDR2 では読み飛ばす
- **EMHEADER の長さコード**：1・2・4・8 オクテットのメンバーには LC 0〜3 を使う（長さ 4 の文字列でも）。LC 5〜7 は読むが書かない（rustorb-cdr の決めたとおり）
- **ペイロードの形式**：`from_payload` はカプセル化の形式が型の拡張性と合わなければエラー（`FormatMismatch`）

## まだないもの

- map（rustorb-idl のフロントエンドがまだ読まない）、struct の継承、bitset
- メンバーの `@default`、`@try_construct`、`@non_serialized`：書き方や読み方を変えるので、無視せずにエラーにする
- mutable の union（discriminator のメンバー ID の決め方を確かめてから）
- TypeObject / TypeIdentifier と型の互換判定（D3）
- Python・C++・Java への DDS の出力（D3 以降）

## idl_parser で確かめたこと

上の注釈はどれも idl_parser（5eef768）で期待どおり取れる（`@hashid("名前")` の引数は引用符つき、`@topic(name=…, platform=…)` は名前つき引数）。取れないのは次の三つで、idl_parser 側の対応が要る。

- union の discriminator の注釈（`switch (@key long)`）：構文エラーになる
- 型の中の注釈（`sequence<@external Foo>`）：エラーにならず、注釈が消える
- コメントの形の注釈（`long a; //@key`、DDS-XTypes 7.3.1.2.3）：コメントとして捨てられる
