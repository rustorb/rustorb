# Ruby バインディング（Ruby Language Mapping 1.2）

Nov 21, 2026 · @ysuga · [#176](https://github.com/rustorb/rustorb/issues/176)

rustorb の Ruby バインディングの構成をまとめる。

- C++・Java・.NET・C と同じく、Rust コアの `extern "C"` の安定 ABI 層（`crates/rustorb-ffi`、`rustorb.h`）の上に載せた
- API は OMG の [Ruby Language Mapping 1.2](https://www.omg.org/spec/RCLM/1.2/)（RCLM、formal/2013-04-05）に沿う
- ランタイムは `ruby/` の gem `rustorb`。C 拡張 `rustorb_ext` と、その上の Ruby のコード `lib/rustorb` から成る
- 生成器は rustorb-idl の `-bruby`
- 計画は [dotnet-c-ruby.md](dotnet-c-ruby.md)

既存 ORB のソースは読まない方針（[clean-room-policy.md](clean-room-policy.md)）は Ruby の ORB（R2CORBA など）にも適用する。API は仕様書の本文だけから書いた。

## 構成

| 場所 | 内容 | ライセンス |
| --- | --- | --- |
| `crates/rustorb-ffi` | C ABI（`librustorb_ffi`）。他のバインディングと共通 | MIT OR Apache-2.0 |
| `ruby/ext/rustorb_ext` | C 拡張。ABI のハンドル（ORB、参照、POA、TypeCode、any、サーバント）と CDR のストリームを包む。GVL とアップコールのスレッドを扱う | MIT OR Apache-2.0 |
| `ruby/lib/rustorb` | マッピング。`exceptions.rb`（システム例外・ユーザー例外）、`typecode.rb`（`CORBA::TypeCode`）、`marshal.rb`（TypeCode に従う読み書き）、`types.rb`（struct・union の基底、`CORBA::Any`、`CORBA::LongDouble`）、`object.rb`（`CORBA::Object` とスタブ）、`orb.rb`、`portable_server.rb`（POA、POAManager、ポリシー、Current、`PortableServer::Servant`） | MIT OR Apache-2.0 |
| `ruby/lib/CosNamingC.rb`・`CosNamingS.rb` | `rustorb-idl -bruby` で生成した CosNaming（`rake cos_naming`） | MIT OR Apache-2.0 |
| rustorb-idl の `python/rustorb_idl/ruby_backend.py` | `rustorb-idl -bruby` | GPL-3.0-or-later（生成物は対象外） |
| `ruby/test` | テスト（minitest） | MIT OR Apache-2.0 |

## 使い方

```sh
cd ruby
rake compile                                # cargo で librustorb_ffi を作り、拡張を lib/rustorb に置く
rustorb-idl -bruby -C gen Example.idl       # gen/ExampleC.rb, gen/ExampleS.rb
```

拡張は `librustorb_ffi.a` を静的にリンクする。ライブラリは既定でチェックアウトの `target/release/librustorb_ffi.a` を使い、`RUSTORB_FFI_LIBRARY` で変えられる。ヘッダは `RUSTORB_FFI_INCLUDE` で変えられる。Ruby 3.0 以降、Linux と macOS。

```ruby
require "corba"
require "ExampleS"

class HelloImpl < POA::Example::Hello
  def say(message) = message
end

orb = CORBA.ORB_init(ARGV)                  # -ORB の引数は ARGV から取り除かれる
poa = orb.resolve_initial_references("RootPOA")
poa.the_POAManager.activate
hello = HelloImpl.new._this                 # RootPOA に暗黙に活性化する
puts orb.object_to_string(hello)
orb.run
```

```ruby
obj = orb.string_to_object(ior)
hello = Example::Hello._narrow(obj)         # 型が違えば CORBA::BAD_PARAM
hello.say("hi")
```

## 生成コードの形

| IDL | Ruby |
| --- | --- |
| 名前 | Ruby の定数になる名前（module、型、定数、列挙子）は先頭を大文字にする（`module demo` → `Demo`。`BEGIN`・`END` は `R_` を付ける）。メソッド・メンバーの名前が Ruby のキーワードか、どのオブジェクトにもあるメソッド（`class`・`hash`・`send`・`raise` など）と重なるときは `r_` を付ける。引数の名前は小文字で始まらなければ `_` を付ける |
| module | Ruby の module |
| 基本型 | 整数は Integer、`float`・`double` は Float、`boolean` は true / false、`char` は1文字の String、`wchar` は Integer（UTF-16 の単位）、`octet` は Integer |
| `long double` | `CORBA::LongDouble`（binary128 のビット。Float との変換） |
| `string` | String（UTF-8） |
| `wstring` | Integer の配列（UTF-16 の単位） |
| `fixed` | BigDecimal（仕様は対応しない。rustorb の拡張） |
| `sequence`・配列 | Array。`octet` の列は String でも送れる（受け取りは Array） |
| struct | `CORBA::Portable::Struct` の派生クラス。`attr_accessor` のメンバーと、IDL の順の位置引数の `new`。`==` はメンバーを比べる |
| union | `CORBA::Portable::Union` の派生クラス。`_disc`、`_value`、メンバーごとの読み（選ばれていなければ `BAD_PARAM`）と書き（最初のラベルを選ぶ）。`_put(name, value, disc)` で他のラベルを、`_default(disc)` でどのメンバーも選ばない値を設定できる |
| enum | Integer の定数を持つ module（`Color::RED`）。列挙子は IDL のスコープどおり外側にも置く（`Mod::RED`）。`ENUMERATORS` は名前の配列 |
| typedef | `_tc` を持つ module。struct・union の typedef はその派生クラス |
| const | Ruby の定数 |
| `int8`（IDL 4） | Integer（-128〜127）。octet として送る |
| bitmask（IDL 4） | `1 << position` の Integer の定数と `BIT_BOUND` を持つ module。値は Integer。`_tc` は保持する符号なし整数（`@bit_bound` から octet・ushort・ulong・ulonglong） |
| bitset（IDL 4） | `Rustorb::Bitset` の派生クラス。ビットフィールドごとの `attr_accessor` と位置引数の `new`。`FIELDS` は `[名前, 位置, ビット数, :unsigned / :signed / :boolean]`。保持する符号なし整数として送る。幅に収まらない値は `MARSHAL` |
| `@default`・`@range`・`@min`・`@max`（IDL 4） | struct の `DEFAULTS`（`new` に渡さなかったメンバーの値）と `RANGES`（`[min, max]`）。範囲外のメンバーは送るときに `BAD_PARAM`、受け取るときに `MARSHAL` |
| exception | `CORBA::UserException` の派生クラス（メンバーは struct と同じ） |
| TypeCode | どの型も `_tc`。初めて使うときに作る（`Rustorb.lazy_tc`。再帰型は作っている途中の型を `create_recursive_tc` で指す） |
| interface | module（`Mod::Iface`）。操作ごとのメソッドと、属性ごとの `x` と `x=` を持つ。基底インターフェースの module を include する。`Mod::Iface._narrow` と `_unchecked_narrow` がスタブ（`Mod::Iface::Stub`、`CORBA::Object` の派生で module を include したもの）を返す |
| 引数と結果 | `in`・`inout` が引数になる。結果と `out`・`inout` は、値が1つならその値、2つ以上なら結果の後ろに並べた配列で返る（サーバントの側も同じ形で返す） |
| サーバント | `POA::Mod::Iface`（`PortableServer::Servant` の派生）。`IDS`（自分と祖先のリポジトリ ID）と操作の表を持つ。実装はこれを継承して操作のメソッドを書く |

## 実装の要点

- **マーシャリング**：生成コードは型と操作を TypeCode と操作の記述（`Rustorb::Operation`）で表し、`Rustorb::Marshal` がそれに従って CDR のストリームを読み書きする。基本型の列と配列は `pack` した String のままひとまとめに読み書きする
  - 型に合わない値は `CORBA::MARSHAL`（COMPLETED_NO）にする。RCLM はこれを送る前の検査の例外としている。範囲外の整数、上限を超える列と文字列、範囲外の列挙子が対象
  - ワイヤーから読んだ any の中身のうち、Ruby のクラスがない型は `Rustorb::GenericStruct` / `GenericUnion` になる。その any はもらったバイト列のまま送り返せる
- **GVL**：ブロックする呼び出し（呼び出し、`ORB#run`、POA の destroy など）は GVL を手放す。そのコールバック（引数を書く、結果を読む）は `rb_thread_call_with_gvl` で取り戻す
- **アップコール**：rustorb のアップコール用スレッドは Ruby のスレッドではないので GVL を取れない
  - 要求は Ruby のディスパッチャースレッドのプールに渡し、終わるのを待つ（`Rustorb::Native.dispatcher_loop`）
  - プールはディスパッチャーが最後の空きを取るたびに1つ増える。サーバントが外へ呼び出して呼び返されても、受けるスレッドがある
  - `PortableServer::Current` とサーバントの「いまの参照」（要求の中の `_this`）は rustorb のスレッドごとの状態にある。そのためディスパッチャーは、それを待っているアップコールのスレッドに問い合わせを頼む
- **サーバント**：C の peer（Ruby のオブジェクトとリポジトリ ID）を rustorb にサーバントとして渡す。rustorb が参照を持っているあいだは、GC の mark で Ruby のオブジェクトを生かしておく
  - `_rustorb_dispatch` は引数を読み、メソッドを呼び、結果を書く
  - 宣言されたユーザー例外はそのまま返す
  - システム例外もそのまま返す
  - それ以外の例外は `UNKNOWN`（COMPLETED_MAYBE）にする
- **ORB**：`CORBA.ORB_init(args)` は `-ORB` の引数を `args` から取り除く。ORB はプロセスに1つ。`RootPOA` と `POACurrent` は Ruby のラッパーを返す
- **rustorb の拡張**：`CORBA::ORB#call_timeout=`・`CORBA::Object#_call_timeout=`（秒）、`Rustorb::Native` の下の低水準の API

## 例外

`CORBA::SystemException` は `minor` と `completed`（`CORBA::COMPLETED_YES`・`_NO`・`_MAYBE`）を持つ。`CORBA::BAD_PARAM.new(reason, minor, completed)` で作る。

POA・POAManager・Current・ORB・TypeCode の例外は `PortableServer::POA::ObjectNotActive` などのユーザー例外。

| 場面 | 結果 |
| --- | --- |
| サーバントが宣言されたユーザー例外を raise | そのユーザー例外を返す |
| サーバントがシステム例外を raise | そのまま返す |
| サーバントが宣言されていないユーザー例外か他の例外を raise | `UNKNOWN`（COMPLETED_MAYBE） |
| メソッドがない | `NO_IMPLEMENT` |
| 操作がない | `BAD_OPERATION`（minor `0x4f4d0002`、COMPLETED_NO） |
| 引数が読めない | `MARSHAL`（COMPLETED_NO） |
| 結果が型に合わない | `MARSHAL`（COMPLETED_YES） |
| 返ってきたユーザー例外が操作で宣言されていない | `UNKNOWN` |
| `_narrow` で型が違う | `BAD_PARAM` |

## まだないもの

次のものは [#188](https://github.com/rustorb/rustorb/issues/188) で足す。

- valuetype と abstract interface
- local インターフェース（servant manager など）
- DII / DSI
- DynAny
- Portable Interceptors
- AMI（`-Wbami`）

`-bruby` はこれらをエラーにする。gem の配布（`librustorb_ffi` の同梱）は [#189](https://github.com/rustorb/rustorb/issues/189) で行う。

## テスト

- `ruby/test`（`cd ruby && rake`）。cargo で `librustorb_ffi` を作り、拡張をビルドし、`rustorb-idl -bruby` でテスト用 IDL のコードを作って minitest を流す。`RUSTORB_IDL` で生成器のコマンドを変えられる
  - `test_runtime`：手書きのスタブとスケルトン（`test/demo`）で、呼び出し、例外、any のいろいろな値、POA の操作と例外、Current、スレッドから並行の呼び出しを確かめる
  - `test_idl4`：`cpp/tests/idl/Idl4.idl` の int8・bitmask・bitset・`@default`・`@range` を ORB 越しに確かめる（any の中も）
  - `test_mapping`：他のバインディングと同じ `cpp/tests/idl/CxxMapping.idl` を ORB 越しに確かめる。定数、再帰型、すべての判別子の union、入れ子の型、キーワード、上限と不正な値、fixed と long double、多重継承、TypeCode と any
  - `test_interop`：相互接続の Ruby クライアントの79ケース（`interop/impls/rustorbrb/lib/cases.rb`）を、同じプロセスの Echo サーバントに向けて流す
- rustorb-idl の `python/tests/test_ruby_backend.py`：生成物の形、名前の規則、include、対応しない構文のエラー、`ruby -wc`。golden は `tests/golden/ruby`
- CI（`ci.yml` の `ruby`）：Linux の Ruby 3.0 と 3.4、macOS の Ruby 3.4 で `rake`。Linux では同梱の CosNaming のコードが生成器の出力と一致することも確かめる
- 相互接続（[#185](https://github.com/rustorb/rustorb/issues/185)）：`interop/run-matrix.sh` に `rustorbrb`（`interop/impls/rustorbrb`）を加えた
  - ランタイムと生成コードはホストで用意する。拡張は相互接続のイメージ（Ruby 3.2、`ruby-dev`・`gcc` を入れた）の中で、ホストで作った `librustorb_ffi.a` に対してビルドする
  - いまは basic（InteropTest.idl）だけ
- naming check（`run-naming-check.sh`）と wire probe（`run-wire-probe.sh`）にも `rustorbrb` を加えた
- ローカルの結果。失敗はすべて既存の `known-failures.txt` の範囲
  - basic：omniORBpy・omniORB C++・JacORB・rustorb の Rust・Python・C++・C・自身と両方向。失敗は omniORBpy の wchar と JacORB の `echo_value[5]` だけ
  - GIOP 1.1、`--fragment 1024`、`--ssl`、`--unix`：他の rustorb の実装と同じ結果（GIOP 1.1 の omniORB C++ の wchar / wstring は既知）
  - naming check（omniNames と rustorb-names、再起動を含む）と wire probe は期待出力と一致
