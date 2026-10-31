# DynAny

Oct 29, 2026 · @ysuga · issue #40

生成コードを持たない型の値を、TypeCode だけを頼りに組み立てたり分解したりする `DynamicAny` モジュールの実装をまとめる。コア（Rust）に一つの実装を持ち、Python（PyO3）、C ABI、その上の C++ と Java（JNI）のバインディングが同じ DynAny を使う。

挙動の基準は CORBA 3.0 の `DynamicAny` の IDL（OMG の `DynamicAny.idl`）と仕様の記述で、仕様に幅があるところは omniORB 4.3.4 の C++ 版を外から測って比べた（`interop/impls/omniorb-cpp/dynany_probe.cc`。clean-room 方針どおりソースもヘッダーの実装も読んでいない）。omniORBpy には DynAny がない（`resolve_initial_references("DynAnyFactory")` が `InvalidName`）。

## 構成

| 場所 | 内容 |
| --- | --- |
| `crates/rustorb-core/src/dynany/mod.rs` | `DynAnyFactory`、`DynAny`、`DynAnyError` |
| `crates/rustorb-core/src/dynany/tests.rs` | コアの単体テスト（再帰型、valuetype の共有と循環、切り詰め、共用体） |
| `crates/rustorb-python/src/dynany.rs` | `rustorb._native.DynAny`：コアの DynAny と Python の値の変換 |
| `python/rustorb/DynamicAny.py` | Python マッピングのクラス（`DynAny`、`DynStruct` ほか）、例外、`NameValuePair` |
| `crates/rustorb-ffi/src/dynany.rs`、`include/rustorb/rustorb.h` | C ABI の `rorb_dynany_*` |
| `cpp/include/rustorb/dynany.h` | IDL to C++11 の `DynamicAny`（`rustorb/cdr.h` が読み込む） |
| `crates/rustorb-jni/src/dynany.rs`、`java/src/main/java/org/omg/DynamicAny/`、`org/rustorb/DynAnyImpl.java`・`DynAnyFactoryImpl.java` | Java の `org.omg.DynamicAny` と、C ABI の上の実装 |
| `cpp/tests/test_dynany.cc`、`java/src/test/java/org/rustorb/test/DynAnyTest.java`、`python/tests/test_dynany.py` | 各バインディングのテスト |
| `interop/impls/*/dynany_probe*`、`dynany-probe`、`DynAnyProbe.java` | omniORB C++・rustorb（Rust・Python・C++・Java）で同じケースを回すプローブ |
| `interop/run-dynany-probe.sh`、`interop/expected/dynany-probe-*.txt` | プローブの実行と期待出力 |

## API

| | Rust（コア） | Python | C++ | Java |
| --- | --- | --- | --- | --- |
| ファクトリ | `DynAnyFactory`（状態を持たない） | `orb.resolve_initial_references("DynAnyFactory")` | 同左を `IDL::traits<DynamicAny::DynAnyFactory>::narrow` | 同左を `DynAnyFactoryHelper.narrow` |
| Any から | `create_dyn_any(&Any)`、`create_dyn_any_without_truncation`、`create_multiple_dyn_anys` | 同名 | 同名 | 同名 |
| TypeCode から | `create_dyn_any_from_type_code(&TypeCode)` | 同名 | 同名 | 同名 |
| 派生インターフェース | すべて `DynAny` のメソッド。`DynAny::kind()`（別名を外した種類）でどれが使えるか分かり、違う種類の操作は `BAD_OPERATION` | 種類に合ったクラス（`DynFixed`、`DynEnum`、`DynStruct`（struct と例外）、`DynUnion`、`DynSequence`、`DynArray`、`DynValue`、`DynValueBox`、それ以外は `DynAny`）が返る | 同じく種類に合ったクラス（`IDL::traits<DynamicAny::DynStruct>::narrow` などで得る） | 同じく種類に合ったインターフェースを実装したオブジェクト（`DynStructHelper.narrow` など） |
| 例外 | `DynAnyError::{InvalidValue, TypeMismatch, InconsistentTypeCode, MustTruncate, System}` | `DynamicAny.DynAny.InvalidValue` / `TypeMismatch`、`DynAnyFactory.InconsistentTypeCode`、`DynamicAny.MustTruncate`、システム例外 | `DynamicAny::DynAny::InvalidValue` / `TypeMismatch`、`DynamicAny::DynAnyFactory::InconsistentTypeCode`、`DynamicAny::MustTruncate` | `org.omg.DynamicAny.DynAnyPackage.InvalidValue` / `TypeMismatch`、`DynAnyFactoryPackage.InconsistentTypeCode`、`MustTruncate` |

Rust の名前は IDL と同じだが、IDL の `type()` は `type_code()`、`DynFixed` の `get_value` / `set_value` は `get_fixed_value` / `set_fixed_value`、`DynEnum` の操作は `get_enum_as_string` などとした（`DynAny` に全部載せるため）。`insert_<型>` / `get_<型>` は `insert_basic(kind, Value)` / `get_basic(kind)`、`insert_<型>_seq` / `get_<型>_seq` は `insert_seq` / `get_seq` が本体で、型ごとの名前はその薄い包み。

```rust
let d = DynAnyFactory.create_dyn_any_from_type_code(&point_tc)?;
d.insert_long(1)?;
d.next()?;
d.insert_double(2.5)?;
let any = d.to_any()?;
```

```python
factory = orb.resolve_initial_references("DynAnyFactory")
d = factory.create_dyn_any_from_type_code(AnyTest._tc_Point)   # DynStruct
d.insert_long(1); d.next(); d.insert_double(2.5)
p = d.to_any().value()                                          # AnyTest.Point
```

```cpp
auto factory = IDL::traits<DynamicAny::DynAnyFactory>::narrow(
    orb->resolve_initial_references("DynAnyFactory"));
auto d = factory->create_dyn_any_from_type_code(AnyTest::_tc_Point);
d->insert_long(1);
d->next();
d->insert_double(2.5);
CORBA::Any a = d->to_any();
```

```java
DynAnyFactory factory =
    DynAnyFactoryHelper.narrow(orb.resolve_initial_references("DynAnyFactory"));
DynStruct d = DynStructHelper.narrow(factory.create_dyn_any_from_type_code(PointHelper.type()));
d.insert_long(1); d.next(); d.insert_double(2.5);
Point p = PointHelper.extract(d.to_any());
```

C++ と Java の型の対応は各マッピングのとおり（`insert_ulong` は C++ が `uint32_t`、Java が `int`、`*_seq` は C++ が `std::vector`、Java が配列）。`insert_longdouble` / `get_longdouble`（と `_seq`）は C++ が `long double` で持ち、Java は持たない（Java のマッピングに `long double` がない。`from_any` / `to_any` で扱える）。`insert_abstract` / `get_abstract` は `NO_IMPLEMENT`。

### C ABI

`rorb_dynany` は一つの DynAny への参照のハンドルで、`rorb_dynany_clone` / `_release` で数える（同じ DynAny かは `rorb_dynany_same`）。派生インターフェースの操作もすべて同じハンドルを取り、種類が違えば `BAD_OPERATION`。値は any で渡す。

- `rorb_dynany_insert(d, kind, any)` は any の値を kind の値として入れる（any の TypeCode は見ない。文字列は上限なしの `string` の any でよい）。`rorb_dynany_get(d, kind)` は kind の型の any（`string` は上限なし、valuetype はインスタンスの型）を返す
- `insert_<型>_seq` / `get_<型>_seq` は要素型の上限なしの sequence の any で渡す
- `insert_val` は、C++ と Java では現在の構成要素（なければ自身）の型（`rorb_dynany_target_type`）で値を書いた any を渡す
- `get_members` / `get_elements` は `member_count` と `member_at`、`element_count` と `element_at` で一つずつ取る
- 失敗は `RORB_LOCAL_EXCEPTION` と `RORB_DYN_INVALID_VALUE`（17）・`_TYPE_MISMATCH`（18）・`_INCONSISTENT_TYPECODE`（19）・`_MUST_TRUNCATE`（20）、またはシステム例外

C++ のクラスは C ABI のハンドルを一つ持つ `CORBA::LocalObject` で、`to_any` などの値は `CORBA::Any` と `<<=` / `>>=` で変換する。Java は `org.rustorb.Native` の `dyn*`（JNI）で同じ C ABI を呼び、値は `AnyImpl`（コアの any のハンドル）で渡す。Java では一つの DynAny に複数の Java オブジェクトがありうるので、同じ DynAny かは `_is_equivalent` で確かめる。

## 表現

DynAny は TypeCode（別名を含めて渡されたまま）と状態の組で、`Arc` で共有する。クローンは同じオブジェクト（参照と同じ）。

- 構成要素のない型（基本型、文字列、`fixed`、enum、オブジェクト参照、`any`、`TypeCode`）は `Value` をそのまま持つ
- struct・例外・sequence・配列はメンバーや要素の DynAny の列、共用体は判別子の DynAny と選ばれたメンバー（番号と DynAny）、valuetype と value box は `null` か状態メンバーの DynAny の列。どれも現在位置を持つ
- 構成要素は親と状態を共有する。`current_component` や `get_members_as_dyn_any`、`get_discriminator` で得た DynAny を変えると親の値が変わる
- 一つの木の DynAny は寿命を共有し、最上位の `destroy()` で木全体が `OBJECT_NOT_EXIST` になる。構成要素の `destroy()` は何もしない
- 錠は DynAny ごとで、子の値を読むときは子の一覧を写してから錠を外す。二つの錠を同時に持たないので、`assign` に自分の構成要素を渡してもデッドロックしない
- valuetype のインスタンスは、値の中で何か所から指されていても一つの DynAny になり（循環も同じ）、`to_any` で共有されたインスタンスに戻る。循環している DynAny は `destroy()` するまで解放されない（Python では `destroy()` を呼ぶ）

## 決めたこと

仕様の記述と omniORB の実測から決めた。プローブの期待出力は `interop/expected/dynany-probe-omniorb-cpp.txt` と `dynany-probe-rustorb.txt` で、違いは下の表の6行だけ。

- `insert_*` / `get_*` は、構成要素のない DynAny ではそれ自身に、ある DynAny では現在の構成要素に働く。構成要素があるのに位置が -1 なら `InvalidValue`、種類が違えば `TypeMismatch`。値を入れても位置は進まない（omniORB と同じ）
- `insert_<型>_seq` / `get_<型>_seq` は、DynAny 自身がその要素型の sequence か配列ならそれ自身に、そうでなければ現在の構成要素に働く。入れたあとの位置は先頭（`set_elements` と同じ）
- `next()` は -1 から 0 に進む。範囲外の `seek` と末尾の `next` は位置を -1 にして false
- `current_component` は、構成要素を持てない型・メンバーのない例外・`null` の valuetype と value box で `TypeMismatch`、空の sequence や位置 -1 では nil
- `current_member_name` / `current_member_kind` は、メンバーがない（`null` を含む）と `TypeMismatch`、位置 -1 で `InvalidValue`。`*_kind` は別名を外さない種類を返す（`tk_alias`）
- TypeCode から作る値：0、false、空の文字列と sequence、最初の列挙子、nil、空の any（`tk_null`）、`tk_null` の TypeCode、`null` の valuetype と value box。共用体は TypeCode で最初のラベルの値を判別子にし（ラベルが default だけなら使われていない値）、それが選ぶメンバーを既定値にする（omniORB と同じ）。`Principal`・`native`・抽象インターフェース・ローカルインターフェース・解決されていない再帰の TypeCode は `InconsistentTypeCode`
- 共用体の判別子を構成要素の DynAny で変えると、次の操作でメンバーがそれに合わせて変わる。複数のラベルを持つ一つのメンバー（TypeCode では同じ名前で並ぶ）の中で判別子が動いたときは値を保つ（omniORB と同じ）
- `set_to_default_member` / `set_to_no_active_member` は、どのラベルも使っていない値のうち 0 から数えて最初のものを判別子にする
- `get_dyn_any` は中の any の新しい DynAny（共有しない、omniORB と同じ）、`insert_dyn_any` は値を写す
- `DynFixed::get_value` は小数部の末尾の 0 を落とした文字列（`0`、`-12.5`）。`set_value` は前後の空白と `d` / `D` の接尾辞を受け付け、整数部が入らなければ `InvalidValue`、数値でなければ `TypeMismatch`、小数部を切り捨てたら false
- valuetype の切り詰め：Any の型より派生した型のインスタンスは、`create_dyn_any` では宣言された型の状態メンバー（基底の型のメンバーが先にある）だけにし、`create_dyn_any_without_truncation` と `allow_truncate` が false の `create_multiple_dyn_anys` では `MustTruncate`。`to_any` は宣言された型のインスタンスを作る
- `insert_reference` はインターフェースの型を確かめない（確かめるには `_is_a` で相手に問い合わせることになる）
- 抽象インターフェース（`insert_abstract` / `get_abstract`）は `NO_IMPLEMENT`

### omniORB との違い

| ケース | omniORB 4.3.4 | rustorb | 理由 |
| --- | --- | --- | --- |
| 最上位を `destroy()` したあとの構成要素 | 使える | `OBJECT_NOT_EXIST` | 仕様は構成要素も破棄されるとしている |
| `set_to_default_member` の判別子（ラベル 1〜3 の long） | `-2147483647` | `0` | 使われていない値ならどれでもよい |
| `set_to_no_active_member` の判別子（ラベル 2・5 の short） | `-32767` | `0` | 同上 |
| `insert_long_seq` のあとの位置 | -1 のまま | 0 | `set_elements` と揃えた |
| long の sequence に `get_long_seq` | `TypeMismatch` | 要素を返す | `insert_long_seq` は自身に働くのに `get_long_seq` が働かないのは不整合なので、両方とも自身に働かせた |

## 相互接続

`--suite any` のクライアントに DynAny のケースを足した（rustorb の Rust・Python・C++・Java のクライアント。omniORBpy は DynAny がないので省く）。各値を `create_dyn_any` で分解し、`create_dyn_any_from_type_code` で作った DynAny に構成要素ごとに入れ直して（`dyn_rebuild`）、次を確かめる。

- `dyn_check[名前]`：組み立て直した any をサーバーの `check` に送る
- `dyn_make[名前]`：サーバーの `make` が返した any を組み立て直して期待値と比べる
- `dyn_echo[名前]`：`AnyTestHidden.idl` の型（サーバーが生成コードを持たない）を組み立て直して `echo` で往復させる

rustorb・rustorbpy・rustorbcpp・rustorbjava のクライアントと omniORBpy・omniORB C++・rustorb（Rust・Python・C++・Java）のサーバーの組み合わせで確かめた。プローブは C++（`interop/impls/rustorbcpp/dynany_probe.cc`）と Java（`interop/impls/rustorbjava/probe/interop/DynAnyProbe.java`）にも移し、どちらも Rust のプローブと同じ期待出力（`dynany-probe-rustorb.txt`）と一致する。

```sh
interop/run-matrix.sh --suite any
interop/run-dynany-probe.sh               # omniorb-cpp、rustorb、rustorbpy、rustorbcpp、rustorbjava
```

## 残り

- 抽象インターフェース（DynAny の中身にできない）
