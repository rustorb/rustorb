# Python バインディング（PyO3）

Oct 28, 2026 · @ysuga · M3

rustorb の Python パッケージ `rustorb` のうち、ORB 本体をつなぐネイティブモジュール `rustorb._native`（`crates/rustorb-python`、PyO3）と、その上に載る標準マッピング（`CORBA.ORB_init`、`PortableServer`）の構成をまとめる。M3 の完了条件は「omniORBpy 向けに書いたクライアント・サーバーが import の書き換えだけで rustorb 上で動き、omniORBpy・omniORB C++ と相互接続できること」で、`interop/run-matrix.sh` の `rustorbpy` で判定する。

## 構成

| 場所 | 内容 |
| --- | --- |
| `crates/rustorb-python` | ネイティブモジュール `rustorb._native`（PyO3、abi3-py39）。`Orb`、`ObjRef`、`Poa`、`PoaManager` と、TypeCode に従うマーシャリング |
| `python/rustorb/CORBA.py` | `ORB_init`、`ORB`（`create_policy`、`register_initial_reference` など、[orb-operations.md](orb-operations.md)）、`Object`（`_narrow`、`_is_a`、`_is_equivalent`、`_hash`、`_non_existent`、ポリシーの操作）、`Policy`、`PolicyManager`、`PolicyCurrent`、例外、TypeCode |
| `python/rustorb/Messaging.py`、`TimeBase.py` | `RelativeRoundtripTimeoutPolicy` と `TimeT` の TypeCode |
| `python/rustorb/PortableServer.py` | `Servant`（`_this`、`_default_POA`）、`POA`、`POAManager`、`Current`、ポリシーの値と `create_*_policy`、`ForwardRequest` と POA の例外 |
| `python/rustorb/PortableServer__POA.py` | `ServantActivator`、`ServantLocator`、`AdapterActivator` のスケルトン |
| `python/rustorb/_native.pyi` | ネイティブモジュールの型スタブ |
| `python/pyproject.toml` | maturin でホイールを作る設定（`pip install ./python`） |
| `python/build-native.sh` | ネイティブモジュールをビルドしてソースツリーの `python/rustorb/` に置く（テスト用） |

ライセンスはコアと同じ MIT OR Apache-2.0。

## 使い方

```sh
pip install ./python                 # maturin でビルドしてインストール
python/build-native.sh               # またはソースツリーで使う
```

```python
import sys
from rustorb import CORBA, PortableServer
import Example, Example__POA          # rustorb-idl -bpython の生成物

orb = CORBA.ORB_init(sys.argv)       # -ORBInitRef などは取り除かれる
poa = orb.resolve_initial_references("RootPOA")
poa._get_the_POAManager().activate()
ref = Servant()._this()              # RootPOA で暗黙に活性化
orb.run()                            # Ctrl-C で KeyboardInterrupt
```

omniORBpy のコードは `from omniORB import CORBA, PortableServer` を `from rustorb import CORBA, PortableServer` に替え、スタブを `rustorb-idl -bpython` で作り直せば動く（`interop/impls/rustorbpy` は omniORBpy の相互接続用クライアント・サーバーを、この書き換えだけで動かしている）。`from omniORB import ...` のまま動かす互換シムは、計画どおり別パッケージで後から用意する。

## 設計

### マーシャリングは2段階

Python の値は、まず GIL を持ったまま型を確かめて Rust の中間値（`value::Value`）にし、CDR への書き込みは GIL を手放してから行う。返事も同様に、CDR の読み取りは GIL なし、Python の値にするところだけ GIL を取る。

- 要求の送信、転送（`LOCATION_FORWARD`）による再送、返事の待ち合わせの間、ほかの Python スレッドが動ける
- サーバント側は、引数の読み取りと結果の書き込みを GIL なしで行い、Python のメソッドを呼ぶ間だけ GIL を取る
- 型の誤りは送信前に `BAD_PARAM`（completed NO）になる

型の情報は生成コードの `CORBA.TypeCode` そのもの。操作ごとに最初の呼び出しで Rust の木（`tc::Tc`）に変換し、`_rt.Operation` の `_native` スロットに置く。再帰型は同じセルを指す循環になる（TypeCode はモジュールと同じだけ生きるので解放しない）。

### スレッドと GIL

決定済みのスレッドモデル（I/O は ORB 内部の tokio、サーバントは長寿命のアップコール用スレッドプール）のまま、Python 側は次のようにする。

- ORB・オブジェクト参照・POA の呼び出しは、待つ可能性があるものはすべて GIL を手放して実行する。サーバント内からの入れ子呼び出しや、同一 ORB 内の呼び出し（呼び出し元のスレッドでサーバントを実行する）でもデッドロックしない
- アップコールのスレッドは長寿命なので、GIL の取得はアップコールごとに1回で済む
- `orb.run()` は 100 ms ごとに Python のシグナルを確認する（コアに `Orb::run_timeout` を追加）。Ctrl-C で `KeyboardInterrupt` になる
- 終了時は `atexit` で ORB を `destroy` し、実行中の要求を Python が動けるうちに終わらせる

### サーバントの同一性

POA はサーバントを `Arc` のアドレスで見分ける（`servant_to_id`、UNIQUE_ID）。Python のサーバントオブジェクトごとに `PyServant` を1つだけ作り、オブジェクトのアドレスから弱参照で引く。POA が手放せば `PyServant` も消え、Python のオブジェクトへの参照も外れる（循環参照を作らない）。`id_to_servant` などで POA から返るサーバントは、同じ Python オブジェクトに戻す。

## 値の表現

IDL to Python マッピングと omniORBpy の Python 3 版に合わせる。

| IDL | Python |
| --- | --- |
| 整数型、`octet` | `int`（範囲外は `BAD_PARAM`） |
| `float`、`double` | `float` |
| `boolean` | `bool`（送信時は真偽値として評価） |
| `char`、`wchar` | 長さ1の `str`（`wchar` は UTF-16 の1単位に収まるもの） |
| `string`、`wstring` | `str`（上限付きは文字数で確認） |
| `sequence<octet>`、`octet` の配列 | `bytes`（送信時は整数のリストも可） |
| `sequence<char>`、`char` の配列 | `str`（送信時は `bytes` も可） |
| その他の sequence・配列 | `list`（送信時はタプルも可。配列は長さを確認） |
| struct・例外 | 生成クラスのインスタンス（メンバー名が Python の予約語なら `_` 付きの属性） |
| union | 生成クラス（`_d`、`_v`）。判別子に合うラベルがなく default もなければ値なし |
| enum | 列挙子（`EnumItem`）。別の enum の列挙子は `BAD_PARAM` |
| `fixed` | `CORBA.fixed`（桁数・小数桁を超えると `BAD_PARAM`） |
| オブジェクト参照 | スタブ（IOR の型 ID が既知でその派生なら最派生のスタブ）、nil は `None` |
| `any` | `CORBA.Any`（受け取った型が生成コードになければ TypeCode とクラスをその場で作る。[any-typecode.md](any-typecode.md)） |
| `TypeCode` | `CORBA.TypeCode` |

valuetype と `long double` はまだ扱わず、`NO_IMPLEMENT` になる。

## 例外

| 場面 | 結果 |
| --- | --- |
| サーバントが宣言されたユーザー例外を送出 | そのユーザー例外を返す |
| サーバントが `CORBA.SystemException` を送出 | そのまま返す |
| サーバントが宣言されていないユーザー例外を送出 | `UNKNOWN`（minor `0x4f4d0001`、COMPLETED_MAYBE） |
| サーバントがその他の Python 例外を送出 | `UNKNOWN`（minor 0、COMPLETED_MAYBE）。トレースバックを標準エラーに出す |
| サーバントに操作のメソッドがない | `NO_IMPLEMENT`（COMPLETED_NO） |
| サーバントの戻り値が型に合わない | `BAD_PARAM`（COMPLETED_MAYBE） |
| クライアントの引数が型に合わない | 送信前に `BAD_PARAM`（COMPLETED_NO） |
| 返ってきたユーザー例外が操作で宣言されていない | `UNKNOWN`（minor `0x4f4d0001`、COMPLETED_YES） |

POA の操作の例外は `PortableServer.POA.ObjectNotActive` など標準の例外クラス（リポジトリIDは `IDL:omg.org/PortableServer/...:2.3`）。servant manager の `incarnate`・`preinvoke` は `PortableServer.ForwardRequest(ref)` で転送できる。

サーバント側の minor コードのうち、メソッドがない場合と Python 例外の場合は omniORBpy と突き合わせていない（omniORBpy のソースは読まない方針のため、必要になったら相互接続で実測して合わせる）。

## POA の範囲

`create_POA`（7種のポリシー）、`find_POA`（AdapterActivator 経由の生成を含む）、`destroy`、`activate_object(_with_id)`、`deactivate_object`、`servant_to_id/reference`、`id_to_servant/reference`、`reference_to_id/servant`、`create_reference(_with_id)`、デフォルトサーバント、`ServantActivator`・`ServantLocator`、`POAManager` の4状態、`PortableServer::Current`。スレッドポリシーは ORB_CTRL・SINGLE_THREAD・MAIN_THREAD（`orb.run()` か `perform_work()` のスレッドで実行）のすべてが使える。

`Servant._this()` は、そのサーバントの要求の実行中ならその要求のオブジェクトを、そうでなければ `_default_POA()`（最初に初期化した ORB の RootPOA）で `servant_to_reference` した参照を返す。

## テスト

- `python/tests/test_orb.py`：同じプロセスにサーバー用とクライアント用の2つの ORB を作り、ループバックの TCP 越しに `InteropTest.idl` と `python/tests/idl/NativeTest.idl`（char の列、上限付きの型、fixed、再帰構造体、予約語の名前、boolean・char の判別子、`Object`）を呼ぶ。POA の各操作、servant manager、POAManager の状態、同一 ORB 内の呼び出しも確かめる。CI では `RUSTORB_REQUIRE_NATIVE=1` でネイティブモジュールと idl\_parser が必須
- `python/tests/test_dynany.py`：`rustorb.DynamicAny`（[dynany.md](dynany.md)）。生成コードの型の DynAny、valuetype の共有と循環、切り詰め
- 相互接続：`interop/impls/rustorbpy`（omniORBpy のクライアント・サーバーの import を替えただけのもの）を、omniORB C++・omniORBpy・rustorb（Rust）とすべての組み合わせで、omniNames と rustorb-names の両方、GIOP 1.1 でも回す

## 残り

- omniORBpy 互換シム（`from omniORB import ...`、別パッケージ）。対象 API は OpenRTM-aist-Python の利用箇所から決める（M5 の後）
- `run-poa-probe.sh` と `run-naming-check.sh` への Python の rustorb の追加
