# C++ バインディング（C ABI + IDL to C++11）

Oct 28, 2026 · @ysuga · M4

rustorb の C++ バインディングの構成をまとめる。計画どおり、Rust コアは `extern "C"` の安定 ABI 層（`crates/rustorb-ffi`）だけを公開し、C++ はその上のヘッダオンリーのランタイム（`cpp/include/rustorb`）と、`rustorb-idl -bc++11` が IDL から生成するスタブ・スケルトンで使う。マッピングは [IDL to C++11](https://www.omg.org/spec/CPP11/Current)（v1.7）に従い、実装と生成コードは C++17 を使う。M4 の完了条件は「生成コードで書いた C++ のクライアント・サーバーが omniORB C++・omniORBpy と相互接続できること」で、`interop/run-matrix.sh` の `rustorbcpp` で判定する。

## 構成

| 場所 | 内容 | ライセンス |
| --- | --- | --- |
| `crates/rustorb-ffi` | C ABI（`librustorb_ffi.a` / `.so`）。ORB、オブジェクト参照、CDR ストリーム、サーバント、POA・POAManager・Current | MIT OR Apache-2.0 |
| `crates/rustorb-ffi/include/rustorb/rustorb.h` | C ABI のヘッダ（`rorb_` 接頭辞） | MIT OR Apache-2.0 |
| `cpp/include/rustorb/corba.h` | CORBA モジュールのマッピング（`IDL::traits`、参照型、例外、`ORB`、`PortableServer`） | MIT OR Apache-2.0 |
| `cpp/include/rustorb/cdr.h` | 生成コードが使うマーシャリング（`rustorb::cdr::codec<T>`）、呼び出し、ディスパッチ | MIT OR Apache-2.0 |
| `crates/rustorb-codegen/python/rustorb_idl/cxx_backend.py` | `rustorb-idl -bc++11`（`-bcxx11` も可） | GPL-3.0-or-later（生成物は対象外） |
| `cpp/CMakeLists.txt`、`cpp/tests` | C++ のテスト（CMake、ctest） | MIT OR Apache-2.0 |
| `interop/impls/rustorbcpp` | 相互接続用のクライアント・サーバー | MIT OR Apache-2.0 |

## 使い方

```sh
rustorb-idl -bc++11 -C gen Example.idl     # gen/Example.hh、gen/Example.cc
cargo build --release -p rustorb-ffi       # target/release/librustorb_ffi.a
c++ -std=c++17 -Icpp/include -Icrates/rustorb-ffi/include -Igen \
    main.cc gen/Example.cc target/release/librustorb_ffi.a -lpthread -ldl -lm
```

ヘッダとソースの拡張子は `-Wbh=.h`、`-Wbs=.cpp` で変えられる（omniidl と同じ書き方）。CMake では `cpp/CMakeLists.txt` の `rustorb` インターフェースライブラリと `rustorb_idl_generate()` がそのまま使える。

```cpp
#include "Example.hh"

class EchoImpl : public CORBA::servant_traits<Example::Echo>::base_type {
 public:
  std::string echo(const std::string& s) override { return s; }
};

int main(int argc, char* argv[]) {
  auto orb = CORBA::ORB_init(argc, argv);    // -ORB... は argv から取り除かれる
  auto poa = IDL::traits<PortableServer::POA>::narrow(
      orb->resolve_initial_references("RootPOA"));
  poa->the_POAManager()->activate();
  auto servant = CORBA::make_reference<EchoImpl>();
  std::cout << orb->object_to_string(servant->_this()) << std::endl;
  orb->run();
  orb->destroy();
}
```

サーバントのクラスの中では、インターフェース名（とその基底の名前）がスケルトンのクラス名として見つかる（C++ の注入クラス名）。戻り値などで型そのものを書くときは `Example::Echo` のように名前空間を付ける。

## C ABI の設計

- **ハンドル**：`rorb_orb`、`rorb_object`、`rorb_poa`、`rorb_poa_manager`、`rorb_servant` は不透明な型で、中身は Rust 側の値（`Orb`、`ObjectRef`、`Arc` など）を `Box` にしたもの。`*_clone` で増やし、`*_release` で解放する。NULL の解放は何もしない
- **エラー**：失敗しうる関数は `rorb_status`（`RORB_OK`、`SYSTEM_EXCEPTION`、`USER_EXCEPTION`、`LOCAL_EXCEPTION`、`CALLBACK_FAILED`）を返し、詳細を呼び出し側の `rorb_exception`（リポジトリID、minor、completed、ローカル例外のコード）に書く。リポジトリIDは `rorb_exception_clear` で解放する。Rust の panic は境界で捕まえて `INTERNAL` にする
- **ストリーム**：`rorb_out` / `rorb_in` はコールバックの中でだけ有効で、エラーは貼り付く（最初の失敗の後は何も書かず、読むと 0 を返す）。生成コードは1値ごとに検査せず、最後にまとめて確かめる。基本型の配列・列は `rorb_*_array` で一度にコピーする
- **呼び出し**：`rorb_object_invoke` に `rorb_invocation`（操作名、oneway、タイムアウト、引数を書くコールバック、返事を読むコールバック、宣言されたユーザー例外を読むコールバック）を渡す。宣言にないユーザー例外は `UNKNOWN`（minor `0x4f4d0001`、COMPLETED_YES）になる
- **サーバント**：呼び出し側のオブジェクト（`self`）と関数表（`dispatch`、`is_a`、`add_ref`、`remove_ref`）から `rorb_servant_get` で作る。同じ `self` には同じコア側のサーバントを返す（POA はサーバントを `Arc` のアドレスで見分けるため）。コア側のサーバントは `self` の参照を1つ持ち、`self` からは登録表を通した弱参照だけなので循環しない。サーバントの関数はアップコール用スレッドプールの任意のスレッドから呼ばれる
- **文字列**：`string` は UTF-8、`wstring` は UTF-16 の単位で渡す。C++ 側で `std::wstring` と相互に変換する（`wchar_t` が4バイトの環境ではサロゲートペアを組み立てる）

## 値の表現

| IDL | C++ |
| --- | --- |
| 整数型、`octet` | `int16_t` 〜 `uint64_t`、`uint8_t` |
| `float`、`double`、`boolean`、`char` | `float`、`double`、`bool`、`char` |
| `wchar` | `wchar_t`（UTF-16 の1単位を超えると送信時に `DATA_CONVERSION`） |
| `string`、`wstring` | `std::string`（UTF-8）、`std::wstring` |
| `string<N>`、`wstring<N>`、`sequence<T, N>` | `IDL::bounded_string<N>` など（標準型の派生）。上限超えは送信時 `BAD_PARAM`、受信時 `MARSHAL` |
| `sequence<T>`、配列 | `std::vector<T>`、`std::array<T, N>`（多次元は入れ子） |
| `fixed<D, S>` | `IDL::Fixed<D, S>`（下記） |
| struct・例外 | 生成クラス。メンバーごとに取得・設定の関数、全メンバーのコンストラクタ、`==` |
| union | 生成クラス。`_d()`、メンバーの関数（設定すると判別子も変わる）、`_default()`。選ばれていないメンバーを読むと `BAD_PARAM` |
| enum | `enum class : uint32_t` |
| `any` | `CORBA::Any`（`<<=` / `>>=`、[any-typecode.md](any-typecode.md)） |
| `TypeCode` | `IDL::traits<CORBA::TypeCode>::ref_type`。型ごとに `_tc_<名前>` |
| インターフェース | `IDL::traits<I>::ref_type`（`CORBA::object_reference<I>`、`shared_ptr` 相当）。nil は `nullptr` |
| valuetype・value box | `IDL::traits<V>::ref_type`、具象クラスは `OBV_<モジュール>::V`（[valuetype.md](valuetype.md)） |
| サーバント | `CORBA::servant_reference<T>`（侵入型の参照カウント）、`CORBA::make_reference<T>(...)` で作る |

C++ の予約語と同じ識別子には `_cxx_` が付く（操作名の GIOP 上の綴りは IDL のまま）。`long double` は C++ の `long double`（[long-double.md](long-double.md)）。abstract interface は [valuetype.md](valuetype.md#abstract-interface) のとおり。`custom` の valuetype、`supports`、valuetype の操作と初期化子は [valuetype.md](valuetype.md) のとおり。

`IDL::Fixed<D, S>` は D 桁（うち小数 S 桁）の10進数で、桁を配列で持つ。型に収まらない値（整数部の桁が多い、0 でない小数の桁が多い）は作るときに `DATA_CONVERSION` になるので、値はいつも TypeCode に収まる。文字列（`"-12.50"`、`d` の接尾辞も可）・整数から作り、`double` からは小数 S 桁に丸めて作る。`to_string()`（小数はいつも S 桁）、`explicit operator double`、比較演算子、単項の `-`、`<<` を持つ。別の桁数の `Fixed` からは `explicit` に変換できる（収まらなければ `DATA_CONVERSION`）。`fixed` の定数はリテラルの桁数と小数の桁数の型（`const fixed C = 12.50d;` は `IDL::Fixed<4, 2>`）で、型付きの定数が型に収まらなければ rustorb-idl がエラーにする。Any には `<<=` / `>>=` で入り、TypeCode は `tk_fixed`。C ABI では `rorb_out_write_fixed` / `rorb_in_read_fixed` が桁の配列と符号を受け渡す（パック10進は Rust のコアが書く）。

四則演算の結果の型は IDL の仕様の fixed の規則に従う。`fixed<d1, s1>` と `fixed<d2, s2>` の和と差は `fixed<max(d1-s1, d2-s2)+max(s1, s2)+1, max(s1, s2)>`、積は `fixed<d1+d2, s1+s2>`、商は `fixed<(d1-s1+s2)+s∞, s∞>`（31桁に収まるだけの小数）。31桁を超える型は整数部の桁を残して小数の桁を切り捨てた型になり（`Fixed<31, 31>` どうしの和は `Fixed<31, 30>`）、値の小数もその桁で切り捨てる。整数部が収まらない結果と 0 での除算は `DATA_CONVERSION`。整数のオペランドは `Fixed<20, 0>` として扱う（`Money + 1` は `Fixed<23, 2>`）。複合代入（`+=` など）と `++` / `--` は左辺の型のままで、小数の桁を切り捨て、整数部が収まらなければ `DATA_CONVERSION`（左辺は変わらない）。`round(s)`（0.5 は 0 から遠い方へ）と `truncate(s)` は小数を s 桁にした値を同じ型で返し、`round` で整数部があふれれば `DATA_CONVERSION`。

### IDL 4 の構文

IDL 4 で加わった構文のうち、GIOP で送れるものを取り込んだ（[#46](https://github.com/rustorb/rustorb/issues/46)）。どれも GIOP に専用の形がないので、送るときは決まった整数の形にする。rustorb 以外の ORB とは、その整数として通じる。

| IDL 4 | C++ | 送る形と TypeCode |
| --- | --- | --- |
| `int8` | `int8_t` | `octet`（`tk_octet`） |
| `uint8`、`int16` 〜 `uint64` | `uint8_t`、`int16_t` 〜 `uint64_t` | `octet`、`short` 〜 `unsigned long long`（今までと同じ） |
| `bitmask`（`@bit_bound`、`@position`） | `enum class : uintN_t`（N は `@bit_bound` が収まる 8 / 16 / 32 / 64、既定は 32）。値は `1 << 位置`。`|` `&` `^` `~`（宣言したビットだけ）と複合代入、`has(a, b)` | `uintN_t` |
| `bitset`（`bitfield<N>`、`bitfield<N, 型>`、名前のない bitfield） | 公開のビットフィールドを持つ `struct` と `==`。型を書かない bitfield は IDL 4 の既定（1ビットは `bool`、8ビットまで `uint8_t` など） | 全ビットが収まる `uintN_t`。各 bitfield を宣言の順に下位ビットから詰める。符号付きの型は読むときに符号を広げる |
| `@default(v)`（struct と例外のメンバー） | メンバーの初期値。整数・浮動小数・`boolean`・`string` | 変わらない |
| `@range(min=, max=)`、`@min`、`@max`（struct と例外のメンバー） | 範囲外の値は送るときに `BAD_PARAM`、受け取るときに `MARSHAL` | 変わらない |

- **エラーにするもの**：`map`、`@optional`、`@external`、`@value`（GIOP は enum を位置で送る）、`int8` の union の判別子（TypeCode がない）、`bitset` の継承、enum の `@default`。`map`・`@optional`・`@external`・`@value` はすべてのバックエンドでエラーになる
- **無視するもの**：`@key`、`@id`、`@autoid`、`@final` / `@appendable` / `@mutable`、`@topic`、`@nested`、`@verbatim` など DDS のためのアノテーションと、利用者が定義したアノテーション
- **他の言語**：`int8`・`bitmask`・`bitset` は C++ だけで、Python・Java・Rust の生成器はエラーにする。`@default` と `@range` はそれらの生成器では無視する
- idl_parser は IDL 4 の構文をすべて読める（足りないものはなかった）

`narrow` は、同じプロセスのスタブなら C++ の型で、そうでなければ生成コードが登録した継承関係（リポジトリID）で判定し、どちらでも決まらなければ相手に `_is_a` を問い合わせる。

## 例外

| 場面 | 結果 |
| --- | --- |
| サーバントが宣言されたユーザー例外を送出 | そのユーザー例外を返す |
| サーバントが `CORBA::SystemException` を送出 | そのまま返す |
| サーバントが宣言されていないユーザー例外を送出 | `UNKNOWN`（minor `0x4f4d0001`、COMPLETED_MAYBE） |
| サーバントがその他の C++ 例外を送出 | `UNKNOWN`（minor 0、COMPLETED_MAYBE）。`what()` を標準エラーに出す |
| 操作がない | `BAD_OPERATION`（minor `0x4f4d0002`、COMPLETED_NO） |
| 結果の書き込みで失敗（上限超えなど） | その例外を COMPLETED_YES で返す（操作は実行済み） |
| 返ってきたユーザー例外が操作で宣言されていない | `UNKNOWN`（minor `0x4f4d0001`、COMPLETED_YES） |

システム例外は40種すべてが `CORBA::SystemException` の派生クラスで、未知のリポジトリIDは `rustorb::OtherSystemException` になる。POA と ORB の例外（`PortableServer::POA::ObjectNotActive`、`CORBA::ORB::InvalidName` など）は C ABI のローカル例外コードから変換する。

## POA の範囲

`create_POA`（7種のポリシー）、`find_POA`、`destroy`、`activate_object(_with_id)`、`deactivate_object`、`servant_to_id/reference`、`id_to_servant/reference`、`reference_to_id/servant`、`create_reference(_with_id)`、デフォルトサーバント（`get_servant` / `set_servant`）、servant manager（`set_servant_manager` / `get_servant_manager`）、`the_activator`、`POAManager` の4状態、`PortableServer::Current`。`id_to_servant` などで返るサーバントは、元の C++ オブジェクトに戻る。

`PortableServer::ServantActivator`・`ServantLocator`・`AdapterActivator` はローカルインターフェースで、アプリケーションが派生クラスを `CORBA::make_reference<T>()` で作って POA に渡す。各操作は ORB のアップコールのスレッドで呼ばれる。

- `incarnate` / `preinvoke` は `PortableServer::ForwardRequest` で転送（`LOCATION_FORWARD`）でき、`CORBA::SystemException` はそのままクライアントに返る。nil のサーバントは `OBJ_ADAPTER`、その他の C++ 例外は `UNKNOWN`（`what()` を標準エラーに出す）
- `ServantLocator::Cookie` は `void*` で、`preinvoke` が設定した値が `postinvoke` に渡る。`preinvoke` が失敗したときは `postinvoke` を呼ばない
- `etherealize` と `postinvoke` が受け取るサーバントは、`incarnate` / `preinvoke` が返した C++ オブジェクト
- `get_servant_manager` と `the_activator()` は設定した C++ オブジェクト（同じ `shared_ptr`）を返す。POA は破棄されるまで参照を持ち、破棄されると手放す
- C ABI では `rorb_poa_set_servant_activator` / `_locator`・`rorb_poa_set_the_activator` が関数表（`rorb_servant_activator_vtable` など）と `self` を受け取り、サーバントと同じく `add_ref` / `remove_ref` で参照を数える。`incarnate` / `preinvoke` の例外は `rorb_manager_error_forward` / `_system` で伝える

`_this()` は、そのサーバントの要求の実行中ならその要求のオブジェクトを、そうでなければ `_default_POA()`（最初に初期化した ORB の RootPOA）で暗黙に活性化した参照を返す。同じ ORB の中の呼び出しは呼び出し元のスレッドでサーバントを実行する（コアの同一 ORB 内呼び出しをそのまま使う）。

## テスト

- `cpp/tests/test_mapping.cc`：`cpp/tests/idl/CxxMapping.idl`（定数、typedef、`fixed`（typedef・struct のメンバー・列・inout / out・定数・Any・四則演算と結果の型・`round` / `truncate`）、上限付きの型、多次元配列、再帰的な struct と union、すべての種類の判別子、入れ子の型、予約語の名前、多重継承、oneway、inout / out、ユーザー例外とシステム例外、オブジェクト参照の列）を、同じプロセスの2つの ORB の間と、同一 ORB 内で往復させる
- `cpp/tests/test_idl4.cc`：`cpp/tests/idl/Idl4.idl`（`int8`、`bitmask`、`bitset`、`@default`、`@range` / `@min` / `@max`）を、ローカルと、同じプロセスの2つの ORB の間と同一 ORB 内で往復させる。Any の中の TypeCode も確かめる
- `cpp/tests/test_interop.cc`：相互接続の78ケース（`interop/impls/rustorbcpp/cases.h`）を同じプロセスの2つの ORB の間で回す
- `cpp/tests/test_poa.cc`：`ServantActivator`（初回の `incarnate`、転送、システム例外、nil、C++ 例外、`deactivate_object` と `destroy` での `etherealize`）、`ServantLocator`（cookie の受け渡し、失敗時に `postinvoke` を呼ばないこと）、`AdapterActivator`（`find_POA` と、存在しない POA への要求での作成）、ポリシーの検査（`WrongPolicy`、種類の違う manager の `OBJ_ADAPTER`、2回目の `BAD_INV_ORDER`）を、2つの ORB の間と同一 ORB 内で確かめる。POA が破棄されると manager が解放されることも見る
- `cpp/tests/test_any.cc`：Any / TypeCode の相互接続の251ケース（DynAny の組み立て直しを含む）（`interop/impls/rustorbcpp/anytest.h`）を同じプロセスの2つの ORB の間で回し、TypeCode の操作、`ORB::create_*_tc`、`<<=` / `>>=` の規則を確かめる
- `cpp/tests/test_value.cc`：valuetype の相互接続の26ケース（`interop/impls/rustorbcpp/valuetest.h`）を同じプロセスの2つの ORB の間で回し、値ファクトリの表、truncatable の切り捨て、ファクトリのない型の `MARSHAL`、`ORB::create_value_tc`、`_copy_value` を確かめる
- `cpp/tests/test_dynany.cc`：`DynamicAny`（[dynany.md](dynany.md)）。生成コードの型の DynAny、valuetype の共有と循環、切り詰め。残りの操作は `interop/impls/rustorbcpp/dynany_probe.cc`（`run-dynany-probe.sh`）で確かめる
- CI（`ci.yml` の `cpp`）は g++ と clang++ の両方で `-Wall -Wextra -Werror` でビルドして ctest を回す。ローカルでは ASan・UBSan・LeakSanitizer でも警告なし
- `crates/rustorb-codegen/python/tests/test_cxx_backend.py`：コマンドライン、生成物の形、対応しない構文のエラー、生成コードのコンパイル
- 相互接続：`interop/impls/rustorbcpp` を omniORB C++・omniORBpy・rustorb（Rust・Python）とすべての組み合わせで、omniNames と rustorb-names の両方、GIOP 1.1 でも回す。失敗は既存の `known-failures.txt`（omniORB 側の wchar）の範囲だけ
- POA プローブ：`interop/impls/rustorbcpp/poa_probe_server.cc`（servant manager・AdapterActivator・デフォルトサーバント・スレッドモデルを含む）で `run-poa-probe.sh` を回し、Rust のプローブサーバーと同じ期待出力（`expected/poa-probe-rustorb.txt`）と一致する
- naming check と wire probe：`interop/impls/rustorbcpp/naming_check.cc`（生成した CosNaming のスタブ）で `run-naming-check.sh`（omniNames と rustorb-names）を、C++ のサーバーに `run-wire-probe.sh` を回し、Rust と同じ期待出力と一致する

## 残り

- IDL 4 の `map` と `@optional`：GIOP に形がないので扱わない（上記）
