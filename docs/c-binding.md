# C バインディング（C Language Mapping 1.0）

Nov 21, 2026 · @ysuga · [#176](https://github.com/rustorb/rustorb/issues/176)

rustorb の C バインディングの構成をまとめる。C++・Java・.NET と同じく、Rust コアの `extern "C"` の安定 ABI 層（`crates/rustorb-ffi`、`rustorb.h`）の上に、OMG の [C Language Mapping 1.0](https://www.omg.org/spec/C/1.0/)（formal/99-07-35）の API を C99 で書いた。ランタイムは `c/` の静的ライブラリ `librustorb_c`、生成器は rustorb-idl の `-bc`。計画は [dotnet-c-ruby.md](dotnet-c-ruby.md)。

既存 ORB のソースは読まない方針（[clean-room-policy.md](clean-room-policy.md)）は C の ORB（ORBit など）にも適用する。API は仕様書の本文と表だけから書いた。

## 構成

| 場所 | 内容 | ライセンス |
| --- | --- | --- |
| `crates/rustorb-ffi` | C ABI（`librustorb_ffi`）。C++・Java・.NET と共通 | MIT OR Apache-2.0 |
| `c/include/rustorb/orb.h` | `CORBA_*` の型、`CORBA_Environment` と例外、`CORBA_free` と確保の関数、列、`CORBA_any`、TypeCode、`CORBA_Object`・`CORBA_ORB`・`CORBA_Policy` | MIT OR Apache-2.0 |
| `c/include/rustorb/poa.h` | `PortableServer_*`（`ServantBase`、POA、POAManager、Current、ポリシー、POA の例外） | MIT OR Apache-2.0 |
| `c/include/rustorb/messaging.h` | AMI の `Messaging_ReplyHandler` と `Messaging_ExceptionHolder` | MIT OR Apache-2.0 |
| `c/include/rustorb/pi.h` | Portable Interceptors（`PortableInterceptor_*`、`Dynamic_*`）と `IOP_*`（サービスコンテキスト、タグ付きコンポーネント、`IOP_Codec`） | MIT OR Apache-2.0 |
| `c/include/rustorb/runtime.h` | 生成コードとの取り決め（型の記述子、操作の記述子、`rustorb_c_invoke`、サーバントのクラス）。アプリケーションは使わない | MIT OR Apache-2.0 |
| `c/include/rustorb/CosNaming.h`・`c/src/CosNaming.c` | `rustorb-idl -bc` で生成した CosNaming（`c/generate-stubs.sh`） | MIT OR Apache-2.0 |
| `c/src` | ランタイム（`memory.c` 確保と解放とコピー、`types.c` 記述子と TypeCode、`marshal.c` 汎用の読み書き、`any.c`、`invoke.c` スタブとスケルトン、`orb.c`、`poa.c`、`env.c`） | MIT OR Apache-2.0 |
| rustorb-idl の `python/rustorb_idl/c_backend.py` | `rustorb-idl -bc` | GPL-3.0-or-later（生成物は対象外） |
| `c/tests` | テスト（CTest） | MIT OR Apache-2.0 |

## 使い方

リリースには `rustorb-c-<版>-<環境>.tar.gz`（Linux x86_64 / aarch64、macOS arm64。0.7.0 から）と `rustorb-c-<版>-windows-x86_64.zip`（Windows x64、MSVC。0.7.0 の次の版から）がある。`include/rustorb/` のヘッダと、`lib/` の `librustorb_c.a`・`librustorb_ffi.a`（Windows は `rustorb_c.lib`・`rustorb_ffi.lib`）が入っている。ソースからは次のとおり。

```sh
rustorb-idl -bc -C gen Example.idl                 # gen/Example.h, gen/Example.c
cmake -S c -B build/c && cmake --build build/c     # cargo で librustorb_ffi も作る
```

CMake の外で使うときは `c/include` と `crates/rustorb-ffi/include` をインクルードパスに入れ、`librustorb_c.a`・`librustorb_ffi.a` とシステムのライブラリ（Linux は `-lpthread -ldl -lm`、macOS は `-framework CoreFoundation -framework Security`）をリンクする。Windows（MSVC）では `rustorb_c.lib`・`rustorb_ffi.lib` と `ws2_32.lib userenv.lib ntdll.lib bcrypt.lib` をリンクし、`rustorb_ffi.lib` に合わせて C ランタイムは `/MD` にする（`cl` の既定は `/MT`、CMake の既定は `/MD`）。`cmake --install` はヘッダと2つのライブラリを入れる。

```c
#include "Example.h"   /* <rustorb/orb.h>・<rustorb/poa.h> を含む */

static CORBA_char* say(PortableServer_Servant s, const CORBA_char* m, CORBA_Environment* ev) {
  return CORBA_string_dup(m);   /* 可変長の結果は呼ばれた側が確保する */
}
static PortableServer_ServantBase__epv base_epv = {NULL, NULL, NULL};
static POA_Example_Hello__epv hello_epv = {NULL, say};
static POA_Example_Hello__vepv hello_vepv = {&base_epv, &hello_epv};

int main(int argc, char** argv) {
  CORBA_Environment ev;
  POA_Example_Hello servant = {NULL, &hello_vepv};
  CORBA_ORB orb = CORBA_ORB_init(&argc, argv, "", &ev);
  PortableServer_POA poa = (PortableServer_POA)CORBA_ORB_resolve_initial_references(orb, "RootPOA", &ev);
  PortableServer_POAManager_activate(PortableServer_POA__get_the_POAManager(poa, &ev), &ev);
  POA_Example_Hello__init(&servant, &ev);
  PortableServer_ObjectId* id = PortableServer_POA_activate_object(poa, &servant, &ev);
  CORBA_Object ref = PortableServer_POA_id_to_reference(poa, id, &ev);
  CORBA_char* ior = CORBA_ORB_object_to_string(orb, ref, &ev);
  puts(ior);
  CORBA_ORB_run(orb, &ev);
  /* 各呼び出しのあとで ev._major を見る */
}
```

`CORBA_ORB_init` は他のバインディングと同じ ORB のオプション（`-ORBInitRef`、`-ORBendPoint` など）を受け、使ったものを `argv` から取り除く。最初の呼び出しで C ABI のバージョン（3）を確かめ、違えば `INITIALIZE`。

## 生成コードの形

| IDL | C |
| --- | --- |
| 名前 | スコープ付きの名前の `::` を `_` に（`Mod_Iface_op`）。C と C++ のキーワードは先頭に `_`（`_class`）。struct や interface の中の型も同じ規則（`Mod_Outer_Inner`、列挙子は `Mod_Outer_OFF`） |
| 基本型 | `CORBA_short`・`CORBA_long`・`CORBA_long_long`・`CORBA_unsigned_*`・`CORBA_float`・`CORBA_double`・`CORBA_long_double`・`CORBA_char`・`CORBA_wchar`・`CORBA_boolean`・`CORBA_octet` |
| `string`・`wstring` | `CORBA_char*`・`CORBA_wchar*`（16 ビットの UTF-16 の単位） |
| `fixed<D, S>` | `CORBA_fixed_D_S`（`_digits`・`_scale`・`_value[]`。BCD で、最後の半バイトが符号） |
| `sequence<T>` | `_maximum`・`_length`・`_buffer`・`_release` の構造体（`CORBA_sequence_T`）、`CORBA_sequence_T__alloc()`、`CORBA_sequence_T_allocbuf(n)`。名前のない列の型はそれを使う IDL ごとに `#ifndef` で守って宣言する |
| 配列 | C の配列と `T_slice`、`T__alloc()` |
| struct・exception | C の構造体と `T__alloc()`。exception はさらに `ex_<名前>`（リポジトリ ID）。メンバーのない exception は `_rustorb_dummy` を1つ持つ |
| union | `_d` と `_u` の構造体 |
| enum | C の enum（列挙子はスコープ付きの名前） |
| const | `#define`（型付きの式。`fixed` と `wstring` は複合リテラル） |
| IDL 4 の `int8` | `CORBA_int8`（`signed char`。C++ のマッピングと同じく octet として送り、TypeCode も `tk_octet`） |
| IDL 4 の bitmask | ビットを収める符号なし整数（`@bit_bound` に合わせて `CORBA_octet`・`CORBA_unsigned_short`・`CORBA_unsigned_long`・`CORBA_unsigned_long_long`）の typedef と、値の定数（`Mod_READ` は `((Mod_Flags)(1U << 0))`。名前は enum の列挙子と同じ規則） |
| IDL 4 の bitset | 同じく整数の typedef。C のビットフィールドは配置が処理系で決まるので、送るときの形の整数のまま持ち、フィールドはマクロ `Mod_Header_kind(v)` / `Mod_Header_set_kind(&v, x)` で読み書きする（符号付きのフィールドは符号を広げる）。継承はエラー。bitmask も bitset も TypeCode と記述子は整数のもの。`@default` と `@range` は C では無視する |
| TypeCode | `TC_<名前>`（借りたもので解放しない。`TC_long` なども同じ） |
| interface | `typedef CORBA_Object Mod_Iface;`、スタブ `Mod_Iface_op(_obj, 引数..., CORBA_Environment* _ev)`、属性は `Mod_Iface__get_x` / `__set_x`。継承した操作も派生の名前で出す |
| サーバント | `POA_Mod_Iface`（`_private`・`vepv`）、`POA_Mod_Iface__epv`（自分の操作）、`POA_Mod_Iface__vepv`（`_base_epv`、祖先の epv を遠い順に、最後に自分の epv）、`POA_Mod_Iface__init` / `__fini` |
| 引数と結果 | 仕様の表 1-2・1-3 のとおり。可変長の `out` と結果は呼ばれた側が確保し、呼んだ側が `CORBA_free`。配列の結果は slice へのポインタ |

## 実装の要点

- **型の記述子**：生成コードの `.c` は型ごとに `rustorb_c_type`（TCKind、`sizeof`、リポジトリ ID と名前、要素の型、上限・長さ、メンバーの `offsetof`、列挙子、union の既定のメンバー、fixed の桁数）を出す。1つの汎用の書き込み・読み込み・解放・コピーの処理がそれを辿るので、スタブ・スケルトン・`CORBA_any`・`CORBA_free` を同じ仕組みで扱う。union はラベルごとに1つの項目を持つ
- **TypeCode**：記述子から最初に使ったときに作り、記述子に覚えておく。再帰型は作っている途中の型を `rorb_tc_recursive` で指す。名前のある型はリポジトリ ID の登録表に入り、ワイヤーから読んだ `any` の中身はこれで C の値にする。IDL ファイルの型は、そのファイルの最初のスタブ呼び出し、サーバントの初期化、TypeCode の使用で登録する。名前のない列・配列・fixed の記述子は TypeCode から作る
- **`CORBA_free`**：ランタイムの確保（`T__alloc`、`CORBA_string_alloc`、`_allocbuf` など）は、領域の前に記述子と個数を持つ見出しを置く。`CORBA_free` はそれを見て中身（列の要素の文字列、参照、`any` など）まで解放する。列の `_buffer` は `_release` が真のときだけ解放する（`CORBA_sequence_set_release`）
- **`CORBA_Object`**：オブジェクト参照、ORB、POA、POAManager、TypeCode、Policy、Current を1つのラッパーで持つ。`CORBA_Object_duplicate` は新しいラッパーを返し、`CORBA_Object_release` はそれを解放する。`TC_long` や `TC_<名前>` は解放しない
- **`any`**：`CORBA_any`（`_type`・`_value`・`_release`）。登録表にない型の値は `_value` を NULL にして、中のバイト列を持ったまま送り返せる（`CORBA_any_equal` はこれも比べる）
- **サーバント**：`POA_X__init` は `_private` にクラス（リポジトリ ID と操作の表）と C ABI のサーバントを置く。要求が来ると、パラメーターごとのセルを確保して引数を読み、epv の関数を呼び、結果と `out` を書いて解放する。`__fini` で C ABI のサーバントを放す
- **スレッド**：ランタイムの共有の状態（登録表、TypeCode の作成）はミューテックス（POSIX は pthread、Windows は SRWLOCK）で守る。サーバントの関数はアップコール用スレッドプールのスレッドから呼ばれる
- **送れない値**：上限を超えた列や文字列、範囲外の列挙子や union の判別子、桁数に収まらない fixed、NULL の文字列は送信時に `BAD_PARAM`（COMPLETED_NO。他のバインディングと同じ）
- **rustorb の拡張**：`CORBA_string_dup`・`CORBA_wstring_dup`、`rustorb_c_copy`（記述子による深いコピー）、`PortableServer_ServantBase__this`、`rustorb_ORB_set_call_timeout`・`rustorb_Object_set_call_timeout`（マイクロ秒）

## 例外

`CORBA_Environment` の `_major` は `CORBA_NO_EXCEPTION`・`CORBA_USER_EXCEPTION`・`CORBA_SYSTEM_EXCEPTION`。`CORBA_exception_id` がリポジトリ ID、`CORBA_exception_value` がユーザー例外の構造体かシステム例外の `CORBA_SystemException`（`minor`・`completed`）を返す。ランタイムの関数は始めに `_major` を `CORBA_NO_EXCEPTION` にする（前の例外は `CORBA_exception_free` で解放しておく）。

| 場面 | 結果 |
| --- | --- |
| サーバントが宣言されたユーザー例外を `CORBA_exception_set` | そのユーザー例外を返す |
| サーバントがシステム例外を設定 | そのまま返す |
| サーバントが宣言されていないユーザー例外を設定 | `UNKNOWN`（COMPLETED_MAYBE） |
| epv の関数が NULL | `NO_IMPLEMENT` |
| 操作がない | `BAD_OPERATION`（minor `0x4f4d0002`、COMPLETED_NO） |
| 引数が読めない | `MARSHAL`（COMPLETED_NO） |
| nil の参照で呼ぶ | `INV_OBJREF` |
| 返ってきたユーザー例外が操作で宣言されていない | `UNKNOWN`（minor `0x4f4d0001`、COMPLETED_YES） |

`CORBA_exception_as_any(ev)` は例外を `CORBA_any` にする（例外がなければ NULL）。仕様のとおり any は Environment のもので、`CORBA_exception_free` まで有効、呼んだ側は解放しない。`_value` は `_params` そのもの（`_release` は FALSE）。

- ユーザー例外の TypeCode は生成コードの記述子のもの。そのファイルの型をまだ使っていない（登録の前の）プログラムや、記述子のない例外のうちメンバーのあるものは NULL。疑似オブジェクトの例外（`CORBA_ORB_InvalidName`、`PortableServer_POA_InvalidPolicy` など）も any になる
- システム例外は `tk_except` の TypeCode（名前はリポジトリ ID の最後の部分、メンバーは `minor` と `CompletionStatus` の `completed`）で、値は `CORBA_SystemException`。同じ形の any を読むと `_value` は `CORBA_SystemException` になる

## fixed の演算

C マッピング 1.14 の `CORBA_fixed_integer_part`・`fraction_part`・`set`・`add`・`sub`・`mul`・`div` と `CORBA_fixed_alloc`。仕様のとおり引数は `void*` で、どの `CORBA_fixed_D_S` も渡せる。結果も含めてすべての `_digits` と `_scale` を先に設定しておく。仕様が決めていないところは次のようにした。

- 計算は十進の桁で正確に行い、結果 `*rp` の形に合わせる。`_scale` より下の桁は 0 の方へ切り捨て（丸めない）、`_digits` に入らない上の桁は捨てる。0 での割り算の結果は 0（関数に `CORBA_Environment` がないので例外は返せない）
- `fraction_part` は小数部を `min(_scale, 9)` 桁の整数で、値の符号を付けて返す（12.345 は 345、-1.5 は -5）。`set(rp, i, f)` は `i + f / 10^min(_scale, 9)`。`integer_part` は 0 の方へ切り捨て、`CORBA_long` に入らないものは端の値
- `CORBA_fixed_alloc(d)` は 31 桁までの `CORBA_fixed_d_s`（`_value` は 16 オクテット）を `_digits = d`、`_scale = 0`、値 0 で返し、`CORBA_free` で解放する

## DynAny

`<rustorb/dynany.h>` に DynamicAny モジュール（`DynamicAny_DynAnyFactory`・`DynAny`・`DynFixed`・`DynEnum`・`DynStruct`・`DynUnion`・`DynSequence`・`DynArray`、`NameValuePair` などの列）を置く。C マッピングの書き方（`DynamicAny_DynAny_insert_long(d, v, &ev)`、結果は呼び出し側が `CORBA_free` / `CORBA_Object_release`）に従い、中身は rustorb-ffi の DynAny（Rust のコアと同じ実装）。

```c
DynamicAny_DynAnyFactory f = CORBA_ORB_resolve_initial_references(orb, "DynAnyFactory", &ev);
DynamicAny_DynStruct d = DynamicAny_DynAnyFactory_create_dyn_any_from_type_code(f, TC_AnyTest_Point, &ev);
DynamicAny_DynAny_insert_long(d, 3, &ev);   /* 現在のメンバー x */
DynamicAny_DynAny_next(d, &ev);
DynamicAny_DynAny_insert_double(d, 2.5, &ev);
CORBA_any* a = DynamicAny_DynAny_to_any(d, &ev);
```

- DynAny は `CORBA_Object` で、`CORBA_Object_is_a` は種類ごとのインターフェース（`IDL:omg.org/DynamicAny/DynStruct:1.0` など）に答える。種類に合わない派生の操作は `BAD_OPERATION`
- 値は `CORBA_any` で出入りする（`from_any`・`to_any`・`get_members` など）。生成コードの登録表にない型の値は、他の `any` と同じくバイト列のまま持つ
- `insert_<型>_seq` / `get_<型>_seq` は CORBA モジュールの列（`CORBA_LongSeq` は `CORBA_sequence_long` など。生成コードと同じ `#ifndef` で守って `<rustorb/dynany.h>` が宣言する）
- C マッピングに valuetype の型はないので `insert_val` / `get_val` はない。`DynamicAny_DynValue`・`DynValueBox`（と `DynValueCommon` の `is_null`・`set_to_null`・`set_to_value`。派生の名前でも呼べる）は TypeCode が表す値（`CORBA_ORB_create_value_tc` で作ったものや線から読んだ any）を、メンバーを any か DynAny で扱う
- TypeCode の生成は `CORBA_ORB_create_struct_tc`・`exception_tc`・`union_tc`・`enum_tc`・`value_tc`・`value_box_tc`・`recursive_tc` もある。メンバーは `CORBA_StructMemberSeq`・`CORBA_UnionMemberSeq`（`label` は判別子の型の any、default は octet の 0）・`CORBA_EnumMemberSeq`・`CORBA_ValueMemberSeq` で、`type_def`（インターフェースリポジトリ）は使わない
- DynAny のプローブ（`run-dynany-probe.sh`、`interop/impls/rustorbc/src/dynany_probe.c`）は rustorb の期待出力と一致する。ctest の `dynany_probe` でも比べる

## DII と DSI

`<rustorb/dii.h>` に C マッピング 1.0 の `CORBA_NVList`・`CORBA_NamedValue`・`CORBA_Request`・`CORBA_ServerRequest`・`PortableServer_DynamicImpl` を置く。中身は rustorb-ffi の `rorb_request` / `rorb_server_request`（Rust のコアの DII と DSI）。

```c
CORBA_Request req;
CORBA_long a = 7, b = 2, rem, q;
CORBA_Object_create_request(echo, NULL, "divide", NULL, NULL, &req, 0, &ev);
CORBA_Request_add_arg(req, "a", TC_long, &a, 0, CORBA_ARG_IN, &ev);
CORBA_Request_add_arg(req, "b", TC_long, &b, 0, CORBA_ARG_IN, &ev);
CORBA_Request_add_arg(req, "remainder", TC_long, &rem, 0, CORBA_ARG_OUT, &ev);
CORBA_Request_add_exception(req, TC_InteropTest_DivideByZero, &ev);
CORBA_Request_result(req, &ev)->argument._type = TC_long;
CORBA_Request_result(req, &ev)->argument._value = &q;
CORBA_Request_invoke(req, 0, &ev);     /* q == 3, rem == 1 */
CORBA_Request_delete(req, &ev);
```

- 値は `CORBA_NamedValue` の `argument`（`CORBA_any`）で受け渡す。in の値は送るときに `_value` から読む（`CORBA_IN_COPY_VALUE` ならリストが複製を持つ）。out・inout の値と結果は、`_value` が指す領域に書く（inout は前の中身を解放してから）。`_value` が NULL なら ORB が確保し（`_release` が真）、リストと一緒に、または `CORBA_NVList_free_memory` で解放される
- `CORBA_Request_invoke`、`CORBA_Request_send`（`CORBA_INV_NO_RESPONSE` で oneway）と `CORBA_Request_get_response`（`CORBA_RESP_NO_WAIT`）、rustorb の拡張 `CORBA_Request_poll_response`
- ユーザー例外は、`CORBA_Request_add_exception`（rustorb の拡張。C マッピング 1.0 には例外のリストがない）で渡した型なら `CORBA_USER_EXCEPTION` になり、値は生成コードの構造体。リストにない例外は `UNKNOWN`（minor `0x4f4d0001`）
- DSI のサーバントは `POA_PortableServer_DynamicImpl`（`vepv` に `invoke` と `primary_interface`）で、`PortableServer_DynamicImpl__init` が `primary_interface` を一度呼んで `_is_a` の答えにする。`invoke(servant, request)` は `CORBA_ServerRequest_arguments`（C マッピング 1.0 の名前 `_params` も使える）に型と向きだけのリストを渡して引数を読み、out の値を設定し、`_set_result` / `_set_exception` で返す。システム例外は `CORBA_ServerRequest_set_system_exception`（rustorb の拡張。`invoke` に `CORBA_Environment` がないため）
- Context は扱わない（`ctx` が NULL でなければ `NO_IMPLEMENT`）。`CORBA_ORB_create_operation_list` と `CORBA_send_multiple_requests` もない

## local インターフェースとサーバントマネージャー

C マッピング 1.0 には local インターフェースがないので、rustorb で次のように決めた。

- `-bc` は local インターフェース `M::L` にも、ふつうのインターフェースと同じスタブ（`M_L_op(obj, ..., ev)`）と `POA_M_L`・`POA_M_L__epv`・`POA_M_L__vepv` を作る。`POA_M_L__init` は構造体を local オブジェクトの実装にする（POA で活性化はしない）。`M_L__create(impl, ev)` がその local オブジェクトを作り、スタブは ORB を通らずに実装を呼ぶ
- 実装の構造体は、local オブジェクトの参照がなくなるまで呼び出し側が持つ（`CORBA_Object_release` は実装を解放しない）。最後に `POA_M_L__fini`
- TypeCode は `tk_local_interface`。local オブジェクトを送ろうとすると `MARSHAL`（minor 4）。`CORBA_Object_is_a` は実装のインターフェースと `IDL:omg.org/CORBA/LocalObject:1.0` に答える

```c
static POA_PortableServer_ServantActivator__epv epv = {NULL, my_incarnate, my_etherealize};
static POA_PortableServer_ServantManager__epv mepv = {NULL};
static POA_PortableServer_ServantActivator__vepv vepv = {&base_epv, &mepv, &epv};
static POA_PortableServer_ServantActivator impl = {NULL, &vepv};

POA_PortableServer_ServantActivator__init(&impl, &ev);
mgr = PortableServer_ServantActivator__create(&impl, &ev);
PortableServer_POA_set_servant_manager(poa, mgr, &ev);
```

- `<rustorb/poa.h>` の `PortableServer_ServantActivator`・`ServantLocator`・`AdapterActivator` も同じ形の local インターフェース（`incarnate` / `etherealize`、`preinvoke` / `postinvoke`、`unknown_adapter`）。`PortableServer_POA_set_servant_manager` / `get_servant_manager` と `PortableServer_POA__set_the_activator` / `_get_the_activator` で POA に渡す
- `incarnate` と `preinvoke` は、`PortableServer_ForwardRequest`（`forward_reference`）を `CORBA_USER_EXCEPTION` で返すと転送になり、システム例外はクライアントに返る。返すサーバントは初期化したもの（`POA_<I>__init` か `PortableServer_DynamicImpl__init`）

## AMI

コールバック型の AMI（[ami.md](ami.md)）は `-bc -Wbami` で入る。C マッピング 1.0 には AMI がないので、他の言語の形に合わせて次のように決めた。

- 暗黙の IDL の `M::AMI_IHandler` はふつうのインターフェース（`M_AMI_IHandler`、`POA_M_AMI_IHandler`）。基底の `Messaging::ReplyHandler` はランタイムの `<rustorb/messaging.h>`（`Messaging_ReplyHandler`、操作のない `POA_Messaging_ReplyHandler__epv`）で、vepv にはその場所もある
- `M_I_sendc_<op>(obj, ami_handler, <in と inout の値>, ev)`、属性は `M_I_sendc_get_<attr>(obj, ami_handler, ev)` / `M_I_sendc_set_<attr>(obj, ami_handler, attr_<attr>, ev)`。生成コードでは各 Handler の後ろに置く。引数は写してから返り、`ev` に入るのは送る前に分かる失敗（nil の参照は `INV_OBJREF`、ORB の終了後は `BAD_INV_ORDER`）だけ。返事は ORB の返事用スレッドで Handler の `<op>`（戻り値、inout と out の値）か `<op>_excep` を呼ぶ。nil の Handler なら返事は捨てる。呼び出しのタイムアウト（`rustorb_Object_set_call_timeout` など）は送ったときのものが効く
- C マッピングには valuetype がないので、`Messaging::ExceptionHolder` は状態メンバーの構造体 `Messaging_ExceptionHolder`（`is_system_exception`・`byte_order`・`marshaled_exception`）にした。線上では valuetype として送り、`<op>_excep` は in の構造体（`const Messaging_ExceptionHolder*`）として受け取る
- `Messaging_ExceptionHolder_raise_exception(holder, ev)` が `ev` を例外にする。ユーザー例外は生成コードの記述子で読むので、そのファイルの型をこのプログラムがまだ使っていないもの（登録の前）や知らないものは `UNKNOWN`（minor 0x4f4d0001）。`raise_exception_with_list(holder, list, count, ev)` は TypeCode の並びにない例外も `UNKNOWN` にする。読めない `marshaled_exception` は `MARSHAL`

```c
static void my_divide(PortableServer_Servant s, CORBA_long r, CORBA_long rem, CORBA_Environment* ev);
static void my_divide_excep(PortableServer_Servant s, const Messaging_ExceptionHolder* h,
                            CORBA_Environment* ev) {
  CORBA_Environment e = {CORBA_NO_EXCEPTION, NULL, NULL};
  Messaging_ExceptionHolder_raise_exception(h, &e);
  /* e は InteropTest_DivideByZero など */
  CORBA_exception_free(&e);
}

InteropTest_Echo_sendc_divide(echo, handler, 7, 2, &ev);
```

## Portable Interceptors

[portable-interceptors.md](portable-interceptors.md) のコアの PI を `<rustorb/pi.h>` で使う。C マッピング 1.0 には PI がないので、local インターフェースの形（上の節）に合わせた。

- インターセプターと初期化子は local インターフェースの実装の構造体。`POA_PortableInterceptor_ClientRequestInterceptor__epv`（`send_request`・`send_poll`・`receive_reply`・`receive_exception`・`receive_other`）、`ServerRequestInterceptor`、`IORInterceptor`（`establish_components`）、`ORBInitializer`（`pre_init`・`post_init`）。vepv は `_base_epv`、`Interceptor` の epv（`_get_name`・`destroy`）、自分の epv の順。`__init` で初期化し、`__create` で参照にする
- 初期化子は `PortableInterceptor_register_orb_initializer(init, ev)` で登録し、次の `CORBA_ORB_init` が呼ぶ。`-ORBid` の値が ORB id。`ORBInitInfo` の `add_client_request_interceptor` などは名前の重複に `DuplicateName`、違う種類の参照に `BAD_PARAM`、`resolve_initial_references` は `pre_init` の中では `BAD_INV_ORDER`。`destroy` は `CORBA_ORB_destroy` で呼ばれる
- 情報のオブジェクト（`ClientRequestInfo`・`ServerRequestInfo`・`IORInfo`・`ORBInitInfo`）はその呼び出しの間だけ使える。複製も解放もしなくてよい。属性は `PortableInterceptor_RequestInfo__get_operation` などで、結果は呼んだ側が `CORBA_free` する。呼べない点では `BAD_INV_ORDER`（minor 14）、静的なスタブとスケルトンの `arguments` などは `NO_RESOURCES`（minor 1）。DII / DSI の要求では `arguments`・`result`・`exceptions` が読める
- インターセプターから返す例外：`PortableInterceptor_ForwardRequest`（`forward`）は転送、システム例外はそのまま、それ以外は `UNKNOWN`。静的なスケルトンのユーザー例外は、`send_exception` の `sending_exception` では `UNKNOWN` の any（コアと同じ）。リポジトリ ID は `received_exception_id`（クライアント）で読める
- `resolve_initial_references` は `"PICurrent"`（`PortableInterceptor_Current`、`get_slot` / `set_slot`、`InvalidSlot`）と `"CodecFactory"`（`IOP_CodecFactory_create_codec`。CDR カプセルの 1.0〜1.2、それ以外は `UnknownEncoding`）を答える。`IOP_Codec_encode` / `decode` は any、`encode_value` / `decode_value` は値だけで、`decode_value` は TypeCode を受け取る。読めない中身は `FormatMismatch`

```c
static void my_send_request(PortableServer_Servant s, PortableInterceptor_ClientRequestInfo ri,
                            CORBA_Environment* ev) {
  IOP_ServiceContext c = {0x50495052, {0, 0, NULL, CORBA_FALSE}};
  /* c.context_data を IOP_Codec_encode_value で作る */
  PortableInterceptor_ClientRequestInfo_add_request_service_context(ri, &c, CORBA_FALSE, ev);
}
```

## まだないもの

valuetype と abstract interface は C マッピングにないので `-bc` はエラーにする（AMI の `ExceptionHolder` だけは上のとおり）。共有ライブラリ版はまだない。

## テスト

- `c/tests`（`ctest --test-dir build/c`。cargo で `librustorb_ffi` を、`rustorb-idl -bc` でテスト用 IDL のコードを作る）
  - `test_interop`：相互接続の C クライアントの79ケース（`interop/impls/rustorbc/src/cases.c`）を、同じプロセスの Echo サーバント（`echo_impl.c`）に向けて流す
  - `test_mapping`：C++・Java・.NET と同じ `cpp/tests/idl/CxxMapping.idl` の定数、再帰型、すべての判別子の union、入れ子の型、キーワード、fixed と long double、インターフェースの多重継承と `NO_IMPLEMENT`、TypeCode、`AnyTest.idl` の型の `any` の往復（名前のない列、NVList、`out` / `inout` の `any`）、`CORBA_free` と `rustorb_c_copy`
  - `test_interop dii` と `test_interop deferred`：同じ79ケースを、DII（`interop/impls/rustorbc/src/dii_adapters.c`：生成スタブの呼び出しをオペレーションの型から作った `CORBA_Request` で送る）から DSI のサーバント（静的なサーバントのスケルトンを呼ぶ `PortableServer_DynamicImpl`）へ、また `CORBA_Request_send` と `get_response` で静的なサーバントへ流す
  - `test_dii`：NVList、in の複製、ORB が確保する out と結果、inout、`send` / `poll_response` / `get_response`、oneway、リストの中と外のユーザー例外、システム例外、引数の誤り、手で書いた `PortableServer_DynamicImpl`
  - `test_local`：.NET のテストと同じ `LocalTest.idl` の local インターフェース（実装の構造体と `__create`、属性、out、ユーザー例外、継承した操作、`is_a` などの参照の操作、引数と結果の local オブジェクト、送れないこと）
  - `test_dynany`：DynAny の基本の型、TypeCode の生成、DynValue と DynValueBox、`insert_long_seq` / `get_long_seq`、`any` の中の `any`、struct（メンバー、`get_members_as_dyn_any` の共有、`seek` / `next`）、union、enum、sequence と array、fixed、`equal`・`copy`・`destroy`、例外（`TypeMismatch`・`InvalidValue`・`BAD_OPERATION`）
  - `test_ami`：AMI のプローブと同じケース（`interop/impls/rustorbc/src/ami_cases.c`）を同じプロセスの Echo サーバントに向けて流し、`interop/expected/ami-probe.txt` と比べる。`ExceptionHolder` の読めない中身、知らない例外、`raise_exception_with_list`
  - `test_pi`：初期化子（ORB id、引数、`DuplicateName`、種類の違う参照、`pre_init` の `resolve_initial_references`）、両方向のサービスコンテキストと重複、PICurrent のスロットが要求のスコープに写ること、IOR のタグ付きコンポーネント、`get_effective_policy`、`InvalidSlot`、静的なスタブの `NO_RESOURCES`、ユーザー例外、`receive_request` からのシステム例外、`send_request` からの `ForwardRequest`、`Codec`、ORB の破棄での `destroy`
  - `test_exception`：`CORBA_exception_as_any` のユーザー例外（メンバーのあるもの、ないもの、記述子のないもの）、システム例外、疑似オブジェクトの例外。どれも `IOP_Codec` で往復させる
  - `test_fixed`：fixed の演算（線上の形、符号、桁の違う引数、切り捨て、上の桁を捨てること、31 桁、0 での割り算、`CORBA_fixed_alloc`）
  - `test_idl4`：C++・Java・.NET と同じ `cpp/tests/idl/Idl4.idl` の int8・bitmask・bitset（定数、フィールドのマクロと符号、同じプロセスのサーバントへの呼び出し、any の TypeCode、`IOP_Codec` での線上の形）
  - `-DRUSTORB_SANITIZE=address` で AddressSanitizer と LeakSanitizer。どのテストもリークなし
- rustorb-idl の `python/tests/test_c_backend.py`：生成物の形、対応しない構文のエラー、生成コードの C99（警告をエラーにする）と、ヘッダの C++ としてのコンパイル。golden は `tests/golden/c`
- CI（`ci.yml` の `c` と `c-sanitize`）：Linux の gcc と clang、macOS の clang、Windows の MSVC で `ctest`、Linux の gcc で ASan。Windows ではテストの pthread とクロックを `interop/impls/rustorbc/src/posix_compat.h` が SRW ロックと条件変数で置き換える。Linux では同梱の CosNaming のスタブが生成器の出力と一致することも確かめる
- 相互接続（[#182](https://github.com/rustorb/rustorb/issues/182)）：`interop/run-matrix.sh` に `rustorbc`（`interop/impls/rustorbc`。ホストで CMake と cc でビルドし、静的にリンクした `bin/client`・`bin/server`・`bin/naming_check` を相互接続のイメージで動かす）を加えた。basic（InteropTest.idl）と any（AnyTest.idl）のスイートに入る。valuetype がないので value・custom・abstract には入らない。ローカルで確認した結果は次のとおりで、失敗はすべて `known-failures.txt` の範囲
  - basic（79ケース）：omniORBpy・omniORB C++・JacORB・rustorb の Rust・Python・C++・.NET・C 自身と両方向で通る。失敗は omniORBpy の wchar（3ケース）と JacORB の union の判別子（`echo_value[5]`）だけで、他の rustorb のバインディングと同じ
  - GIOP 1.1、`--fragment 1024`、`--ssl`、`--unix` でも omniORB・omniORBpy・自身との組（GIOP 1.1 と Fragment は JacORB とも）が同じ結果（GIOP 1.1 の omniORB 側の wchar / wstring は既知）
  - any（259ケース、[#187](https://github.com/rustorb/rustorb/issues/187)）：omniORBpy・omniORB C++・rustorb の Rust・Python・C++・.NET・C 自身と両方向で通る。失敗は long double（omniORBpy にはなく、omniORB C++ は x87 の形式で送る）だけで、他の rustorb のバインディングと同じ。サーバーは AnyEcho を名前に、Echo を名前に `-Echo` を付けたところに置き、AnyTestHidden.idl の型はクライアントだけが持つ
  - DII と DSI（`rustorbc-dii`。`RUSTORB_DII=1` で、クライアントは生成スタブの呼び出しを `CORBA_Request` で送り、サーバーは静的なサーバントを `PortableServer_DynamicImpl` で包む）：basic と GIOP 1.1 は rustorb の全バインディング・omniORBpy・omniORB C++・JacORB と、any は全サーバーから `rustorbc-dii` のサーバーへ通る。失敗は上と同じ既知のものだけ。CI では basic・GIOP 1.1・any を流す
  - Java（`rustorbjava`）との組は CI（`interop.yml`）で確かめる
- naming check（`run-naming-check.sh`。omniNames と rustorb-names、再起動を含む）と wire probe（`run-wire-probe.sh`）は、`rustorbc` でも期待出力と一致する。POA プローブ（`run-poa-probe.sh`。`interop/impls/rustorbc/src/poa_probe_server.c`：servant activator、servant locator、adapter activator、スレッドポリシー、POAManager の状態）も期待出力（rustorb のもの）と一致する。AMI プローブ（`run-ami-probe.sh`。`interop/impls/rustorbc/src/ami_probe.c`）は omniORB C++・omniORBpy・JacORB・rustorb・`rustorbc` のサーバーで期待出力と一致する。PI プローブ（`run-pi-probe.sh`。`interop/impls/rustorbc/src/pi_probe.c`）は GIOP 1.2・1.1・1.0 で、rustorb の Rust・Python・C++・.NET・C 自身と JacORB・omniORBpy とは両方向、omniORB C++ のクライアントから `rustorbc` のサーバーへ通る（JacORB のクライアントは他と同じく 1.2 だけ）
