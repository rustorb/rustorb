# POA・スレッドモデル設計書

Oct 28, 2026 · @ysuga · レビュー待ち（[#4](https://github.com/rustorb/rustorb/issues/4)）

M2（サーバー側）に着手する前に、POA、サーバー側の接続処理、サーバント呼び出しのスレッド、言語バインディングからの呼び出し、`ORB::run` / `shutdown` の設計を決める。計画書のリスク表で最大のリスクとしている「POAとスレッドモデルの設計ミスによる作り直し」を、ここで omniORB の実測と突き合わせて潰す。

## 前提（決定済み）

- I/O は rustorb-core 内部のマルチスレッド tokio ランタイムで処理し、公開APIは同期APIを基本にする（[#3](https://github.com/rustorb/rustorb/issues/3)）
- ORB\_CTRL\_MODEL のサーバント呼び出しは、tokio のワーカーとは別の、ORB専用の長寿命OSスレッドプールで実行する。SINGLE\_THREAD\_MODEL と MAIN\_THREAD\_MODEL はプールの上のキューで実現する（[#4](https://github.com/rustorb/rustorb/issues/4)）
- 既存ORBとの互換は仕様と実測だけを根拠にし、既存ORBのソースは読まない（[#17](https://github.com/rustorb/rustorb/pull/17)）

## omniORBの実測結果

`interop/run-poa-probe.sh` で、omniORB C++ と omniORBpy（どちらも 4.3.4）のプローブサーバーに手組みのGIOPメッセージを送って調べた。サーバーは [`interop/idl/PoaProbe.idl`](../interop/idl/PoaProbe.idl) の Target オブジェクトを、ポリシーの異なるPOAに置く。プローブ（`interop/scripts/poa-probe`）は1本の接続に要求をパイプラインで送ったり、スタブが送らないオブジェクトキーを送ったりする。結果は `interop/expected/poa-probe-omniorb-cpp.txt` と `poa-probe-omniorbpy.txt` に保存してあり、CIで毎回比べる。**omniORB C++ と omniORBpy の結果は全項目で一致した。**

### オブジェクトキー

| POA | キー（実測） | 再起動後 |
| --- | --- | --- |
| PERSISTENT / USER\_ID | `\xff` POA名 `\x00` ObjectId（例 `\xffpersistent_user\x00obj`） | 同じ |
| 入れ子のPOA | POA名を `\xff` で区切る（`\xffpersistent_user\xffnested\x00obj`） | 同じ |
| PERSISTENT / SYSTEM\_ID | `\xff` POA名 `\x00` の後に生成ID（起動時刻を含む） | 変わる（IDの生成が起動ごと） |
| TRANSIENT | `\xff` POA名 `\xfe` の後に起動時刻（秒、4バイト）とPOAごとの番号、ObjectId | 変わる |
| RootPOA | `\xfe` で始まり、起動時刻とID | 変わる |

- 再起動前のTRANSIENTキー、知らないPOA名・ObjectId、壊れたキー、空のキーは、どれも `OBJECT_NOT_EXIST`（minor `0x4f4d0001`、COMPLETED\_NO）、LocateRequest は `UNKNOWN_OBJECT` になる
- TRANSIENTキーの起動識別子は秒単位の時刻なので、**同じ秒のうちに再起動すると、前のプロセスのTRANSIENT参照がそのまま通る**ことを実測で確認した（キー記録と再起動の間を2秒空けないとプローブの結果が変わる）
- IORは IIOP 1.2 のプロファイル1つで、コンポーネントは ORB\_TYPE（0）と CODE\_SETS（1）。RootPOAのオブジェクトも子POAと同じエンドポイントで公開される

### ディスパッチとPOAManager

| 状況 | Request | LocateRequest |
| --- | --- | --- |
| 有効なオブジェクト | 実行 | `OBJECT_HERE` |
| 存在しないキー・POA・ObjectId、破棄したPOA、`deactivate_object` 後 | `OBJECT_NOT_EXIST` minor 1 | `UNKNOWN_OBJECT` |
| 知らない操作名 | `BAD_OPERATION`（omniORB独自minor `0x41540026`） | `OBJECT_HERE` |
| 引数が足りない | `MARSHAL`（omniORB独自minor） | — |
| POAManager が holding | 返事を保留し、activate 後に実行して返す | すぐ `OBJECT_HERE` |
| discarding | `TRANSIENT` minor 1 | `OBJECT_HERE` |
| inactive | `OBJ_ADAPTER` minor 1 | `UNKNOWN_OBJECT` |

- inactive から activate に戻すと `AdapterInactive`（仕様どおり）
- ServantActivator：最初の要求で `incarnate` を1回呼び、以後は呼ばない。`incarnate` が投げたシステム例外（`OBJECT_NOT_EXIST` minor 0）はそのまま返り、`ForwardRequest` は `LOCATION_FORWARD` になる。`etherealize` は `deactivate_object` が戻った後に非同期で呼ばれる
- ServantLocator：要求ごとに `preinvoke` / `postinvoke`。`ForwardRequest` と例外の扱いは ServantActivator と同じで、例外のときは `postinvoke` を呼ばない
- **LocateRequest は servant manager を呼ばない**：未活性化のオブジェクトでも、`incarnate` で失敗するオブジェクトでも `OBJECT_HERE` を返す
- AdapterActivator：知らない子POA名で Request でも LocateRequest でも呼ばれ、破棄したPOAの名前でも呼ばれる。false を返すと `OBJECT_NOT_EXIST` minor 1
- デフォルトサーバント（USE\_DEFAULT\_SERVANT）は任意の ObjectId で呼ばれ、POACurrent でそのIDが取れる
- 組み込み操作：`_is_a` と `_non_existent` は応答する（存在しないオブジェクトへの `_non_existent` は TRUE ではなく `OBJECT_NOT_EXIST`）。`_interface` は `INTF_REPOS`、`_not_existent`・`_get_interface`・`_repository_id`・`_component` は `BAD_OPERATION`
- Reply / LocateReply のGIOPバージョンは要求と同じ（1.0〜1.2）。バイト順は要求によらず常にサーバーのネイティブ（リトルエンディアン）

### スレッド

| 計測 | 結果 |
| --- | --- |
| ORB\_CTRL、1接続に4件・16件をパイプライン | すべて並行に実行（1件ごとに別スレッド、16件同時） |
| ORB\_CTRL、1接続で逐次に6件 | 同じ1スレッド |
| ORB\_CTRL、別の接続で逐次 | 前の接続のスレッドを再利用 |
| ORB\_CTRL、1接続にonewayを4件（1件目だけ300ms） | 2, 3, 4 が先に終わり、1 が最後（**onewayの順序も保たれない**。2〜4 の間の順序は実行ごとに変わりうるので、プローブは「any order」と出す） |
| SINGLE\_THREAD、1接続にパイプライン | 直列、要求順に返る。スレッドは固定されない |
| SINGLE\_THREAD、同じPOAの別オブジェクトを2接続から | 直列（POA単位） |
| SINGLE\_THREAD、別のPOA同士 | 並行 |
| SINGLE\_THREAD、onewayを4件 | 1, 2, 3, 4（要求順） |
| MAIN\_THREAD | すべて `orb.run()` のスレッドで直列。ORB\_CTRL のPOAとは並行 |
| サーバント内から同じプロセスのオブジェクトを呼ぶ | 呼び出し元のスレッドでそのまま実行（SINGLE\_THREAD の同じPOAへの再入も通る）。MAIN\_THREAD のPOA宛てだけはメインスレッドに渡る |

計画書では「omniORBとの差：同じ接続からの呼び出しが同じスレッドで処理される保証はない点が異なる」としていたが、実測では **omniORB も同じ接続の要求を並行に処理し、順序（onewayを含む）を保証しない**。逐次の呼び出しが同じスレッドになるのは、前の呼び出しが終わってから次が届くための結果にすぎない。したがって、ORB\_CTRL のプールで同じ接続の要求を並行に処理しても omniORB と挙動は変わらず、**接続ごとの順序保持オプションは MVP では設けない**。順序が必要なアプリは omniORB と同じく SINGLE\_THREAD\_MODEL を使う。

### ORBの終了

- サーバント内で `shutdown(true)` を呼ぶと `BAD_INV_ORDER` minor 3（仕様どおり）
- `shutdown(false)` は、実行中の呼び出しの返事を送ってから、すべての接続に CloseConnection を送って閉じる。サーバント内から呼んだ場合も、その呼び出しの返事は届く

## 全体構成

```
              tokio ランタイム（rustorb-io スレッド）                 アップコール層（長寿命OSスレッド）
 TCP ─▶ accept ─▶ 接続タスク ─▶ ヘッダ・Fragment結合 ─▶ ディスパッチャ ─┬─▶ ORB_CTRL: 共有キュー ──▶ プールのスレッド ─▶ サーバント
                    ▲            （ユーザーコードは呼ばない）         ├─▶ SINGLE_THREAD: POAごとの直列キュー ─▶ プール
                    │                                              └─▶ MAIN_THREAD: メインキュー ─▶ orb.run() のスレッド
                    └──── 書き込みタスク（溜まった分）◀── Reply（プールのスレッドが直接書く）◀──────────┘
```

- I/O層は読み書きと GIOP の解釈、オブジェクトキーの解析とPOAの探索だけを行い、ユーザーのコード（サーバント、servant manager、AdapterActivator）は一切呼ばない
- ユーザーのコードはすべてアップコール層のスレッドで呼ぶ。I/O層はブロッキングしない

## サーバー側の接続処理

- **エンドポイント**：`-ORBendPoint giop:tcp:<host>:<port>`（複数可）で待ち受ける。省略時は全インターフェース・OSが選ぶポート。IORに載せるホスト名は endPoint のホスト名、なければ `-ORBendPointPublish` 相当のオプション、なければホスト名。クライアント側の `OrbOptions` に追加する
- **PERSISTENT と固定ポート**：PERSISTENT のPOAの参照が再起動後も有効なのは、同じホスト・ポートで待ち受けたときだけ。omniORB と同じく、ポート未指定で PERSISTENT のPOAを作ってもエラーにはせず、警告をログに出す。Naming Service（`rustorb-naming` のデーモン）は `-start <port>` 相当で固定ポートを必須にする
- **受信**：接続ごとに読み取りタスクを1つ持つ。`FragmentAssembler` で結合した Request / LocateRequest をディスパッチャに渡す。CancelRequest は該当要求に取り消し印を付け、まだキューにあれば捨て、実行中なら返事を捨てる。CloseConnection（クライアントから）と MessageError は接続を閉じる。ヘッダが壊れていれば MessageError を送って閉じる
- **送信**：返事は完了した順に書く（要求順には並べない。omniORB と同じ）。返事を作ったスレッドが、ソケットにノンブロッキングで直接書く。前に溜まった返事がなく、ソケットが全部受け取れたときはそれで終わり、受け取りきれなかった分とその後の返事は接続ごとの書き込みタスクがソケットの空きを待って書く（#64。それまではすべて mpsc で書き込みタスクに渡していた）。Reply / LocateReply のGIOPバージョンは要求と同じにし、バイト順は rustorb のネイティブにする
- **コードセット**：接続で最初に受け取った CodeSets サービスコンテキストをその接続の変換に使う（仕様どおり接続単位）。サーバー側の IOR には rustorb のネイティブ（char UTF-8、wchar UTF-16）と変換可能なコードセットを載せる
- **流量制御**：接続ごとの実行待ち＋実行中の要求数に上限を設け、超えたら読み取りを止める（TCPの背圧に任せる）。メッセージサイズの上限も設定可能にし、超えたら MessageError
- **アイドル接続**：一定時間要求のない接続は、実行中の要求がないことを確かめてから CloseConnection を送って閉じる（既定は無効、設定で有効化）
- **双方向GIOP（BiDir）**：MVPでは扱わない。サーバーからクライアントへのコールバックは、クライアント側も待ち受けて受ける（相互接続テストと同じ構成）

## オブジェクトキー形式

> 決定（2026-10-28、ysuga）：PERSISTENT のキーは omniORB と同じ形式にし、TRANSIENT は rustorb 独自の起動識別子を使う。

- **PERSISTENT**：omniORB と同じ `\xff` POAパス `\x00` ObjectId とし、入れ子は `\xff` で区切る。omniORB で動いていたサーバーを rustorb に置き換えても、保存済みの IOR や corbaloc（`corbaloc::host:port/%FFpoa%00id`）が引き続き使える
- **TRANSIENT**：`\xff` POAパス `\xfe` 起動識別子（8バイト） ObjectId（RootPOA はパスなしで `\xfe` から始まる）。起動識別子はORBの起動ごとの64ビット乱数にし、omniORB のような同じ秒での再起動による取り違えを防ぐ。TRANSIENT の参照は再起動で無効になる前提なので、omniORB と形式を合わせる意味はない
- **SYSTEM\_ID**：生成するIDは、起動識別子（8バイト）とPOA内の通し番号（4バイト、ビッグエンディアン）。PERSISTENT のPOAでも再起動をまたいで重複しない
- **POA名の制約**：POA名に NUL を許さない（UTF-8 では `\xfe` / `\xff` は現れないので、区切りと衝突しない）
- **単純キー**：`\xff` / `\xfe` で始まらないキーは、ORBに登録した名前付きオブジェクト（`corbaloc::host:port/NameService` の `NameService` など）として引く。登録APIは `Orb::register_object_key(key, obj)`（単純キーの参照を返す。取り消しは `unregister_object_key`。omniORBpy 互換シムでは対応する独自APIに割り当てる）。単純キーの要求は登録したオブジェクトのPOAキーの要求として、転送なしでそのまま処理する
- **探索**：キーを分解し、POAパスを先頭から辿る。途中のPOAが無ければ親の AdapterActivator を（あれば）アップコール層で呼ぶ。見つからなければ `OBJECT_NOT_EXIST`（minor は omniORB の実測に合わせて 1）。キーの解析とPOAの探索は I/O層で済ませ、AdapterActivator を呼ぶ必要がある要求だけアップコール層に回す

## POA

### API（Rust）

```rust
let root: Poa = orb.root_poa()?;                       // resolve_initial_references("RootPOA") でも可
let manager: PoaManager = root.the_poa_manager();
let poa = root.create_poa("Echo", Some(&manager), &[
    Policy::Lifespan(Lifespan::Persistent),
    Policy::IdAssignment(IdAssignment::User),
])?;
poa.activate_object_with_id(b"Echo", Arc::new(EchoServant::new()))?;
let obj: ObjectRef = poa.id_to_reference(b"Echo")?;
manager.activate()?;
orb.run();
```

- `Poa` と `PoaManager` は `ObjectRef` と同じく安価に複製できるハンドル（内部は `Arc`）
- サーバントは `Arc<dyn Servant>`。生成されるスケルトン（Rust）と各言語のアダプタが実装する

```rust
pub trait Servant: Send + Sync {
    /// 最派生インターフェースのリポジトリID（IORの type_id と _is_a に使う）
    fn repository_id(&self) -> &str;
    fn is_a(&self, repository_id: &str) -> bool;
    /// op の引数を input から読み、結果を reply に書く
    fn dispatch(&self, op: &str, input: &mut InStream, reply: &mut ServerReply) -> Result<(), SystemException>;
}
```

- `ServerReply` は戻り値・out引数を書く `OutStream`（`output()`）と、ユーザー例外を返す `user_exception(repository_id)`（メンバーを書く `OutStream` を返す）を持つ。システム例外は `Err(SystemException)` で返す
- 知らない操作はサーバント（生成されるスケルトン）が `BAD_OPERATION` minor 2（仕様の「対象が操作・属性を知らない」）で返す。サーバント内で引数を読めなければ `MARSHAL`（COMPLETED\_NO）
- `_is_a`・`_non_existent` はディスパッチャが処理し、サーバントには来ない。`_interface` は `INTF_REPOS`、`_repository_id` は仕様（CORBA 3.x）どおりに応答する（omniORB は `BAD_OPERATION` だが、追加しても既存クライアントに影響しない）。それ以外の未知の操作は `BAD_OPERATION`
- POACurrent（`orb.resolve_initial_references("POACurrent")`）はアップコール中のスレッドローカルに、POA・ObjectId・サーバントを置いて実装する

### ポリシー

| ポリシー | 値 | MVP |
| --- | --- | --- |
| Thread | ORB\_CTRL\_MODEL（既定）、SINGLE\_THREAD\_MODEL、MAIN\_THREAD\_MODEL | すべて |
| Lifespan | TRANSIENT（既定）、PERSISTENT | すべて |
| IdUniqueness | UNIQUE\_ID（既定）、MULTIPLE\_ID | すべて |
| IdAssignment | SYSTEM\_ID（既定）、USER\_ID | すべて |
| ImplicitActivation | NO\_IMPLICIT\_ACTIVATION（既定）、IMPLICIT\_ACTIVATION | すべて |
| ServantRetention | RETAIN（既定）、NON\_RETAIN | すべて |
| RequestProcessing | USE\_ACTIVE\_OBJECT\_MAP\_ONLY（既定）、USE\_DEFAULT\_SERVANT、USE\_SERVANT\_MANAGER | すべて |

- RootPOA は仕様どおり ORB\_CTRL、TRANSIENT、UNIQUE\_ID、SYSTEM\_ID、IMPLICIT\_ACTIVATION、RETAIN、USE\_ACTIVE\_OBJECT\_MAP\_ONLY
- 矛盾する組み合わせ（NON\_RETAIN と USE\_ACTIVE\_OBJECT\_MAP\_ONLY、IMPLICIT\_ACTIVATION と USER\_ID など）は `create_POA` で `InvalidPolicy`

### POAManager

- 状態は active・holding・discarding・inactive で、実測した omniORB の挙動（上の表）に合わせる
- holding の間に届いた要求は POAManager ごとのキューに保持し、active に戻ったら通常の経路に流す。キューに上限を設け、溢れたら `TRANSIENT`（仕様どおり）
- holding 中の LocateRequest は保留せず `OBJECT_HERE` を返す（omniORB と同じ）

### Servant manager と AdapterActivator

- ServantActivator の `incarnate` は ObjectId ごとに1回に絞る。同じIDへの要求が同時に来たら、2件目以降は1件目の `incarnate` の完了を待つ
- `etherealize` は、そのオブジェクトの実行中の要求がすべて終わってから、アップコール層で呼ぶ（`deactivate_object` は待たずに戻る）
- LocateRequest では servant manager を呼ばず、POAが存在して POAManager が inactive でなければ `OBJECT_HERE` を返す（holding・discarding でも `OBJECT_HERE`。omniORB と同じ）
- AdapterActivator の `unknown_adapter` は POA名ごとに直列化し、作成中に届いた同じ名前への要求は結果を待たせる
- `incarnate` / `preinvoke` の `ForwardRequest` は `LOCATION_FORWARD`、システム例外はそのまま返す

### 終了処理

- `deactivate_object`：アクティブオブジェクトマップから外し、以後の要求は `OBJECT_NOT_EXIST`。実行中の要求は最後まで実行する
- `POA::destroy(etherealize, wait_for_completion)`：子POAから順に破棄する。アップコール中に `wait_for_completion = true` で呼ぶと `BAD_INV_ORDER` minor 3

## スレッドモデル

### アップコール層

- **プール**：長寿命のOSスレッドで、最小数（既定4）は常駐し、最大数（既定100、`-ORBmaxServerThreadPoolSize` 相当で設定）まで必要に応じて増やす。最小数を超えて増えたスレッドは、一定時間（既定60秒）仕事がなければ終了する。JNI の都合で、スレッドを使い捨てにはしない
- **ORB\_CTRL\_MODEL**：共有キューから空いたスレッドが取る。同じ接続の要求も並行に実行し、順序を保証しない（omniORB の実測と同じ）
- **SINGLE\_THREAD\_MODEL**：POAごとの直列キュー。キューに要求があり、そのPOAを実行中のスレッドがなければ、プールに「このPOAのキューを1件処理する」仕事を投げる。専用スレッドは持たないので、実行スレッドは毎回変わりうる（omniORB と同じ）。同じスレッドからの入れ子の呼び出し（下記の同一ORB内呼び出し）は再入を許す
- **MAIN\_THREAD\_MODEL**：ORB全体で1つのメインキュー。`orb.run()` または `orb.perform_work()` を呼んでいるスレッドが処理する。誰も呼んでいなければ要求は待つ
- **枯渇の扱い**：プールが最大数まで埋まったら、要求は共有キューで待つ。サーバントから別のサーバーへ同期呼び出しをしても、待つのはそのスレッドだけで、I/O層は止まらない。入れ子呼び出しによるプール枯渇は、同一ORB内呼び出しをプールに回さないこと（下記）と最大数の大きさで抑え、負荷試験（計画書のテスト6、[load-testing.md](load-testing.md)）で確認する

### 同一ORB内の呼び出し

> 決定（2026-10-28、ysuga）：omniORB と同じく、同一ORB内の呼び出しは呼び出し元スレッドで直接実行する。

- 自ORBのエンドポイントを指す参照への呼び出しは、ネットワークを通さず、引数をCDRに書いた後、呼び出し元のスレッドでそのままディスパッチする（omniORB の実測と同じ挙動）。マーシャリングは残すので、言語をまたいでも引数のコピーの意味は変わらない
- MAIN\_THREAD\_MODEL のPOA宛ては、呼び出し元がメインスレッドでなければメインキューに渡して待つ
- SINGLE\_THREAD\_MODEL のPOAに、そのPOAを実行中のスレッドから入るときは再入を許す（デッドロックしない）。別のスレッドからなら直列キューに並ぶ
- 判定は、IORのホスト・ポートが自分の待ち受けアドレスと一致し、オブジェクトキーが自分のPOAとして解析できることとする

### クライアント呼び出しとの関係

- 同期APIの呼び出しは、要求を書いて oneshot チャネルで返事を待つ（実装済み）。アップコール中のスレッドからも呼べる
- tokio のワーカー上から同期APIを呼ぶと `BAD_INV_ORDER`（実装済み）。サーバントはアップコール層で動くので、この制約に当たらない

## 言語バインディングからのサーバント呼び出し

各言語は `Servant` トレイトを実装するアダプタを持ち、アップコール層のスレッドから呼ばれる。

### Python（PyO3）

- アップコールのたびに GIL を取り（`Python::with_gil`）、引数をPythonオブジェクトに変換してからサーバントのメソッドを呼び、結果を CDR に書いてから GIL を放す
- Python からの同期呼び出し、`orb.run()`、`perform_work()` の待ちは GIL を放して待つ（`allow_threads`）
- `orb.run()` は短い間隔（100ms程度）で GIL を取り直して `PyErr_CheckSignals` を呼び、Ctrl-C で `KeyboardInterrupt` を上げられるようにする
- MAIN\_THREAD\_MODEL のPOAは、Python のメインスレッドで `orb.run()` を呼べば、サーバントがメインスレッドで動く（GUIツールキットやシグナル処理との併用向け）
- Pythonの例外：IDLのユーザー例外はそのまま返し、それ以外の例外は `UNKNOWN`（COMPLETED\_MAYBE）にしてトレースバックをログに出す
- サーバントの寿命：Rust 側は `Py<PyAny>` を保持し、解放は GIL を取れるときに行う（PyO3 の仕組みに任せる）。`_this()` は `_default_POA()`（既定は RootPOA）で暗黙に活性化する
- フリースレッド版 Python（3.13t 以降）でも同じ設計で動く（GIL の取得が実質なくなるだけ）

### C ABI（rustorb-ffi）と C++

- サーバントは vtable とコンテキストの組で渡す

```c
typedef struct rustorb_servant_vtbl {
    const char* (*repository_id)(void* ctx);
    int  (*is_a)(void* ctx, const char* repository_id);
    /* 0: 正常、1: ユーザー例外を out に書いた、2: システム例外を exc に書いた */
    int  (*dispatch)(void* ctx, const char* op, rustorb_instream* in,
                     rustorb_reply* out, rustorb_system_exception* exc);
    void (*release)(void* ctx);   /* サーバントを手放すとき（etherealize 後など） */
} rustorb_servant_vtbl;
```

- コールバックはアップコール層のスレッドから呼ぶ。C++ ラッパは `dispatch` の中で C++ の例外をすべて捕まえて戻り値に変換し、FFI 境界を越えて巻き戻さない。Rust 側も `catch_unwind` でパニックを止める
- 参照カウントは Rust 側に一元化し（計画書どおり）、`release` は最後の参照が消えたときに1回だけ呼ぶ

### Java（JNI）

- プールのスレッドは、最初の Java サーバント呼び出しで `AttachCurrentThreadAsDaemon` し、スレッドが終了するときに `DetachCurrentThread` する。スレッドは長寿命なのでアタッチは1回で済む
- サーバントは `GlobalRef` で保持し、手放すときにアタッチ済みのスレッドで `DeleteGlobalRef` する
- 呼び出し後に `ExceptionCheck` を行い、IDL のユーザー例外は CDR に、それ以外の Throwable は `UNKNOWN` に変換する

### 共通

- Rust のサーバントがパニックしたら `UNKNOWN`（COMPLETED\_MAYBE）を返し、スレッドは使い続ける

## ORB::run / shutdown / destroy

- `run()`：`shutdown` されるまでブロックし、その間メインキューの要求を処理する。複数のスレッドから呼んでもよく、どのスレッドも MAIN\_THREAD の要求を処理しうる
- `work_pending()` / `perform_work()`：メインキューに要求があるか、1件処理する。独自のイベントループに組み込む用途
- `shutdown(wait_for_completion)`：新しい接続の受け付けを止め、すべてのPOAを破棄（etherealize あり）する。実行中の要求は返事まで送り、その後すべての接続に CloseConnection を送って閉じる（omniORB の実測と同じ）。`wait_for_completion = true` ならこの完了まで待ち、アップコール中に呼ぶと `BAD_INV_ORDER` minor 3。`false` なら別スレッドで進めてすぐ戻る。サーバント内から `shutdown(false)` を呼んだ場合も、その呼び出しの返事は送る
- `destroy()`：`shutdown(true)` 相当を済ませた後、プールのスレッドを終了させ（JNI のデタッチを含む）、tokio ランタイムを止める。`Orb` の最後のハンドルが消えたときも同じ処理をする

## omniORB との意図的な差

| 項目 | omniORB（実測） | rustorb |
| --- | --- | --- |
| TRANSIENT の起動識別子 | 秒単位の時刻（同じ秒の再起動で取り違える） | 64ビット乱数 |
| `_repository_id` | `BAD_OPERATION` | 仕様どおり応答 |
| 知らない操作の minor | omniORB 独自の VMCID（`0x41540026`） | OMG 標準の minor 2（`0x4f4d0002`） |
| 引数不足の `MARSHAL`、`_interface` の `INTF_REPOS` の minor | omniORB 独自の VMCID | 現状は 0（OMG 標準の minor は未選定） |
| IOR のコンポーネント | ORB\_TYPE（omniORB の値）と CODE\_SETS | CODE\_SETS のみ（rustorb には OMG が割り当てた ORB type がない） |

それ以外（キーの探索結果、POAManager の各状態、LocateRequest の応答、servant manager の呼び方、スレッドへの割り当て、終了時の CloseConnection）は omniORB の実測に合わせる。

## 検証方法

- `interop/run-poa-probe.sh` に rustorb のプローブサーバー（`impls/rustorb/src/poa_probe_server.rs`）を加え、omniORB と同じ期待出力になることを確認する。上の「意図的な差」に当たる行だけが `interop/expected/poa-probe-rustorb.txt` で omniORB と異なる（M2 段階4で確認済み）
- 相互接続マトリクスに rustorb サーバーを加え、コールバックの2ケース（現在 skip）を rustorb クライアントでも通す
- 負荷試験（パイプライン、多接続、入れ子呼び出し）でプールの上限と枯渇時の挙動を確認する

## M2 の実装順

1. 待ち受けとサーバー側の接続（Request / LocateRequest / CancelRequest / CloseConnection、Reply / LocateReply の送信）、オブジェクトキーの符号化と解析
2. RootPOA、`create_POA`、アクティブオブジェクトマップ、ORB\_CTRL のプール、`Servant` トレイト、`ORB::run` / `shutdown`。手書きスケルトンで interop の Echo サーバーを作り、マトリクスに加える
3. POAManager の状態、servant manager、AdapterActivator、デフォルトサーバント、SINGLE\_THREAD / MAIN\_THREAD、同一ORB内呼び出し
4. rustorb のプローブサーバーで `run-poa-probe.sh` を通す
5. Naming Service デーモン（PERSISTENT のPOA、固定ポート、`NameService` の単純キー）
