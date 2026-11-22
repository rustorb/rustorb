# .NET バインディング（P/Invoke + IDL4 to C#）

Nov 21, 2026 · @ysuga · [#176](https://github.com/rustorb/rustorb/issues/176)

rustorb の .NET バインディングの構成をまとめる。C++・Java と同じく、Rust コアの `extern "C"` の安定 ABI 層（`crates/rustorb-ffi`）の共有ライブラリ `librustorb_ffi` を、.NET 8 の P/Invoke（`LibraryImport`）で直接呼ぶ。マッピングは [IDL4 to C#](https://www.omg.org/spec/IDL4-CSHARP/1.0)（formal/21-07-01）に従い、ランタイム（`dotnet/src/Rustorb.Corba`、NuGet の `Rustorb`）は .NET 8 以降で動く。計画は [dotnet-c-ruby.md](dotnet-c-ruby.md)。

既存 ORB のソースは読まない方針（[clean-room-policy.md](clean-room-policy.md)）は .NET の ORB（IIOP.NET など）にも適用する。API は仕様書（Annex A の `Corba`・`PortableServer` の宣言）だけから書いた。

## 構成

| 場所 | 内容 | ライセンス |
| --- | --- | --- |
| `crates/rustorb-ffi` | C ABI の共有ライブラリ（`librustorb_ffi.so` / `.dylib` / `rustorb_ffi.dll`）。C++・Java と共通 | MIT OR Apache-2.0 |
| `dotnet/src/Rustorb.Corba/Native` | C ABI の関数の宣言（`Ffi.g.cs`。`dotnet/tools/gen-pinvoke.py` が `rustorb.h` から生成し、CI で `--check`）、ハンドル（`SafeHandle`）、コールバック、エラーの変換、ライブラリの探索 | MIT OR Apache-2.0 |
| `dotnet/src/Rustorb.Corba/Corba`・`PortableServer`・`Omg` | 標準の API（`Corba.ORB`・`Object`・`TypeCode`・`Any`・システム例外40種、`PortableServer.POA`・`POAManager`・`Current`・`Servant`、`Omg.Types.ISequence<T>`） | MIT OR Apache-2.0 |
| `dotnet/src/Rustorb.Corba/Rustorb` | 生成コードとの取り決め（`CdrOutput` / `CdrInput`、`Stubs`、`Anys`、`TypeCodes`、`Seq`、`ServerReply`） | MIT OR Apache-2.0 |
| `dotnet/src/Rustorb.Corba/CosNaming` | `rustorb-idl -bcsharp` で生成した CosNaming（`dotnet/generate-stubs.sh`） | MIT OR Apache-2.0 |
| rustorb-idl の `python/rustorb_idl/csharp_backend.py` | `rustorb-idl -bcsharp` | GPL-3.0-or-later（生成物は対象外） |
| `dotnet/tests/Rustorb.Tests` | xUnit のテスト | MIT OR Apache-2.0 |

## 使い方

```sh
rustorb-idl -bcsharp -C gen Example.idl         # gen/Example.cs
cargo build --release -p rustorb-ffi            # target/release/librustorb_ffi.so
dotnet build -c Release dotnet/src/Rustorb.Corba
```

```csharp
Corba.ORB orb = Corba.ORB.init(args);
var poa = PortableServer.POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
poa.the_POAManager.activate();
IEcho echo = EchoHelper.narrow(poa.servant_to_reference(new EchoImpl()));  // EchoImpl : EchoPOA
Console.WriteLine(orb.object_to_string(echo));
orb.run();
```

ライブラリは、環境変数 `RUSTORB_FFI_LIBRARY` のパス、アセンブリのディレクトリ（NuGet の `runtimes/<rid>/native`）、OS の動的ローダーの探す場所の順に探す。最初の呼び出しで C ABI のバージョン（3）を確かめ、違えば `INITIALIZE`。`ORB.init(args)` は他のバインディングと同じ ORB のオプション（`-ORBInitRef`、`-ORBendPoint` など）を受ける。

## 生成コードの形

| IDL | C# |
| --- | --- |
| module | namespace |
| interface `Echo` | `IEchoOperations`（操作と属性）、`IEcho : Corba.IObject, IEchoOperations`、スタブの `class Echo : Corba.Object, IEcho`、スケルトンの `abstract class EchoPOA : PortableServer.Servant, IEchoOperations`、`EchoHelper`（`id`・`type`・`narrow`・`unchecked_narrow`・`insert`・`extract`・`read`・`write`） |
| 属性 | プロパティ（readonly は getter だけ） |
| in / inout / out | 値 / `ref` / `out` |
| struct・exception | クラス（`IEquatable<T>`、既定・コピー・全メンバーのコンストラクタ、メンバーはプロパティで、列のメンバーは getter だけ）。exception は `Corba.UserException` の派生 |
| union | クラス。`Discriminator { get; private set; }`、メンバーのプロパティ（get は選ばれていなければ `InvalidOperationException`、set は最初のラベルを選ぶ）、`Set<Member>(value, discriminator)`（ラベルが合わなければ `ArgumentException`）、既定のメンバーがないときの `_default()` |
| enum | `enum`（`uint`） |
| sequence | `Omg.Types.ISequence<T>`（実体は `Omg.Types.Sequence<T> : List<T>`） |
| 配列 | C# の配列（多次元は配列の配列） |
| typedef | 元の型。`<Alias>Helper` が別名の TypeCode を持つ |
| const | `static partial class Constants` の `const`（モジュールごと。インターフェースの定数はスタブのクラスの中） |
| string / wstring | `string`（UTF-8 / UTF-16） |
| char / wchar | `char`（char は ISO 8859-1 の範囲。外は送信時 `DATA_CONVERSION`） |
| long double・fixed | `decimal`（long double は binary128 との間で偶数丸め。fixed は桁数と scale に収まらなければ送信時 `BAD_PARAM`） |
| any・TypeCode・Object | `Corba.Any`・`Corba.TypeCode`・`Corba.IObject` |

名前は IDL の名前のまま（IDL の命名規則、7.1.1.1）。`-Wbdotnet_naming` で .NET の命名規則（型とメンバーは PascalCase、パラメーターは camelCase）にする。C# のキーワードは `@` を付け（`@class`）、`Object` のメンバーや囲む型と同じ名前には `_` を付ける。

## 実装の要点

- **関数**：`Native/Ffi.g.cs` の `LibraryImport`。止まらない短い関数（ストリームの基本型の読み書き、長さ、参照カウント）は `SuppressGCTransition`。ポインタとハンドルは `nint`
- **ハンドル**：C ABI のハンドルは `SafeHandle` の派生で持ち、GC で解放する。ネイティブ呼び出しの間は `GC.KeepAlive` で生かす
- **コールバック**：引数の書き込み、返事の読み込み、サーバントの `dispatch` などは `UnmanagedCallersOnly` の static メソッドで、C 側の `context` / `self` は `GCHandle`。コールバックの中の .NET の例外は捕まえて `RORB_CALLBACK_FAILED` を返し、C の呼び出しから戻ったところで元の例外を投げ直す
- **サーバント**：サーバントごとに1つの peer（`GCHandle`）が C ABI の `self` になる。C 側がサーバントを持っている間（`add_ref` / `remove_ref`）だけ強い参照、それ以外は弱い参照なので、同じサーバントには同じコア側サーバントが返り、POA の判定（`servant_to_id` など）が保たれる
- **参照の比較**：`Corba.Object.Equals` は `_is_equivalent`（呼び出しなし）なので、オブジェクト参照のメンバーを持つ構造体も参照先で比べられる
- **送れない値**：上限を超えた列や文字列、長さの違う配列、null は送信時に `BAD_PARAM`（COMPLETED_NO。他のバインディングと同じ）

## 例外

| 場面 | 結果 |
| --- | --- |
| サーバントが宣言されたユーザー例外を送出 | そのユーザー例外を返す |
| サーバントが `Corba.SystemException` を送出 | そのまま返す |
| サーバントがその他の例外を送出 | `UNKNOWN`（minor 0、COMPLETED_MAYBE） |
| 操作がない | `BAD_OPERATION`（minor `0x4f4d0002`、COMPLETED_NO） |
| 引数が読めない | `MARSHAL`（COMPLETED_NO） |
| 結果の書き込みで失敗 | その例外を COMPLETED_YES で返す |
| 返ってきたユーザー例外が操作で宣言されていない | `UNKNOWN`（minor `0x4f4d0001`、COMPLETED_YES） |

## DynAny

`DynamicAny` 名前空間に、CORBA 3.0 の `DynamicAny` のローカルインターフェースを IDL4 to C# の形で置いた（`IDynAny`、`IDynFixed`、`IDynEnum`、`IDynStruct`、`IDynUnion`、`IDynSequence`、`IDynArray`、`IDynValueCommon`、`IDynValue`、`IDynValueBox`、`IDynAnyFactory`）。実装はコアの DynAny（C ABI の `rorb_dynany_*`）で、他のバインディングと同じ挙動になる（[dynany.md](dynany.md)）。

- ファクトリは `orb.resolve_initial_references("DynAnyFactory")` を `DynAnyFactoryHelper.narrow` で得る。作られる DynAny は型の種類に合ったクラス（`DynStruct` など）で、`DynStructHelper.narrow` などで派生インターフェースにする
- 例外は `DynAny.InvalidValue`・`DynAny.TypeMismatch`・`DynAnyFactory.InconsistentTypeCode`・`MustTruncate`
- `insert_<型>_seq` / `get_<型>_seq` は `Omg.Types.ISequence<T>`、`get_members` などの列も `ISequence`
- `insert_val` / `get_val` は値（`Corba.IValueBase`）を入れ、読む。値でないものは `TypeMismatch`。`insert_abstract` / `get_abstract` は他のバインディングと同じく `NO_IMPLEMENT`
- `ORB` に `create_value_tc`・`create_value_box_tc`・`create_native_tc`・`create_abstract_interface_tc`・`create_local_interface_tc`、`Corba.ValueMember` と `Corba.Constants`（`VM_NONE` など）を足した

## servant manager と adapter activator

`PortableServer` に `IServantActivator`・`IServantLocator`・`IAdapterActivator`（IDL4 to C# のローカルインターフェース）と、それを実装する基底クラス `ServantActivator`・`ServantLocator`・`AdapterActivator`（`Corba.LocalObject` の派生）、`ForwardRequest` を置いた。

- `POA.set_servant_manager` / `get_servant_manager`、`POA.the_activator`（プロパティ。null で外す）。`get_servant_manager` と `the_activator` は渡したオブジェクトそのものを返す
- servant locator の `Cookie`（native）は `object` で、`preinvoke` の `out` の値がそのまま `postinvoke` に渡る
- `incarnate` / `preinvoke` が投げた `ForwardRequest` はクライアントを転送し、システム例外はそのまま返り、それ以外の例外は `UNKNOWN`
- コアは POA が持っている間だけ manager を強く参照する（サーバントと同じく `add_ref` / `remove_ref`）

## DII と DSI

`Corba` に IDL4 to C# の DII（`Request`、`NVList`、`NamedValue`、`ExceptionList`、`Environment`、`Bounds`、`UnknownUserException`、`Constants.ARG_IN` / `ARG_OUT` / `ARG_INOUT`）を、`PortableServer` に DSI の `DynamicImplementation` と `Corba.ServerRequest` を置いた。実装は C ABI の `rorb_request_*` と `rorb_server_request_*` で、決めたことは他のバインディングと同じ（[dii-dsi.md](dii-dsi.md)）。

- `obj._request(op)`、`obj._create_request(ctx, op, arg_list, result[, exclist, ctxlist])`、`orb.create_list` / `create_named_value` / `create_exception_list` / `create_environment`、`orb.send_multiple_requests_oneway` / `_deferred`、`poll_next_response`、`get_next_response`
- `Request` の `add_in_arg` などは `Any` を返す。`out` には型だけを入れる（`any.type = tc`）。`invoke`・`send_oneway`・`send_deferred`・`poll_response`・`get_response`
- 呼び出しの例外は投げ、`env.exception` にも入れる（C# の他の呼び出しと同じく例外で知らせる）。ユーザー例外は `UnknownUserException`（`except` は例外リストの TypeCode で読んだ本体の Any）
- DSI のサーバントは `DynamicImplementation` を継承して `invoke(ServerRequest)` と `_primary_interface(oid, poa)` を書く。`invoke` が投げた `UnknownUserException` は `set_exception` と同じ
- コンテキスト（null でない `ctx`、`ServerRequest.ctx()`、`get_default_context`）は `NO_IMPLEMENT`

## Portable Interceptors と IOP::Codec

`PortableInterceptor`・`IOP`・`Dynamic` に、CORBA 3.0 の Portable Interceptors のローカルインターフェースを IDL4 to C# の形で置いた（属性はプロパティ：`ri.operation`、`ri.request_id`、`interceptor.name`）。実装は C ABI の `rorb_cri_*`・`rorb_sri_*`・`rorb_ior_info_*`・`rorb_init_info_*`・`rorb_codec_*` で、挙動は他のバインディングと同じ（[portable-interceptors.md](portable-interceptors.md)）。

- インターセプターは `IClientRequestInterceptor`・`IServerRequestInterceptor`・`IIORInterceptor` を実装する（`Corba.LocalObject` を継承し、1つのクラスで複数を実装してもよい）。操作が何もしない基底クラス `ClientRequestInterceptor`・`ServerRequestInterceptor`・`IORInterceptor`・`ORBInitializer` もある
- 初期化子の登録は `PortableInterceptor.ORBInitializerRegistry.register_orb_initializer(init)`（それより後に作るすべての ORB。仕様の `register_orb_initializer`）か、その ORB だけの `Corba.ORB.init(args, orb_id, init1, init2, ...)`
- 情報のオブジェクト（`IClientRequestInfo` など）はその点の間だけ使える。後から使うと `BAD_INV_ORDER`（minor 14）、`ORB.init` の後の `IORBInitInfo` は `OBJECT_NOT_EXIST`（minor 10）
- 点が投げた `PortableInterceptor.ForwardRequest` は転送、`Corba.SystemException` はそのまま、その他は `UNKNOWN`（minor 0、COMPLETED_MAYBE）
- `orb.resolve_initial_references("PICurrent")`（`PortableInterceptor.CurrentHelper.narrow`）と `("CodecFactory")`（`IOP.CodecFactoryHelper.narrow`）。`IOP.ServiceContext`・`TaggedComponent` のオクテット列は `byte[]`（`ObjectId` と同じ）、その他の列は `ISequence<T>`
- 静的なスタブとスケルトンの要求の `arguments`・`result`・`exceptions` は `NO_RESOURCES`（minor 1）。DII と DSI の要求では使える
- `post_init` で `RootPOA` を解決すると、RootPOA が IOR インターセプターの登録より前に作られ、そのコンポーネントが入らない（コアの順序のとおり）
- `register_policy_factory` は `NO_IMPLEMENT`

## valuetype と abstract インターフェース

IDL4 to C# 7.6 が決めているのは、valuetype が抽象クラスになること（状態メンバーはプロパティで、private のものは `protected`、操作は抽象メソッド、初期化子は `[Omg.Types.Factory]` を付けた抽象の `void` メソッド）だけで、基底クラス、ファクトリ、value box、custom、abstract インターフェース、truncatable は決めていない。rustorb は C++ と Java のマッピングに倣って次のようにした。

| IDL | C# |
|---|---|
| `valuetype V` | `abstract class V : Corba.ValueBase`（具象の基底があればそれを継承）。サポートするインターフェース `I` の `IIOperations` と、abstract の基底 valuetype・abstract インターフェースのインターフェースを実装する |
| 操作も初期化子もサポートするインターフェースもない valuetype | 加えて `OBV_<Module>.V`（すべての状態メンバーを取るコンストラクタ付きの具象クラス。モジュール外なら `OBV_V`）。既定のファクトリはこれを作る |
| `abstract valuetype A` | インターフェース `IA : Corba.IValueBase` |
| `valuetype B long;`（value box） | `class B : Corba.ValueBase`。中身はプロパティ `value`（型によらず同じ形）。既定のファクトリがある |
| `custom valuetype C` | `abstract class C : Corba.ValueBase, Corba.ICustomMarshal`。実装は `marshal(Corba.DataOutputStream)` と `unmarshal(Corba.DataInputStream)` を書き、ファクトリを登録する。値は常にチャンクで送る |
| `abstract interface N` | インターフェース `IN`（操作）と、参照として受けたときのスタブ `N : Corba.Object, IN`。`NHelper.narrow(object)` は値ならそのまま、参照ならスタブにする。送るときは参照なら TRUE と参照、値なら FALSE と値（null は nil 参照） |
| `interface I`（すべて） | タイ `IPOATie : IPOA`（`IIOperations` の実装に委ねる。サポートするインターフェースの valuetype をサーバントにするのに使う） |

- ファクトリは `Corba.IValueFactory`（`create_for_unmarshal()`。C++ と同じく、ORB はできたインスタンスに状態を読み込む）。`new Corba.ValueFactory(() => new MyV())` で関数から作れる。`orb.register_value_factory(id, f)`（置き換えたものを返す）・`unregister_value_factory`・`lookup_value_factory` はプロセスで1つの表で、登録したものがなければ生成コードの既定のファクトリを返す
- 初期化子のある valuetype は `lookup_value_factory(id).create_for_unmarshal()` で作ったインスタンスの初期化子（`init(...)` など）を呼ぶ
- 生成コードはモジュール初期化子（`[ModuleInitializer]`）で既定のファクトリと valuetype の TypeCode を登録する。TypeCode は、基底の型として送られた派生の値を any の中で読むときにコアが使う
- 同じメッセージで同じインスタンスを2度書くと間接参照になり、読む側でも1つのインスタンスになる（循環も保つ）。any に入れたインスタンスはその any から取り出すと同じインスタンスで、DII の引数のように複数の any に入れても1つのまま（Java の `Hosts` と同じ仕組み）
- 受け取った値の型にファクトリがなければ（truncatable の基底のファクトリもなければ）`MARSHAL`
- `Corba.Any` に `insert_Value(v, tc)` と `extract_Value()` を足した

## AMI

コールバック型の AMI（[ami.md](ami.md)）を `-bcsharp -Wbami` で生成する。形は Java のマッピングと同じにした（IDL4 to C# には AMI の写し方の定めがない）。

- `sendc_<op>(handler, in と inout の引数)` と属性の `sendc_get_<attr>` / `sendc_set_<attr>` はスタブクラス（`Demo.Calc`。インターフェースとその基底の分）に置き、`ICalc` / `ICalcOperations` には足さない（サーバントや DII のアダプターが実装しなくてよいように）。使う側は `((Demo.Calc)calc).sendc_add(h, 1, 2)` のようにスタブにキャストする
- `AMI_<I>Handler` は普通のインターフェースとして生成する（`IAMI_CalcHandlerOperations`・`AMI_CalcHandlerPOA`・タイ `AMI_CalcHandlerPOATie` など）。例外は `<op>_excep(Messaging.ExceptionHolder)` に届く
- 生成した `sendc_` は `Rustorb.Ami.Sendc`（C ABI の `rorb_object_sendc`）を呼ぶ。in の引数は呼んだ時点で写し取る。Handler は返事用のプールのスレッドで呼ばれ、Handler が投げた例外は捨てる。nil の Handler なら返事は読んで捨てる。タイムアウトはスタブの `_call_timeout_us` がそのまま効く
- `Messaging.ReplyHandler` 一式は生成したもの（`dotnet/generate-stubs.sh`）、`Messaging.ExceptionHolder` は手で書いた valuetype（`Rustorb/Ami.cs`）。`raise_exception()`・`raise_exception_with_list(IList<Corba.TypeCode>)` がユーザー例外またはシステム例外を投げる。ユーザー例外は、生成コードがモジュール初期化子で `Rustorb.UserExceptions.Register` に登録した読み書きの関数でリポジトリ ID から戻す。登録のない ID は `UNKNOWN`（minor 0x4f4d0001、`COMPLETED_YES`）
- `ExceptionHolder` は valuetype なので、Handler を別のプロセスに置いても届く（状態メンバーの `marshaled_exception` は C ABI の `rorb_cdr_encode` で書いた CDR）

## local インターフェースと IDL 4 の構文

- `local interface L` は、`IL`（`ILOperations` と、基底の `I…` を継承）と、実装の基底になる抽象クラス `L : Corba.LocalObject, IL`（すべての操作と属性が抽象メンバー。中で定義した型と定数も持つ）にする。スタブ、スケルトン、タイはない。`LHelper.narrow(object)` は `IL` ならそのまま、ほかは `BAD_PARAM`。ローカルなオブジェクトは送れないので、`LHelper.write`・`read`（any への `insert` も）は `MARSHAL`（minor 4）。TypeCode は `tk_local_interface`
- `bitmask` は `[Flags]` の enum（基になる型は `@bit_bound` に合わせて `byte`・`ushort`・`uint`・`ulong`）。`bitset` は bitfield ごとのプロパティを持つ struct（値で比べる）。どちらも C++ のマッピングと同じく、ビットを収める符号なし整数として送り、TypeCode もその整数のもの。符号付きの bitfield は読むときに符号を広げる。bitset の継承はエラー
- `int8` は `sbyte`。GIOP に int8 の TypeCode はないので、C++ のマッピングと同じく octet の TypeCode にした
- 構造体と例外のメンバーの `@default` は既定のコンストラクタでの初期値、`@range` / `@min` / `@max` は送るときに外れていれば `BAD_PARAM`、読むときに外れていれば `MARSHAL`（C++ と同じ）

## テスト

- `dotnet/tests/Rustorb.Tests`（`dotnet test dotnet/tests/Rustorb.Tests`。cargo で `librustorb_ffi` を、`rustorb-idl -bcsharp` でテスト用 IDL のコードを作る）
  - `EchoTests`：相互接続の `InteropTest.idl` の全操作（基本型、構造体、union、配列、列、パラメーターの向き、ユーザー例外とシステム例外、属性、oneway、参照、コールバック、Any）を、文字列化した参照を通して同じプロセスで呼ぶ
  - `DynAnyTests`：`AnyTest.idl` の型での DynAny（構造体、union、列、enum、例外、同一性）
  - テストのクラスは順に走らせる（各 fixture がプロセスの ORB を `init` し、終わりに `destroy` するので、並行だと他のクラスの ORB を壊す）
  - `ServantManagerTests`：servant activator（incarnate、例外、ForwardRequest、etherealize）、servant locator（cookie）、adapter activator
  - `DiiTests`：静的なサーバントと DSI のサーバントへの DII（`out`、同じ要求の再送、ユーザー例外とリストにないもの、deferred と `get_next_response`、oneway）、静的なスタブから DSI のサーバント、`NVList` と `ExceptionList`、`_create_request`
  - `PiTests`：初期化子（`pre_init` / `post_init`、`DuplicateName`、段階ごとの `resolve_initial_references`）、サービスコンテキストの往復と PICurrent のスロット、IOR インターセプターのコンポーネント、サーバーのインターセプターのシステム例外、`ForwardRequest`、DII の `arguments`、点の後の情報、`Codec`
  - `ValueTests`：`ValueTest.idl`・`CustomTest.idl`・`AbstractTest.idl` の値（null、truncatable の派生、共有と循環、value box、ファクトリと初期化子、タイ、ファクトリのない型の `MARSHAL`、any の中の値と同一性、DynAny の `insert_val` / `get_val`）、custom の値、abstract インターフェース（参照と値）
  - `AmiTests`：`InteropTest.idl` の `sendc_`（戻り値、inout と out、ユーザー例外とシステム例外の `ExceptionHolder`、属性、nil の Handler）。Handler は `DispatchProxy` をタイに渡したもの
  - `Idl4Tests`：C++ の `test_idl4.cc` と同じ `cpp/tests/idl/Idl4.idl`（bitmask、bitset、int8、`@default`、`@range`、any の中の bitset と bitmask）
  - `LocalTests`：`dotnet/tests/Rustorb.Tests/idl/LocalTest.idl` の local インターフェース（基底の操作、属性、ユーザー例外、`_is_a`、`narrow`、送れないこと）
  - `MappingTests`：C++・Java と同じ `cpp/tests/idl/CxxMapping.idl` の定数、再帰型、すべての判別子の union、入れ子の型、キーワード、fixed と long double、インターフェースの多重継承、オブジェクト参照の列、TypeCode と Any
- rustorb-idl の `python/tests/test_csharp_backend.py`：生成物の形、`-Wbdotnet_naming`、対応しない構文のエラー、生成コードとランタイムのコンパイル（警告をエラーにする）。golden は `tests/golden/csharp`
- CI（`ci.yml` の `dotnet`）：Linux、macOS、Windows で `dotnet test`。Linux では同梱の CosNaming のスタブが生成器の出力と、`Ffi.g.cs` が `rustorb.h` と一致することも確かめる
- 相互接続（[#179](https://github.com/rustorb/rustorb/issues/179)）：`interop/run-matrix.sh` に `rustorbcs`（`interop/impls/rustorbcs`。ホストで `dotnet publish` し、イメージの .NET 8 ランタイムで動かす）を加えた。basic（InteropTest.idl）、any（AnyTest.idl。`interop/impls/rustorbcs/src/AnyTests.cs`）、value・custom・abstract（`ValueTests.cs`・`CustomTests.cs`・`AbstractTests.cs`）のすべてに入る。ローカルで確認した結果は次のとおりで、失敗はすべて `known-failures.txt` の範囲
  - basic（79ケース）：omniORBpy・omniORB C++・JacORB・rustorb の Rust・Python・C++・.NET 自身と両方向で通る。失敗は omniORBpy の wchar（3ケース）と JacORB の union の判別子（`echo_value[5]`）だけで、他の rustorb のバインディングと同じ
  - GIOP 1.1、`--fragment 1024`、`--ssl`、`--unix` でも omniORB・omniORBpy・自身との組（GIOP 1.1 と Fragment は JacORB とも）が同じ結果（GIOP 1.1 の omniORB 側の wchar / wstring は既知）
  - Java（`rustorbjava`）との組はローカルに Java 22 がなく CI（`interop.yml`）で確かめる
- AMI プローブ（`run-ami-probe.sh`）：`rustorbcs`（`interop/impls/rustorbcs/src/AmiProbe.cs`）の AMI クライアントから omniORB C++・omniORBpy・JacORB・rustorb（Rust、.NET）のサーバーへ、すべて期待出力と一致する。rustorbpy・rustorbcpp のクライアントから `rustorbcs` のサーバーへも同じ。Java との組は CI で確かめる
- DynAny プローブ（`run-dynany-probe.sh`）：`rustorbcs`（`interop/impls/rustorbcs/src/DynAnyProbe.cs`）の出力は rustorb の期待出力と一致する
- naming check（`run-naming-check.sh`。omniNames と rustorb-names、再起動を含む）と wire probe（`run-wire-probe.sh`）は、`rustorbcs` でも期待出力と一致する。POA プローブ（`run-poa-probe.sh`）の `rustorbcs`（`interop/impls/rustorbcs/src/PoaProbeServer.cs`）の出力も rustorb の期待出力と一致する
- 相互接続：`interop/impls/rustorbcs-dii` は `rustorbcs` のクライアントとサーバーを、スタブの呼び出しを DII に、サーバントを DSI に差し替えて動かす（`interop/impls/rustorbcs/src/DiiAdapters.cs`、環境変数 `RUSTORB_DII=1`）。basic（GIOP 1.2 / 1.1）で、omniORBpy・omniORB C++・JacORB・rustorb（Rust、Python、C++、.NET、Python と C++ の DII）との組が両方向で `rustorbcs` と同じ結果になる（失敗は既知のものだけ）。CI（`interop.yml`）でも回す
- PI プローブ（`run-pi-probe.sh`）：`rustorbcs`（`interop/impls/rustorbcs/src/PiProbeMain.cs`）が rustorb の Rust・Python・C++・.NET、JacORB、omniORBpy、omniORB C++ との組で GIOP 1.2 / 1.1 / 1.0 とも期待どおり（JacORB のクライアントは仕組みどおり 1.2 のみ）。Java との組は CI で確かめる
- any（`run-matrix.sh --suite any`）：omniORBpy・omniORB C++・rustorb の Rust・Python・C++・.NET 自身と両方向で、失敗は `known-failures.txt` の long double だけ（omniORB 側のものと、.NET の `decimal` が double の 0.1 を28桁までしか持てず、値をビットで比べる Rust と合わないもの）。`fixed<31,4>` の値は `decimal` に収まらないので、表の値は `DynFixed` の文字列で作り、比べる
  - value（26ケース）・abstract（12ケース）：omniORBpy・omniORB C++・JacORB・rustorb の Rust・Python・C++・.NET 自身と両方向で、失敗は `known-failures.txt` の JacORB のものだけ。custom（7ケース）：JacORB・rustorb の Rust・Python・C++・.NET 自身と両方向で、失敗は JacORB の既知のものだけ（custom のスイートはもともと omniORB と組まない）。value は GIOP 1.1 でも同じ結果
