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
- `insert_val` / `get_val` は valuetype の対応（#186 の後の段）までは `NO_IMPLEMENT`。値は `from_any` / `to_any` で扱える。`insert_abstract` / `get_abstract` は他のバインディングと同じく `NO_IMPLEMENT`
- `ORB` に `create_value_tc`・`create_value_box_tc`・`create_native_tc`・`create_abstract_interface_tc`・`create_local_interface_tc`、`Corba.ValueMember` と `Corba.Constants`（`VM_NONE` など）を足した

## まだないもの

valuetype、abstract / local インターフェース（servant manager などのローカルインターフェースを含む）、DII / DSI、Portable Interceptors、AMI、IDL 4 の bitmask / bitset は [#186](https://github.com/rustorb/rustorb/issues/186) で足す。`-bcsharp` はこれらの構文をエラーにする。

## テスト

- `dotnet/tests/Rustorb.Tests`（`dotnet test dotnet/tests/Rustorb.Tests`。cargo で `librustorb_ffi` を、`rustorb-idl -bcsharp` でテスト用 IDL のコードを作る）
  - `EchoTests`：相互接続の `InteropTest.idl` の全操作（基本型、構造体、union、配列、列、パラメーターの向き、ユーザー例外とシステム例外、属性、oneway、参照、コールバック、Any）を、文字列化した参照を通して同じプロセスで呼ぶ
  - `DynAnyTests`：`AnyTest.idl` の型での DynAny（構造体、union、列、enum、例外、同一性）
  - テストのクラスは順に走らせる（各 fixture がプロセスの ORB を `init` し、終わりに `destroy` するので、並行だと他のクラスの ORB を壊す）
  - `MappingTests`：C++・Java と同じ `cpp/tests/idl/CxxMapping.idl` の定数、再帰型、すべての判別子の union、入れ子の型、キーワード、fixed と long double、インターフェースの多重継承、オブジェクト参照の列、TypeCode と Any
- rustorb-idl の `python/tests/test_csharp_backend.py`：生成物の形、`-Wbdotnet_naming`、対応しない構文のエラー、生成コードとランタイムのコンパイル（警告をエラーにする）。golden は `tests/golden/csharp`
- CI（`ci.yml` の `dotnet`）：Linux、macOS、Windows で `dotnet test`。Linux では同梱の CosNaming のスタブが生成器の出力と、`Ffi.g.cs` が `rustorb.h` と一致することも確かめる
- 相互接続（[#179](https://github.com/rustorb/rustorb/issues/179)）：`interop/run-matrix.sh` に `rustorbcs`（`interop/impls/rustorbcs`。ホストで `dotnet publish` し、イメージの .NET 8 ランタイムで動かす）を加えた。いまは basic（InteropTest.idl）だけで、any・value・custom・abstract のマトリクスには入れない（#186）。ローカルで確認した結果は次のとおりで、失敗はすべて `known-failures.txt` の範囲
  - basic（79ケース）：omniORBpy・omniORB C++・JacORB・rustorb の Rust・Python・C++・.NET 自身と両方向で通る。失敗は omniORBpy の wchar（3ケース）と JacORB の union の判別子（`echo_value[5]`）だけで、他の rustorb のバインディングと同じ
  - GIOP 1.1、`--fragment 1024`、`--ssl`、`--unix` でも omniORB・omniORBpy・自身との組（GIOP 1.1 と Fragment は JacORB とも）が同じ結果（GIOP 1.1 の omniORB 側の wchar / wstring は既知）
  - Java（`rustorbjava`）との組はローカルに Java 22 がなく CI（`interop.yml`）で確かめる
- DynAny プローブ（`run-dynany-probe.sh`）：`rustorbcs`（`interop/impls/rustorbcs/src/DynAnyProbe.cs`）の出力は rustorb の期待出力と一致する
- naming check（`run-naming-check.sh`。omniNames と rustorb-names、再起動を含む）と wire probe（`run-wire-probe.sh`）は、`rustorbcs` でも期待出力と一致する。POA プローブは servant manager（ローカルインターフェース）が要るので #186 で加える
