# omniORBpy 互換シム

Oct 28, 2026 · @ysuga · M5 の次

omniORBpy 向けに書いたアプリケーションを、書き換えずに rustorb 上で動かすための互換パッケージ `rustorb-omniorb-compat`（`python/omniorb-compat`）をまとめる。この段階の完了条件は「OpenRTM-aist-Python の RTC が互換シム経由で rustorb 上で動き、omniORB 上の RTC とデータポート・サービスポートで両方向に接続できること」（[development-plan.md](development-plan.md) の第2段階）。OpenRTM-aist-Python 同士（rustorb と omniORBpy）、OpenRTM-aist（C++）の RTC との組み合わせ、rtshell からの操作を確かめた。

## 構成

| 場所 | 内容 |
| --- | --- |
| `python/omniorb-compat/omniORB/__init__.py` | `omniORB` パッケージ。`CORBA`・`PortableServer` は rustorb のモジュールそのもの。`cdrMarshal` / `cdrUnmarshal`、`findType`、マイナーコードの定数 |
| `python/omniorb-compat/omniORB/any.py` | `to_any` / `from_any` |
| `python/omniorb-compat/omniORB/tcInternal.py` | `findType` の型記述子の種類番号（`tv_*`） |
| `python/omniorb-compat/CORBA.py` など | omniORBpy がトップレベルに置く `CORBA`・`PortableServer`・`PortableServer__POA`（rustorb のモジュールを返す） |
| `python/omniorb-compat/CosNaming*` | `CosNaming` のスタブ（`rustorb-idl -bpython` の生成物を置いたもの。`generate-stubs.sh` で作り直す） |
| `crates/rustorb-python/src/orb.rs` | `Orb.cdr_marshal` / `cdr_unmarshal`、`register_object_key` |
| `python/rustorb/CORBA.py`、`PortableServer.py` | `omniINSPOA`、文字列の ObjectId、`CORBA.Object._nil` |
| `python/tests/test_omniorb_compat.py` | 単体テスト（期待値は omniORBpy 4.3 で同じ呼び出しを実測した値） |
| `interop/run-openrtm.sh`、`interop/openrtm/` | OpenRTM-aist-Python の例を両方の ORB で動かし、OpenRTM-aist（C++）の例ともつなぐ相互接続テストと、rtshell のテスト |

ライセンスは rustorb 本体と同じ MIT OR Apache-2.0。本物の omniORBpy と同じ名前（`omniORB`、`CORBA`、`CosNaming`）を持つので、rustorb 本体とは別のパッケージにして、使うときだけ入れる。

```sh
pip install ./python ./python/omniorb-compat
```

```python
from omniORB import CORBA, PortableServer   # rustorb の CORBA と PortableServer
import CosNaming
```

アプリケーションのスタブは `rustorb-idl -bpython` で生成する（omniidl の生成物は omniORBpy の内部 API を呼ぶので使えない）。

## 対象 API の洗い出し

OpenRTM-aist-Python（[OpenRTM/OpenRTM-aist-Python](https://github.com/OpenRTM/OpenRTM-aist-Python) の b58852f）が使う `omniORB` の API を grep で洗い出し、ライブラリ本体（`examples` 以外）で使うものを入れた。

| API | 使う場所 | 互換シム |
| --- | --- | --- |
| `from omniORB import CORBA, PortableServer`、`import omniORB.CORBA` | ほぼ全体 | rustorb のモジュールを `omniORB.CORBA` などとして `sys.modules` に登録 |
| `import CosNaming` | `CorbaNaming.py` ほか | 生成済みスタブを同梱 |
| `cdrMarshal(tc, data, endian)` / `cdrUnmarshal(tc, encap, endian)` | データポートの CDR 直列化（`CORBA_CdrMemoryStream.py`）、共有メモリ、IOR の解析 | ネイティブの `Orb.cdr_marshal` / `cdr_unmarshal` |
| `any.to_any` / `any.from_any(keep_structs=)` | NVList のプロパティ（`NVUtil.py`、`CorbaPort.py`、データポートのコンシューマ） | Python で実装 |
| `findType(repoId)` と `tcInternal.tv_*` | `RTCUtil.instantiateDataType`（データ型の既定値を作る） | 生成コードの TypeCode から記述子を作る |
| `COMM_FAILURE_WaitingForReply` | データポートの再送判定 | 定数（下記） |
| `resolve_initial_references("omniINSPOA")` | `Manager.py`、`ManagerServant.py`（`corbaloc::host:2810/manager` など） | rustorb の ORB に追加 |
| `CORBA.Object._nil`、`PortableServer.POA._nil` | 初期値 | rustorb の `CORBA.Object` に追加 |
| `activate_object_with_id("文字列", servant)` | INS POA への登録 | rustorb の POA が str を UTF-8 で受け付ける |
| `-ORBendPointPublish all(addr)` | `corba.endpoints: all`（既定値） | rustorb のコアが受け付ける |

rtshell（[OpenRTM/rtshell](https://github.com/OpenRTM/rtshell) の d35459c）とそれが使う rtctree（[OpenRTM/rtctree](https://github.com/OpenRTM/rtctree) の 9803b6b）からも同じように洗い出した。

| API | 使う場所 | 互換シム |
| --- | --- | --- |
| `TRANSIENT_ConnectFailed` | 接続できない RTC をゾンビとして扱う（rtctree）、マネージャのアドレスの誤り（rtmgr） | 定数（下記） |
| `UNKNOWN_UserException` | マネージャのモジュール読み込みの失敗（rtctree） | 定数（標準のマイナーコード） |
| `CORBA.TRUE` / `CORBA.FALSE` | `orb.shutdown(wait_for_completion=CORBA.FALSE)` | rustorb の `CORBA` に追加（Python マッピングの定数） |
| `from rtctree.rtc import RTC` | rtctree のスタブ（omniidl の `-Wbpackage=rtctree.rtc` の生成物） | `rustorb-idl -Wbpackage` を追加 |

`sslTP` / `httpTP` は SSL・HTTP の設定があるときだけ読むので入れていない。
入れなかったもの：`omniORB.URI`（`examples/AutoTest` だけ）、`sslTP` / `httpTP`（SSL・HTTP トランスポートの拡張）、`TIMEOUT` の例外ハンドラ登録（OpenRTM-aist-Python は使わない）。`setClientCallTimeout` などのタイムアウト関数は #38 で入れた（[timeouts.md](timeouts.md)）。

## 実装

### cdrMarshal / cdrUnmarshal

- `endian` を省くと CDR のカプセル化（先頭がホストのバイト順の octet、続けて値）、`True` / `False` を渡すとその値だけをリトルエンディアン / ビッグエンディアンで書く。omniORBpy の実測と同じ
- 文字列は ISO 8859-1、ワイド文字列は UTF-16。rustorb のネイティブのコードセット（UTF-8）ではなく、omniORB のネイティブのコードセットに合わせた。OpenRTM-aist のデータポートは直列化したバイト列を octet のシーケンスとして送り、受け手が `cdrUnmarshal` するので、コードセットのネゴシエーションが入らない。omniORB 上の RTC と `TimedString` をやり取りするには同じ符号化が要る。ISO 8859-1 にない文字は `DATA_CONVERSION`（omniORB も同じ）
- 読み込みで値の後にバイトが残る、または足りないときは `MARSHAL`
- オブジェクト参照は最初に初期化した ORB に属する。ORB の初期化前は `BAD_INV_ORDER`（omniORBpy と同じ）

### omniORB.any

`to_any` は値から TypeCode を選ぶ。選び方は omniORBpy の実測に合わせた。

- int は `long` に収まれば `long`、次に `long long`、`unsigned long long`、それも超えたら `BAD_PARAM`
- float は `double`、str は `string`、bytes は `CORBA::OctetSeq`、None は `tk_null`
- 生成コードの struct・union・例外・enum の値とオブジェクト参照は、その型の TypeCode
- list / tuple は、要素の TypeCode がすべて同じならその sequence、違えば `any` の sequence
- dict はその場で作った struct（リポジトリ ID は `rustorb:UnknownStruct:<番号>`。omniORBpy は `omni:` で始まる ID）

`from_any` は入れ子の any を外し、sequence は要素ごとに変換する。struct と例外は `keep_structs` が偽ならメンバー名の dict に、真ならそのまま返す（真のときは struct の中の any も外さない）。

### findType

OpenRTM-aist-Python はデータポートの型の既定値を作るのに、omniORBpy の型記述子（`findType` が返すタプル）を読む。記述子の形は omniORBpy で `findType` を呼んだ結果だけから合わせた（omniORBpy のソースは読んでいない）。

| 型 | 記述子 |
| --- | --- |
| 基本型 | 種類番号（`tcInternal.tv_long` など。値は TCKind と同じ） |
| struct・例外 | `(tv_struct, クラス, ID, 名前, メンバー名, 記述子, ...)` |
| union | `(tv_union, クラス, ID, 名前)`（メンバーは省いた） |
| enum | `(tv_enum, ID, 名前, 列挙子のタプル)` |
| typedef | `(tv_alias, ID, 名前, 元の型の記述子)` |
| インターフェース | `(tv_objref, ID, 名前)` |
| string・wstring | `(tv_string, 上限)` |
| sequence・array | `(tv_sequence, 要素の記述子, 上限)`、`(tv_array, 要素の記述子, 長さ)` |

記述子は生成コードの TypeCode（`_rt.typecodes`）から作る。rustorb の生成コードとランタイムは omniORBpy の記述子を持たないので（[python-codegen.md](python-codegen.md)）、互換シムの中だけで作る。

### omniINSPOA

omniORB の `omniINSPOA` は RootPOA の子の POA で、参照のオブジェクトキーが ObjectId そのものになる（`corbaloc::host:port/ID` で引ける）。OpenRTM-aist はマネージャ（`manager`）と RTC（`カテゴリ.インスタンス名`）をこれで公開する。実測では、RootPOA の子で、自分の POAManager を持ち（初期状態は HOLDING）、USER_ID（`activate_object` は `WrongPolicy`）。

rustorb では `resolve_initial_references("omniINSPOA")` で同じ性質の POA を作り、`activate_object_with_id` のたびにコアの `Orb::register_object_key`（Naming Service の `NameService` キーと同じ仕組み）で ObjectId をキーとして登録する。`id_to_reference` などが返す参照はそのキーを持つ。キーは再起動しても同じなので、寿命のポリシーは TRANSIENT にした（固定ポートが要らない）。

### その他

- **文字列の ObjectId**：POA の操作に str の ObjectId を渡すと UTF-8 にする（omniORBpy の実測と同じ）
- **`COMM_FAILURE_WaitingForReply`**：omniORB のマイナーコード（0x41540007）。rustorb は自分のマイナーコードで例外を上げるので、この定数と比べても一致しない。OpenRTM-aist はこれを「1回再送する」判定に使うだけで、一致しなければ通常のエラー処理に進む
- **`TRANSIENT_ConnectFailed`**：接続できなかったときの `TRANSIENT` のマイナーコード。omniORB の値（0x41540002）ではなく、rustorb が上げる値（0）にした。rtctree はこれと一致する `TRANSIENT` を「名前は残っているがプロセスがない RTC（ゾンビ）」と判定し、一致しなければ例外をそのまま上げて rtls などが止まるため。rustorb が同じマイナーコード 0 で上げる他の `TRANSIENT`（転送の回数超過など）もゾンビ扱いになるが、どちらもその RTC と通信できないことに変わりはない。呼び出しのタイムアウトは `TIMEOUT` なので混ざらない
- **`UNKNOWN_UserException`**：CORBA の標準のマイナーコード（`OMG_VMCID | 1`）で、omniORB も rustorb も同じ値を上げる
- **`-ORBendPointPublish all(addr)`**：omniORB の「ホストのすべてのアドレスを公開する」指定。rustorb はもともとホスト名を公開するので、既定の公開と同じに扱う

## 相互接続

`interop/run-openrtm.sh` が OpenRTM-aist-Python・rtshell・rtctree（固定のコミット）を取得し、次の2つを omniORB のイメージで動かす。イメージには OpenRTM-aist（C++、[OpenRTM/OpenRTM-aist](https://github.com/OpenRTM/OpenRTM-aist) の 7e08ae0）の例を omniORB 4.3 でビルドして入れてある。

### 例の RTC の接続（`openrtm/scenarios.py`）

| シナリオ | 例 | 確かめること |
| --- | --- | --- |
| `simpleio` | `SimpleIO`（ConsoleIn → ConsoleOut） | `TimedLong` のデータポート（corba_cdr、push、flush） |
| `seqio` | `SeqIO`（SeqOut → SeqIn） | 基本型と sequence の10本のデータポート |
| `simpleservice` | `SimpleService`（MyServiceProvider ← MyServiceConsumer） | サービスポート（`set_value`、`get_value_history`、非同期の `echo` など） |

それぞれを、Python の RTC（rustorb・omniORBpy）の4通りと、C++ の RTC と rustorb 上の Python の RTC の両方向（C++ → rustorb、rustorb → C++）の計6通りで動かす。接続と activate は `interop/openrtm/connect.py`（Python の側の ORB で動かす）。

C++ と Python の `SimpleService` は、同じ IDL（`SimpleService::MyService`）でもサービスポートのインターフェースの名前が違う（C++ は型名 `SimpleService::MyService`・インスタンス名 `myservice`、Python は `MyService`・`myservice0`）。そのため名前の一致では対応が付かず、コネクタのプロパティでコンシューマのインターフェースをプロバイダのものに対応付けて接続する（`<コンシューマ>.port.MyService.required.<型>.<名前>` = `<プロバイダ>.port.MyService.provided.<型>.<名前>`。OpenRTM-aist の CorbaPort の仕組み）。また C++ のプロバイダの `set_value` は10秒かかり、Python のコンシューマは同期で呼ぶので、このシナリオでは OpenRTM-aist の既定の呼び出しタイムアウト（10秒）を30秒にする。

### rtshell（`openrtm/rtshell.py`）

`SimpleIO` の2つの RTC に対して rtshell のコマンドを順に実行し、出力と RTC の状態を確かめる。

1. `rtls` に2つの名前が出る。`rtcat` がプロファイルとポートを出す。`rtconf ... list` が `default` を出す
2. `rtcon` で接続し、`rtcat -l` のポートに接続先が出る
3. `rtact` で2つとも Active になり（`rtls -l`）、ConsoleOut にデータが届く。`rtdeact` で Inactive に戻る
4. `rtdis` で接続が消える。`rtexit` で ConsoleOut のプロセスが終わる
5. ConsoleIn を SIGKILL で止めると、`rtls` がゾンビ（`*ConsoleIn0.rtc`）として表示する

組み合わせは、RTC が rustorb・omniORBpy・C++ のそれぞれで rtshell が rustorb のものと、RTC が rustorb で rtshell が omniORBpy のもの。C++ の ConsoleIn は deactivate でエラー状態になる（omniORBpy の rtshell でも同じ）ので、そこだけ Error も受け付ける。

rtshell と rtctree は pip で入れず、ソースツリーを `PYTHONPATH` に置いて動かす。rtctree は自分のスタブを `rtctree.rtc` パッケージから import するので、rtctree のコピーに `rustorb-idl -Wbpackage=rtctree.rtc`（omniORBpy 側は `omniidl -bpython -Wbpackage=rtctree.rtc`）でスタブを生成する。

### ネーミング

既定は omniNames。`--naming rustorb` で rustorb-names に替えて、同じシナリオと rtshell を動かす。CI は両方を動かす。

```sh
interop/run-openrtm.sh                     # omniORB のイメージで
interop/run-openrtm.sh --naming rustorb    # rustorb-names で
OMNIORBPY_DIR=... OPENRTM_CXX_EXAMPLES=... interop/run-openrtm.sh --local
```

## 残り

- `omniORB.URI`、`setClientThreadCallDeadline` など、使うアプリケーションが出てきたものの追加
