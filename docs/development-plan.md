# Rust製CORBA ORB 開発計画

Oct 28, 2026 · @ysuga

## 目的とスコープ

omniORBを置き換えられる、コアを純Rustで実装したCORBA ORBを作る。最初の到達点は「omniORB/omniORBpyの一般的な用途（Naming Service経由の呼び出し・サーバー公開）をPythonから置き換えられるMVP」とする。

**対象**

- GIOP 1.2 / IIOP（クライアント・サーバー双方）、CDRエンコード
- IOR、corbaloc、corbaname
- POA（RootPOA＋主要ポリシー）、サーバントの登録と活性化
- CosNaming（NamingContext / NamingContextExt / BindingIterator）とNaming Serviceデーモン
- IDL生成：Rust、Python、C++（CPP11準拠）、Java

**当面の対象外**（後続フェーズで判断）

- CORBA Component Model、Event / Notification Service
- Interface Repository、DII / DSI
- SSLIOP、CSIv2 などのセキュリティ
- 旧来のIDL to C++（C++98）マッピング

## 準拠する仕様

C++はIDL to C++11（CPP11）を基準にし、IDL4-CPPで追加された構文は必要に応じて取り込む。Rustは公式マッピングがまだないため独自定義とし、OMGのIDL4-Rust RFPの動向を追う。

| 領域 | 仕様 | 方針 |
| --- | --- | --- |
| コア | CORBA 3.x（GIOP 1.2 / IIOP / CDR） | GIOP 1.0〜1.2を受け付け、送信は1.2 |
| ネーミング | CosNaming（Naming Service） | NamingContextExt まで対応 |
| Rust | 独自マッピング | [IDL4-Rust RFP](https://www.omg.org/gettingstarted/rfp-IDL4-RLP.htm) の動向に追随 |
| C++ | [IDL to C++11](https://www.omg.org/spec/CPP11/Current)（v1.7） | 基準。実装・生成コードはC++17を使用 |
| C++（IDL4） | [IDL4-CPP](https://www.omg.org/spec/IDL4-CPP)（v1.0） | アノテーション・拡張型を順次取り込み |
| Python | IDL to Python マッピング | omniORBpyとの互換性を優先 |
| Java | IDL to Java マッピング | 生成APIは標準準拠、実装はRustコア経由 |

## 全体アーキテクチャ

プロトコル実装はRustコアの4クレートに集約し、各言語はその上に薄く載せる。IDLからのコード生成は、既存のidl\_parserのASTを入力にする生成器クレートが担う。

&#91;embedded content: クレート構成 · コア4クレート、FFI層、生成器\]

> **注:** 上記の図は計画書の作成元にあった埋め込み図で、このファイルには含まれていません。図は今後追加予定のプレースホルダです。

実線は実行時の呼び出し、破線はビルド時のコード生成。C++とJavaは安定C ABI層を経由し、PythonはPyO3でコアを直接呼ぶ。

## マイルストーン

M0〜M3の約4〜7か月でMVPに到達し、その後C++とJavaを順に追加する。各マイルストーンは、既存ORBとの相互接続で完了を判定する。

&#91;embedded content: マイルストーン · M0〜M6と完了条件\]

> **注:** 上記の図は計画書の作成元にあった埋め込み図で、このファイルには含まれていません。図は今後追加予定のプレースホルダです。

期間は1人で進める前提の目安。M4とM5はM3完了後なら並行して進められる。

## 言語バインディング設計

各言語はRustコアの上に薄く載せ、プロトコル実装は持たない。共通の設計課題は、オブジェクト参照のライフタイム、例外の変換、サーバント呼び出し時のスレッドの扱いの3点。

| 言語 | FFI手段 | 生成されるもの | 主な課題 |
| --- | --- | --- | --- |
| Python | PyO3 + maturin | スタブ・スケルトンのPythonモジュール、型ヒント（.pyi） | GILとORBスレッドの協調、omniORBpy互換API |
| C++ | C ABI（安定ABI層）＋ヘッダオンリーのC++ラッパ | CPP11準拠のヘッダ・スケルトン | RAIIと参照カウントの対応、例外の伝播 |
| Java | JNI（将来Java 22以降のFFM APIも検討） | IDL to Java準拠のクラス群 | JavaのGCとRust側参照の解放、コールバックのスレッドアタッチ |

- Rustコアは`extern "C"`の安定ABI層（`rustorb-ffi`）を1つだけ公開し、C++とJavaはそこから使う。Pythonは開発効率を優先してPyO3で直接つなぐ
- オブジェクト参照はRust側で参照カウントし、各言語のハンドルが解放されたら減算する
- 例外はシステム例外・ユーザー例外ともにCDRで表現し、各言語の例外クラスに変換する

## テストと相互接続検証

完了の判定は「既存ORBとの相互接続テストが通ること」を基準にする。単体テストだけでは非互換を見落とすため、早い段階からCIに組み込む。

1. **単体テスト**：CDRのエンコード・デコード（境界アラインメント、エンディアン、ネスト型）、IOR・corbalocのパース
2. **ゴールデンテスト**：omniORBが送受信したGIOPメッセージのキャプチャを保存し、バイト列単位で比較
3. **相互接続マトリクス**：自作ORBとomniORB / omniORBpy / JacORB を、クライアント・サーバーの両方向で組み合わせて実行
4. **Naming Service互換**：omniNamesと同じ操作列（bind、rebind、resolve、list、unbind）を両方に流して結果を比較
5. **ファジング**：cargo-fuzzでGIOPメッセージ・CDRデコーダを常時ファジング
6. **負荷・長時間試験**：同時接続数、リクエストレート、接続断と再接続、メモリリーク

相互接続テストはDockerで各ORBの環境を用意し、GitHub Actionsで毎回回す。

## リスクと対策

最大のリスクは、POAとスレッドモデルの設計を誤って後から作り直しになること。M2の前に設計レビューを挟んで抑える。

| リスク | 影響 | 対策 |
| --- | --- | --- |
| POA・スレッドモデルの設計ミス | サーバー側全体の作り直し | M2着手前に設計書を作り、omniORBの挙動と突き合わせる |
| 既存ORBとの細かい非互換 | 実運用で接続できない | ゴールデンテストと相互接続マトリクスを最初からCIに入れる |
| 言語間のオブジェクト参照の寿命管理 | メモリリーク、解放済み参照の利用 | 参照カウントをRust側に一元化し、各言語のテストでリークを検出 |
| Any / TypeCode の実装負荷 | MVPの遅れ | MVPから外し、M5に回す |
| idl\_parser の拡張が必要になる | コアとパーサの並行改修 | M0でAST上の不足を洗い出してから着手 |
| 4言語の保守負担 | 進捗の分散 | Pythonを先行させ、C++・Javaはコア安定後に着手 |

## 未決事項と次のアクション

**決定事項**

- プロジェクト名は rustorb。クレート名は `rustorb-` を接頭辞にする（例：`rustorb-core`、`rustorb-ffi`）（[#2](https://github.com/rustorb/rustorb/issues/2)）
- ライセンスは MIT OR Apache-2.0 のデュアル。ただし、GPL-3.0-or-later の idl\_parser を取り込むIDLコード生成器だけは GPL-3.0-or-later とし、生成されたコードは生成器のライセンスの対象外とする（[#2](https://github.com/rustorb/rustorb/issues/2)）

**未決事項**

- 非同期ランタイム：tokioに固定するか、ランタイム非依存にするか
- POAのデフォルトスレッドモデル（ORB\_CTRL\_MODEL の実装方針）
- Pythonバインディングで omniORBpy とのAPI互換をどこまで保つか
- Naming Serviceの永続化方式（ログファイル形式か、KVSか）
- 最初に置き換えたい具体的な用途（相互接続テストの優先順位を決めるため）

**次のアクション**

- [ ] idl\_parser のリポジトリを共有し、ASTの構造と不足を確認する
- [ ] Cargoワークスペースの雛形とクレート分割を作る
- [ ] omniORBとomniORBpyの相互接続テスト環境（Docker）を用意する
- [ ] CDRエンコーダとGIOPメッセージ型の実装に着手する
