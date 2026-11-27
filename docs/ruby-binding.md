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

リリースからは `gem install rustorb` で入る（0.7.0 から）。gem は Linux x86_64 / aarch64 と macOS arm64 の `librustorb_ffi.a` を `vendor/` に持ち、拡張は入れるときにビルドする（Ruby のヘッダと C コンパイラが要る）。

拡張は `librustorb_ffi.a` を静的にリンクする。ライブラリは既定で gem の `vendor/<環境>/librustorb_ffi.a`、なければチェックアウトの `target/release/librustorb_ffi.a` を使い、`RUSTORB_FFI_LIBRARY` で変えられる。ヘッダは `RUSTORB_FFI_INCLUDE` で変えられる。Ruby 3.0 以降、Linux と macOS。

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
| `sequence`・配列 | Array。`octet` の列は String でも送れる（受け取りは Array。`Rustorb.octets_as_string = true` でバイナリの String。[c-ruby-octets.md](c-ruby-octets.md)） |
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
  - アップコールの中からの rustorb の呼び出し（呼び出し、`ORB#shutdown`、POA の destroy、POAManager の操作など）も、待っているアップコールのスレッドで行う。こうすると rustorb は呼び出しがアップコールの中だとわかる。SINGLE_THREAD の POA への入れ子の呼び出しは外の呼び出しと同じスレッドで受け、アップコールの中の `shutdown(true)` は `BAD_INV_ORDER` になる
- **サーバント**：C の peer（Ruby のオブジェクトとリポジトリ ID）を rustorb にサーバントとして渡す。rustorb が参照を持っているあいだは、GC の mark で Ruby のオブジェクトを生かしておく
  - `_rustorb_dispatch` は引数を読み、メソッドを呼び、結果を書く
  - 宣言されたユーザー例外はそのまま返す
  - システム例外もそのまま返す
  - それ以外の例外は `UNKNOWN`（COMPLETED_MAYBE）にする
- **ORB**：`CORBA.ORB_init(args, orb_id = "")` は `-ORB` の引数を `args` から取り除く。同じ ORB id の2度目の呼び出しは（destroy するまで）同じ ORB を返す。rustorb の拡張の `initializers:` には、その ORB だけの `PortableInterceptor::ORBInitializer` を渡す（`rorb_orb_init_pi`）。`RootPOA`・`POACurrent`・`PICurrent`・`CodecFactory` は Ruby のラッパーを返す
- **DynAny**：`orb.resolve_initial_references("DynAnyFactory")`。`DynamicAny::DynAny`・`DynStruct`・`DynUnion`・`DynEnum`・`DynFixed`・`DynSequence`・`DynArray`・`DynValue`・`DynValueBox` は操作を IDL の名前（`insert_long`、`get_members`、`current_component` など）で持つ。値は Ruby の値、any は `CORBA::Any`、TCKind は `CORBA::TK_` の Integer、`DynFixed#get_value` は String（`set_value` は BigDecimal も受ける）。例外は `DynamicAny::DynAny::InvalidValue`・`TypeMismatch`、`DynAnyFactory::InconsistentTypeCode`、`DynamicAny::MustTruncate`。中身は C 拡張の `Rustorb::Native::DynAny` が `rorb_dynany_*` を包み、値は any で受け渡す
- **DII / DSI**：`CORBA::Request`（`obj._request(op)`、`_create_request`）・`NVList`・`NamedValue`・`ServerRequest` と `PortableServer::DynamicImplementation`。C 拡張の `Rustorb::Native::Request`・`ServerRequest` が `rorb_request_*`・`rorb_server_request_*` を包む。詳しくは [dii-dsi.md](dii-dsi.md)
- **local インターフェース**：`-bruby` はモジュール（`ID`、`_tc` は tk_local_interface、入れ子の型と定数）だけを作り、スタブもスケルトンも作らない。実装は `CORBA::LocalObject` を継承してそのモジュールを include し、操作をメソッドで定義する。`_narrow` はそのモジュールを include したオブジェクトをそのまま返し、ほかは `BAD_PARAM`。`_is_a?` は祖先のモジュールの ID と `CORBA::LocalObject`・`CORBA::Object` に答える。呼び出しの引数や any に入れて送ろうとしたり `object_to_string` したりすると `MARSHAL`（minor 4、COMPLETED_NO）
- **valuetype**：生成コードの形は次のとおり（RCLM には valuetype の定めが少ないので、Python のマッピングと他のバインディングに合わせた）。中身は C 拡張が C ABI の `rorb_out_begin_value`・`rorb_in_read_value_header` などを包み、`Rustorb::Values` が状態を読み書きする。詳しくは [valuetype.md](valuetype.md)

  | IDL | Ruby |
  |---|---|
  | `valuetype V` | `class V`（`include ::CORBA::ValueBase`。具象の基底があれば `class V < Base`）。`ID`・`TRUNCATABLE_IDS`・`MEMBERS`、状態メンバーごとの `attr_accessor`（private も）、`_tc`。`V.new(...)` は基底のものから順に状態メンバーを取る |
  | 操作・初期化子・サポートするインターフェースのない valuetype | そのクラスがファクトリ（`Rustorb::Values.register(V, default: true)`） |
  | 操作か初期化子のある valuetype | アプリケーションがサブクラスで操作を書き、`orb.register_value_factory(V::ID, Impl)` で登録する（クラス、`create_for_unmarshal` を持つオブジェクト、ブロックのどれでもよい。クラスなら `allocate` で作って状態を読む）。初期化子はそのサブクラスの `new` で書く |
  | `valuetype T supports I` | 加えて `include ::PortableServer::ServantMethods`、`IDS`・`_operations`。実装のインスタンスは `I` のサーバントでもある（`t._this`） |
  | `abstract valuetype A` | `module A`（`include ::CORBA::ValueBase`）。派生のクラスが include する |
  | `custom valuetype C` | 加えて `include ::CORBA::CustomMarshal`。実装は `marshal(os)`・`unmarshal(is)`（`CORBA::DataOutputStream`・`DataInputStream` の `write_long`・`read_Value`・`write_long_array(seq, offset, length)` など）を書いてファクトリを登録する。値は常にチャンクで送る |
  | `valuetype B long;`（value box） | 値は中身そのもの（null は nil）。`module B` は型の名前と `_tc` だけ |
  | `abstract interface N` | インターフェースと同じモジュールとスタブ（`extend ::Rustorb::AbstractInterfaceModule`、`_tc` は tk_abstract_interface）。`N._narrow` は値ならそのまま、参照ならスタブにする。送るときは参照（nil を含む）なら TRUE と参照、値なら FALSE と値。valuetype が `supports N` なら N を include し、N の操作は実装が書く |

  - 同じメッセージで同じインスタンスを2度書くと間接参照になり、受ける側でも1つのインスタンスになる（循環も保つ）。any に入れたインスタンスはその any から取り出すと同じインスタンスで、複数の any に入れても1つのまま。ストリームと any のコアのインスタンスが持つ Ruby のオブジェクトは C 拡張の表が GC から守り、手放されると外す（`Rustorb::Native.kept_values` がその数）
  - 受けた値の型にファクトリがなければ（truncatable の基底のファクトリもなければ）`MARSHAL`。truncatable の値を基底のファクトリで受けると、基底の状態だけを読んで残りを飛ばす
  - any の中で基底の型として届いた派生の値を読めるように、コアが知らない valuetype の TypeCode は生成コードの登録から渡す（`rorb_set_value_type_resolver`）
  - `orb.lookup_value_factory(id)`・`unregister_value_factory(id)`（登録がなければ `BAD_PARAM`）
- **AMI**：コールバック型の AMI（[ami.md](ami.md)）は `-bruby -Wbami` で入る。RCLM に AMI の定めはないので、他のバインディングに合わせた
  - 暗黙の IDL の `M::AMI_IHandler` はふつうのインターフェース（モジュールとスタブ、`POA::M::AMI_IHandler`）。基底の `Messaging::ReplyHandler` はランタイムにある
  - スタブに `sendc_<op>(ami_handler, <in と inout の値>)`、属性は `sendc_get_<attr>(ami_handler)` / `sendc_set_<attr>(ami_handler, attr_<attr>)`。引数はその場で any に写す（後から値を変えても送る内容は変わらない）。返事は ORB の返事用スレッドから GVL を取って、Handler の参照の `<op>(戻り値、inout と out の値)` か `<op>_excep(excep_holder)` を普通の呼び出しとして呼ぶ。nil の Handler なら返事は捨て、Handler が投げた例外も捨てる。呼び出しのタイムアウトは `_call_timeout=` などの、送ったときのものが効く
  - `Messaging::ExceptionHolder` は valuetype（状態メンバーは `is_system_exception`・`byte_order`・`marshaled_exception`）。`raise_exception` が例外を投げ、`raise_exception_with_list(tcs)` は一覧にないユーザー例外を `UNKNOWN`（minor 0x4f4d0001）にする。別のプロセスから届いたものは、登録された生成コードのクラスで `marshaled_exception` から戻し、知らないものは `UNKNOWN`
  - C 拡張の `Ref#sendc` が `rorb_object_sendc` を、`Native.cdr_encode` / `cdr_decode` が `rorb_cdr_encode` / `rorb_in_from_bytes` を包む。`interop/run-ami-probe.sh rustorbrb:SERVER` の結果は他の ORB と同じ
- **Portable Interceptors**：[portable-interceptors.md](portable-interceptors.md)。`PortableInterceptor::ClientRequestInterceptor`・`ServerRequestInterceptor`・`IORInterceptor`・`ORBInitializer` は何もしない既定のメソッドを持つモジュールで、1つのクラスが複数を include できる。`name` の既定は `""`（無名）。初期化子は `PortableInterceptor.register_orb_initializer(init)`（そのあとに作るすべての ORB）か `CORBA.ORB_init(args, orb_id, initializers: [...])`
  - 情報は `ClientRequestInfo`・`ServerRequestInfo`・`IORInfo`・`ORBInitInfo`。属性は引数のないメソッド（`ri.operation`・`ri.reply_status` など）、`ServerRequestInfo` の `object_id` は Ruby の `Object#object_id` と重なるので生成コードと同じく `r_object_id`。オクテット列は binary の String、`arguments` は `Dynamic::Parameter`（`argument` は `CORBA::Any`、`mode` は `CORBA::PARAM_IN` など）、ポリシーは `PortableServer::Policy`。情報はその点のあいだだけ使え、あとで使うと `BAD_INV_ORDER`
  - 点は C 拡張のディスパッチャーのスレッドで GVL を取って動き、rustorb が点を呼んだスレッド（クライアントは呼び出しの、サーバーはアップコールのスレッド）はその間待つ。点が `PortableInterceptor::ForwardRequest.new(ref)` を raise すると転送、システム例外はそのまま、それ以外は `UNKNOWN`（COMPLETED_MAYBE）
  - `PICurrent` のスロットはスレッドごとなので、アップコールの中の `get_slot`・`set_slot` は rustorb がアップコールを動かすスレッドで行う（サーバントには `receive_request_service_contexts` で入れた値が見える）。DSI の `ServerRequest` の返事も同じスレッドで書くので、サーバーのインターセプターに DSI の引数と結果が見える
  - `IOP::ServiceContext`・`TaggedComponent`・`TaggedProfile`・`Encoding`、`IOP::CodecFactory#create_codec` と `IOP::Codec`（`encode`・`decode`・`encode_value`・`decode_value`）。例外は `PortableInterceptor::InvalidSlot`・`ORBInitInfo::DuplicateName`・`InvalidName`・`IOP::CodecFactory::UnknownEncoding`・`IOP::Codec::InvalidTypeForEncoding`・`FormatMismatch`・`TypeMismatch`
  - 中身は C 拡張が C ABI の関数表（`rorb_client_interceptor_vtable` など）と `rorb_cri_*`・`rorb_sri_*`・`rorb_ior_info_*`・`rorb_init_info_*`・`rorb_pi_current_*`・`rorb_codec_*` を包む。`interop/run-pi-probe.sh` の `rustorbrb`（`interop/impls/rustorbrb/lib/pi_probe.rb`）は他の ORB と GIOP 1.0〜1.2 で同じ結果
- **servant manager**：`PortableServer::ServantActivator`（`incarnate`・`etherealize`）、`ServantLocator`（`preinvoke` は `[servant, cookie]` かサーバントだけを返す。`postinvoke` はその cookie を受ける）、`AdapterActivator`（`unknown_adapter`）を継承し、`POA#set_servant_manager`・`the_activator=` で渡す。`PortableServer::ForwardRequest.new(ref)` で別のオブジェクトへ転送する。POA のアップコールのスレッドから GVL を取って呼ぶ。`interop/run-poa-probe.sh rustorbrb` の結果は rustorb のものと同じ
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

gem の配布（`librustorb_ffi` の同梱）は [#189](https://github.com/rustorb/rustorb/issues/189) で行う。

## テスト

- `ruby/test`（`cd ruby && rake`）。cargo で `librustorb_ffi` を作り、拡張をビルドし、`rustorb-idl -bruby` でテスト用 IDL のコードを作って minitest を流す。`RUSTORB_IDL` で生成器のコマンドを変えられる
  - `test_runtime`：手書きのスタブとスケルトン（`test/demo`）で、呼び出し、例外、any のいろいろな値、POA の操作と例外、Current、スレッドから並行の呼び出しを確かめる
  - `test_idl4`：`cpp/tests/idl/Idl4.idl` の int8・bitmask・bitset・`@default`・`@range` を ORB 越しに確かめる（any の中も）
  - `test_mapping`：他のバインディングと同じ `cpp/tests/idl/CxxMapping.idl` を ORB 越しに確かめる。定数、再帰型、すべての判別子の union、入れ子の型、キーワード、上限と不正な値、fixed と long double、多重継承、TypeCode と any
  - `test_local`：`LocalTest.idl`（.NET のテストと同じ）の local インターフェースの実装、`_narrow`、TypeCode、送れないこと
  - `test_ami`：`sendc_` の結果、inout と out、属性、ユーザー例外とシステム例外の `ExceptionHolder`（状態だけのものも）、nil の Handler、例外を投げる Handler、並行の 50 件、引数の写し
  - `test_values`：値のファクトリの登録と探索、ファクトリがないときの `MARSHAL`、TypeCode の問い合わせ、any の中のインスタンス、ストリームが持ったものを手放すこと
  - `test_servant_managers`：servant activator（転送、例外、etherealize）、servant locator（cookie）、adapter activator
  - `test_pi`：自分の ORB id と初期化子の ORB で、サービスコンテキストとスロットがクライアントからサーバントまで行って戻ること、IOR のコンポーネント、`ForwardRequest` での転送、点が投げたシステム例外と他の例外、`receive_exception`、`DuplicateName`、PICurrent（スレッドごと、`InvalidSlot`）、`Codec` と `UnknownEncoding`・`FormatMismatch`
  - `test_interop`：相互接続の Ruby クライアントの79ケース（`interop/impls/rustorbrb/lib/cases.rb`）、any スイートのケース（`anytest.rb`）、value・abstract・custom スイートのケース（`valuetest.rb`・`abstracttest.rb`・`customtest.rb`）を、同じプロセスのサーバントに向けて流す
  - `test_dynany`：生成した型の DynAny、クラスと例外
  - `test_dii`：DII と DSI
- rustorb-idl の `python/tests/test_ruby_backend.py`：生成物の形、名前の規則、include、対応しない構文のエラー、`ruby -wc`。golden は `tests/golden/ruby`
- CI（`ci.yml` の `ruby`）：Linux の Ruby 3.0 と 3.4、macOS の Ruby 3.4 で `rake`。Linux では同梱の CosNaming のコードが生成器の出力と一致することも確かめる
- 相互接続（[#185](https://github.com/rustorb/rustorb/issues/185)）：`interop/run-matrix.sh` に `rustorbrb`（`interop/impls/rustorbrb`）を加えた
  - ランタイムと生成コードはホストで用意する。拡張は相互接続のイメージ（Ruby 3.2、`ruby-dev`・`gcc` を入れた）の中で、ホストで作った `librustorb_ffi.a` に対してビルドする
  - basic（InteropTest.idl）と any（AnyTest.idl。値を DynAny でばらして組み直すケースを含む）
- naming check（`run-naming-check.sh`）と wire probe（`run-wire-probe.sh`）にも `rustorbrb` を加えた
- ローカルの結果。失敗はすべて既存の `known-failures.txt` の範囲
  - basic：omniORBpy・omniORB C++・JacORB・rustorb の Rust・Python・C++・C・自身と両方向。失敗は omniORBpy の wchar と JacORB の `echo_value[5]` だけ
  - GIOP 1.1、`--fragment 1024`、`--ssl`、`--unix`：他の rustorb の実装と同じ結果（GIOP 1.1 の omniORB C++ の wchar / wstring は既知）
  - naming check（omniNames と rustorb-names、再起動を含む）と wire probe は期待出力と一致
  - any：omniORBpy・omniORB C++・rustorb の Rust・Python・C++・.NET・C・自身と両方向。失敗は long double の既知のもの（omniORBpy に long double がない、omniORB C++ が x87 の形式で送る）だけ
  - DynAny プローブ（`run-dynany-probe.sh rustorbrb`）は rustorb の期待出力と一致
  - `rustorbrb-dii`（呼び出しを DII、サーバントを DSI にしたもの）：basic（GIOP 1.2 / 1.1）と any で `rustorbrb` と同じ結果
