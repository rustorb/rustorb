# 残りの ORB 操作

issue #39。標準の `CORBA::ORB` と `CORBA::Object` の操作のうち、まだなかったものをコアと Rust・Python・C++・Java に揃えた。挙動の基準は CORBA 3.0 の仕様（4.3.9、4.5、4.8、4.9）で、omniORBpy 4.3.4 を外から測って比べた（`interop/scripts/orb-ops-probe`、clean-room 方針どおりソースは読んでいない）。

## 範囲

| 操作 | 前から | 今回 |
| --- | --- | --- |
| `ORB::id` | Python | C++（`ORB_init` の orb_id、既定は `rustorb`） |
| `object_to_string` / `string_to_object` / `resolve_initial_references` / `run` / `work_pending` / `perform_work` / `shutdown` / `destroy` / `create_*_tc` / 値ファクトリ | 全言語 | － |
| `list_initial_services` | 全言語（`-ORBInitRef` の名前と `RootPOA`・`POACurrent`） | 登録した名前、`ORBPolicyManager`、`PolicyCurrent` を加えた |
| `register_initial_reference` | なし | 全言語 |
| `create_policy` | なし | 全言語。POA の7つのポリシーと `Messaging::RelativeRoundtripTimeoutPolicy` |
| `get_service_information` | なし | 全言語。提供するサービスがないので常に false |
| `Object::get_policy` / `get_client_policy` / `set_policy_overrides` / `get_policy_overrides` / `validate_connection` | なし | 全言語 |
| `PolicyManager`（`ORBPolicyManager`）/ `PolicyCurrent` | なし | 全言語 |

入れなかったもの：DII の `create_list`・`create_operation_list`・`get_default_context`・`send_multiple_requests_*`・`poll_next_response`・`get_next_response`（#44）、`Object::get_domain_managers`（ドメインの仕組みがない）、`get_interface`（インターフェースリポジトリがない）。

## API

| | Rust（コア） | Python | C++ | Java |
| --- | --- | --- | --- | --- |
| ポリシーを作る | `Orb::create_policy(type, &Any)` → `PolicyValue` | `orb.create_policy(type, any)` | `orb->create_policy(type, any)` | `orb.create_policy(type, any)` |
| 初期参照を登録 | `Orb::register_initial_reference(name, Option<&ObjectRef>)`、ローカルオブジェクトは `register_local_initial_reference` | `orb.register_initial_reference(name, obj)` | `orb->register_initial_reference(name, obj)` | `orb.register_initial_reference(name, obj)` |
| 参照のオーバーライド | `ObjectRef::set_policy_overrides` / `policy_overrides` / `client_policy` | `obj._set_policy_overrides` / `_get_policy_overrides` / `_get_policy` / `_get_client_policy` | `obj->_set_policy_overrides` ほか | `obj._set_policy_override` / `_get_policy_overrides` / `_get_policy` / `_get_client_policy` |
| ORB のオーバーライド | `Orb::set_policy_overrides` / `policy_overrides` | `resolve_initial_references("ORBPolicyManager")` | 同左 | 同左 |
| スレッドのオーバーライド | `policy::set_thread_policy_overrides` / `thread_policy_overrides` | `resolve_initial_references("PolicyCurrent")` | 同左 | 同左 |
| 接続の確認 | `ObjectRef::validate_connection` | `obj._validate_connection()` → `(True, [])` | `obj->_validate_connection(inconsistent)` | `obj._validate_connection(holder)` |

- 名前は各言語の標準マッピングに従った。Java の `_set_policy_override`（単数）は IDL to Java の名前
- POA の値を any に入れるため、Python に `PortableServer._tc_LifespanPolicyValue` などを、C++ に `rustorb::cdr::type_code` / `codec` の特殊化（`#include <rustorb/cdr.h>` で `a <<= PortableServer::LifespanPolicyValue::PERSISTENT`）を、Java に `LifespanPolicyValueHelper` などを足した
- `TimeBase::TimeT` は Python では `TimeBase._tc_TimeT`、C++ は `uint64_t`、Java は `insert_ulonglong`
- C ABI はポリシーを型と値の組で渡す（`rorb_orb_create_policy`、`rorb_object_set_policy_overrides` ほか、`rustorb.h` の policies の節）。`PolicyError` は `RORB_POLICY_ERROR`（理由は `index`）

## create_policy

| 入力 | 結果 |
| --- | --- |
| POA のポリシーの型（16〜22）と、その値の enum の any | そのポリシー（Python・C++・Java の POA の `create_*_policy` と同じクラス）。`create_POA` に渡せる |
| 32（`RELATIVE_RT_TIMEOUT_POLICY_TYPE`）と `unsigned long long`（別名の `TimeT` でもよい）の any | `RelativeRoundtripTimeoutPolicy`（`relative_expiry` は 100 ns 単位） |
| 型に合わない any（別の enum、数値など） | `PolicyError`、`BAD_POLICY_TYPE`（2） |
| enum の範囲外の値 | `PolicyError`、`BAD_POLICY_VALUE`（3） |
| 知らない型 | `PolicyError`、`BAD_POLICY`（0） |

enum のリポジトリ ID はバージョン（`:1.0` / `:2.3`）を見ずに比べる。omniORBpy の enum は `:1.0`、rustorb は `:2.3` で、どちらの any も受け付ける。

omniORBpy との違い（期待値ファイルに残している）：

- 型に合わない any に omniORBpy は `BAD_POLICY_VALUE` を返す。仕様（4.8.2.1）は「値の型がそのポリシーの型として正しくない」を `BAD_POLICY_TYPE` としているので、仕様に従った
- omniORBpy は 32 を知らない（`BAD_POLICY`）。rustorb はタイムアウトのポリシーとして作る

## ポリシーのオーバーライドとタイムアウト

rustorb のクライアント側のポリシーは `RelativeRoundtripTimeoutPolicy` だけで、#38 の呼び出しタイムアウト（[timeouts.md](timeouts.md)）の標準の書き方になる。

| オーバーライド | 同じ設定 |
| --- | --- |
| `obj._set_policy_overrides([rt], ...)` が返す参照 | その参照の `set_call_timeout` |
| `PolicyCurrent.set_policy_overrides` | スレッドのタイムアウト（`set_thread_call_timeout`） |
| `ORBPolicyManager.set_policy_overrides` | ORB のタイムアウト（`Orb::set_call_timeout`） |

- `SET_OVERRIDE` はその範囲のオーバーライドを置き換え、`ADD_OVERRIDE` は同じ型のものだけ置き換えて残りは残す。空のリストで `SET_OVERRIDE` すると、その範囲のタイムアウトは未設定になる（ORB では制限なし）
- `set_policy_overrides` は新しい参照を返し、元の参照は変わらない。新しい参照のタイムアウトはその参照だけのもの（元の参照の複製とは共有しない）
- `get_policy_overrides` はその範囲で設定されたものだけを返す（空の型リストはすべての型）。`get_client_policy` は参照、スレッド、ORB の順に最初に設定されたものを返し、どこにもなければ nil（制限なし）。rustorb は IOR からポリシーを読まないので `get_policy` は `get_client_policy` と同じ
- オーバーライドできないポリシー（POA のポリシー）は `NO_PERMISSION`、同じ型が2つあると `BAD_PARAM`（マイナー 30）。どちらも仕様（4.3.9.1、4.9.3.1）のとおり
- 内部のタイムアウトの単位をマイクロ秒から 100 ns にして、ポリシーで設定した値がそのまま読み戻せるようにした
- omniORB の API で設定した「制限なし」（参照・スレッドの `Some(None)`）はポリシーでは表せないので、オーバーライドとしては返さない

omniORBpy の `CORBA.Object` にはこれらの操作がない（測った）。

## register_initial_reference

- 空の名前、すでにある名前（`-ORBInitRef` で与えた名前、登録済みの名前、バインディングが自分で答える `RootPOA`・`POACurrent`・`ORBPolicyManager`・`PolicyCurrent`・Python の `omniINSPOA`）は `InvalidName`
- nil は `BAD_PARAM`、マイナー 27（omniORBpy の実測 0x4f4d001b と同じ）
- 登録したものは `resolve_initial_references` が最初に引き、`list_initial_services` に出る
- ローカルオブジェクト（POA など、IOR のないもの）も登録できる。コアは IOR の代わりにバインディングのオブジェクトを預かり（`register_local_initial_reference`、C ABI は解放関数つきのポインタ）、ORB の `destroy` で手放す

omniORBpy との違い：omniORBpy は `RootPOA` の登録を受け付ける。rustorb は「すでに初期参照である名前は `InvalidName`」という仕様どおりにした。それ以外（空・`-ORBInitRef` の名前・2回目・nil・ローカルオブジェクト）は同じ結果だった。

## validate_connection

参照を `LocateRequest` で結び付け（`LOCATION_FORWARD` も追う）、成功すれば true。rustorb には呼び出しを禁じるポリシーがないので false は返さず、`inconsistent_policies` はいつも空。結び付けに失敗するとその例外（`TRANSIENT`、`TIMEOUT` など）、サーバーがオブジェクトを知らなければ `OBJECT_NOT_EXIST`。ローカルオブジェクトは `NO_IMPLEMENT`。

## 試験

- `crates/rustorb-core/tests/policy.rs`：`create_policy` の型と値の検査、`register_initial_reference`（ローカルオブジェクトと `destroy` を含む）、参照・スレッド・ORB のオーバーライドの優先順位、オーバーライドしたタイムアウトで呼び出しが切れること、`validate_connection`
- `python/tests/test_orb_ops.py`、`cpp/tests/test_orb_ops.cc`、`java/.../OrbOpsTest.java`：各言語の API で同じ内容。`create_policy` で作ったポリシーで `create_POA` できることも見る
- `interop/run-orb-ops-probe.sh`：同じスクリプトを omniORBpy と rustorbpy（互換シム経由）で動かし、`interop/expected/orb-ops-probe-*.txt` と比べる
