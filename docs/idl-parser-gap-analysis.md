# idl_parser のAST不足の洗い出し（M0）

Oct 28, 2026

開発計画の次のアクション「idl\_parser のASTの構造と不足を確認する」の結果をまとめる。対象は [sugarsweetrobotics/idl_parser](https://github.com/sugarsweetrobotics/idl_parser) の main（`6eecfd2`, 2026-10-27）。Rust・Python・C++11・Java の生成器の入力として足りるかを、小さなIDL断片と実際のIDL（JacORB同梱の CosNaming.idl、OpenRTM-aist-Python の RTM\_IDL 一式）を読ませて確認した。

## 結論

基本的な型・宣言はほぼ揃っており、大きな作り直しは要らない。ただし、MVPの題材である **CosNaming.idl がそのままでは読めない**ことと、生成器に必須の **宣言順・リポジトリID・定数値・ユニオンのラベル** が取れないことの4点は、生成器の着手前（M1）に idl\_parser 側で直す必要がある。OpenRTM-aist のIDLは `CORBA::TypeCode` と文字コードの2点で読めず、M5までに直せばよい。

## 確認できたこと（十分なもの）

- module（入れ子、再オープンは1ノードに統合）、interface（継承は `IDLInterface` に解決、`local` / `abstract`、前方宣言）
- オペレーション：戻り値、`in` / `out` / `inout`、`oneway`、`raises`（`IDLException` に解決、継承元インターフェースで定義された例外も解決）、`context`
- attribute：`readonly`、複数宣言子（`attribute long x, y;`）、`getraises` / `setraises`
- struct（継承、`all_members`、再帰 `sequence<Self>`、前方宣言）、exception、enum（`@value`）、typedef（複数宣言子、typedef の連鎖は `IDLTypedef` をたどれる）、native、valuetype / eventtype
- sequence（有界・ネストした境界も取れる）、多次元配列（定数式のサイズを評価済み）、有界 string / wstring、map、fixed、any、Object、ValueBase、IDL4 の整数型（int8〜uint64）、wchar、long double
- 型名の解決：スコープ付き名（`A::B::S`、`::M::T`）、インターフェース内で宣言した型を外から参照
- `#include`（複数ディレクトリ、条件付き include）、`#define` / `#if`、アノテーション、`#pragma` の記録（種類・引数・ファイル・行）
- 各定義の `filepath`（include 元のファイルが取れるので、生成対象を主ファイルの定義に絞れる）

## 不足の一覧

優先度は、P0 = M1（Python生成器）着手前に必要、P1 = M5（Any / OpenRTM-aist）までに必要、P2 = あると良い。

| # | 優先度 | 不足 | 再現 | 生成器への影響 |
| --- | --- | --- | --- | --- |
| 1 | P0 | インターフェース内の入れ子定義から、そのインターフェース自身を参照できない | `interface NC { exception CP { NC cxt; }; };` が `InvalidDataTypeException`。CosNaming.idl の `NamingContext::CannotProceed` がこれに当たり、ファイル全体が読めない | MVPの題材がそのまま読めない。現状は `interface NamingContext;` を前に足すと回避できる |
| 2 | P0 | 宣言順が残らない | 定義は種類ごとのリスト（`structs`、`enums`、`typedefs` …）に分かれて保持され、種類をまたいだ出現順が分からない | C++ヘッダとPythonモジュールは「使う前に定義」が必要。前方宣言を含むIDLでは型の依存関係からの並べ替えだけでは元の順序を再現できず、omniidl と同じ出力順にもできない |
| 3 | P0 | リポジトリIDを求められない | `#pragma prefix` / `ID` / `version` は記録されるだけで、どの定義に効くかが分からない（定義ノードに行番号がなく、pragma との前後関係が取れない）。`typeprefix` / `typeid` は警告を出して読み飛ばす（[idl_parser#100](https://github.com/sugarsweetrobotics/idl_parser/issues/100)） | IOR の型ID、`_is_a`、例外のCDR表現、TypeCode にすべて必要。CosNaming は `omg.org`、OpenRTM は `omg.org` / `openrtm.aist.go.jp` を使っており、誤ると相互接続できない |
| 4 | P0 | ユニオンの判別子とラベルが文字列のまま | `descriminator_kind` は `'Color'` などの文字列で型ノードに解決されない。ラベルは `['N+1']`、`['T::RED']`、`["'a'"]` のように書いたままで、定数式・enum値・char・boolean が評価されない。さらに `switch (unsigned long)`、`switch (long long)` など複数語の整数型で構文エラーになる | ユニオンのCDR直列化（判別子の型とサイズ）と、各言語のユニオン型の生成に必要 |
| 5 | P0 | 整数以外の定数値が評価されない | `int_value` は整数だけ。float / double（`1.0/3` は式のまま）、string（`"a" "b"` の連結、エスケープ）、char / wchar、boolean（`'TRUE'`）、wstring、enum値（`'A::Y'` のまま）は書いたままの文字列。`const fixed F = 1.25d;` は `.type` が `IndexError` で落ちる | 各言語のリテラルを出力するには、型付きの値（Python の float / str / bool、enum は `IDLEnumValue`）が必要 |
| 6 | P1 | `CORBA::TypeCode` など組み込みの CORBA 型を知らない | SDOPackage.idl（`#define TypeCode CORBA::TypeCode`）と、それを include する RTC.idl・Manager.idl・OpenRTM.idl が `Can not find Data Type (CORBA::TypeCode)` で読めない。`module CORBA { native TypeCode; };` を前に足すと全ファイル読める | OpenRTM-aist（M5以降の完了条件）が読めない。`orb.idl` 相当の組み込み定義（TypeCode、必要に応じて Principal など）を持たせたい |
| 7 | P1 | UTF-8 以外のIDLを読めない | RTC.idl（ISO-8859 系のコメントを含む）で `UnicodeDecodeError` | 実在のIDLを扱えない。コメント中の非ASCIIは omniidl では問題にならない。読み込み時の文字コード指定かエラー置換が必要 |
| 8 | P1 | エスケープ付き識別子の `_` が残る | `long _module;` のメンバー名が `'_module'` のまま（仕様では `module` として扱う） | 生成コードの名前とリポジトリIDが omniidl とずれる。生成器側で言語ごとの予約語回避をするため、IDL上の名前は `_` を外して渡してほしい |
| 9 | P2 | 定義ノードに行番号がない | `filepath` はあるが行番号がない | #3 の pragma の適用範囲の判定、生成器のエラーメッセージに必要 |
| 10 | P2 | ファイル情報の不整合 | 複数ファイルにまたがって再オープンしたモジュールの `filepath` は最後に読んだファイルになる。include 先の定義の `filepath` は絶対パス、主ファイルは渡したままのパスになる | 生成対象の絞り込みと、Python の `xxx_idl.py` 単位の出力で、比較前に正規化が必要 |
| 11 | P2 | 意味検査が足りない | 同名の定義の重複、大文字小文字だけが違う名前の衝突、`oneway` に `out` 引数や戻り値がある、定数の値域（`const octet O = 300;`）がエラーにならない | 不正なIDLから壊れたコードを生成してしまう。omniidl はいずれもエラーにする |
| 12 | P2 | ユニオンの case 内で型を定義できない | `case 1: struct In { long q; } inn;` が構文エラー | 実用上はまれ |

### 生成器側で吸収するもの（idl\_parser の不足としない）

- 言語ごとの予約語の回避、名前の変換規則（Rust の snake\_case など）
- TypeCode・CDR のレイアウト計算（AST から生成器が組み立てる）
- コメント（ドキュメントコメント）の保持。生成コードへの転記は後回しにし、必要になったら idl\_parser に依頼する

## 生成器とのつなぎ方

idl\_parser は Python 製なので、Rust で書く生成器クレートから直接は使えない。候補は2つある。

1. **生成器を Python で書く**：idl\_parser のオブジェクトをそのまま使う。最も手早く、Python・C++・Java の生成は omniidl のバックエンドと同じ構造になる
2. **idl\_parser から JSON の中間表現を出して Rust で読む**：`to_dic()` はあるが、型が `'sequence<long>'` のような文字列で、スコープ解決の結果や宣言順が失われるため、そのままでは中間表現に使えない

不足 #2〜#5 を直す際に、型参照を完全修飾名と種類で表す JSON 出力も一緒に整えておくと、どちらの方式も選べる。どちらにするかは M1 の着手時に決める。

## 確認に使ったIDL

- JacORB 同梱の `idl/omg/CosNaming.idl`：不足 #1 で失敗。回避後は NamingContext / BindingIterator / NamingContextExt の全オペレーション、例外、継承が取れた
- OpenRTM-aist-Python の `RTM_IDL`：BasicDataType、ExtendedDataTypes、InterfaceDataTypes、DataPort、SharedMemory は読める。SDOPackage、RTC、Manager、OpenRTM は不足 #6・#7 で失敗し、回避後は読める
- 機能ごとの小さなIDL断片（上表の「再現」列）

## 次のアクション

- [ ] 不足 #1〜#5（P0）を idl\_parser の issue にし、M1 の前に直す
- [ ] 不足 #6〜#8（P1）を issue にし、M5 の前に直す
- [ ] 生成器を Python で書くか、JSON 中間表現を介して Rust で書くかを M1 着手時に決める
