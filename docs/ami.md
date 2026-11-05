# Messaging AMI 設計書

Nov 3, 2026

[#135](https://github.com/rustorb/rustorb/issues/135)（sub-issue は着手順に [#136](https://github.com/rustorb/rustorb/issues/136) コア・C ABI、[#137](https://github.com/rustorb/rustorb/issues/137) 生成器の前段、[#138](https://github.com/rustorb/rustorb/issues/138) Python、[#141](https://github.com/rustorb/rustorb/issues/141) C++、[#139](https://github.com/rustorb/rustorb/issues/139) Java、[#140](https://github.com/rustorb/rustorb/issues/140) 相互接続と文書）。CORBA 3.x の Messaging の章にある非同期呼び出し（AMI、Asynchronous Method Invocation）のうち、コールバック型（暗黙の IDL の `sendc_<op>` と `AMI_<I>Handler`）を、C++・Java・Python の生成コードとランタイムに入れる。この文書は実装の前に書いた設計で、実装の状況は次の節にまとめる。実装で設計から変えたところは各節を直した。

根拠は CORBA 3.x の仕様（Messaging の章の「AMI」と「暗黙の IDL」、`Messaging` モジュールの IDL）だけで、他の ORB のソースは読まない（[development-plan.md](development-plan.md) の方針）。AMI の呼び出しは線上では普通の要求と返事なので、相互接続は rustorb の AMI クライアントから他の ORB のサーバーを呼んで確かめる。

## 実装の状況

- **コア（`rustorb-core`）**：実装済み（#136）。`ObjectRef::request_callback` → `CallbackRequest`（下の「コア」の節）。試験は `crates/rustorb-core/tests/ami.rs`（6 件）
- **C ABI（`rustorb-ffi`）**：実装済み（#136）。`rorb_object_sendc` と `rorb_sendc_done_fn`。`ABI_VERSION` は 3。試験は `crates/rustorb-ffi/tests/sendc.rs`（2 件）
- **生成器の前段（rustorb-idl、#137）**：実装済み（rustorb/rustorb-idl#4）。`-Wbami` で Handler を各言語に出す
- **Python（#138）**：実装済み。`rustorb.Messaging` の `ReplyHandler`・`ExceptionHolder`、`rustorb.Messaging__POA`、`CORBA.Object._sendc`、ネイティブの `ObjRef.sendc`。生成器の `sendc_` は rustorb/rustorb-idl#6。試験は `python/tests/test_ami.py`（12 件。rustorb-idl のリリースまでは CI が rustorb-idl のコミットを使う）
- **C++（#141）**：実装済み。`<rustorb/messaging.h>` の `Messaging::ReplyHandler`・`Messaging::ExceptionHolder`・`POA_Messaging::ReplyHandler`・`rustorb::detail::sendc`、C ABI の `rorb_cdr_encode` / `rorb_in_from_bytes`。生成器の `sendc_` は rustorb/rustorb-idl#7。試験は `cpp/tests/test_ami.cc`（12 項目、ASan / LSan でも通す）と `crates/rustorb-ffi/tests/holder_cdr.rs`（4 件）。rustorb-idl のリリースまでは CI が rustorb-idl のコミットを使う
- Java（#139）、相互接続（#140）はまだ

## 目的

C++・Java・Python には、呼び出しの返事を待たずに先に進む標準の手段がない。Rust には `async fn op_async`（[async-api.md](async-api.md)）があり、Python には DII の `send_deferred` もあるが、どちらも標準の AMI の生成コードではない。CORBA の AMI を使うアプリケーション（omniORB C++ の AMI を使うものなど）を rustorb に移せるように、標準の形を入れる。

## 範囲

| 機能 | 初回 | 備考 |
| --- | --- | --- |
| コールバック型：`sendc_<op>`、`sendc_get_<attr>`、`sendc_set_<attr>` | 入れる | C++・Java・Python |
| `AMI_<I>Handler`（`Messaging::ReplyHandler` の派生）と `<op>_excep` | 入れる | 普通のインターフェースとして生成する |
| `Messaging::ExceptionHolder`（`raise_exception`） | 入れる | valuetype として送れる形にする |
| `raise_exception_with_list` | 入れる | 引数の `Dynamic::ExceptionList` の TypeCode で探す |
| ポーリング型：`sendp_<op>` と `AMI_<I>Poller` | 入れない（決めたこと 1） | 需要が出たら |
| Rust の生成コードの AMI | 入れない（決めたこと 2） | `op_async` で足りる |
| TII（時間に依存しない呼び出し）、ルーター、`Messaging::ReplyHandler` への返事の転送 | 対象外 | ORB の外で返事を中継する仕組みで、需要がない |
| `SyncScope`・`RebindPolicy`・`RequestPriority` など他の Messaging のポリシー | 対象外 | unsupported-features.md の別の行。AMI とは独立 |
| PI の `send_poll` | 呼ばない | TII だけの点（[portable-interceptors.md](portable-interceptors.md)） |

## 暗黙の IDL

仕様は、AMI の API を「元の IDL から機械的に作る IDL（暗黙の IDL）」として定め、それを各言語の標準のマッピングで写す。rustorb もこれに従う。生成器は暗黙の IDL を AST に足し、Handler は普通のインターフェースとして、`sendc_` はスタブにだけ出す。

元の IDL：

```idl
module Demo {
  exception Busy { long retry_after; };
  interface Calc {
    long add(in long a, in long b, out long carry) raises (Busy);
    attribute string label;
    readonly attribute long count;
    oneway void ping();
  };
};
```

暗黙の IDL（`-Wbami` のとき）：

```idl
module Demo {
  interface AMI_CalcHandler : Messaging::ReplyHandler {
    void add(in long ami_return_val, in long carry);
    void add_excep(in Messaging::ExceptionHolder excep_holder);
    void get_label(in string ami_return_val);
    void get_label_excep(in Messaging::ExceptionHolder excep_holder);
    void set_label();
    void set_label_excep(in Messaging::ExceptionHolder excep_holder);
    void get_count(in long ami_return_val);
    void get_count_excep(in Messaging::ExceptionHolder excep_holder);
  };
  // Calc のスタブにだけ足す（サーバントは実装しない）
  //   void sendc_add(in AMI_CalcHandler ami_handler, in long a, in long b);
  //   void sendc_get_label(in AMI_CalcHandler ami_handler);
  //   void sendc_set_label(in AMI_CalcHandler ami_handler, in string attr_label);
  //   void sendc_get_count(in AMI_CalcHandler ami_handler);
};
```

規則：

- `sendc_<op>` の引数は、先頭の `ami_handler` と、元の操作の `in` と `inout` の引数（どちらも `in` として、元の順）。戻り値は `void` で、`raises` はない
- Handler の `<op>` の引数は、戻り値が `void` でなければ先頭の `ami_return_val`、続いて元の `inout` と `out` の引数（どちらも `in` として、元の順）
- Handler の `<op>_excep(in Messaging::ExceptionHolder excep_holder)` はユーザー例外とシステム例外の両方を受ける
- 属性は `get_<attr>` / `get_<attr>_excep`、読み書きできるものは `set_<attr>` / `set_<attr>_excep` も。`sendc_set_<attr>` の引数は `attr_<attr>`
- oneway の操作には何も足さない
- Handler の継承は元のインターフェースの継承をなぞる：`interface B : A` なら `AMI_BHandler : AMI_AHandler`。基底のないものは `Messaging::ReplyHandler` を継承する
- Handler は元のインターフェースと同じモジュールに置き、リポジトリ ID も同じ規則（`#pragma prefix` と `typeprefix` を含む）で付ける：`IDL:Demo/AMI_CalcHandler:1.0`
- 名前の衝突：`AMI_<I>Handler` が既にある名前と重なれば `AMI_` を足し続ける（`AMI_AMI_CalcHandler`）。`sendc_<op>` が元のインターフェース（と基底）の名前と重なれば `sendc_` の後に `ami_` を足し続ける（`sendc_ami_add`）。Handler の `<op>_excep` が重なれば同じく `ami_` を挟む。仕様の文言は #137 で実装の前に確かめる
- local interface と abstract interface には足さない。abstract を基底に持つインターフェースでは、基底の操作も（具象の側で）`sendc_` を持つ

`Messaging.idl` は生成器に同梱する（`rustorb-idl --idl-dir` の下、`CosNaming.idl` と同じ扱い）。中身は仕様の IDL から AMI に要るところだけを写す：

```idl
module Messaging {
  interface ReplyHandler {};
  valuetype ExceptionHolder {
    void raise_exception() raises (UserExceptionBase);
    void raise_exception_with_list(in Dynamic::ExceptionList exc_list)
      raises (UserExceptionBase);
    private boolean is_system_exception;
    private boolean byte_order;
    private sequence<octet> marshaled_exception;
  };
};
```

`UserExceptionBase` は IDL では書けない（仕様の擬似的な型）ので、`raise_exception` の `raises` は生成器が特別に扱い、各言語のランタイムが手で書いた `ExceptionHolder` を使う。`-Wbami` の生成物は `Messaging.idl` を `#include` しなくてよい（生成器が暗黙に読む）。

## 振る舞い

### 送るとき

- `sendc_` は引数の値を写し取ってから返す。返った後に呼び出し元が引数の値を変えても、送る内容は変わらない。コアは引数を書く関数を送るとき（転送や送り直しではそのたび）に呼ぶので、写しは各言語のスタブが持つ（C++ は値のコピー、Java は #139 で決める。Python はネイティブモジュールが `sendc_` の中で引数を IDL の値（Rust の値）に変換して持つ）。送る前に CDR にしておく形は、接続ごとに決まるコードセットと、GIOP 1.0 / 1.1 の本体の位置合わせのために取らなかった
- 送信そのもの（接続、転送、アドレスの順の試行、送り直し）は ORB のランタイム（tokio）で行い、`sendc_` は待たない
- `sendc_` が直接投げる例外は、要求を ORB に渡す前に分かるものだけ：ORB が終了している（`BAD_INV_ORDER`、minor 4）、マーシャルの失敗（`MARSHAL` / `BAD_PARAM`）、対象が nil（`INV_OBJREF` か言語の慣習）。それ以外（接続できない、`TRANSIENT`、タイムアウトなど）はすべて Handler の `<op>_excep` に届く
- `ami_handler` が nil なら返事は捨てる（仕様の規則）。要求は普通に送る

### 返事を届けるとき

- 返事（または例外）は ORB の返事用のスレッドプール（サーバントのアップコール用とは別。最初の `sendc_` で作り、`-ORBmaxServerThreadPoolSize` までスレッドを増やし、使わないスレッドは終わる。ORB の `destroy` で止める）のスレッドで受け、生成コードがそれを言語の値に戻して、`ami_handler` の参照の `<op>`（か `<op>_excep`）を **普通の呼び出しとして** 呼ぶ
  - Handler が同じ ORB のサーバントなら同一 ORB 内呼び出しになり、POA のポリシー（スレッド、POAManager の状態）とサーバー側の PI がそのまま効く
  - Handler が別のプロセスにあっても動く（`ExceptionHolder` が valuetype として送れるため）
- 呼び出し元のスレッドが ORB の `run` / `perform_work` を呼んでいなくても届く。サーバー側を持たない ORB（RootPOA を使っていないもの）でも届く。MAIN_THREAD_MODEL の POA の Handler だけは、メインスレッドが `run` か `perform_work` を呼ぶまで待つ
- Handler の呼び出しが例外を投げても捨てる（呼び出し元に返す先がない）。Python は `logging` の `rustorb.ami` に INFO で記録する
- 返事の順は保証しない。同じ対象に続けて `sendc_` しても、Handler の呼ばれる順は返事の届いた順で、Handler はプールの複数のスレッドで並行に呼ばれうる（POA が SINGLE_THREAD_MODEL なら順に）
- PI：クライアントの要求インターセプターは async API の呼び出しと同じに、ORB のランタイムで呼ぶ（`send_request` は送るとき、`receive_*` は返事を受けたとき、どちらも Handler を呼ぶ前）。PICurrent のスロットは `sendc_` を呼んだスレッドのものを要求に写す（DII の `send_deferred` と同じ）

### `ExceptionHolder`

- `is_system_exception`・`byte_order`・`marshaled_exception` は仕様の状態メンバーで、`marshaled_exception` は例外を返事の本体と同じ形で CDR にしたもの（リポジトリ ID の string、続いてユーザー例外ならメンバー、システム例外なら minor と完了状態の unsigned long）。オフセット 0 から位置を合わせる素の CDR（カプセル化ではない）で、`byte_order` はそのバイト順（TRUE がリトルエンディアン）。char / wchar のコードセットは UTF-8 / UTF-16（C ABI の `rorb_cdr_encode` / `rorb_in_from_bytes` と同じ。言語の間で同じ形にする）
- `raise_exception()` は、ユーザー例外なら生成コードの読み取り関数（操作の `raises` の例外）で戻して投げ、知らないリポジトリ ID なら `UNKNOWN`（minor 1、同期の呼び出しと同じ）。システム例外ならその例外を投げる
- 返事の例外をいったん CDR に戻すのは、別のプロセスの Handler に送れるようにするため。同じプロセスの Handler に届く `ExceptionHolder` には、エンコードした内容と並べて、言語の例外の値と読み取り関数を持たせ、`raise_exception` でそれを使う（戻し直さない）
- 別のプロセスから届いた `ExceptionHolder` の `raise_exception` は、各言語のランタイムの例外の登録（リポジトリ ID → 型）で探す。登録のない例外は `UNKNOWN`
- `raise_exception_with_list` は引数の TypeCode の一覧から ID で探して戻す。一覧にない例外は `UNKNOWN`

### タイムアウトと終了

- タイムアウト（[timeouts.md](timeouts.md)）は同期の呼び出しと同じ優先順位で、`sendc_` を呼んだ時点で決める。切れたら Handler の `<op>_excep` に `TIMEOUT` が届く（送る前なら `COMPLETED_NO`、送った後なら `COMPLETED_MAYBE`）。`CancelRequest` も同期と同じに送る
- `shutdown` / `destroy` の時点で返事を待っている AMI の要求は、同期の呼び出しと同じく接続が閉じて `COMM_FAILURE` か `TRANSIENT` で終わり、コアの完了の関数はそれを受ける（必ずちょうど 1 回。ランタイムが先に終わって送信のタスクが捨てられた場合も `COMM_FAILURE` を渡す。捨てたのがランタイムのスレッドなら、同期 API が使えないので別のスレッドで呼ぶ）。ただし ORB が終了した後は Handler を呼ぶ呼び出しも `BAD_INV_ORDER` になるので、同じ ORB の Handler には届かない（スタブはその失敗を捨てる）。設計の段階では `BAD_INV_ORDER` を Handler に届けるとしていたが、終了した ORB からは Handler を呼べないので、この形にした
- `destroy` は返事用のプールを止め、プールで待っている完了の関数は `destroy` を呼んだスレッドで呼ぶ

## 各層の作り

### コア（`rustorb-core`、#136）

```rust
impl ObjectRef {
    pub fn request_callback(&self, operation: &str) -> CallbackRequest;
}

impl CallbackRequest {
    /// 引数を書く関数。送るときに（転送や送り直しではそのたび）
    /// ORB のランタイムで呼ぶので、書くものを自分で持つ。
    pub fn args<F>(self, f: F) -> CallbackRequest
    where
        F: Fn(&mut OutStream<'_>) -> Result<(), SystemException> + Send + Sync + 'static;
    pub fn timeout(self, timeout: Option<Duration>) -> CallbackRequest;
    /// 待たずに送る。`done` は返事か例外を、返事用のプールのスレッドで
    /// ちょうど 1 回受ける。失敗は ORB が終了しているとき（BAD_INV_ORDER）
    /// だけで、そのときは `done` を呼ばない。
    pub fn send<F>(self, done: F) -> Result<(), SystemException>
    where
        F: FnOnce(Result<Reply, Exception>) + Send + 'static;
}
```

- 送信は `AsyncRequest` を ORB のランタイムの新しいタスクで動かす（DII の `send_deferred` と同じ形）。タイムアウトと PICurrent のスロットは `send` を呼んだスレッドで決める
- 結果は返事用のプール（`Orb` が持つ。サーバントのプールとは別）に渡す。I/O のワーカーでは `done` を呼ばない。プールのスレッドはランタイムの外なので、`done` の中で同期 API を使える（Handler の呼び出し）
- `done` と結果をまとめた値（`PendingReply`）は、渡されずに捨てられると（ランタイムの終了でタスクが、`destroy` でプールのジョブが捨てられると）捨てたスレッドで `done` を呼ぶ（ランタイムのスレッドなら、`done` の中の同期の呼び出しが `BAD_INV_ORDER` になるので、`rustorb-reply` という別のスレッドで呼ぶ。#138 の試験で見つけて直した）。これで「ちょうど 1 回」を守る。`done` のパニックは捕まえて捨てる
- 同一 ORB 内の対象は、async API と同じくサーバントをアップコール用のプールで動かす
- DII の `send_deferred` は今の作りのまま（チャネルで受ける）

### C ABI（`rustorb-ffi`、#136）

```c
typedef void (*rorb_sendc_done_fn)(void* context, rorb_status status, const rorb_exception* exc);

rorb_status rorb_object_sendc(rorb_object* obj, const rorb_invocation* inv, rorb_sendc_done_fn done,
                              rorb_exception* exc);
```

- `rorb_invocation` は `rorb_object_invoke` と同じものを使う。`inv` は写し取り、操作名もコピーするので、呼び出し元はすぐ手放してよい。`context` と各コールバックは `done` が呼ばれるまで、どのスレッドからでも使えるようにしておく
- `write_args` は送るときに ORB のランタイムのスレッドで呼ぶ（転送や送り直しではそのたび）。`read_reply` / `read_user_exception` は返事用のプールのスレッドで、同期の呼び出しと同じ約束で呼ぶ
- `done` はその後に必ずちょうど 1 回呼ぶ。`status` は `rorb_object_invoke` の戻り値と同じ（`RORB_OK`・`RORB_USER_EXCEPTION`・`RORB_SYSTEM_EXCEPTION`・`RORB_CALLBACK_FAILED`）で、`exc` は `RORB_SYSTEM_EXCEPTION` の中身（`done` の中だけで有効）。`done` の後に呼ぶコールバックはないので、`done` で `context` を解放する（設計の段階の `release_context` は要らなくなった）
- `rorb_object_sendc` が失敗を返したとき（oneway、`done` が NULL なら `BAD_PARAM`、ORB が終了していれば `BAD_INV_ORDER`）は `done` を呼ばない
- `ABI_VERSION` を 3 に上げた（Java の `Native` の検査も 3 にした）

### 生成器（rustorb-idl、#137・#138・#141・#139）

- `-Wbami`：フロントエンドが暗黙の IDL を足す。backend には「Handler は普通のインターフェース」「`sendc_` はスタブだけの操作で、元の操作への参照を持つ」という形で渡し、`sendc_` の生成コードは元の操作の引数の書き込みと返事の読み取りの関数を使い回す
- `-Wbami` なしでは今と同じ出力（golden は変わらない）
- 前段（#137）では `sendc_` を backend に渡さず、Handler だけが各言語に出る。各言語の `sendc_` は #138・#141・#139 でそれぞれ足す
- `-brust -Wbami` は「Rust では使えない」としてエラーにする（決めたこと 2）

### Python（#138）

- 暗黙の IDL を標準の IDL to Python マッピングで写す：`Demo.AMI_CalcHandler`（スタブ）、`Demo__POA.AMI_CalcHandler`（スケルトン）、スタブ `Demo._objref_Calc` に `sendc_add(ami_handler, a, b)` など。生成コードの `Messaging` はランタイムの `rustorb.Messaging` / `rustorb.Messaging__POA`（`Messaging.idl` からは生成しない）
- `rustorb.Messaging`（既存）に `ReplyHandler`・`_objref_ReplyHandler`・`_tc_ReplyHandler`、`ExceptionHolder`（valuetype、`raise_exception()`・`raise_exception_with_list(exc_list)`、`_tc_ExceptionHolder`、値のファクトリも登録）を足し、`rustorb.Messaging__POA.ReplyHandler` を新しく置く
- 生成した `sendc_` は `CORBA.Object._sendc(Operation, ami_handler, "<op>", "<op>_excep", args)` を呼ぶ。`_sendc` はネイティブの `ObjRef.sendc(op, args, done)` に渡し、ネイティブ側は引数をその場で IDL の値に変換して `CallbackRequest` で送る。返事は返事用のプールのスレッドで GIL なしで読み、GIL を取って `done(kind, value, data, little)` を呼ぶ（kind 0 は結果の list、1 はユーザー例外、2 はシステム例外で、どちらも `marshaled_exception` の CDR を添える）。`done` が Handler の `<op>(*結果)` か `<op>_excep(ExceptionHolder)` を普通の呼び出しとして呼ぶ
- 同じプロセスで作った `ExceptionHolder` は例外の値も持ち（`_NP_exception`）、`raise_exception` はそれを投げる。別の ORB から届いたもの（状態メンバーだけ）は、システム例外は `CORBA.system_exceptions` から、ユーザー例外は `_rt.typecodes`（`raise_exception_with_list` では一覧）の TypeCode で `ORB.unmarshal_exception`（ネイティブ）を使って戻す
- omniORBpy 互換のシム（`omniorb-compat`）には何も足さない（omniORBpy の AMI の API との互換は、使うアプリケーションが出てから仕様・ドキュメント・実際の動きで確かめる）
- 試験：`python/tests/test_ami.py`（結果、`inout` / `out` と引数の写し、属性、ユーザー例外、システム例外、タイムアウト、nil の Handler、別の ORB の Handler（`ExceptionHolder` を valuetype として送る）、並行する 100 件の `sendc_`、例外を投げる Handler、oneway と nil の対象、終了時）

### C++（#141）

- 暗黙の IDL を IDL to C++11 で写す：`Demo::AMI_CalcHandler`（`IDL::traits<>`、`_ref_type`）、`POA_Demo::AMI_CalcHandler`、`Calc` のスタブ（`IDL::traits<Calc>::ref_type` の指す型）に仮想関数 `sendc_add(IDL::traits<AMI_CalcHandler>::ref_type ami_handler, int32_t a, int32_t b)`。属性は `sendc_get_x(ami_handler)` / `sendc_set_x(ami_handler, attr_x)`。生成コードの `Messaging` はランタイムの `<rustorb/messaging.h>`（`Messaging.idl` からは生成しない）
- `<rustorb/messaging.h>`：`Messaging::ReplyHandler`（`_tc_ReplyHandler`）、`POA_Messaging::ReplyHandler`、`Messaging::ExceptionHolder`（valuetype、`raise_exception()`・`raise_exception_with_list(exc_list)` が C++ の例外を投げる）と `OBV_Messaging::ExceptionHolder`。`ExceptionHolder` の値のファクトリはヘッダーの inline 変数で登録する。状態メンバーは仕様どおり private（C++ では protected）
- 生成した `sendc_` は `rustorb::detail::sendc<raises の例外...>(this, "<op>", handler, write, read, excep)` を呼ぶ。`write` は引数を値で写し取ったラムダ（引数の写しはこれ）、`read` は返事を読んで「Handler の `<op>(結果...)` を呼ぶ関数」を返すラムダ、`excep` は `<op>_excep(holder)` を呼ぶラムダ。`sendc` は `context`（Handler の参照と三つの関数）をヒープに作って `rorb_object_sendc` に渡し、`done` で Handler を呼んでから解放する。Handler が nil なら返事は読むだけで捨てる。Handler の呼び出しが投げた例外は捨てる（返事用のプールを止めない）
- `done` で作る `ExceptionHolder`：ユーザー例外は raises の型で読み（知らないものは `UNKNOWN`）、システム例外は `throw_system` と同じ対応で C++ の例外にする。どちらも例外そのもの（`std::exception_ptr`）と `marshaled_exception` の CDR（`rorb_cdr_encode`、ホストのバイト順）を持つ。同じプロセスの Handler には例外そのものを投げ直し、別の ORB から届いたもの（状態メンバーだけ）は `rorb_in_from_bytes` で読む。システム例外はリポジトリ ID から、ユーザー例外は `-Wbami` の生成コードがファイルごとに登録する読み手（`rustorb::detail::UserExceptionRegistration<E>`）で戻す。登録のない ID は `UNKNOWN`（minor 0x4f4d0001、`COMPLETED_YES`）、壊れた CDR は `MARSHAL`
- C ABI に足したもの（`ABI_VERSION` は 3 のまま）：`rorb_cdr_encode(write, context, little_endian, bytes, exc)`（オフセット 0 からの素の CDR を書く）と `rorb_in_from_bytes(data, len, little_endian, orb, input, exc)`（その形を読むストリーム、`rorb_in_release` で手放す）。コードセットはどちらも UTF-8 / UTF-16。Python の `ExceptionHolder` もこの形に揃えた
- CMake：`rustorb_idl_generate(<var> <idl>... OPTIONS -Wbami)`（インストールした `rustorbConfig.cmake` も同じ）
- 試験：`cpp/tests/test_ami.cc`（結果、`inout` / `out` と引数の写し、属性、ユーザー例外、システム例外、タイムアウト、nil の Handler、別の ORB の Handler と `_copy_value`、並行する 100 件の `sendc_`、例外を投げる Handler、終了した ORB の対象、終了時）

### Java（#139）

- 暗黙の IDL を IDL to Java で写す：`Demo.AMI_CalcHandler`・`AMI_CalcHandlerOperations`・`AMI_CalcHandlerPOA`・`AMI_CalcHandlerHelper` など
- `sendc_` はスタブクラス `Demo._CalcStub` に置く。`Calc` / `CalcOperations` には足さない（サーバントが実装しなくてよいように）。使う側は `((Demo._CalcStub) calc).sendc_add(handler, a, b)` のようにスタブにキャストする。これは仕様の Java の写し方を #139 で実装の前に確かめ、仕様が別の置き場所を定めていればそれに従う
- `org.omg.Messaging`（既存）に `ReplyHandler` 一式と `ExceptionHolder`（valuetype、`raise_exception()` が `org.omg.CORBA.UserException` を投げる）を足す
- FFM：`rorb_object_sendc` の `done` を upcall にする。プールのスレッドは今と同じく JVM にアタッチする
- 試験：`AmiTest`

## 相互接続（#140）

AMI のクライアントの要求は線上では普通の要求なので、サーバー側の ORB には何も要らない。確かめるのは、rustorb の AMI クライアントが他の ORB のサーバーの返事・例外を正しく Handler に届けることと、他の ORB のクライアントから rustorb のサーバーを呼んでも何も変わらないこと。

- `interop/run-ami-probe.sh`：rustorbpy・rustorbcpp・rustorbjava の AMI クライアントから、rustorb・rustorbpy・omniORB C++・omniORBpy・JacORB のサーバーを呼ぶ。結果、`out` / `inout`、ユーザー例外、システム例外（サーバーが投げる `BAD_PARAM` など）、タイムアウト
- Handler を別のプロセスに置く組（`ExceptionHolder` を valuetype として送る）は、rustorb の各言語どうしで確かめる。他の ORB の Handler に rustorb から返事を届ける組は、その ORB の AMI の生成コードが要るので、omniORB C++ の生成器にそのオプションがあるかを実際に動かして確かめてからにする

## 決めたこと

2026-11-03 に 3 点とも推奨どおりと決めた。

1. **ポーリング型（`sendp_` と `AMI_<I>Poller`）**：入れない。Poller は valuetype で、ORB 側に「返事の受け取り」を持つ必要があり、コールバック型より大きい。C++・Java は `std::future` / `CompletableFuture` を自前で作ればコールバック型の上に載せられる
2. **Rust の AMI**：出さない。`async fn op_async` が同じ役割で、Rust の利用者には自然。`-brust -Wbami` はエラーにする
3. **`-Wbami` で選ぶか、常に出すか**：`-Wbami` のときだけ。Handler のインターフェースが増えて生成物が大きくなり、名前の衝突の規則で既存の利用者の名前が変わることもないようにするため

## 試験

- コア：`crates/rustorb-core/tests/ami.rs`（サーバー側のないクライアントから TCP 越しに 50 個並行に送り、返事用のプールのスレッドで受けること、`send` の後に書く引数、ユーザー例外・システム例外・相手がいないとき、要求と参照のタイムアウト、同一 ORB 内の対象と `done` の中の同期呼び出し、`destroy` で待っている要求がちょうど 1 回終わることと終了後の `BAD_INV_ORDER`、共有したランタイムのワーカーから送ること）。転送と PI は async API の経路そのままなので、そちらの試験に任せた
- C ABI：`crates/rustorb-ffi/tests/holder_cdr.rs`（`rorb_cdr_encode` の両方のバイト順の並び、`rorb_in_from_bytes` での往復、足りない入力、書く関数の失敗）、`crates/rustorb-ffi/tests/sendc.rs`（返事・ユーザー例外・システム例外・`write_args` の失敗がそれぞれの `status` で `done` にちょうど 1 回届くこと、oneway と NULL の `done` を断ること、`destroy` で待っている要求、終了後の失敗）
- 生成器：rustorb-idl の golden に `-Wbami` の出力、名前の衝突の IDL
- 各言語：上の各節
