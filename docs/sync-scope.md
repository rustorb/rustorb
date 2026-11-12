# SyncScope ポリシー

issue #153（計画 #150）。Messaging の `SyncScopePolicy`（`SYNC_SCOPE_POLICY_TYPE` = 24）を、コアと Python・C ABI / C++・Java に入れた。oneway の要求を送った呼び出しがいつ戻るかを決めるポリシーで、タイムアウト（`RelativeRoundtripTimeoutPolicy`、[timeouts.md](timeouts.md)・[orb-operations.md](orb-operations.md)）と同じ仕組みで参照・スレッド・ORB に設定する。

根拠は CORBA 3.x の仕様（Messaging の `SyncScope`、GIOP 1.2 の `RequestHeader` の `response_flags`）と、omniORB C++ / omniORBpy に手で組んだ GIOP の要求を送って外から見た答え（`interop/scripts/wire-probe`）。clean-room 方針どおり、他の ORB のソースは読んでいない。

## 値と線上の形

| `Messaging::SyncScope` | 呼び出しが戻るとき | `response_flags` | サーバーの返事 |
| --- | --- | --- | --- |
| `SYNC_NONE`（0） | 要求を書き込む前（書き込みは ORB のランタイムが後で行う） | `0x00` | なし |
| `SYNC_WITH_TRANSPORT`（1、既定） | 要求を書き込んだとき（今までの oneway） | `0x00` | なし |
| `SYNC_WITH_SERVER`（2） | サーバーが要求と対象のオブジェクトを見つけ、サーバントを呼ぶ前に返す空の返事を受けたとき | `0x01` | `NO_EXCEPTION`（本体なし）、またはシステム例外・`LOCATION_FORWARD` |
| `SYNC_WITH_TARGET`（3） | サーバントが終わった後の返事を受けたとき | `0x03` | 同上 |

二方向の呼び出しはポリシーに関係なく `0x03`（`SYNC_WITH_TARGET`）。`SYNC_NONE` と `SYNC_WITH_TRANSPORT` は線上では区別できない。GIOP 1.0 / 1.1 は `response_expected` の1ビットしかないので、`SYNC_WITH_SERVER` は `SYNC_WITH_TARGET` と同じ要求になる（サーバーはサーバントの後に返事をする）。

`SYNC_WITH_SERVER` と `SYNC_WITH_TARGET` の oneway は返事を待つので、二方向の呼び出しと同じシステム例外（`OBJECT_NOT_EXIST`・`TRANSIENT`・`TIMEOUT` など）が上がり、`LOCATION_FORWARD` を追いかける。タイムアウトも同じように効く。`SYNC_NONE` は接続を作るところまで（接続できないときの `TRANSIENT` など）は呼び出しの中で行い、書き込みだけを後に回す。書き込みの失敗は呼び出し元には届かない（トレースに出る）。

### 決めたこと

- `SYNC_NONE` は書き込みの完了を待たずに返す（推奨どおり）。`SYNC_WITH_TRANSPORT` と同じに扱う案もあったが、相手が読まずに送信バッファが詰まったとき、`SYNC_WITH_TRANSPORT` は書き込みを待つ（タイムアウトがあれば `TIMEOUT`）のに対し、`SYNC_NONE` はすぐ戻る、という違いが使い分けの理由になる。接続の確立まで後に回さなかったのは、要求ごとのアドレスの選び直し（転送・別のプロファイル）を呼び出しの中に残すため。

## 優先順位

oneway の要求のとき、参照のオーバーライド、なければ呼び出したスレッドの `PolicyCurrent`、なければ ORB の `ORBPolicyManager`、どれもなければ `SYNC_WITH_TRANSPORT`。タイムアウトと同じ順。async の `send_oneway` と DII の oneway は、呼び出したときのスレッドの設定を使う。

`get_client_policy` / `get_policy` は、この順で見つかったポリシーを返し、どこにもなければ nil（`None`）。

## API

| | Rust（コア） | Python | C++ | Java |
| --- | --- | --- | --- | --- |
| 型の番号 | `policy::SYNC_SCOPE_POLICY_TYPE` | `Messaging.SYNC_SCOPE_POLICY_TYPE` | `Messaging::SYNC_SCOPE_POLICY_TYPE` | `org.omg.Messaging.SYNC_SCOPE_POLICY_TYPE.value` |
| 値 | `pi::sync_scope::SYNC_NONE` など（`i16`） | `Messaging.SYNC_NONE` など | `Messaging::SYNC_NONE` など（`Messaging::SyncScope` = `int16_t`） | `SYNC_NONE.value` など（`short`） |
| 作る | `Orb::create_policy(24, &Any)`（`short`、別名の `Messaging::SyncScope` も可）→ `PolicyValue::SyncScope(v)` | `orb.create_policy(Messaging.SYNC_SCOPE_POLICY_TYPE, CORBA.Any(Messaging._tc_SyncScope, v))` → `Messaging.SyncScopePolicy` | `orb->create_policy(...)`、または `CORBA::make_reference<Messaging::SyncScopePolicy>(v)` | `orb.create_policy(...)`（`SyncScopeHelper.insert`）→ `SyncScopePolicy` |
| 値を読む | `PolicyValue::synchronization()` | `p.synchronization` | `p->synchronization()` | `p.synchronization()` |

C ABI は他のポリシーと同じく型と値の組で渡す（`RORB_SYNC_SCOPE_POLICY_TYPE` = 24、値は 0〜3）。

`create_policy` の `PolicyError`：`short` 以外の any は `BAD_POLICY_TYPE`、0〜3 以外は `BAD_POLICY_VALUE`。

## Portable Interceptors

- クライアント：`ClientRequestInfo::sync_scope()` は oneway の要求を送るときの値（上の優先順位で決まったもの）。二方向は `SYNC_WITH_TARGET`。`get_request_policy(24)` は実効の `SyncScopePolicy`。oneway は `response_expected` が false なので、返事が来ても（`SYNC_WITH_SERVER` / `SYNC_WITH_TARGET`）終わりの点は今までどおり `receive_other`。例外なら `receive_exception`。
- サーバー：`ServerRequestInfo::sync_scope()` は `response_flags` から読んだ値。`0x00` は `SYNC_NONE`、`0x01` は `SYNC_WITH_SERVER`、`0x03` は `SYNC_WITH_TARGET`（二方向の要求と区別できない）。`response_expected` は `response_flags` の下位ビット。

## サーバー側

`response_flags` が `0x01` の要求は、対象の POA とサーバントが見つかった後（サーバントマネージャーも含む）、サーバントを呼ぶ前に空の `NO_EXCEPTION` の返事を返し、サーバントの後の返事は送らない。サーバントが見つからないときは、その例外（`OBJECT_NOT_EXIST` など）や `LOCATION_FORWARD` を返事にする。`0x03` の oneway は二方向と同じく、サーバントが終わってから返事をする。

## omniORB との比較

`interop/scripts/wire-probe` は、`InteropTest::Echo` の oneway の `post` を `response_flags` 0x00・0x01・0x03 で、存在するオブジェクトとしないオブジェクトに送り、返事が来るかと中身を記録する（`interop/expected/wire-probe-*.txt`）。rustorb のクライアントからは、相互接続のマトリクスの `oneway post with SyncScope`（Rust のクライアント）が omniORB C++ / omniORBpy のサーバーに `SYNC_WITH_SERVER` と `SYNC_WITH_TARGET` で送り、`SYNC_WITH_TARGET` が戻った時点でサーバントが動いていることを確かめる。

## テスト

- コア：`crates/rustorb-core/tests/sync_scope.rs`（`create_policy`、オーバーライドの順、線上の `response_flags` と戻るタイミング、`SYNC_NONE` が書き込みを待たないこと、rustorb のサーバーが `SYNC_WITH_SERVER` にサーバントの前に答えること、同じプロセス内の呼び出し、インターセプターの見える値）
- Python：`python/tests/test_sync_scope.py`
- C++：`cpp/tests/test_orb_ops.cc` の `sync_scope`
- Java：`java/src/test/java/org/rustorb/test/SyncScopeTest.java`
