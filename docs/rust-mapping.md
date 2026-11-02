# IDL to Rust マッピングと Rust 生成器（rustorb-idl -brust）

Oct 29, 2026 · @ysuga · [#37](https://github.com/rustorb/rustorb/issues/37)

rustorb の Rust 用 IDL マッピングと、その生成器 `rustorb-idl -brust` の設計をまとめる。Rust には OMG の採択済みマッピングがない（[IDL4 to Rust の RFP](https://www.omg.org/gettingstarted/rfp-IDL4-RLP.htm) は 2026年6月に出た段階）ため、独自のマッピングを定義する。将来 OMG の仕様が採択されたら、それに寄せる版を別に検討する。

それまで手書きだった Rust のスタブとスケルトン（`interop/impls/rustorb` の `idl.rs` とその利用側、`anytest.rs`・`valuetest.rs`・`poa_probe_server.rs`、`rustorb-naming` の CosNaming）は、生成コードに置き換えた。完了条件の「手書きのスタブを生成コードで置き換えても、相互接続マトリクスの結果が変わらないこと」は満たしている（「置き換えと検証」）。

マッピングは利用者向けの API になるので、設計書のレビューを経てから実装した。合意した点は「決定事項」にまとめてある。実装で設計から変えた点も、それぞれの節に反映した。

## 方針

- **素直な Rust にする**：Rust の命名規約、`Result` による例外、所有と借用に沿った引数の渡し方、標準の型（`String`、`Vec`、配列、`Option`）を使う。IDL の型の区別（`string` と `wstring` など）は Rust の型には持ち込まず、生成コードが IDL の型を知っていることで済ませる
- **同期 API と async 版**：コアと同じく、スタブの呼び出しはブロックする。操作ごとに `async fn <名前>_async` も出す（コアの async API の上、[async-api.md](async-api.md#生成コードの-async-版スタブ)、[#77](https://github.com/rustorb/rustorb/issues/77)）。サーバントのトレイトは同期のまま
- **生成コードは安全な Rust だけ**：`unsafe` を使わない。マーシャリングはコアの `OutStream` / `InStream` の上に直接書き、動的な `Value` を経由しない（Any を除く）
- **ランタイムは `rustorb-core` に置く**：生成コードが使うトレイトと補助の型は `rustorb_core::idl` モジュールに足す。新しいクレートは作らない
- **生成器は Python**：他の backend と同じく `crates/rustorb-codegen/python/rustorb_idl/rust_backend.py`（GPL-3.0-or-later、生成物は対象外）。フロントエンド（`frontend.py`）は共通

## 決定事項（合意済み）

| # | 論点 | 提案 | 主な代案 |
| --- | --- | --- | --- |
| 1 | 名前 | Rust の命名規約に変換する（モジュール・フィールド・メソッドは snake\_case、型と列挙子は UpperCamelCase、定数は SCREAMING\_SNAKE\_CASE）。変換後に衝突したら生成器がエラー | IDL の名前をそのまま使い、`#[allow(non_snake_case)]` などで警告を消す（Python / C++ / Java の生成器と同じ考え方） |
| 2 | ユーザー例外 | 操作ごとのエラー型（`enum`、ユーザー例外の各型と `System(SystemException)`）を返す。`raises` のない操作は `Result<T, SystemException>` | すべての操作で `Result<T, rustorb_core::Exception>` を返し、利用者が型を調べる |
| 3 | `out` / `inout` | `out` は戻り値のタプル（戻り値が先頭）、`inout` は `&mut T` | どちらも `&mut T`（C++ と同じ形） |
| 4 | `char` / `wchar` | どちらも Rust の `char`。送れない文字は `DATA_CONVERSION` | `char` は `u8`、`wchar` は `u16` の新しい型 |
| 5 | 生成コードの置き場所 | `rustorb-naming` と `interop/impls/rustorb`（と生成器のテスト）は生成コードをリポジトリに入れ、CI で再生成の差分がないことを確かめる。利用者向けに `build.rs` から呼ぶ補助を `rustorb-codegen` クレートに置く（GPL だが build-dependency にしかならない） | ビルドのたびに `build.rs` で生成する（`cargo build` に Python と idl\_parser が要る） |
| 6 | valuetype | 共有される参照のハンドル型（`Arc` の上）。状態は生成した構造体で、クロージャで読み書きする | コアの動的な `ValueRef` をそのまま使う（型の安全さがない） |

以下は、この提案どおりに決まったものとして書いている。

## 使い方

```sh
rustorb-idl -brust -C src/generated InteropTest.idl     # src/generated/interop_test.rs
```

IDL ファイル1つから Rust のソース1つ（ファイル名は IDL のファイル名を snake\_case にしたもの）ができる。中身は IDL のモジュールに対応する入れ子の `pub mod` で、利用側はふつうのモジュールとして取り込む。

```rust
#[path = "generated/interop_test.rs"]
mod interop_test_idl;
use interop_test_idl::interop_test::{Echo, EchoServant, EchoPoa, Point};
```

`build.rs` で生成する場合は `rustorb-codegen` の補助を使う（`rustorb-idl` が `PATH` にあること）。

```rust
// build.rs
fn main() {
    rustorb_codegen::Config::new()
        .include("idl")
        .compile(&["idl/Example.idl"])   // $OUT_DIR/example.rs
        .unwrap();
}
// src/lib.rs
include!(concat!(env!("OUT_DIR"), "/example.rs"));
```

`#include` した IDL の定義も同じ出力に入れる（出力1つで閉じる）。別のクレートにある生成コードを使うときは `-Wbextern=MODULE=PATH` で、そのモジュールの定義を出力せずに `PATH` を参照させる。たとえば `-Wbextern=CosNaming=rustorb_naming::idl::cos_naming` とすると、CosNaming の型は `rustorb-naming` のものを使う。

オプション：

- `-Wbextern=MODULE=PATH`：上記。複数指定できる。`PATH` が `::`・`crate::`・`super::`・`self::` で始まらなければ `::` を前に付ける（ほかのクレート）。同じクレートの中なら `crate::idl::any_test` のように書く
- `-Wbcore=PATH`：コアのクレートのパス（既定 `::rustorb_core`）
- `-Wbno_rustfmt`：出力を `rustfmt` に通さない。`rustfmt` が見つからないときも通さない（どちらでもコンパイルできる。整形が違うだけ）

生成コードは絶対パス（コアは `::rustorb_core`）か、`super::` の相対パスで型を参照するので、`include!` や `#[path]` でどのモジュールの下に置いてもよい。

生成コードの先頭には `rustorb_core::idl::GENERATED_CODE_VERSION_1` を参照する行を入れ、生成器とコアの版が合わないとコンパイルエラーになるようにする。

## ランタイム（`rustorb_core::idl`）

生成コードとコアの取り決めは、IDL の型ごとの記述子のトレイトにまとめる。

```rust
pub trait IdlType {
    /// この IDL の型を表す Rust の型。
    type Rust: Borrow<Self::Borrowed> + 'static;
    /// 送るときに受け取る形（`String` なら `str`、`Vec<T>` なら `[T]`）。
    type Borrowed: ?Sized;
    /// CDR での最小の大きさ（受け取ったシーケンスの長さの検査に使う）。
    const MIN_SIZE: usize;
    fn type_code() -> TypeCode;
    fn write(v: &Self::Borrowed, out: &mut OutStream<'_>) -> Sys<()>;
    fn read(input: &mut InStream<'_>) -> Sys<Self::Rust>;
    // シーケンスと配列の要素。既定は1つずつ、octet は一括でコピーする。
    fn write_items(items: &[Self::Rust], out: &mut OutStream<'_>) -> Sys<()>;
    fn read_items(len: usize, input: &mut InStream<'_>) -> Sys<Vec<Self::Rust>>;
}
```

（`Sys<T>` は `Result<T, SystemException>`。）

- 名前のある型（struct、union、enum、例外、インターフェース、valuetype）は、生成した型そのものが `IdlType<Rust = Self>` を実装する
- 基本型と無名の型には、コアが記述子の型を持つ：`idl::Long`（`Rust = i32`）、`idl::WString`（`Rust = String`）、`idl::WChar`（`Rust = char`）、`idl::Sequence<T>`（`Rust = Vec<T::Rust>`）、`idl::BoundedString<N>`、`idl::BoundedSequence<T, N>`、`idl::Array<T, N>`、`idl::Fixed<D, S>`、`idl::Object`、`idl::Any`、`idl::TypeCodeType` など
- typedef は Rust の `type` 別名と、別名の TypeCode を持つ記述子の型を作る（下記）
- `octet` のシーケンスと配列は一括でコピーする（`sequence<octet>` が1バイトずつにならない）。ほかの数値型は、バイト順とアラインメントの扱いをコアに任せて1つずつ書く

同じ Rust の型でも IDL の型は違うことがある（`string` と `wstring` はどちらも `String`）が、生成コードは IDL の型を知っているので、正しい記述子を使う。利用者が記述子を意識するのは Any に入れるときくらい。

コアに足すもの：

- `rustorb_core::idl`（上記のトレイトと記述子、生成コード用の補助関数）
- `Fixed`（`fixed` の値の型、下記）
- valuetype のハンドルと、読み書きのときのインスタンスの表（下記）
- `Any::insert::<T: IdlType>(v) -> Result<Any, SystemException>` / `Any::extract::<T>() -> Option<T::Rust>`
- `ObjectRef` の `PartialEq`（`is_equivalent` で比べる。オブジェクト参照を持つ struct も `PartialEq` を derive できるように）

## 名前

| IDL | Rust | 例 |
| --- | --- | --- |
| モジュール | snake\_case の `pub mod` | `CosNaming` → `cos_naming`、`InteropTest` → `interop_test` |
| 型（struct、union、enum、例外、インターフェース、valuetype、typedef） | UpperCamelCase | `NamingContextExt` はそのまま、`point` → `Point` |
| struct・例外・valuetype のメンバー、操作、引数 | snake\_case | `binding_name` はそのまま、`getValue` → `get_value` |
| 属性 | 読み出しは名前そのまま、書き込みは `set_` を付ける | `attribute long counter` → `counter()` / `set_counter(v)` |
| 列挙子、union のメンバー（バリアント） | UpperCamelCase | `RED` → `Red`、`nobject` → `Nobject` |
| 定数 | SCREAMING\_SNAKE\_CASE | `MAGIC` はそのまま、`maxLength` → `MAX_LENGTH` |

- 変換は決まった規則で行う：`_` で区切り、大文字の連続（`URLString` の `URL`）は1語として扱う。全部大文字の語は先頭だけ大文字にする（`RED_ALERT` → `RedAlert`）
- Rust のキーワードになる名前は生の識別子（`r#type`）にする。生の識別子にできないもの（`self`、`Self`、`super`、`crate`）は末尾に `_` を付ける
- 変換した結果、同じスコープで名前がぶつかったら（`getValue` と `get_value`、操作 `set_counter` と属性 `counter` の書き込み）、生成器がエラーにする
- 生成コードが足す名前（`EchoServant`、`EchoPoa`、エラー型、`alias` モジュール）と IDL の名前がぶつかったときも、生成器がエラーにする
- リポジトリIDは IDL の名前のままで、変換しない

## 型の対応

| IDL | Rust（値） | `in` 引数 |
| --- | --- | --- |
| `boolean` | `bool` | `bool` |
| `octet` | `u8` | `u8` |
| `short`・`unsigned short` | `i16`・`u16` | 同じ |
| `long`・`unsigned long` | `i32`・`u32` | 同じ |
| `long long`・`unsigned long long` | `i64`・`u64` | 同じ |
| `float`・`double` | `f32`・`f64` | 同じ |
| `long double` | `rustorb_core::LongDouble`（binary128 のビット、[long-double.md](long-double.md)） | 同じ |
| `char`・`wchar` | `char` | `char` |
| `string`・`wstring`（上限付きも） | `String` | `&str` |
| `sequence<T>`（上限付きも） | `Vec<T>` | `&[T]` |
| `T x[N][M]` | `[[T; M]; N]` | `&[[T; M]; N]` |
| enum | `enum`（`Copy`） | 値 |
| struct・union・例外 | `struct` / `enum` | `&T` |
| インターフェース | `Option<I>`（`None` が nil） | `Option<&I>` |
| `Object` | `Option<ObjectRef>` | `Option<&ObjectRef>` |
| `any` | `rustorb_core::Any` | `&Any` |
| `TypeCode` | `rustorb_core::TypeCode` | `&TypeCode` |
| `fixed<D, S>` | `rustorb_core::Fixed` | `&Fixed` |
| valuetype・value box | `Option<V>`（`None` が null） | `Option<&V>` |

- **`char` / `wchar`**：Rust の `char` にする。`char` は交渉した `char` のコードセットで1バイトに、`wchar` は UTF-16 の1単位にならない文字を送ると `DATA_CONVERSION`。サロゲートの片方だけの `wchar` を受け取ったときも `DATA_CONVERSION`
- **`string` / `wstring`**：`String`。`string` の中の NUL は送れない（`BAD_PARAM`）
- **上限付きの型**：Rust の型は上限のない型と同じで、送るときに上限を超えていたら `BAD_PARAM`、受け取ったときは `MARSHAL`
- **配列**：Rust の固定長配列。要素の型が `Default + Copy` でなくても読めるよう、読み込みは `Vec` に読んでから変換する

## struct・enum・union・typedef・定数

**struct** はすべてのメンバーが `pub` の構造体にする。`Debug`、`Clone`、`PartialEq` は常に、`Default` はすべてのメンバーが持つとき、`Copy` はすべてのメンバーが `Copy` のとき、`Eq` と `Hash` は浮動小数点・`any`・`TypeCode`・オブジェクト参照・valuetype を含まないときに derive する。再帰する struct（`sequence<Node>` を持つ `Node`）は `Vec` を通るので、そのまま書ける。

```rust
// struct Point { double x; double y; };
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
```

**enum** は `#[repr(u32)]` の Rust の `enum` で、`Debug`、`Clone`、`Copy`、`PartialEq`、`Eq`、`Hash`、`PartialOrd`、`Ord`、`Default`（最初の列挙子）を derive する。範囲外の値を受け取ったら `MARSHAL`。

**union** は Rust の `enum` にし、メンバーごとにバリアントを1つ作る。

- ラベルが1つのメンバーは `Member(T)`
- ラベルが複数あるメンバーと `default` のメンバーは、どの判別子だったかを保つため `Member(D, T)`（`D` は判別子の型）
- `default` がなく、ラベルが判別子のすべての値を覆っていない union には、どのメンバーも持たない状態の `Default(D)` を足す
- `discriminator(&self) -> D` を持つ。`Member(D, T)` の判別子が、そのメンバーのラベル（`default` なら他のメンバーのどのラベルでもない値）でないものを送ると `BAD_PARAM`

```rust
// union Value switch (long) {
//   case 1: long l; case 2: string s; case 3: Point p;
//   case 4: case 5: LongSeq seq; default: boolean flag; };
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    L(i32),
    S(String),
    P(Point),
    Seq(i32, Vec<i32>),
    Flag(i32, bool),
}
```

**typedef** は Rust の `type` 別名と、別名の TypeCode を持つ記述子の型を作る。記述子は型と名前がぶつからないよう、そのモジュールの `alias` サブモジュールに置く。

```rust
// typedef sequence<long> LongSeq;
pub type LongSeq = Vec<i32>;
pub mod alias {
    pub struct LongSeq;   // IdlType<Rust = Vec<i32>>、TypeCode は tk_alias
}
```

struct や union の typedef（`typedef Point Pt;`）は `pub type Pt = Point;` になる。

**定数** は `pub const` にする。`string` / `wstring` の定数は `&str`、列挙型の定数は列挙子にする。`fixed` の定数は `Fixed` を返す `pub fn` で、名前は関数なので snake\_case（`const Money C_FIXED = 12.50d;` → `pub fn c_fixed() -> Fixed`。`Fixed` は `const` で作れないため）。

## 例外とエラー

**ユーザー例外** は struct と同じ形の構造体にし、`std::error::Error` と `Display` を実装する。

**操作ごとのエラー型**：`raises` のある操作は、インターフェースの名前の snake\_case のモジュールに、操作の名前の UpperCamelCase に `Error` を付けた `enum` を作る。インターフェースの中で定義した型（`NamingContext::NotFound` など）も同じモジュールに入る（`cos_naming::naming_context::NotFound`）。

```rust
// long divide(in long a, in long b, out long remainder) raises (DivideByZero);
pub mod echo {
    #[derive(Debug, Clone)]
    pub enum DivideError {
        DivideByZero(super::DivideByZero),
        System(rustorb_core::SystemException),
    }
}
```

- 各ユーザー例外と `SystemException` からの `From` を実装するので、サーバントでもクライアントでも `?` で変換できる
- `Display`、`std::error::Error` を実装する。`System` 以外のバリアントの例外のリポジトリIDも得られる（`repository_id()`）
- `raises` のない操作と属性は `Result<T, SystemException>` を返す
- クライアントが `raises` にないユーザー例外を受け取ったら、`System(UNKNOWN)`（minor `UNLISTED_USER_EXCEPTION`）にする

## オブジェクト参照とスタブ

インターフェース `I` は、オブジェクト参照を包む型 `I`（`ObjectRef` の新しい型）になる。

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Echo(ObjectRef);

impl Echo {
    pub const REPOSITORY_ID: &str = "IDL:rustorb.org/InteropTest/Echo:1.0";
    /// IOR の型IDか `_is_a` で確かめる。違う型なら Ok(None)。
    pub fn narrow(obj: &ObjectRef) -> Result<Option<Echo>, SystemException>;
    pub fn unchecked_narrow(obj: ObjectRef) -> Echo;
    pub fn object(&self) -> &ObjectRef;
    pub fn into_object(self) -> ObjectRef;

    pub fn echo_long(&self, v: i32) -> Result<i32, SystemException>;
    pub fn swap(&self, a: &mut i32, b: &mut i32) -> Result<i32, SystemException>;   // inout 2つ
    pub fn divide(&self, a: i32, b: i32) -> Result<(i32, i32), echo::DivideError>;  // (戻り値, remainder)
    pub fn counter(&self) -> Result<i32, SystemException>;
    pub fn set_counter(&self, v: i32) -> Result<(), SystemException>;
    pub fn post(&self, message: &str) -> Result<(), SystemException>;   // oneway
}
```

- **引数**：`in` は上の表のとおり（`Copy` の型は値、それ以外は借用）。`inout` は `&mut T`（文字列は `&mut String`、シーケンスは `&mut Vec<T>`、参照は `&mut Option<I>`）。`out` は戻り値のタプルに入る（戻り値が先頭、続けて `out` を宣言の順に）。戻り値と `out` が1つだけならタプルにせず、ないなら `()`
- **継承**：基底のインターフェースの操作も、派生の型にすべて生成する（多重継承でもそのまま使える）。祖先それぞれへの `From<Derived> for Base` を実装する（広げる変換）
- **`AsRef<ObjectRef>`** を実装し、`ObjectRef` の操作（`is_a`、`non_existent`、`is_equivalent` など）はそこから呼ぶ
- **oneway**：`send_oneway` で送り、`Result<(), SystemException>` を返す
- `IdlType` を実装し（`Rust = Option<Echo>`）、TypeCode は `tk_objref`

## サーバント

インターフェース `I` ごとに、サーバントのトレイト `IServant` と、それをコアの `Servant` にする包み `IPoa<T>` を作る。

```rust
pub trait EchoServant: Send + Sync {
    fn echo_long(&self, v: i32) -> Result<i32, SystemException>;
    fn divide(&self, a: i32, b: i32) -> Result<(i32, i32), echo::DivideError>;
    fn counter(&self) -> Result<i32, SystemException>;
    fn set_counter(&self, v: i32) -> Result<(), SystemException>;
    fn post(&self, message: &str) -> Result<(), SystemException>;
    // ...
}

pub struct EchoPoa<T: EchoServant>(pub T);

impl<T: EchoServant> rustorb_core::Servant for EchoPoa<T> { /* 生成された dispatch */ }
```

- メソッドのシグネチャはクライアント側と同じ（`&self`）。サーバントは複数のアップコールのスレッドから同時に呼ばれるので、状態は `Mutex` などで持つ
- **継承**：`trait DerivedServant: BaseServant` のように基底のトレイトを親トレイトにする。`DerivedPoa<T>` は基底の操作も受け付け、`is_a` はすべての祖先のリポジトリIDに真を返す
- 受け付ける操作の名前は IDL の名前（属性は `_get_counter` / `_set_counter`）。知らない操作は `BAD_OPERATION`（minor `UNKNOWN_OPERATION`）
- ユーザー例外のバリアントを返すと、そのユーザー例外の返事になる
- 活性化は `poa.activate_object(Arc::new(EchoPoa(MyEcho::new())))` などコアの API のまま。型付きの参照が欲しいときのために `EchoPoa::activate(self, &Poa) -> Result<(ObjectId, Echo), PoaError>` と、USER\_ID の POA 用の `EchoPoa::activate_with_id(self, &Poa, id) -> Result<Echo, PoaError>` がある。`Echo::unchecked_narrow(poa.servant_to_reference(&s)?)` も使える
- サーバントの中で、自分の参照やオブジェクトIDはコアの `PoaCurrent` から得る

```rust
struct MyEcho { counter: Mutex<i32> }

impl EchoServant for MyEcho {
    fn echo_long(&self, v: i32) -> Result<i32, SystemException> { Ok(v) }
    fn divide(&self, a: i32, b: i32) -> Result<(i32, i32), echo::DivideError> {
        if b == 0 {
            return Err(DivideByZero {}.into());
        }
        Ok((a.wrapping_div(b), a.wrapping_rem(b)))
    }
    // ...
}
```

## Any と TypeCode

`any` はコアの `Any`、`TypeCode` はコアの `TypeCode` をそのまま使う。生成した型（と記述子）は `IdlType::type_code()` で TypeCode を返す（`OnceLock` に作っておく）。

```rust
let a = Any::insert::<Point>(&p)?;                      // tk_struct
let b = Any::insert::<alias::LongSeq>(&[1, 2, 3])?;     // tk_alias
let c = Any::insert::<idl::WString>("wide")?;
let t = Any::insert::<idl::TypeCodeType>(&Point::type_code())?;
let p: Option<Point> = a.extract::<Point>();            // TypeCode が等価でなければ None
```

`insert` / `extract` は、値をその TypeCode の CDR に書いてからコアの `Value` として読む（逆も同じ）ことで変換する。`insert` は、送れない値（上限を超えた文字列など）を送るときと同じく `BAD_PARAM` などで失敗する。生成コードが型ごとに `Value` との変換を持たずに済み、valuetype の共有と循環もそのまま保たれる。Any に入れるのは速さが要らない用途なので、このコストは許容する。オブジェクト参照は CDR を通さずに受け渡す（コアの単独のストリームに、参照を横に持たせる口を足す）。

## fixed

`fixed<D, S>` はコアに足す `rustorb_core::Fixed`（符号、桁、スケール）にする。文字列との変換（`"123.45".parse::<Fixed>()`、`Display`）と、桁の調整（`Fixed::round` / `truncate`）を持つ。算術は [#47](https://github.com/rustorb/rustorb/issues/47) の範囲で、ここでは入れない。送るときに型の桁数とスケールに合わせる。スケールより下の桁は切り捨て、整数部が入らないと `DATA_CONVERSION`。

## valuetype

valuetype は参照で共有され（同じインスタンスはメッセージの中で1回だけ送られる）、循環し、`null` があり、派生の型のインスタンスが基底の型として届く（truncatable）。Rust の値の型ではこれを表せないので、ハンドル型にする。

valuetype `V` ごとに、状態の構造体 `VState` とハンドル `V` を作る。

```rust
// valuetype Point { public long x; public long y; };
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PointState { pub x: i32, pub y: i32 }

// valuetype Point3 : truncatable Point { public long z; private string label; };
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Point3State { pub base: PointState, pub z: i32, pub label: String }

#[derive(Clone)]
pub struct Point(rustorb_core::idl::ValueHandle);   // Point か、その派生のインスタンス

impl Point {
    pub fn new(state: PointState) -> Point;
    pub fn get(&self) -> PointState;                       // 写しを返す
    pub fn set(&self, state: PointState);
    pub fn with<R>(&self, f: impl FnOnce(&PointState) -> R) -> R;
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut PointState) -> R) -> R;
    pub fn ptr_eq(&self, other: &Point) -> bool;           // 同じインスタンスか
    pub fn handle(&self) -> &ValueHandle;                  // 型のないハンドル
    pub fn from_handle(h: ValueHandle) -> Option<Point>;   // Point かその派生なら
}
impl From<Point3> for Point { /* 基底への変換 */ }
impl TryFrom<Point> for Point3 { /* 派生への変換。違えば元の Point を返す */ }
```

- ハンドルは `Arc` で共有し、状態はインスタンスごとの `Mutex` に入れる。`Point` のハンドルが `Point3` のインスタンスを指しているときは、`with` などは `Point3State` の中の `base` を見せる
- メンバーの `public` / `private` は Rust では区別せず、どちらも `pub` のフィールドにする（Rust には派生クラスがないため）
- 値の型のメンバーは `Option<V>`（`None` が `null`）。`Node { next: Option<Node> }` のように循環も作れる。循環した `Arc` は解放されないので、要らなくなったら利用者が切る（Rust の `Rc` の循環と同じ）
- `with` / `with_mut` の中で同じインスタンスに触るとデッドロックするので、そのときは `get` を使う（ドキュメントに書く）
- 送るときは、同じインスタンスを2回目からは間接参照で書く。受け取るときは位置ごとのインスタンスの表で共有と循環を戻す。truncatable な型は chunk 形式で、すべての基底のリポジトリIDを付けて送る（コアの `begin_value` / `end_value` をそのまま使う）
- **知らない派生の型**：受け取った値の型が、期待した型でも登録された型でもないときは、truncatable な基底のうち知っている最初の型として読み、なければ `MARSHAL`（コアと同じ）。生成した型を受け取り側に知らせるため、出力ファイルごとに `register_value_types()` を作る（Rust には静的な初期化がないので、利用者が1回呼ぶ）
- **value box**：`valuetype LongBox long;` は `#[derive(Clone)] pub struct LongBox(Arc<i32>)` にする（`new`、`Deref`、`ptr_eq`）。中身は変えられない。値の型のメンバーと同じく `Option<LongBox>`
- **custom**（[#42](https://github.com/rustorb/rustorb/issues/42)）：`custom valuetype V` の状態の構造体 `VState` に、利用者が `rustorb_core::idl::CustomMarshal`（`marshal(&self, &mut DataOutputStream)` / `unmarshal(&mut self, &mut DataInputStream)`）を実装する。値は常に chunk 形式で送る。custom の値も Any に入る（#85）：`register_value_types()` が custom の型の読み込み関数をコアに登録し、コアは `unmarshal` を走らせて、`DataInputStream::read` / `read_array` で読んだものをコアのインスタンスの状態（項目の列）として持つ。このため Any に入れる custom の値の `unmarshal` は、`stream()` で直接読まずに `read` / `read_array` で読む。自分の状態の中から自分を参照する custom の値は Any に入れられない（`MARSHAL`）
- **操作と supports**（#42）：操作か `supports I` を持つ valuetype には、トレイト `VOperations`（valuetype の操作と、`I` とその基底の操作。`&mut self` で、例外は `SystemException` だけ）を作り、利用者が `VState` に実装する。ハンドル `V` には valuetype 自身の操作を同名のメソッドとして作り、状態をロックしてトレイトを呼ぶ（中で同じインスタンスに触るとデッドロックする）。`supports I` なら `impl IServant for V` を作るので、`IPoa(v.clone()).activate(&poa)` で値をサーバントにできる。初期化子（`factory`）は生成せず、`V::new(state)` で作る。例外を投げる valuetype の操作と、ハンドルのメソッド（`new`、`get` など）と同じ名前の操作は生成器のエラー

```rust
impl AccountOperations for AccountState {
    fn doubled(&mut self) -> Sys<i32> {
        Ok(self.balance * 2)
    }
}
let a = Account::new(AccountState { balance: 21 });
assert_eq!(a.doubled()?, 42);
```

- 抽象 valuetype は生成器がエラーにする

## abstract interface

`abstract interface A` は、オブジェクト参照か、`A` を `supports` する valuetype の値のどちらかとして渡る（[#104](https://github.com/rustorb/rustorb/issues/104)、コアの扱いは [valuetype.md](valuetype.md)）。生成器は、どちらかを持つ enum `A` と、操作のトレイト `AServant` を作る。

```rust
// abstract interface Named { string name(); };
// interface Person : Named { long age(); };
// valuetype Card supports Named { public string title; };
#[derive(Debug, Clone)]
pub enum Named {
    Object(ObjectRef),                                  // オブジェクト参照
    Value(rustorb_core::idl::AbstractValue<dyn NamedServant>), // 値と、その上の操作
}

impl Named {
    pub fn narrow(obj: &ObjectRef) -> Sys<Option<Named>>;
    pub fn unchecked_narrow(obj: ObjectRef) -> Named;
    pub fn from_value(h: &ValueHandle) -> Option<Named>;   // 値の型が Named を supports するなら
    pub fn object(&self) -> Option<&ObjectRef>;
    pub fn value(&self) -> Option<&ValueHandle>;
    pub fn name(&self) -> Result<String, SystemException>; // 参照なら呼び出し、値ならこのプロセスで
    pub async fn name_async(&self) -> Result<String, SystemException>;
}

pub trait NamedServant: Send + Sync {
    fn name(&self) -> Result<String, SystemException>;
}

impl From<Person> for Named { /* Object */ }
impl From<Card> for Named { /* Value */ }
impl NamedServant for Card { /* CardOperations を呼ぶ */ }
```

- **型**：`A` の値は `Option<A>`（`None` は nil）。`in` は `Option<&A>`。PartialEq は持たない（値の型が持たないため）
- **操作**：enum のメソッドが、`Object` なら参照のスタブとして呼び、`Value` なら値の操作（`AServant` の実装）をこのプロセスで呼ぶ。シグネチャとエラー型は普通のインターフェースと同じ
- **具象インターフェース**：`interface I : A` のサーバントのトレイトは `IServant: AServant`。スケルトン `IPoa<T>` が `A` の操作も受ける。`From<I> for A` がある
- **valuetype**：`valuetype V supports A` には `impl AServant for V`（`VOperations` を呼ぶ）と `From<V> for A` を作る。`A` を継承する具象インターフェースを `supports` するときも同じ
- **継承**：`abstract interface A2 : A1` は `A2Servant: A1Servant`、`From<A2> for A1`。値の操作を基底のトレイトとして渡すために、コアの `idl::Upcast<T>` に各 `AServant` を実装する（Rust 1.85 にはトレイトの upcast がないため）
- **送るとき**：`Object` は TRUE とオブジェクト参照、`Value` は FALSE と値、`None` は TRUE と nil の参照（コアと同じ）。値は他の値と同じく共有が保たれる
- **受け取るとき**：値は `ValueBase` として読み（`ValueHandle::read_base`、型は `register_value` で登録されたもの）、その valuetype の出力ファイルの `register_value_types()` が登録した変換（`idl::register_supported`）で `A::Value` にする。登録されていない型や `A` を `supports` しない型の値は `MARSHAL`
- TypeCode は `tk_abstract_interface`。Any にも入る

相互接続は `interop/run-matrix.sh --suite abstract` に `rustorb` を入れて確かめる。

## local interface

`local interface L` は、トレイト `L` だけになる。スタブ、スケルトン、マーシャリングはない。アプリケーションが実装し、`Arc<dyn L>` として渡す。コアのサーバントマネージャー（`ServantActivator` など）と同じ形。

```rust
// local interface Hook { long fire(in long n, in Hook next); };
pub trait Hook: Send + Sync {
    fn fire(&self, n: i32, next: Option<&Arc<dyn Hook>>) -> Result<i32, SystemException>;
}
// hook::REPOSITORY_ID
```

- 型は `Option<Arc<dyn L>>`、`in` は `Option<&Arc<dyn L>>`
- 継承は親トレイト（`local interface L2 : L1` は `L2: L1`。基底が普通のインターフェースなら `BServant`）。`Arc<dyn L2>` から `Arc<dyn L1>` への変換は作らない（Rust 1.85 にトレイトの upcast がないため。具体的な型から作る）
- リポジトリIDは、名前のモジュールの定数 `l::REPOSITORY_ID`（トレイトに定数を置くと `dyn` にできないため）
- `valuetype V supports L` には `impl L for V` を作る
- 送ることになる使い方（普通のインターフェースの操作の引数、struct のメンバー、typedef、Any）は生成器のエラー

## 対象外（生成器がエラーにするもの）

- 抽象 valuetype、例外を投げる valuetype の操作（上記）
- 操作の `context` 句。コアに `CORBA::Context` がなく、要求の引数のあとに Context を送れない。黙って省くと、`context` 句を宣言した相手の ORB との間で要求の形が食い違うので、エラーにする
- local interface を送ること（上記）
- IDL4 の新しい構文（[#46](https://github.com/rustorb/rustorb/issues/46)）

生成したスタブの呼び出しのタイムアウトは、スタブの参照（`stub.object().set_call_timeout(...)`）、スレッド（`rustorb_core::set_thread_call_timeout`）、ORB（`Orb::set_call_timeout`）で設定する（[timeouts.md](timeouts.md)）。

## 生成器の構成

- `rust_backend.py`：Python・C++・Java の backend と同じく、`frontend.py` の `definitions` / `type_of` を使って出力を組み立てる。名前の変換と衝突の検査は `rust_backend.py` の中で、書き出しながら行う。union のラベルや valuetype の基底など、C++ の backend と同じ計算はその関数を使う
- 出力は `rustfmt` が見つかれば通す（`-Wbno_rustfmt` で止められる）。チェックインした生成コードは `rustfmt` を通したもので、`cargo fmt --check` にもそのまま通る
- 生成コードには `#![allow(clippy::all)]` を付けず、clippy の警告が出ない形で書く（`-D warnings` で検査する）。個別に要る `allow`（`clippy::type_complexity` など）だけを付ける
- 生成器のテスト：`crates/rustorb-codegen/python/tests/test_rust_backend.py`（コマンドライン、出力の形、生成器がエラーにするもの）と、`crates/rustorb-codegen/tests/rust_mapping.rs`（`tests/idl/RustMapping.idl` の生成コード `tests/generated/rust_mapping.rs` を使い、往復、送るときの検査、TypeCode と Any、valuetype、TCP 越しの呼び出しを `cargo test` で確かめる）

## 置き換えと検証

1. **コアのランタイム**：`rustorb_core::idl`、`Fixed`、valuetype のハンドル、`Any::insert` / `extract`。コアの単体テスト
2. **生成器**：`-brust` と生成器のテスト。`rustorb-codegen` の `build.rs` 用の補助（`rustorb_codegen::Config`）
3. **interop の置き換え**：`interop/impls/rustorb` の手書きのスタブとスケルトンを、`InteropTest.idl`・`AnyTest.idl`・`AnyTestHidden.idl`・`ValueTest.idl`・`PoaProbe.idl` の生成コード（`src/idl/`、`src/idl.rs` が `include!` する）にした。ケースは変えていない。`run-matrix.sh` の basic / `--suite any` / `--suite value` を GIOP 1.2 と 1.1 で、omniORBpy・omniORB C++・JacORB・rustorb との組で走らせ、すべて PASS か既知の失敗（`known-failures.txt`）だけで、置き換え前と同じ。`--naming rustorb`、`run-poa-probe.sh rustorb` も同じ
4. **CosNaming の置き換え**：`rustorb-naming` に生成した CosNaming（`src/idl/cos_naming.rs`、`rustorb_naming::idl::cos_naming` として公開）を入れ、クライアントのスタブは生成したスタブを呼び、`rustorb-names` のサーバントは生成したスケルトンのトレイトを実装する形にした。公開 API（`NamingContext`、`NamingError`、`to_name` など）はそのまま残し、`NamingError` と生成したエラー型を相互に変換する。`run-naming-check.sh`、`run-matrix.sh --naming rustorb`、`run-openrtm.sh --naming rustorb` が置き換え前と同じ結果
5. **再生成の確認**：`crates/rustorb-codegen/generate-rust.sh` で生成コードを作り直し、CI（Python のジョブ）で差分がないことを確かめる（Java の `java/generate-stubs.sh` と同じ）

PR は1つ（[#55](https://github.com/rustorb/rustorb/pull/55)）にまとめた。

## 参考

- 他の IDL to Rust マッピングとの比較：[rust-mapping-comparison.md](rust-mapping-comparison.md)
- [IDL4 to Rust Language Mapping RFP](https://www.omg.org/gettingstarted/rfp-IDL4-RLP.htm)（OMG、2026年6月）
- [IDL 4.2](https://www.omg.org/spec/IDL/4.2)、[CORBA 3.4 Part 2](https://www.omg.org/spec/CORBA/3.4)（CDR、valuetype の符号化）
- 既存の生成器の設計：[python-codegen.md](python-codegen.md)、[cpp-binding.md](cpp-binding.md)、[java-binding.md](java-binding.md)、[valuetype.md](valuetype.md)
