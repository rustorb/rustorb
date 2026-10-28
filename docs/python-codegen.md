# Python生成器（rustorb-idl -bpython）

Oct 28, 2026 · @ysuga · M1

IDL から rustorb の Python バインディング向けのコードを生成する `rustorb-idl -bpython` の構成と、生成コードとランタイムの取り決めをまとめる。M1 の完了条件は「omniidl -bpython と同じ公開APIのコードを生成できること」で、`interop/run-pymapping-check.sh` で omniORBpy 上の実測と比べて判定する。実際の呼び出し（マーシャリング、POA への登録）は M3 の PyO3 バインディングでつなぐ。

## 決定事項

- **生成器は Python で書く**（2026-10-28）。idl\_parser が Python 製で、どちらの案でも生成時に Python と idl\_parser が要るため、JSON の中間表現を挟んで Rust で書く利点は小さい。idl\_parser のオブジェクトを直接使い、omniidl と同じ構成で書く。Rust/C++/Java の生成器も同じパッケージにバックエンドとして足す
- 置き場所は `crates/rustorb-codegen/python/rustorb_idl/`（GPL-3.0-or-later）。`pip install ./crates/rustorb-codegen/python` で `rustorb-idl` コマンドが入る。idl\_parser は修正を含むコミットに固定する
- 生成コードが呼ぶランタイムは `python/rustorb/` の `rustorb` パッケージ（MIT OR Apache-2.0）。今は純 Python の部分（`CORBA`、`PortableServer`、`_types`、`_rt`）だけで、M3 でネイティブモジュールを足す
- 互換は omniidl -bpython の**公開API**に合わせる。omniORBpy の内部表現（操作や型の記述子の形式）は見ず、写さない。型の情報は CORBA の TypeCode そのもので持つ

## 使い方

```sh
pip install ./crates/rustorb-codegen/python
rustorb-idl -bpython [-I DIR] [-D NAME[=VALUE]] [-C OUTDIR] FILE.idl...
```

オプションは omniidl と同じ意味のものを同じ名前にした。`-Wbglobal=NAME`（モジュール外の定義を入れるパッケージ、既定 `_GlobalIDL`）、`-Wbno_stubs`（`.pyi` を書かない）、`--encoding` も受ける。

## 出力のレイアウト

`Foo.idl` から次を書く。omniidl -bpython と同じ配置で、既存の `import` 文がそのまま使える。

| ファイル | 内容 |
| --- | --- |
| `Foo_idl.py` | ファイル内のすべての定義。include したファイルの `*_idl.py` を先に import する |
| `Module/__init__.py` | IDL モジュールのパッケージ。そのモジュールに定義を持つ `*_idl.py` を import する。入れ子のモジュールはサブパッケージ |
| `Module__POA/__init__.py` | スケルトンのパッケージ |
| `_GlobalIDL/`, `_GlobalIDL__POA/` | モジュール外の定義 |
| `__init__.pyi` | 各パッケージの型スタブ |

1つのモジュールに複数の IDL ファイルが定義を持つ場合、各ファイルを同じ出力先に順に生成すれば、`__init__.py` の import の一覧と `.pyi` のファイルごとの区画が足し込まれる。

## 生成コードとランタイムの取り決め

生成コードは `rustorb._rt`、`rustorb.CORBA`、`rustorb.PortableServer` だけを使う。どれも `_0_` 付きの名前で import するので、IDL の名前（`_0_` で始まることはない）とぶつからない。取り決めの版は `_rt.GENERATED_CODE_VERSION`（いまは1）で、生成コードは最初に `_rt.check_version(1)` を呼び、合わなければ ImportError で読み込みを止める。

| IDL | 生成されるもの |
| --- | --- |
| モジュール | `_rt.open_module(name)` が返すモジュールオブジェクト。パッケージが import されていれば、パッケージそのもの |
| 定数 | モジュール（またはクラス）の属性。boolean は `True` / `False`（omniORBpy は `1` / `0`、`==` では同じ）、fixed は `CORBA.fixed` |
| enum | `_rt.Enum(repoId, names)`（`_items`、`_item(n)`）と、同じスコープの列挙子（`EnumItem`、`_n` / `_v` / `_parent_id`） |
| struct | `_rt.Struct` の派生クラス。メンバーを順に受ける `__init__` |
| union | `_rt.Union` の派生クラス。`U(d, v)` と `U(member=v)`、`_d` / `_v`、メンバー名での読み書き |
| exception | `CORBA.UserException` の派生クラス |
| typedef | struct / union の typedef はそのクラスの派生クラス、それ以外は生成できないクラス（`_rt.Typedef`）。どちらも `_NP_RepositoryId` を持つ |
| interface | 抽象クラス（`Module.Iface`、生成できない、`_nil = None`）、スタブ（`Module._objref_Iface`、`CORBA.Object` の派生）、スケルトン（`Module__POA.Iface`、`PortableServer.Servant` の派生）。継承は3つとも IDL の継承に従う |
| 型 | すべての型に `_tc_Name`（`CORBA.TypeCode`） |

- **TypeCode**：struct・union・例外は `_rt.declare_*` で先に殻を作り、メンバーは `_rt.define_*` で後から埋める。再帰型（`struct Tree { sequence<Tree> children; }`）と前方宣言はこれで同じ TypeCode を指す。リポジトリIDごとの TypeCode は `_rt.typecodes` に登録される
- **操作**：インターフェースの `_NP_operations` に、操作名（属性は `_get_x` / `_set_x`）から `_rt.Operation` への辞書を置く。`Operation` は `in_types`（in と inout の引数の TypeCode）、`out_types`（戻り値と inout・out の TypeCode）、`exceptions`（ユーザー例外の TypeCode）、`oneway`、`contexts` を持つ。基底インターフェースの操作は MRO をたどって引く（`_rt.operation`）
- **スタブ**：各メソッドは `self._invoke(Operation, args)` を呼ぶ。結果は none / 単一の値 / タプル（IDL to Python の規則）。属性は `_get_x` / `_set_x` に加えて property `x` も持つ（omniORBpy と同じ）。`_invoke` は ORB のオブジェクト（M3 のネイティブ側）に渡す
- **登録**：スタブとスケルトンは `_rt.register_interface` / `_rt.register_skeleton` でリポジトリIDから引けるようにする。`_narrow` はこれでスタブのクラスを選ぶ

## omniORBpy の実測と合わせた挙動

`interop/scripts/pymapping-dump` で omniidl -bpython の生成物を omniORBpy 上で調べ、次の挙動を合わせた（`interop/expected/pymapping-omniorbpy.txt`）。

- 抽象インターフェースクラスは `CORBA.Object` を継承しない。スタブは `CORBA.Object` と基底のスタブを継承する
- struct / union の typedef はそのクラスの派生クラスで、自分のリポジトリIDを持つ。enum・interface・シーケンスなどの typedef は、生成すると RuntimeError になるクラス
- union を `default` のメンバーで作ったときの判別子は、型の値を順に見て、どのラベルにも使われていない最初の値（整数は最小値から、enum は先頭の列挙子から）。`long` なら -2147483648
- 複数のラベルを持つメンバーを名前で設定したときの判別子は、最後のラベル（`case 4: case 5: LongSeq seq;` の `Value(seq=...)` は `_d == 5`）
- `CORBA.fixed("12.50")` は末尾の0を落とし、3桁・小数1桁になる。`value()` は 10 の小数桁乗をかけた整数
- ローカルインターフェース・抽象インターフェースもスタブとスケルトンを持つ

意図的に仕様に従った差は4点で、[interop/README.md](../interop/README.md#python-mapping-check) に挙げた（ローカルインターフェースの TypeCode の種類、TypeCode のメンバー名、`TC_Object` のリポジトリID、fixed の typedef のクラス）。

## idl\_parser について

- オペレーションの戻り値が `unsigned long` など複数語の型だと構文エラーになる（[idl\_parser#159](https://github.com/sugarsweetrobotics/idl_parser/issues/159)）。InteropTest.idl が読めないため、生成器に回避策を入れた（`frontend.py` の `_install_return_type_workaround`、修正後に削除）
- omniidl は IDL の名前の衝突（メンバー名と型名の大文字小文字違い、`apple` と `APPLE` など）をエラーにするが、idl\_parser は通す。生成器は通ったものをそのまま生成する
- valuetype・eventtype・map・bitset・bitmask はまだ生成しない（エラーにする）

## 残り（M3 以降）

- ネイティブモジュール：`ORB_init`、`_invoke` の実装（TypeCode を見て Python の値を CDR にする）、POA と `_this()`、スケルトンへの振り分け
- 生成コードを使った相互接続（`interop/impls` に Python の rustorb サーバー・クライアントを足し、マトリクスに入れる）
- `.pyi` の精度（sequence<char> の扱いなど、ネイティブ側の値の表現が決まってから）
