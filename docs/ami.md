# Messaging AMI 設計書

Nov 3, 2026

[#135](https://github.com/rustorb/rustorb/issues/135)（sub-issue は着手順に [#136](https://github.com/rustorb/rustorb/issues/136) コア・C ABI、[#137](https://github.com/rustorb/rustorb/issues/137) 生成器の前段、[#138](https://github.com/rustorb/rustorb/issues/138) Python、[#141](https://github.com/rustorb/rustorb/issues/141) C++、[#139](https://github.com/rustorb/rustorb/issues/139) Java、[#140](https://github.com/rustorb/rustorb/issues/140) 相互接続と文書）。CORBA 3.x の Messaging の章にある非同期呼び出し（AMI、Asynchronous Method Invocation）のうち、コールバック型（暗黙の IDL の `sendc_<op>` と `AMI_<I>Handler`）を、C++・Java・Python の生成コードとランタイムに入れる。この文書は実装の前に書いた設計で、実装が進んだら「実装の状況」の節を足す。

根拠は CORBA 3.x の仕様（Messaging の章の「AMI」と「暗黙の IDL」、`Messaging` モジュールの IDL）だけで、他の ORB のソースは読まない（[development-plan.md](development-plan.md) の方針）。AMI の呼び出しは線上では普通の要求と返事なので、相互接続は rustorb の AMI クライアントから他の ORB のサーバーを呼んで確かめる。

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

- `sendc_` は引数を呼び出し元のスレッドでマーシャルしてから返す。返った後に呼び出し元が引数の値を変えても、送る内容は変わらない
- 送信そのもの（接続、転送、アドレスの順の試行、送り直し）は ORB のランタイム（tokio）で行い、`sendc_` は待たない
- `sendc_` が直接投げる例外は、要求を ORB に渡す前に分かるものだけ：ORB が終了している（`BAD_INV_ORDER`、minor 4）、マーシャルの失敗（`MARSHAL` / `BAD_PARAM`）、対象が nil（`INV_OBJREF` か言語の慣習）。それ以外（接続できない、`TRANSIENT`、タイムアウトなど）はすべて Handler の `<op>_excep` に届く
- `ami_handler` が nil なら返事は捨てる（仕様の規則）。要求は普通に送る

### 返事を届けるとき

- 返事（または例外）は ORB のアップコール用スレッドプールのスレッドで受け、生成コードがそれを言語の値に戻して、`ami_handler` の参照の `<op>`（か `<op>_excep`）を **普通の呼び出しとして** 呼ぶ
  - Handler が同じ ORB のサーバントなら同一 ORB 内呼び出しになり、POA のポリシー（スレッド、POAManager の状態）とサーバー側の PI がそのまま効く
  - Handler が別のプロセスにあっても動く（`ExceptionHolder` が valuetype として送れるため）
- 呼び出し元のスレッドが ORB の `run` / `perform_work` を呼んでいなくても届く（rustorb のアップコールはもともと ORB のプールで動く）。MAIN_THREAD_MODEL の POA の Handler だけは、メインスレッドが `run` か `perform_work` を呼ぶまで待つ
- Handler の呼び出しが例外を投げても捨てる（呼び出し元に返す先がない）。`-ORBtraceLevel` が 10 以上なら記録する
- 返事の順は保証しない。同じ対象に続けて `sendc_` しても、Handler の呼ばれる順は返事の届いた順で、Handler はプールの複数のスレッドで並行に呼ばれうる（POA が SINGLE_THREAD_MODEL なら順に）
- PI：クライアントの要求インターセプターは同期の呼び出しと同じに呼ぶ。`send_request` は送るとき（ランタイムのスレッドではなく `sendc_` の呼び出しの中で）、`receive_reply` / `receive_exception` / `receive_other` は返事を受けたプールのスレッドで、Handler を呼ぶ前に。PICurrent のスロットは `sendc_` を呼んだスレッドのものを要求に写す

### `ExceptionHolder`

- `is_system_exception`・`byte_order`・`marshaled_exception` は仕様の状態メンバーで、`marshaled_exception` は例外を CDR でエンコードしたもの（リポジトリ ID から。`byte_order` はそのバイト順）
- `raise_exception()` は、ユーザー例外なら生成コードの読み取り関数（操作の `raises` の例外）で戻して投げ、知らないリポジトリ ID なら `UNKNOWN`（minor 1、同期の呼び出しと同じ）。システム例外ならその例外を投げる
- 返事の例外をいったん CDR に戻すのは、別のプロセスの Handler に送れるようにするため。同じプロセスの Handler に届く `ExceptionHolder` には、エンコードした内容と並べて、言語の例外の値と読み取り関数を持たせ、`raise_exception` でそれを使う（戻し直さない）
- 別のプロセスから届いた `ExceptionHolder` の `raise_exception` は、各言語のランタイムの例外の登録（リポジトリ ID → 型）で探す。登録のない例外は `UNKNOWN`
- `raise_exception_with_list` は引数の TypeCode の一覧から ID で探して戻す。一覧にない例外は `UNKNOWN`

### タイムアウトと終了

- タイムアウト（[timeouts.md](timeouts.md)）は同期の呼び出しと同じ優先順位で、`sendc_` を呼んだ時点で決める。切れたら Handler の `<op>_excep` に `TIMEOUT` が届く（送る前なら `COMPLETED_NO`、送った後なら `COMPLETED_MAYBE`）。`CancelRequest` も同期と同じに送る
- `shutdown` / `destroy` の時点で返事を待っている AMI の要求は、`shutdown(wait_for_completion)` が返る前に Handler の `<op>_excep` に `BAD_INV_ORDER`（minor 4、`COMPLETED_MAYBE`）を届ける。ただし Handler が同じ ORB にあって POA が既に止まっている場合は届かない（呼び出しが `OBJ_ADAPTER` などで失敗し、捨てる）。終了を待つ側が Handler の呼び出しを待ち続けないように、Handler の呼び出しは終了の待ち合わせに含めない

## 各層の作り

### コア（`rustorb-core`、#136）

```rust
impl<'a> Request<'a> {
    /// 引数をここで書き、送信は ORB のランタイムで行う。返事か例外は
    /// アップコール用スレッドプールで `done` に渡す。
    pub fn send_callback<F>(self, done: F) -> Result<(), SystemException>
    where
        F: FnOnce(Result<Reply, Exception>) + Send + 'static;
}
```

- 引数の書き込みは `Request::args` の関数を `send_callback` の中で 1 回だけ呼び、エンコードした本体を持って非同期の呼び出し（`invoke_async` の `Walk`）に渡す。転送や送り直しでは同じ本体を送る。GIOP 1.0 / 1.1 で要求のヘッダーの長さによって本体の位置合わせが変わる場合（#126 と同じ問題）は、本体を書き直す。1.0 / 1.1 の位置合わせの扱いは `send_deferred` の作りを確かめてから実装で決める
- 返事は接続の読み取りタスクで受け、`done` をプールに渡す（I/O のワーカーで `done` を呼ばない）
- 同一 ORB 内の対象は、サーバントの実行もプールに渡す（async API と同じ）
- 待っている AMI の要求を ORB が数えて、`shutdown` で上の「終了」の規則どおりに `done` を呼ぶ
- DII の `send_deferred` は今の作りのまま（チャネルで受ける）。`send_callback` と作りを揃えるかは実装のときに決める

### C ABI（`rustorb-ffi`、#136）

```c
typedef void (*rorb_sendc_done_fn)(void *context, rorb_status status,
                                   rorb_in *reply_or_exception,
                                   const char *user_exception_id,
                                   size_t user_exception_id_len,
                                   const rorb_exception *system_exception);

rorb_status rorb_object_sendc(rorb_object *obj, const rorb_invocation *inv,
                              rorb_sendc_done_fn done, void *done_context,
                              void (*release_context)(void *),
                              rorb_exception *exc);
```

- `inv` の `write_args` は `rorb_object_sendc` の中で呼ぶ。`read_reply` と `read_user_exception` は使わず、`done` が返事の本体かユーザー例外の本体を `rorb_in` で受ける（`done` の中だけで有効）
- `done` は必ずちょうど 1 回呼ぶ（`rorb_object_sendc` が失敗を返したときは呼ばない）。`release_context` は `done` の後に呼ぶ
- `ABI_VERSION` を 3 に上げる

### 生成器（rustorb-idl、#137・#138・#141・#139）

- `-Wbami`：フロントエンドが暗黙の IDL を足す。backend には「Handler は普通のインターフェース」「`sendc_` はスタブだけの操作で、元の操作への参照を持つ」という形で渡し、`sendc_` の生成コードは元の操作の引数の書き込みと返事の読み取りの関数を使い回す
- `-Wbami` なしでは今と同じ出力（golden は変わらない）
- 前段（#137）では `sendc_` を backend に渡さず、Handler だけが各言語に出る。各言語の `sendc_` は #138・#141・#139 でそれぞれ足す
- `-brust -Wbami` は「Rust では使えない」としてエラーにする（決めたこと 2）

### Python（#138）

- 暗黙の IDL を標準の IDL to Python マッピングで写す：`Demo.AMI_CalcHandler`（スタブ）、`Demo__POA.AMI_CalcHandler`（スケルトン）、スタブ `Demo._objref_Calc` に `sendc_add(ami_handler, a, b)` など
- `rustorb.Messaging`（既存）に `ReplyHandler`・`_objref_ReplyHandler`・`Messaging__POA.ReplyHandler`、`ExceptionHolder`（valuetype、`raise_exception()`・`raise_exception_with_list(exc_list)`）を足す
- `rustorb-python`：`send_callback` を Python の呼び出し可能なオブジェクトに橋渡しする。`done` はプールのスレッドで GIL を取って呼ぶ
- omniORBpy 互換のシム（`omniorb-compat`）には何も足さない（omniORBpy の AMI の API との互換は、使うアプリケーションが出てから仕様・ドキュメント・実際の動きで確かめる）
- 試験：`python/tests/test_ami.py`（結果、`inout` / `out`、属性、ユーザー例外、システム例外、タイムアウト、nil の Handler、別の ORB の Handler、並行する多数の `sendc_`、終了時）

### C++（#141）

- 暗黙の IDL を IDL to C++11 で写す：`Demo::AMI_CalcHandler`（`IDL::traits<>`、`_ref_type`）、`POA_Demo::AMI_CalcHandler`、`Calc` のスタブ（`IDL::traits<Calc>::ref_type` の指す型）に `sendc_add(IDL::traits<AMI_CalcHandler>::ref_type ami_handler, int32_t a, int32_t b)`
- `<rustorb/messaging.h>`：`Messaging::ReplyHandler`、`Messaging::ExceptionHolder`（`raise_exception()` が C++ の例外を投げる）。`ExceptionHolder` の valuetype のファクトリを ORB に登録する
- `rorb_object_sendc` の `context` はスタブが作る（Handler の参照と返事を読む関数）。`done` で C++ の値に戻して Handler を呼ぶ
- 試験：`cpp/tests/test_ami.cc`（Python と同じ項目、ASan / LSan で漏れがないこと）

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

- コア：`crates/rustorb-core/tests/ami.rs`（TCP 越しと同一 ORB 内の対象、返事、ユーザー例外、システム例外、転送（`LOCATION_FORWARD`）、タイムアウト、`shutdown` で待っている要求、nil の Handler に当たるもの、PI のインターセプターの呼ばれ方）
- C ABI：`crates/rustorb-ffi/tests` に `rorb_object_sendc`（`done` がちょうど 1 回呼ばれること、`release_context`）
- 生成器：rustorb-idl の golden に `-Wbami` の出力、名前の衝突の IDL
- 各言語：上の各節
