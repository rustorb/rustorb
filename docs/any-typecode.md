# Any と TypeCode

Oct 28, 2026 · @ysuga · M5

rustorb の `any` と `TypeCode` の実装をまとめる。M5 の範囲は Rust コア、Python バインディング、C++ バインディング（C ABI、`cpp/include`、`rustorb-idl -bc++11`）。完了条件は「Any / TypeCode を使う操作が omniORB C++・omniORBpy と相互接続できること」で、`interop/run-matrix.sh --suite any` で判定する。

## 構成

| 場所 | 内容 |
| --- | --- |
| `crates/rustorb-core/src/typecode/mod.rs` | `TCKind`、`TypeCode`（構築、問い合わせ、`equal` / `equivalent` / `get_compact_typecode`） |
| `crates/rustorb-core/src/typecode/cdr.rs` | TypeCode の CDR（カプセル化、間接参照） |
| `crates/rustorb-core/src/value.rs` | `Value`（TypeCode に従う値の表現）と `Any`、その読み書き |
| `crates/rustorb-core/src/stream.rs` | `OutStream::write_typecode` / `write_value` / `write_any`、`InStream` の対応する読み込み |
| `crates/rustorb-python/src/any.rs` | `CORBA.Any` / `CORBA.TypeCode` とコアの型の変換 |
| `python/rustorb/CORBA.py`、`_rt.py` | `ORB.create_*_tc`、実行時に受け取った型のクラス生成 |
| `crates/rustorb-ffi/src/any.rs` | C ABI の `rorb_typecode` / `rorb_any` |
| `cpp/include/rustorb/corba.h`、`cdr.h` | `CORBA::TypeCode`、`CORBA::Any`、`<<=` / `>>=`、`ORB::create_*_tc` |
| `crates/rustorb-codegen/python/rustorb_idl/cxx_backend.py` | 型ごとの `_tc_<名前>` と `rustorb::cdr::type_code` |
| `interop/idl/AnyTest.idl`、`AnyTestHidden.idl` | 相互接続の IDL |

## TypeCode の表現

TypeCode は不変の値で、`Arc` で共有するノードの配列（アリーナ）と、その中の位置で表す。子の TypeCode は同じアリーナの別のノードを指し、再帰型は外側の struct / union / valuetype のノードを指す後ろ向きの添字になる。`Arc` の循環がないのでリークしない。

- 構築：`TypeCode::struct_tc` などの構築関数は、メンバーの TypeCode を新しいアリーナにコピーする。`TypeCode::recursive(id)` は仮置きのノードで、同じリポジトリ ID の struct・union・valuetype を構築したときにそのノードへの参照に置き換わる。置き換わらずに残った仮置きを書こうとすると `BAD_TYPECODE`
- 構築時の検査は CORBA 3.4 Part 1 の 8.11 に従う：名前が IDL の識別子として正しいか、メンバー名の重複、union のラベルの型と重複、配列の長さ0などは `BadParam`
- `equal` は名前・メンバー名まで含めた比較、`equivalent` は別名を外し、リポジトリ ID が両方にあれば ID だけで比べる。`compact` は名前・メンバー名・列挙子名を空にする
- `TypeCode` の `PartialEq` は `equal`

## CDR

- 単純な種類（`tk_null`〜`tk_ulonglong` など）は種類だけ、`tk_string` / `tk_wstring` は上限、`tk_fixed` は桁数と小数桁、それ以外はカプセル化（長さ、バイト順の octet、パラメータ）
- 書き出しで間接参照（`0xffffffff` と、オフセットの位置からの相対オフセット）を使うのは再帰のときだけ。同じ型が2回出てきても展開して書く（受け取り側の負担を減らすため。仕様上どちらでもよい）
- 読み込みは間接参照をすべて受け付ける。カプセル化の中から外側の TypeCode を指す参照も、すでに読んだ TypeCode を指す参照も同じノードになる。入れ子の深さは128まで、値の入れ子は256まで（超えると `MARSHAL`）
- union の default のラベルは octet の 0
- Any の中の例外は、応答と同じくリポジトリ ID の文字列から書く（omniORB の実測でも同じ）

## Value と Any

`Value` は TypeCode に従う値の表現で、Python バインディングもこれを使う。struct・例外・配列・sequence は `List`、octet の sequence は `Octets`、char の sequence は `Chars`、union は判別子と選ばれたメンバー（番号と値）、enum は番号、`fixed` は数字列と符号、オブジェクト参照は `ObjectRef`、入れ子の `any` / `TypeCode` はそのまま入る。

`Any` は TypeCode と値の組で、`Any::new(tc, value)` で作る。値が TypeCode に合わなければ書き出しのときに `BAD_PARAM`（COMPLETED_NO）。valuetype・`long double`・`native` の値は `NO_IMPLEMENT`。

```rust
let point = TypeCode::struct_tc(
    "IDL:AnyTest/Point:1.0", "Point",
    &[("x", TypeCode::simple(TCKind::Long)?), ("y", TypeCode::simple(TCKind::Long)?)],
)?;
let any = Any::new(point, Value::List(vec![Value::Long(1), Value::Long(2)]));
out.write_any(&any)?;
```

## Python

標準マッピングのとおり、`CORBA.Any(tc, value)` と `CORBA.TypeCode`（生成コードの `_tc_Foo`、`CORBA.TC_long` など）を使う。

- 送信：Python の TypeCode をコアの TypeCode に変換し、`_NP_native` 属性に覚えておく（2回目からは変換しない）。値は M3 と同じ変換で `Value` にする
- 受信：コアの TypeCode のノードを `_rt.typecode_from_nodes` に渡して Python の TypeCode にする。生成コードに同じリポジトリ ID で `equal` な型があれば生成コードの TypeCode とクラスを使い、なければその場で TypeCode とクラスを作る（`_rt.dynamic_types` に登録し、同じ型が届いたら使い回す）。生成コードに型がない struct もメンバー名の属性を持つ値になる
- `ORB.create_struct_tc` など `create_*_tc` 一式、`CORBA.StructMember`、`CORBA.UnionMember` を追加した。`create_recursive_tc` は外側の型を作るときに解決する
- TypeCode の問い合わせ（`kind`、`id`、`member_count` など）と `equal` / `equivalent` / `get_compact_typecode` は M1 の純 Python 実装のまま

## C++

IDL to C++11 のとおり、`CORBA::Any` に `<<=` で入れて `>>=` で取り出し、TypeCode は `IDL::traits<CORBA::TypeCode>::ref_type` で扱う。

```cpp
CORBA::Any a;
a <<= AnyTest::Point(1, 2.5);
AnyTest::Point p;
if (a >>= p) { /* ... */ }
a <<= int32_t{7};
a.type(AnyTest::_tc_Count);   // 同じ型の別名に付け替える（違えば BAD_TYPECODE）
```

- **C ABI**：`rorb_typecode` はコアの `TypeCode`、`rorb_any` はコアの `Any`（TypeCode と `Value`）を指すハンドル。C++ の値の出し入れはコールバックで、生成コードの `codec<T>` が引数と同じように単独の CDR ストリーム（ネイティブのコードセット）に書いたものをコアが `Value` に読み、取り出しはその逆。例外の値はリポジトリ ID を除いて渡す。読み戻すオブジェクト参照は、値の中の最初の参照の ORB（なければ最初に初期化した ORB）に属する
- **`<<=` / `>>=`**：`rustorb::cdr::type_code<T>` がある型に使える。基本型、`std::string`・`std::wstring`、上限付きの型、`std::vector`・`std::array`（TypeCode は要素の型から作る）、`CORBA::Any`、TypeCode、オブジェクト参照、生成コードの struct・union・enum・例外・インターフェース。`>>=` は any の TypeCode が C++ の型の TypeCode と `equivalent` のときだけ値を設定して true を返す
- **typedef**：C++ では typedef は同じ型なので、`<<=` で入るのは元の型の TypeCode になる。別名の TypeCode にするには `Any::type()` で付け替える（`equivalent` なら受け付ける）
- **生成コード**：型ごとに `_tc_<名前>`（`extern const`、インターフェースの中の型なら静的メンバー）と、それを最初に使うときに作る `_rorb_tc_<名前>()`。静的オブジェクトの初期化順に依存しない。再帰型は、作りかけの struct・union に戻る型をその場で組み立て、戻る参照を `create_recursive_tc` と同じ仮置きにする
- `CORBA::_tc_long` などの基本型の TypeCode、`ORB::create_*_tc`（`StructMember` の `type_def` は省いた）、`TypeCode::BadKind` / `Bounds`
- `CORBA::Any` には IDL to C++11 にない `==` を足した（TypeCode が `equal` で値が同じ）。any のメンバーを持つ struct の生成コードの `==` が使う

## 相互接続

`interop/idl/AnyTest.idl` の `AnyTest::AnyEcho` を、すべての実装がクライアントとサーバーの両方で持つ。名前つきの27種の値（基本型、上限つき文字列、別名、struct、enum、union の各分岐、sequence、配列、再帰型、`NVList` 形の struct の sequence、入れ子の any、TypeCode、オブジェクト参照、nil、例外、空の any）について次を確かめる（178ケース）。

- `make(name)`：サーバーが作った any をクライアントの期待値と比べる
- `check(name, a)`：クライアントが作った any をサーバーが比べる
- `echo(a)`：送った any がそのまま返る
- `make_tc` / `check_tc` / `echo_tc`：TypeCode だけで同じことをする
- `out_inout`、`echo_nvlist`
- `AnyTestHidden.idl` の型：クライアントだけが知る型の any をサーバーへ送り、`describe` が返す TypeCode の構造と値を比べる（サーバーが生成コードを持たない型を受け取れるか）
- 型が合わない any を送って `BAD_PARAM` になること、別名を外した比較

```sh
interop/run-matrix.sh --suite any               # 5×5 の組み合わせ、GIOP 1.2
interop/run-matrix.sh --suite any --giop 1.1
```

ローカルで omniORB 4.3.4 / omniORBpy をビルドして、rustorb（Rust・Python・C++）・omniORBpy・omniORB C++ の25組すべてで、GIOP 1.2 と 1.1 の両方で178ケースが通ることを確かめた。`cpp/tests/test_any.cc` は同じケースを1プロセスの2つの ORB の間で回し、TypeCode の操作と `create_*_tc`、`<<=` / `>>=` の規則も確かめる。

omniORB C++ の相互接続用プログラムは Any に omniidl の `-Wba`（と `-Wbd=DynSK.cc`）と `-lomniDynamic4` が要る。

### 実測で分かったこと

- omniORBpy の `TypeCode.member_count()` は float を返し、`str(tc.kind())` は `"CORBA.tk_struct"` の形（テスト側で吸収している）
- omniidl は同じスコープで大文字小文字だけ違う名前を衝突とみなす（`AnyTestHidden.idl` の `PointSeq`）

## 残り

- DynAny、valuetype の値、`long double`、`fixed`（C++）
- omniORBpy 互換シム（`omniORB.any` など）と OpenRTM-aist-Python の動作確認
