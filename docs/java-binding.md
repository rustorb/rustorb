# Java バインディング（JNI + IDL to Java）

Oct 29, 2026 · @ysuga · M6

rustorb の Java バインディングの構成をまとめる。C++ と同じく、Rust コアの `extern "C"` の安定 ABI 層（`crates/rustorb-ffi`）の上に載せる。Java からは JNI のライブラリ `librustorb_jni`（`crates/rustorb-jni`）を通して C ABI を呼ぶ。マッピングは [IDL to Java](https://www.omg.org/spec/I2JAV/1.3)（v1.3）に従い、ランタイム（`java/`）は Java 11 以降で動く。Java 11 で JDK から `org.omg` が外れたため、`org.omg.CORBA`・`org.omg.PortableServer`・`org.omg.CosNaming` もランタイムが持つ。M6 の完了条件は「生成コードで書いた Java のクライアント・サーバーが JacORB と相互接続できること」で、`interop/run-matrix.sh` の `rustorbjava` と `jacorb` で判定する。

既存 ORB のソースは読まない方針（[development-plan.md](development-plan.md)）は JacORB にも適用する。Java の API は仕様書と標準の javadoc（`org.omg.CORBA` などのクラスとメソッドの一覧）だけから書き、JacORB はリリースの jar を動かした結果だけを使った。

## 構成

| 場所 | 内容 | ライセンス |
| --- | --- | --- |
| `crates/rustorb-jni` | JNI ライブラリ（`librustorb_jni.so` / `.dylib` / `rustorb_jni.dll`）。`org.rustorb.Native` のネイティブメソッドで、1つずつ C ABI の関数を呼ぶ | MIT OR Apache-2.0 |
| `java/src/main/java/org/omg` | 標準の API（`CORBA`、`CORBA.portable`、`CORBA.TypeCodePackage`、`PortableServer` とその Package、`CosNaming`） | MIT OR Apache-2.0 |
| `java/src/main/java/org/rustorb` | 実装（ORB、Delegate、ストリーム、Any / TypeCode、POA）と、生成コードとの取り決め（`Stubs`、`Call`、`Anys`） | MIT OR Apache-2.0 |
| `java/src/main/java/org/omg/CosNaming` | `rustorb-idl -bjava -Wbpackage=org.omg` で生成した CosNaming（`java/generate-stubs.sh`） | MIT OR Apache-2.0 |
| `crates/rustorb-codegen/python/rustorb_idl/java_backend.py` | `rustorb-idl -bjava` | GPL-3.0-or-later（生成物は対象外） |
| `java/pom.xml`、`java/src/test` | Maven のビルドと JUnit のテスト | MIT OR Apache-2.0 |
| `interop/impls/java` | 相互接続用のクライアント・サーバーと試験ケース（`rustorbjava` と `jacorb` で共通のソース） | MIT OR Apache-2.0 |
| `interop/impls/rustorbjava`、`interop/impls/jacorb` | それぞれのビルドと起動スクリプト。JacORB は Maven Central のリリース jar（3.9）を相互接続用のイメージに入れる | MIT OR Apache-2.0 |

## 使い方

```sh
rustorb-idl -bjava -C gen Example.idl           # gen/<module>/*.java
cargo build --release -p rustorb-jni            # target/release/librustorb_jni.so
mvn -f java/pom.xml -Didl.parser.dir=path/to/idl_parser package   # java/target/rustorb-*.jar
javac --release 11 -cp java/target/rustorb-0.0.0-SNAPSHOT.jar -d classes $(find gen src -name '*.java')
java -Drustorb.jni.library=target/release/librustorb_jni.so \
     -cp java/target/rustorb-0.0.0-SNAPSHOT.jar:classes Main
```

ライブラリは、システムプロパティ `rustorb.jni.library`、環境変数 `RUSTORB_JNI_LIBRARY` のパス、`java.library.path` の `rustorb_jni` の順に探す。`-Wbpackage=PKG` はすべてのパッケージを `PKG` の下に置く（モジュールの外の定義は `PKG` に入る。名前のあるパッケージからは無名パッケージを参照できないため、モジュールの外に定義がある IDL では必要になる）。

```java
ORB orb = ORB.init(args, null);
POA poa = POAHelper.narrow(orb.resolve_initial_references("RootPOA"));
poa.the_POAManager().activate();
Echo ref = EchoHelper.narrow(poa.servant_to_reference(new EchoImpl()));  // EchoImpl extends EchoPOA
System.out.println(orb.object_to_string(ref));
orb.run();
```

`ORB.init(args, props)` は ORB のオプション（`-ORBInitRef`、`-ORBendPoint` など、他のバインディングと同じもの）を受け、`props` の `ORBxxx`（`org.omg.CORBA.ORBxxx` も可）を `-ORBxxx 値` として足す。`ORBInitialHost` / `ORBInitialPort` は NameService の初期参照になる。引数なしの `ORB.init()` は TypeCode と Any を作るためのシングルトンで、通信はしない。

## JNI と C ABI の対応

- **ハンドル**：C ABI のハンドルは Java の `long` で受け渡す。Java のオブジェクトは `java.lang.ref.Cleaner` で解放を登録し、ネイティブ呼び出しの間は `Reference.reachabilityFence` で生かす
- **エラー**：失敗した C 呼び出しは、`rorb_exception` に対応する Java の例外を投げる（`org.rustorb.Bridge` の static メソッドが作る）。システム例外は40種の `org.omg.CORBA` のクラスで、POA・ORB の例外は C ABI のローカル例外コードから変換する。Rust の panic は `INTERNAL`
- **呼び出し**：`-bjava` のスタブは操作ごとのローカルクラス（`org.rustorb.Call` の派生）に引数の書き込み、返事の読み込み、ユーザー例外の読み込みを持たせ、`Stubs.invoke` で `rorb_object_invoke` を呼ぶ。他の IDL コンパイラが生成した標準の形（`_request` / `_invoke` / `_releaseReply`）のスタブも動く（下の「portable stub とストリーム」）
- **スケルトン**：標準どおり `FooPOA extends Servant implements InvokeHandler` の `_invoke(op, in, ResponseHandler)`。`createExceptionReply()` のストリームは、最初に書かれた文字列を例外のリポジトリIDとして `rorb_server_reply_user_exception` を呼ぶ。クライアント側の例外のストリームは最初の `read_string` でリポジトリIDを返すので、生成された `Helper.read` がそのまま使える
- **サーバント**：Java のサーバントごとに1つの peer（Rust 側の `Box`）を持ち、それが C ABI の `self` になる。peer はサーバントを弱参照し、C 側がサーバントを持っている間（`add_ref` / `remove_ref`）だけ強い参照（GlobalRef）も持つ。同じサーバントには同じコア側サーバントが返るので、POA の判定（`servant_to_id` など）が保たれる
- **スレッド**：アップコール用スレッドプールのスレッドは、最初に Java を呼ぶときにデーモンとして JVM にアタッチし、スレッドの終了で自動的にデタッチする。アップコールはローカル参照のフレームの中で実行する
- **文字列**：`string` は UTF-8 で、`wstring` は Java の `String` の UTF-16 の単位をそのまま渡す。`char` は C ABI と同じく1バイト（ISO 8859-1 の範囲）で、範囲外は送信時 `DATA_CONVERSION`

## portable stub とストリーム

他の IDL コンパイラ（JacORB の IDL コンパイラなど）が生成したコードは、標準の portable stub の形で呼ぶ。`_request(op, responseExpected)` が返すストリームに引数を書き、`_invoke(out)` が返事のストリームを返し（ユーザー例外は `ApplicationException`）、`_releaseReply(in)` で手放す。Helper は `Any.type(t)` と `create_output_stream()` / `create_input_stream()` で Any に値を出し入れする。C ABI のストリームはコールバックの中でだけ有効なので、次のようにした。

- **出力**：`_request`、`ORB.create_output_stream()`、`Any.create_output_stream()` のストリーム（`org.rustorb.PortableOutputStream`）は、書かれた内容をバイト列にせず、書き込みの呼び出しの列として持つ。`_invoke` はそれを `rorb_object_invoke` と同じ引数のコールバックで本物の要求のストリームに書き直すので、接続ごとに決まる文字コード（omniORB には ISO 8859-1）、`wchar` の交渉、GIOP 1.0 / 1.1 の本体のアラインメント、valuetype の共有（同じインスタンスは indirection）がそのまま正しくなる。配列は書いた時点で写し、`write_any` は Any の値をその時点で写す。valuetype は送るときの状態を書く
- **入力**：C ABI に、呼び出しの後も有効な「所有する入力ストリーム」（`rorb_in_release` まで有効）を加えた。`rorb_object_invoke_stream`（返事、ユーザー例外はその ID と本体）、`rorb_in_new`（コールバックが標準のストリームに書いたもの。ネイティブの文字コード）、`rorb_any_input`（Any の値）。Java では `org.rustorb.OwnedInputStream` で、読むたびにエラーを確かめて投げる（コールバックのストリームはコールバックが戻るときにまとめて報告する）。`_releaseReply` か GC で解放する
- **ApplicationException**：ストリームは例外の本体の位置にあり、最初の `read_string` が例外のリポジトリIDを返す。生成された `Helper.read` はそのまま使える。コアが LOCATION_FORWARD を自分で辿るので `RemarshalException` は出さない。`_is_local()` は偽、`_servant_preinvoke` は null で、同じ ORB の中の呼び出しもコアの同一 ORB 内呼び出しになる
- **Any**：`read_value(in, tc)` は rustorb のストリームから型 tc の値を読み（`rorb_in_read_any_value`）、`write_value(out)` は TypeCode なしで値を書く（`rorb_out_write_any_value`）。`create_input_stream()` は値の上のストリーム（例外はリポジトリIDから）。`type(t)` は同等でない型なら値のない状態にし（その後の取り出しは `BAD_OPERATION`）、次の insert・`read_value`、または `create_output_stream()` のストリームに書かれたもの（次に Any を使うときにその型の値として読む）が値になる。`insert_Value(v)` は `StreamableValue` の `_type()` の TypeCode、`insert_Value(v, tc)` は tc（value box は生成された `<Box>Helper` で書く）、`extract_Value()` は valuetype と value box の値を返す
- **スケルトン**：スケルトンの `_invoke` が `createReply()` の後に送出した `SystemException` は、従来は返事の書き込みの失敗として COMPLETED_YES にしていた。操作を呼ぶ前に返事を作るスケルトン（JacORB の生成コード）では操作の例外まで YES になるので、例外のスタックで `_invoke` のすぐ上がサーバントのクラスのメソッド（操作）なら、例外自身の完了状態を返すようにした
- `org.omg.CORBA.portable.ServantObjectExt` を足した（JacORB の生成コードが参照する。rustorb は使わない）

## 値の表現

| IDL | Java |
| --- | --- |
| `short`・`long`・`long long`（`unsigned` も） | `short`・`int`・`long`（符号なしは同じ型にビット列のまま） |
| `octet`、`boolean`、`char`、`wchar` | `byte`、`boolean`、`char`、`char` |
| `float`、`double` | `float`、`double` |
| `string`、`wstring` | `String`。上限付きは送信時に長さを検査して `BAD_PARAM`、受信時 `MARSHAL` |
| `sequence<T>`、配列 | `T[]`（多次元配列は `T[][]`）。上限と配列の長さも同じく検査する |
| struct・例外 | 公開フィールドと、引数なし・全メンバーのコンストラクタを持つクラス（例外は `reason` 付きのコンストラクタも） |
| union | `discriminator()`、メンバーの取得・設定（複数のラベルを持つメンバーと default は判別子付きの設定も）、`__default()`。選ばれていないメンバーの取得は `BAD_OPERATION` |
| enum | `value()`、`from_int()`、`_NAME` 定数、`NAME` のインスタンスを持つクラス |
| `any`、`TypeCode` | `org.omg.CORBA.Any`、`org.omg.CORBA.TypeCode` |
| `fixed<D, S>` | `java.math.BigDecimal`。`out` / `inout` は `org.omg.CORBA.FixedHolder` |
| valuetype・value box | `StreamableValue` のクラスと `DefaultFactory`、基本型の箱は `value` を持つクラス、それ以外の箱は中身の型（[valuetype.md](valuetype.md)） |
| インターフェース | シグネチャ（`Foo`）と操作（`FooOperations`）のインターフェース、`_FooStub`、`FooHelper`（`narrow`、`unchecked_narrow`）、`FooHolder`、`FooPOA`、`FooPOATie`。nil は `null` |
| 定数 | モジュールの定数は `value` を持つインターフェース、インターフェースの定数はそのフィールド |
| 型の中の型 | `<Name>Package` パッケージ |

`Helper` は `insert`・`extract`・`type`・`id`・`read`・`write` を持つ。`type()` は再帰型を `create_recursive_tc` で作り、別名の `insert` は別名の TypeCode を付ける。Java の予約語と `java.lang.Object` のメソッド名（`notify`、`wait` など）と同じ識別子には `_` が付く（GIOP 上の操作名は IDL のまま）。`long double` は Java の `double`（[long-double.md](long-double.md)）。abstract interface は [valuetype.md](valuetype.md#abstract-interface) のとおり。`custom` の valuetype（`CustomValue`）、`supports`（`IOperations` と `IPOATie`）、valuetype の操作（抽象メソッド）と初期化子（`<名前>ValueFactory`）は [valuetype.md](valuetype.md) のとおり。

`fixed<D, S>` の値は `BigDecimal` で、生成コードは `write_fixed(v, D, S)` / `read_fixed(D, S)` で読み書きする。読んだ値の scale はいつも S。小数が S 桁を超える（0 でない桁がある）値や整数部が D − S 桁を超える値は送信時に `BAD_PARAM`（Python と同じ。丸めない）。パック10進の読み書きは `org.omg.CORBA.portable.OutputStream` / `InputStream` 自身が `write_octet_array` / `read_octet_array` の上で行う。桁数を取らない `write_fixed(v)` は v の桁数と scale で書き、`read_fixed()` は符号の半オクテットまで読んで整数（scale 0）を返す。`Any.insert_fixed(v)` は v 自身の桁数と scale の `tk_fixed` を、`insert_fixed(v, tc)` は tc（別名も可。`tk_fixed` でなければ `BAD_INV_ORDER`）を付け、`extract_fixed()` は TypeCode の scale の値を返す。`fixed` の定数は `BigDecimal` で、scale は型のもの（`const fixed C = 12.50d;` は `new BigDecimal("12.50")`）。

## 例外

| 場面 | 結果 |
| --- | --- |
| サーバントが宣言されたユーザー例外を送出 | そのユーザー例外を返す |
| サーバントが `SystemException` を送出 | そのまま返す |
| サーバントがその他の例外を送出 | `UNKNOWN`（minor 0、COMPLETED_MAYBE） |
| 操作がない | `BAD_OPERATION`（minor `0x4f4d0002`、COMPLETED_NO。C++ と同じ） |
| 結果の書き込みで失敗（上限超えなど） | その例外を COMPLETED_YES で返す（操作は実行済み） |
| 返ってきたユーザー例外が操作で宣言されていない | `UNKNOWN`（minor `0x4f4d0001`、COMPLETED_YES） |

## POA の範囲

`create_POA`（7種のポリシー）、`find_POA`、`destroy`、`activate_object(_with_id)`、`deactivate_object`、`servant_to_id/reference`、`id_to_servant/reference`、`reference_to_id/servant`、`create_reference(_with_id)`、デフォルトサーバント（`get_servant` / `set_servant`）、servant manager（`set_servant_manager` / `get_servant_manager`）、`the_activator`、`POAManager` の4状態、`PortableServer::Current`。

servant manager と adapter activator は `org.omg.CORBA.LocalObject` を継承して `ServantActivator`・`ServantLocator`・`AdapterActivator` を実装する（CORBA 3 の Java マッピングのローカルインターフェース）。各メソッドは ORB のアップコールのスレッドで呼ばれる。

- `incarnate` / `preinvoke` は `org.omg.PortableServer.ForwardRequest` で転送でき、`SystemException` はそのままクライアントに返る。null のサーバントは `OBJ_ADAPTER`、その他の例外は `UNKNOWN`（標準エラーに出す）
- cookie は `ServantLocatorPackage.CookieHolder` に入れた任意の Java オブジェクトで、`postinvoke` に渡る
- `get_servant_manager` と `the_activator()` は設定した Java オブジェクトそのものを返す。C 側はグローバル参照を数えて持ち、POA が手放すと解放する
- `etherealize`・`postinvoke`・`unknown_adapter` が送出した例外は標準エラーに出して捨てる

`_this()` は、そのサーバントの要求の実行中ならその要求のオブジェクトを、そうでなければ `_default_POA()`（最初に初期化して破棄していない ORB の RootPOA）で暗黙に活性化した参照を返す。同じ ORB の中の呼び出しは、コアの同一 ORB 内呼び出しで呼び出し元のスレッドで実行する。

## テスト

- `java/src/test`（`mvn -f java/pom.xml -Didl.parser.dir=... verify`。cargo で `librustorb_jni` を、`rustorb-idl -bjava` でテスト用 IDL のコードを作る）
  - `MappingTest`：C++ と同じ `cpp/tests/idl/CxxMapping.idl`（`-Wbpackage=mapping`）の定数、union の規則、すべての型を、同じプロセスの2つの ORB の間と同一 ORB 内で往復させる
  - `PoaTest`：C++ の `test_poa.cc` と同じ内容（ServantActivator・ServantLocator・AdapterActivator、転送と例外、ポリシーの検査）を、2つの ORB の間と同一 ORB 内で確かめる
  - `InteropCasesTest`：相互接続の78ケース（`interop/impls/java/interop/Cases.java`）と Any / TypeCode の251ケース（DynAny の組み立て直しを含む）（`AnyTests.java`）を、2つの ORB の間で回す
  - `DynAnyTest`：`org.omg.DynamicAny`（[dynany.md](dynany.md)）。生成コードの型の DynAny、valuetype の共有と循環、切り詰め。残りの操作は `interop/impls/rustorbjava/probe/interop/DynAnyProbe.java`（`run-dynany-probe.sh`）で確かめる
  - `PortableTest`：手書きの portable stub（`_request` / `_invoke` / `ApplicationException` / oneway）を2つの ORB の間と同一 ORB 内で、`_releaseReply` の後のストリーム、返事を作ってから操作を呼ぶスケルトンの例外の完了状態、`ORB.create_output_stream` の読み戻し、Any のストリーム操作と `insert_Value` / `extract_Value`
  - `ValuetypeTest`：valuetype の26ケース（`ValueTests.java`）、生成された既定のファクトリ、truncatable の切り捨て、ファクトリのない型の `MARSHAL`、`create_value_tc`
- `crates/rustorb-codegen/python/tests/test_java_backend.py`：コマンドライン、生成物の形、対応しない構文のエラー、生成コードとランタイムの `javac -Werror` でのコンパイル
- CI（`ci.yml` の `java`）：Linux（Java 11 と 21）、macOS、Windows で `mvn verify`。Linux では同梱の CosNaming のスタブが生成器の出力と一致することも確かめる
- 相互接続：`interop/run-matrix.sh` に `rustorbjava`（ホストでビルドし、イメージの Java 17 で動かす）と `jacorb`（イメージの中で JacORB の IDL コンパイラで同じソースをビルドする）を加えた。Java が関わる24組（omniORBpy・omniORB C++・JacORB・rustorb の Rust・Python・C++・Java）の結果は次のとおり（ローカルで確認。失敗はすべて `known-failures.txt` の範囲）
  - basic（78ケース）：omniNames と rustorb-names、GIOP 1.2 と 1.1 で、`rustorbjava` はすべての相手と通る（omniORB 側の wchar の既知の失敗を除く）。`jacorb` との組は、下の union の判別子の1ケース（`echo_value[5]`）と、GIOP 1.1 で omniORB のクライアントが分割したメッセージ（JacORB が GIOP 1.1 の Fragment を受け付けない）だけが失敗する
  - any（190ケース）：`rustorbjava` は omniORBpy・omniORB C++・rustorb（Rust・Python・C++）・自身とのすべての組で GIOP 1.2 / 1.1 とも通る。`jacorb` は下の Helper の違いで大半の `make` / `check` が失敗するため、既定では any のマトリクスに入れない（組を指定すれば回せる）
  - value（26ケース）：`rustorbjava` と `jacorb` はすべての相手と GIOP 1.2 / 1.1 とも通る。ただし JacORB は Java 17 で値の入った Any を書けない（[valuetype.md](valuetype.md)）ので、`jacorb` の組の `echo_any` の3ケースは既知の失敗

- portable stub：`interop/impls/rustorbjava-portable` は、`jacorb` と同じ共通のソースと JacORB の IDL コンパイラが生成したスタブ・スケルトン・Helper（相互接続のイメージから取り出す）を、JacORB ではなく rustorb の Java ランタイムでビルドしたもの。対になる全実装（omniORBpy・omniORB C++・JacORB・rustorb の Rust・Python・C++・Java と自身）と両方向で回した結果（ローカルで確認）
  - basic（78ケース）：すべての組で、JacORB の union の判別子の1ケース（`echo_value[5]`、JacORB 自身と同じ）だけが失敗する
  - value（26ケース）：`echo_any[derived]` だけが失敗する。JacORB の生成コードの違い（下）による
  - basic を GIOP 1.1 でも回し、同じ結果
  - any：JacORB 自身の組と同じく Helper の違いで多くの `make` / `check` が失敗するので、CI では回さない。相手ごとの失敗するケースは `jacorb` の組と一致する。違いは2種類で、`jacorb` のクライアントだけが失敗する DynAny の16ケース（JacORB の DynAny の問題）と、`rustorbjava-portable` のクライアントだけが失敗する `dyn_make[count]`（`CountHelper.insert` が別名を落とした比較用の値に対し、rustorb の DynAny は受け取った別名の TypeCode を保つ）
  - `Any.insert_Object(o)` の TypeCode の名前を、空からリポジトリIDの最後の名前（`IDL:AnyTest/AnyEcho:1.0` なら `AnyEcho`）に変えた。JacORB の `AnyEchoHelper.insert` はこれを使い、Helper の `type()` と `equal()` で比べられる
- POA プローブ：`interop/impls/rustorbjava/probe/interop/PoaProbeServer.java` で `run-poa-probe.sh` を回し、Rust のプローブサーバーと同じ期待出力（`expected/poa-probe-rustorb.txt`）と一致する
- naming check と wire probe：`interop/impls/rustorbjava/probe/interop/NamingCheck.java`（ランタイムの CosNaming のスタブ）で `run-naming-check.sh`（omniNames と rustorb-names）を、Java のサーバーに `run-wire-probe.sh` を回し、Rust と同じ期待出力と一致する

## JacORB の実測

JacORB 3.9 を Java 11 以降で動かすには `javax.rmi.CORBA` の API の jar（`jboss-rmi-api_1.0_spec`）が要る（Any の中の値を書くときには javax.transaction の API の jar も読み込む）。JacORB の IDL コンパイラは `CORBA::TypeCode` に `#include <orb.idl>` を求めるので、`interop/impls/jacorb/idl/orb.idl`（`native TypeCode;` だけ）を自前で用意した。プロパティは `OAIAddr` / `OAPort`（待ち受けアドレス）、`jacorb.implname`（PERSISTENT の POA に必要）、`jacorb.giop_minor_version`（GIOP 1.1 のクライアント）を使う。

JacORB は、UTF-8 に決まった `char` のデータ（rustorb とはどちらのネイティブも UTF-8 なのでこれになる。omniORB とは ISO 8859-1）を JVM の既定の文字セットで変換するようで、既定が ASCII の環境（C ロケールの Java 17 以前）では `"café"` の返事が UTF-8 でなくなり、rustorb のクライアントが `DATA_CONVERSION` になる。Java 18 以降は既定が UTF-8 なので起きない。相互接続のイメージは Java 17 なので、`jacorb` のスクリプトで `-Dfile.encoding=UTF-8` を付けている。

生成された JacORB のクラスを呼んで見つけた、IDL に合わない点（`known-failures.txt` に載せた。通信の問題ではない）：

- union の default メンバーの判別子付きの設定（`Value.flag(99, true)`）は、JacORB が選んだ1つの値（0）以外で `BAD_OPERATION` になる。受信した値もその判別子に置き換わる
- 別名の `Helper.insert`（`CountHelper.insert`）は元の型（`tk_long`）の TypeCode を付ける
- 複数のラベルを持つ union のメンバーの TypeCode は、ラベルが IDL と逆の順になる（`Choice` の `s` が 3, 2 の順）ので、`TypeCode.equal` が偽になる
- インターフェースの `Helper.insert` に nil を渡すと、TypeCode が `CORBA::Object` になる
- valuetype の `Helper.type()` の TypeCode に、具象の基底と truncatable の修飾がない（`Point3` が `Point` の派生にならない）。また valuetype の `Helper.insert` は、渡した値のクラスの TypeCode を付ける（`PointHelper.insert` に `Point3` を渡すと `Point3`）。JacORB 自身も同じ（rustorb の上で動かして見つけ、JacORB のランタイムでも確かめた）
- GIOP 1.1 の Fragment メッセージを受け付けない（`NO_IMPLEMENT`）。omniORB のクライアントは GIOP 1.1 でも大きな要求を分割するので、その組の大きな値の2ケースが失敗する。rustorb は送るメッセージを分割しない

## 残り

- DII / DSI（#44 でコアと Python に加えた。Java は需要が出たら）
- Java 22 以降の FFM API への置き換え：試作と比較は [java-ffm.md](java-ffm.md)（#51）
