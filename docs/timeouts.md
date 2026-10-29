# クライアントのタイムアウト

issue #38。呼び出しのタイムアウトを Rust・Python・C++・Java の API と omniORB 互換シムに出す。挙動は omniORBpy 4.3.4 を外から測って合わせた（`interop/scripts/timeout-probe`、clean-room 方針どおりソースは読んでいない）。

## 設定の種類と優先順位

| 設定 | 範囲 | Rust | Python | C++ | Java | omniORB 互換 |
| --- | --- | --- | --- | --- | --- | --- |
| リクエスト | 1 回の呼び出し | `Request::timeout` | － | － | － | － |
| 参照 | オブジェクト参照（narrow したものと共有） | `ObjectRef::set_call_timeout` | `rustorb.timeouts.set_object_call_timeout` | `rustorb::setClientCallTimeout(obj, ms)` | `Timeouts.setClientCallTimeout(obj, ms)` | `omniORB.setClientCallTimeout(obj, ms)` |
| スレッド | 呼び出すスレッド（すべての ORB） | `rustorb_core::set_thread_call_timeout` | `rustorb.timeouts.set_thread_call_timeout` | `rustorb::setClientThreadCallTimeout(ms)` | `Timeouts.setClientThreadCallTimeout(ms)` | `omniORB.setClientThreadCallTimeout(ms)` |
| ORB | ORB 全体（`-ORBclientCallTimeOutPeriod` が初期値） | `Orb::set_call_timeout` | `rustorb.timeouts.set_call_timeout` | `rustorb::setClientCallTimeout([orb,] ms)` | `Timeouts.setClientCallTimeout([orb,] ms)` | `omniORB.setClientCallTimeout(ms)` |
| 接続 | ORB 全体（`-ORBclientConnectTimeOutPeriod` が初期値） | `Orb::set_connect_timeout` | `rustorb.timeouts.set_connect_timeout` | `rustorb::setClientConnectTimeout([orb,] ms)` | `Timeouts.setClientConnectTimeout([orb,] ms)` | `omniORB.setClientConnectTimeout(ms)` |

呼び出しはリクエスト、参照、スレッド、ORB の順で最初に設定されているものを使う。参照がスレッドより強いのは omniORB の実測に合わせた（「参照 300 ms・スレッド 1000 ms」は 300 ms、「参照 1000 ms・スレッド 300 ms」は 1000 ms で切れる）。ORB 引数のない C++・Java・互換シムの関数は、最初に初期化されてまだ破棄されていない ORB に効く（omniORB はプロセスに ORB が一つ）。

- **0 の意味**：omniORB の関数と同じ名前の API（C++・Java・互換シム）では、ORB の 0 は「制限なし」、参照とスレッドの 0 は「未設定（次の設定に任せる）」。omniORB で「全体 300 ms・参照 0」「全体 300 ms・スレッド 0」がどちらも 300 ms で切れたのに合わせた。Rust と `rustorb.timeouts` では未設定（`None`）と制限なし（`Some(None)`・`math.inf`）を分けて書ける。C ABI ではマイクロ秒で、負が未設定、0 が制限なし
- **スレッドの設定**：omniORB は `-ORBsupportPerThreadTimeOut 1` がないとスレッドの設定を無視する（測った）。rustorb は常に効かせ、このオプションは受け付けて無視する
- **参照の設定**：同じ参照の複製（Rust の `clone`、C++ の narrow、Python の narrow、Java のスタブ）とは共有し、同じ IOR から `string_to_object` で作り直した参照とは共有しない（omniORB も別の参照には効かなかった）

## タイムアウトが効く範囲と例外

タイムアウトは呼び出し全体の締め切りで、接続、`LOCATION_FORWARD` の追跡、返事を待つ間のすべてにかかる。接続のタイムアウトを設定すると、接続している間だけそちらが締め切りになる（omniORB で「接続 1000 ms・呼び出し 300 ms」で応答しないリスナーにつなぐと 1000 ms、「接続 300 ms・呼び出し 1000 ms」は 300 ms）。

| 切れたところ | 例外 | rustorb がサーバーに送るもの |
| --- | --- | --- |
| 接続中 | `TIMEOUT`、COMPLETED_NO | なし（ほかのアドレスも試さない） |
| リクエストを送った後 | `TIMEOUT`、COMPLETED_MAYBE | `CancelRequest`。接続はそのまま使う |

omniORB も同じ例外と完了状態だった。違いは次のとおりで、どちらも rustorb の選択として期待値ファイルに残している。

- マイナーコード：omniORB は自社の `TIMEOUT_CallTimedOutOnClient`（0x41540008）、rustorb は 0。互換シムの `omniORB.TIMEOUT_CallTimedOutOnClient` は rustorb が上げる 0 にした（`TRANSIENT_ConnectFailed` と同じ考え方）
- 返事を待つ間に切れたとき、omniORB は `CancelRequest` を送らず接続を閉じる。rustorb は GIOP の `CancelRequest` を送り、接続は他の呼び出しに使い続ける
- `corbaloc` の参照への最初の呼び出しで omniORB は `LocateRequest` を先に送る。rustorb は送らない

以前は接続のタイムアウトが切れると `TRANSIENT` だったが、omniORB に合わせて `TIMEOUT`（COMPLETED_NO）にした。接続を拒否されたときは今までどおり `TRANSIENT`。

## 試験

- `crates/rustorb-core/tests/client.rs`：優先順位、スレッドごと・参照ごとの範囲、未設定への戻し、`CancelRequest`、接続中のタイムアウト（accept キューが詰まったリスナー）
- `python/tests/test_timeouts.py`、`cpp/tests/test_timeout.cc`、`java/.../TimeoutTest.java`：各言語の API で同じ内容を、返事をしないサーバーに対して確かめる
- `interop/run-timeout-probe.sh`：同じスクリプトを omniORBpy と rustorbpy（互換シム経由）で動かし、`interop/expected/timeout-probe-*.txt` と比べる。時間は設定した値のどれに近いかで出すので、マシンの速さに左右されない

標準の `Messaging::RelativeRoundtripTimeoutPolicy`（`ORB::create_policy` で作り、参照の `_set_policy_overrides`、`ORBPolicyManager`、`PolicyCurrent` で設定する）は同じタイムアウトの別の書き方で、[orb-operations.md](orb-operations.md) にまとめた。

## 残り

- `omniORB.setClientThreadCallDeadline`（絶対時刻の締め切り）と `installTimeoutExceptionHandler`（`TIMEOUT` での再試行）。使うアプリケーションが出てきたら入れる
