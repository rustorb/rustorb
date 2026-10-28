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
| Python | IDL to Python マッピング | 標準マッピングとomniidlの生成レイアウトは互換、omniORBpy独自拡張は厳選して互換シムで提供 |
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

### 相互接続の優先順位

最初の置き換え対象は2段階で決める。MVP（M0〜M3）は「Naming Service経由の汎用的なPython呼び出し」で完了を判定し、Any / TypeCode を実装するM5の完了後に「OpenRTM-aist-Pythonの動作」を次の完了条件にする（[#7](https://github.com/rustorb/rustorb/issues/7)）。OpenRTM-aistはSDOのプロパティ（NVList、値が any）やデータポートのCDR直列化を使うため、最初から対象にするとAnyをMVPへ前倒しすることになるので、段階を分ける。

**第1段階：Naming経由の汎用Python呼び出し（MVPの完了条件）**

| 優先度 | クライアント | サーバー | ネーミング |
| --- | --- | --- | --- |
| 1 | rustorb（Python） | omniORBpy | omniNames |
| 1 | omniORBpy | rustorb（Python） | omniNames |
| 1 | omniORBpy・rustorb（Python） | 両方 | rustorbのNaming Service |
| 2 | rustorb（Python・Rust） | omniORB（C++） | omniNames |
| 2 | omniORB（C++） | rustorb（Python・Rust） | omniNames |
| 3 | rustorb ⇔ JacORB（両方向） | | Javaバインディングの着手時に追加 |

- **題材のIDL**：自作の小さなテスト用IDLと CosNaming.idl を使う。テスト用IDLは基本型、string / wstring、struct、enum、union、sequence、配列、typedef、ユーザー例外、属性、oneway、オブジェクト参照の受け渡し、コールバック（サーバーからクライアントへの呼び返し）を含め、Anyは含めない
- **ゴールデンテスト**：同じテスト用IDLで omniORBpy・omniORB が送受信するGIOPメッセージをキャプチャし、比較の基準にする
- **Naming Service**：rustorbのNaming Serviceに omniORBpy と omniORB から bind・resolve・list を行い、omniNamesと同じ結果になることを確認する

**第2段階：OpenRTM-aist-Python（M5完了後の完了条件）**

- **組み合わせ**：互換シム経由で rustorb 上に載せた OpenRTM-aist-Python のRTCと、omniORB上の OpenRTM-aist（C++）のRTCを、データポートとサービスポートで両方向に接続する。ネーミングは omniNames と rustorb のNaming Service の両方で確認し、rtshell など既存ツールからの状態遷移（activate / deactivate）も通す
- **題材のIDL**：OpenRTM-aist の RTC.idl、SDOPackage.idl、DataPort.idl（CORBA\_CdrDataPort）、Manager.idl など、RTCの通信に使うIDL一式
- **互換シムの洗い出し**：OpenRTM-aist-Pythonが実際に呼ぶomniORBpy独自API（`cdrMarshal` / `cdrUnmarshal`、タイムアウト設定、例外ハンドラなど）をこの段階で確定し、互換シムに入れる範囲を決める

## 非同期ランタイムとスレッドモデル

通信I/Oは内部のtokioで処理し、サーバント呼び出しはORB専用のスレッドプールに渡す。I/Oとアップコールを分けるのは、Python・Java・C++のサーバントがすべてブロッキング処理で、tokioのワーカー上で呼ぶとI/O全体が止まるため。

- **I/O層**：rustorb-core が内部でマルチスレッドのtokioランタイムを持ち、接続の受け付け、GIOPメッセージの送受信、リクエストの多重化を担う。既存のtokioランタイムを使いたい利用者向けに、ハンドルを渡して共有する手段も用意する
- **アップコール層**：デコード済みのリクエストを、長寿命のOSスレッドからなるプールに渡してサーバントを呼ぶ。プールの上限は設定可能にする（omniORBの maxServerThreadPoolSize に相当）
- **クライアント呼び出し**：同期APIは結果をチャネルで待つ実装にし、tokioのコンテキスト外からも、アップコール中のスレッドからも呼べるようにする。サーバント内からの入れ子呼び出しやコールバックで、プールが枯渇しないよう上限と待ち合わせを設計する
- **言語バインディングとの協調**：プールのスレッドは入れ替わらないため、JNIの AttachCurrentThread はスレッドごとに1回で済み、Pythonはサーバント呼び出しの間だけGILを取得する
- **omniORBとの差**：omniORBの既定は「接続ごとのスレッド＋上限を超えたらスレッドプール」で、サーバントが並行に呼ばれる点は同じ。同じ接続からの呼び出しが同じスレッドで処理される保証はない点が異なるため、M2着手前の設計書でomniORBの挙動と突き合わせ、必要なら接続ごとの順序を保つオプションを設ける

## omniORBpyとの互換範囲

omniORBpyの既存コードを、少ない書き換えで動かせることを目標にする。互換は3層に分け、標準部分は完全互換、独自拡張は実際に使われているものだけを取り込む。

- **標準マッピング（完全互換）**：OMGのIDL to Pythonマッピングに従う部分。`CORBA.ORB_init`、`resolve_initial_references`、`PortableServer`（POA・POAManager・ポリシー）、`CosNaming`、システム例外・ユーザー例外のクラス階層、`_narrow` / `_this`、構造体・列挙・シーケンス・ユニオンのPython表現を、omniORBpyと同じ挙動にする
- **生成コードのレイアウト（互換）**：IDLコード生成器は omniidl -bpython と同じ構成（`Module` / `Module__POA` パッケージと `xxx_idl.py`）を出力し、既存の `import` 文をそのまま使えるようにする。加えて型ヒント（.pyi）を生成する
- **omniORBpy独自拡張（厳選して互換）**：`omniORB` パッケージ名で提供される独自APIのうち、よく使われるものを互換シムとして用意する。候補は `cdrMarshal` / `cdrUnmarshal`、`setClientCallTimeout` などのタイムアウト設定、TRANSIENT等の例外ハンドラ登録、`omniORB.any` のAny変換ヘルパ。対象はOpenRTM-aist-Pythonなど実際のアプリで必要なAPIを洗い出して決め、使われていないものは入れない
- **パッケージ構成**：本体は `rustorb` パッケージ（`from rustorb import CORBA, PortableServer`）。`from omniORB import CORBA` を通す互換シムは、本物のomniORBpyと同じ環境に入れても衝突しないよう、別パッケージとして任意でインストールする形にする
- **対象外**：omniORBpy内部の実装モジュール（`_omnipy` など）、omniORB固有の設定ファイルとすべての設定パラメータの再現。ORB初期化引数（`-ORBInitRef`、`-ORBendPoint` など）は主要なものだけ受け付ける
- **検証**：既存のomniORBpyアプリとテストを、互換シム経由でrustorb上で動かすことを相互接続テストに加える。動かなかったAPIは互換シムへの追加を検討する

## Naming Serviceの永続化

Naming Serviceは、保存先をトレイトで抽象化し、既定の実装はomniNamesと同じ「追記ログ＋チェックポイント」方式にする。ネーミングのデータは小さく書き込みもまれで、KVSの性能上の利点が小さいため、依存のない単純な方式を選ぶ。

- **保存層の抽象化**：bind・rebind・unbind・コンテキストの生成と破棄を記録する保存トレイトを定義し、Naming Serviceの本体はこのトレイトだけに依存する。永続化しないインメモリ実装も用意し、テストと一時利用に使う
- **既定実装（追記ログ＋チェックポイント）**：更新操作を1件ずつログファイルに追記し、応答を返す前にfsyncする。一定の操作数または時間ごとに全状態をチェックポイントとして一時ファイルに書き出し、renameで置き換えてからログを切り詰める。起動時はチェックポイントを読み込み、続けてログを再生する。末尾の書きかけのレコードは破棄する
- **ファイル形式**：1行1レコードのテキスト形式にし、名前とオブジェクト参照（文字列化IOR）を人が読める形で残す。先頭に形式のバージョンを書き、将来の変更に備える
- **参照の永続性**：再起動後も同じIORで引けるよう、各NamingContextはPERSISTENTなPOAで固定のオブジェクトキーを持ち、デーモンは固定ポートで待ち受ける（omniNamesの `-start` に相当）。ルートコンテキストのオブジェクトキーは `NameService` とし、`corbaloc::host:port/NameService` で引けるようにする
- **KVS**：組み込みKVS（純Rustのredbを想定）は、規模や要件から必要になった時点で、機能フラグ付きの別実装として追加する。MVPには含めない
- **対象外**：omniNamesのログファイルの読み込み・移行はMVPでは対象外とする

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
- 非同期ランタイムは tokio に固定する。ただしランタイムはORB内部に閉じ込め、公開APIは同期APIを基本にして、利用者にtokioを要求しない。async APIは任意で追加する（[#3](https://github.com/rustorb/rustorb/issues/3)）
- POAのデフォルトスレッドモデル（ORB\_CTRL\_MODEL）では、サーバント呼び出しをtokioのワーカーとは別の、ORB専用の長寿命OSスレッドプールで実行する。SINGLE\_THREAD\_MODEL（POAごとの直列化）と MAIN\_THREAD\_MODEL（`orb.run()` のスレッドで実行）は、このプールの上にキューを足して実現する（[#4](https://github.com/rustorb/rustorb/issues/4)）
- Pythonバインディングは、OMGのIDL to Python標準マッピングと omniidl -bpython の生成レイアウトを omniORBpy と互換にし、omniORBpy独自拡張は実アプリで必要なものだけを別パッケージの互換シムで提供する（[#5](https://github.com/rustorb/rustorb/issues/5)）
- Naming Serviceの永続化は保存先をトレイトで抽象化し、既定実装はomniNamesと同じ追記ログ＋チェックポイント方式とする。組み込みKVSは必要になった時点で機能フラグ付きで追加する（[#6](https://github.com/rustorb/rustorb/issues/6)）
- 最初に置き換える用途は2段階とし、MVPはNaming Service経由の汎用的なPython呼び出し（omniORBpy・omniNamesとの相互接続を最優先）、M5（Any / TypeCode）の完了後はOpenRTM-aist-Pythonの動作を完了条件にする（[#7](https://github.com/rustorb/rustorb/issues/7)）

**未決事項**

- なし（M2着手前のPOA・スレッドモデル設計書は [#4](https://github.com/rustorb/rustorb/issues/4) で継続）

**次のアクション**

- [ ] idl\_parser のリポジトリを共有し、ASTの構造と不足を確認する
- [x] Cargoワークスペースの雛形とクレート分割を作る
- [ ] omniORBとomniORBpyの相互接続テスト環境（Docker）と、第1段階のテスト用IDLを用意する
- [ ] CDRエンコーダとGIOPメッセージ型の実装に着手する
