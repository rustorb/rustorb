//! rustorb client for the interop tests.
//!
//! Resolves the Echo object from the naming service (with the
//! `rustorb-naming` client) and runs the same cases
//! as the omniORB clients (see `interop/README.md`), with hand-written stubs
//! until the code generator exists. For the callback cases it serves an
//! `InteropTest::Callback` object from a PERSISTENT / USER_ID POA named
//! `InteropTestClient` (ObjectId `Callback`), as the omniORB clients do.
//!
//! Options: `--naming CORBALOC --name PATH [--host HOST] [--port N]
//! [--timeout S] [--shutdown-server]`, plus ORB options such as
//! `-ORBmaxGIOPVersion 1.1` (also read from the `ORBmaxGIOPVersion`
//! environment variable, as omniORB does). `--host` and `--port` give the
//! endpoint of the callback object.

mod anytest;
mod idl;
mod valuetest;

use std::fmt;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use idl::{
    Char, Color, DIVIDE_BY_ZERO_ID, ECHO_ID, Get, MAGIC, OOPS_ID, Point, Put, Record, Shade, Value,
    WChar, WString,
};
use rustorb_core::{
    CompletionStatus, Exception, IdAssignment, InStream, Lifespan, ObjectRef, Orb, Policy, Servant,
    ServerReply, SystemException, SystemExceptionKind, minor,
};
use rustorb_naming::{NamingContext, NamingError, to_name};

const CALLBACK_ID: &str = "IDL:rustorb.org/InteropTest/Callback:1.0";

/// `InteropTest::Callback`: `notify` returns `"ack:" + message`.
struct Callback;

impl Servant for Callback {
    fn repository_id(&self) -> &str {
        CALLBACK_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        match operation {
            "notify" => {
                let message = input.read_string()?;
                reply.output().write_string(&format!("ack:{message}"))
            }
            _ => Err(SystemException::new(
                SystemExceptionKind::BAD_OPERATION,
                minor::UNKNOWN_OPERATION,
                CompletionStatus::No,
            )),
        }
    }
}

/// Activates the Callback object and returns its reference.
fn serve_callback(orb: &Orb) -> Result<ObjectRef, String> {
    let root = orb.root_poa().map_err(|e| format!("RootPOA: {e}"))?;
    let manager = root.the_poa_manager();
    let poa = root
        .create_poa(
            "InteropTestClient",
            Some(&manager),
            &[
                Policy::Lifespan(Lifespan::Persistent),
                Policy::IdAssignment(IdAssignment::User),
            ],
        )
        .map_err(|e| format!("create_POA: {e}"))?;
    poa.activate_object_with_id(b"Callback", Arc::new(Callback))
        .map_err(|e| format!("activate_object_with_id: {e}"))?;
    manager.activate().map_err(|e| format!("activate: {e}"))?;
    poa.id_to_reference(b"Callback")
        .map_err(|e| format!("id_to_reference: {e}"))
}

// --- invocation helpers ----------------------------------------------------

fn call<A: Put, T: Get>(obj: &ObjectRef, op: &str, args: A) -> Result<T, Exception> {
    let reply = obj.request(op).args(|out| args.put(out)).invoke()?;
    let mut input = reply.input();
    let value = T::get(&mut input)?;
    Ok(value)
}

/// Whether `e` is worth retrying while a server starts: TRANSIENT or
/// COMM_FAILURE.
fn starting(e: &SystemException) -> bool {
    e.is(SystemExceptionKind::TRANSIENT) || e.is(SystemExceptionKind::COMM_FAILURE)
}

/// Retries `f` while it fails with an error `retriable` accepts.
fn retry<T, E: fmt::Display>(
    what: &str,
    timeout: Duration,
    retriable: impl Fn(&E) -> bool,
    mut f: impl FnMut() -> Result<T, E>,
) -> Result<T, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                if !retriable(&e) || Instant::now() >= deadline {
                    return Err(format!("{what}: {e}"));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

// --- test runner -------------------------------------------------------------

struct Runner {
    passed: usize,
    failed: Vec<String>,
}

impl Runner {
    fn case(&mut self, name: &str, f: impl FnOnce() -> Result<(), String>) {
        match f() {
            Ok(()) => {
                self.passed += 1;
                println!("ok   {name}");
            }
            Err(e) => {
                self.failed.push(name.to_owned());
                println!("FAIL {name}: {e}");
            }
        }
    }
}

fn check(cond: bool, msg: impl FnOnce() -> String) -> Result<(), String> {
    if cond { Ok(()) } else { Err(msg()) }
}

fn echo<T>(obj: &ObjectRef, op: &str, v: T) -> Result<(), String>
where
    T: Put + Get + PartialEq + std::fmt::Debug,
{
    let got: T = call(obj, op, (&v,)).map_err(|e| e.to_string())?;
    check(got == v, || format!("sent {v:?}, got {got:?}"))
}

fn s(e: Exception) -> String {
    e.to_string()
}

fn run_cases(r: &mut Runner, e: &ObjectRef, cb: &ObjectRef) {
    for (i, v) in [true, false].into_iter().enumerate() {
        r.case(&format!("echo_boolean[{i}]"), || echo(e, "echo_boolean", v));
    }
    for (i, v) in [0u8, 0x7f, 0xff].into_iter().enumerate() {
        r.case(&format!("echo_octet[{i}]"), || echo(e, "echo_octet", v));
    }
    for (i, v) in [b'a', 0, b'~'].into_iter().enumerate() {
        r.case(&format!("echo_char[{i}]"), || echo(e, "echo_char", Char(v)));
    }
    for (i, v) in ['a', 'あ', 'é'].into_iter().enumerate() {
        r.case(&format!("echo_wchar[{i}]"), || {
            echo(e, "echo_wchar", WChar(v as u16))
        });
    }
    for (i, v) in [0i16, i16::MIN, i16::MAX].into_iter().enumerate() {
        r.case(&format!("echo_short[{i}]"), || echo(e, "echo_short", v));
    }
    for (i, v) in [0u16, u16::MAX].into_iter().enumerate() {
        r.case(&format!("echo_ushort[{i}]"), || echo(e, "echo_ushort", v));
    }
    for (i, v) in [0i32, i32::MIN, i32::MAX, MAGIC].into_iter().enumerate() {
        r.case(&format!("echo_long[{i}]"), || echo(e, "echo_long", v));
    }
    for (i, v) in [0u32, u32::MAX].into_iter().enumerate() {
        r.case(&format!("echo_ulong[{i}]"), || echo(e, "echo_ulong", v));
    }
    for (i, v) in [0i64, i64::MIN, i64::MAX].into_iter().enumerate() {
        r.case(&format!("echo_longlong[{i}]"), || {
            echo(e, "echo_longlong", v)
        });
    }
    for (i, v) in [0u64, u64::MAX].into_iter().enumerate() {
        r.case(&format!("echo_ulonglong[{i}]"), || {
            echo(e, "echo_ulonglong", v)
        });
    }
    for (i, v) in [0.0f64, -1.5, 1e300, f64::from_bits(1)]
        .into_iter()
        .enumerate()
    {
        r.case(&format!("echo_double[{i}]"), || echo(e, "echo_double", v));
    }
    for (i, v) in ["", "hello", &"x".repeat(10000), "caf\u{e9}"]
        .into_iter()
        .enumerate()
    {
        r.case(&format!("echo_string[{i}]"), || {
            echo(e, "echo_string", v.to_owned())
        });
    }
    for (i, v) in ["", "hello", "こんにちは", "\u{1F980}"]
        .into_iter()
        .enumerate()
    {
        r.case(&format!("echo_wstring[{i}]"), || {
            echo(e, "echo_wstring", WString(v.to_owned()))
        });
    }
    for (i, v) in [0.0f32, -1.5, f32::MAX, f32::MIN_POSITIVE]
        .into_iter()
        .enumerate()
    {
        r.case(&format!("echo_float[{i}]"), || {
            let got: f32 = call(e, "echo_float", (v,)).map_err(s)?;
            check(got.to_bits() == v.to_bits(), || {
                format!("sent {v:?}, got {got:?}")
            })
        });
    }

    let p = Point { x: 1.25, y: -2.5 };
    let rec = Record {
        id: 7,
        name: "seven".into(),
        hue: Color::Green,
        pos: p,
        values: vec![1, -2, 3],
        active: true,
    };
    let rec2 = Record {
        id: -1,
        name: String::new(),
        hue: Color::Red,
        pos: Point { x: 0.0, y: 0.0 },
        values: vec![],
        active: false,
    };
    r.case("echo_color", || {
        [Color::Red, Color::Green, Color::Blue]
            .into_iter()
            .try_for_each(|c| echo(e, "echo_color", c))
    });
    r.case("echo_point", || echo(e, "echo_point", p));
    r.case("echo_record", || echo(e, "echo_record", rec.clone()));
    r.case("echo_records", || {
        echo(
            e,
            "echo_records",
            vec![rec.clone(), rec2.clone(), rec.clone()],
        )
    });
    r.case("echo_records[empty]", || {
        echo(e, "echo_records", Vec::<Record>::new())
    });
    let large: Vec<Record> = (0..400)
        .map(|i| Record {
            id: i,
            name: format!("r{i}"),
            hue: [Color::Red, Color::Green, Color::Blue][(i % 3) as usize],
            pos: Point {
                x: f64::from(i) + 0.5,
                y: -0.25 * f64::from(i),
            },
            values: vec![i, i + 1],
            active: i % 2 == 0,
        })
        .collect();
    r.case("echo_records[large]", || {
        echo(e, "echo_records", large.clone())
    });
    let values = [
        Value::L(-5),
        Value::S("union".into()),
        Value::P(p),
        Value::Seq(4, vec![1, 2]),
        Value::Seq(5, vec![]),
        Value::Flag(99, true),
        Value::Flag(0, false),
    ];
    for (i, v) in values.into_iter().enumerate() {
        r.case(&format!("echo_value[{i}]"), || echo(e, "echo_value", v));
    }
    for (i, v) in [Shade::Level(200), Shade::Label("leaf".into()), Shade::Blue]
        .into_iter()
        .enumerate()
    {
        r.case(&format!("echo_shade[{i}]"), || echo(e, "echo_shade", v));
    }
    r.case("echo_long_array", || {
        echo(e, "echo_long_array", [1i32, -1, i32::MAX, 0])
    });
    r.case("echo_matrix", || {
        echo(e, "echo_matrix", [[1.0f64, 2.0, 3.0], [-4.0, 0.5, 1e-9]])
    });
    r.case("echo_long_seq", || {
        echo(e, "echo_long_seq", (-500..500).collect::<Vec<i32>>())
    });
    r.case("echo_long_seq[empty]", || {
        echo(e, "echo_long_seq", Vec::<i32>::new())
    });
    r.case("echo_string_seq", || {
        echo(
            e,
            "echo_string_seq",
            vec!["a".to_owned(), String::new(), "ccc".to_owned()],
        )
    });
    r.case("echo_octets", || {
        echo(e, "echo_octets", (0..=255u8).collect::<Vec<u8>>())
    });

    r.case("swap", || {
        let got: (i32, i32, i32) = call(e, "swap", (3i32, -10i32)).map_err(s)?;
        check(got == (-7, -10, 3), || format!("got {got:?}"))
    });
    r.case("divide", || {
        let got: (i32, i32) = call(e, "divide", (17i32, 5i32)).map_err(s)?;
        check(got == (3, 2), || format!("got {got:?}"))?;
        let got: (i32, i32) = call(e, "divide", (-17i32, 5i32)).map_err(s)?;
        check(got == (-3, -2), || format!("got {got:?}"))?;
        match call::<_, (i32, i32)>(e, "divide", (1i32, 0i32)) {
            Err(Exception::User(u)) if u.repository_id() == DIVIDE_BY_ZERO_ID => Ok(()),
            other => Err(format!("expected DivideByZero, got {other:?}")),
        }
    });
    r.case("make", || {
        let (pt, rc): (Point, Record) = call(e, "make", (12i32,)).map_err(s)?;
        let want_p = Point { x: 12.0, y: -12.0 };
        let want_r = Record {
            id: 12,
            name: "r12".into(),
            hue: Color::Blue,
            pos: want_p,
            values: vec![12, 13],
            active: true,
        };
        check(pt == want_p && rc == want_r, || {
            format!("got {pt:?} {rc:?}")
        })
    });

    r.case("fail", || match call::<_, ()>(e, "fail", (7i32, "boom")) {
        Err(Exception::User(u)) if u.repository_id() == OOPS_ID => {
            let mut input = u.input().map_err(|e| e.to_string())?;
            let (code, reason): (i32, String) = Get::get(&mut input).map_err(|e| e.to_string())?;
            check(code == 7 && reason == "boom", || {
                format!("got {code} {reason:?}")
            })
        }
        other => Err(format!("expected Oops, got {other:?}")),
    });
    r.case("fail_system", || {
        match call::<_, ()>(e, "fail_system", ()) {
            Err(Exception::System(x))
                if x.is(SystemExceptionKind::BAD_PARAM)
                    && x.minor == 42
                    && x.completed == CompletionStatus::No =>
            {
                Ok(())
            }
            other => Err(format!("expected BAD_PARAM 42, got {other:?}")),
        }
    });

    r.case("attribute counter", || {
        for v in [41i32, -1] {
            call::<_, ()>(e, "_set_counter", (v,)).map_err(s)?;
            let got: i32 = call(e, "_get_counter", ()).map_err(s)?;
            check(got == v, || format!("set {v}, got {got}"))?;
        }
        Ok(())
    });
    r.case("attribute name", || {
        let got: String = call(e, "_get_name", ()).map_err(s)?;
        check(got == "rustorb-interop-echo", || format!("got {got:?}"))
    });

    r.case("self_ref", || {
        let got: Option<ObjectRef> = call(e, "self_ref", ()).map_err(s)?;
        let got = got.ok_or("nil reference")?;
        check(got.is_equivalent(e), || "not equivalent".into())?;
        let v: i32 = call(&got, "echo_long", (5i32,)).map_err(s)?;
        check(v == 5, || format!("got {v}"))
    });
    r.case("echo_ref", || {
        for arg in [Some(e.clone()), None] {
            let got: Option<ObjectRef> = call(e, "echo_ref", (arg,)).map_err(s)?;
            let got = got.ok_or("nil reference")?;
            check(got.is_equivalent(e), || "not equivalent".into())?;
        }
        Ok(())
    });
    r.case("is_nil", || {
        let nil: bool = call(e, "is_nil", (None::<ObjectRef>,)).map_err(s)?;
        let not_nil: bool = call(e, "is_nil", (Some(e.clone()),)).map_err(s)?;
        check(nil && !not_nil, || format!("got {nil} {not_nil}"))
    });

    r.case("call_back", || {
        let got: String = call(e, "call_back", (Some(cb.clone()), "hi")).map_err(s)?;
        check(got == "ack:hi", || format!("got {got:?}"))
    });
    r.case("register_callback/trigger", || {
        call::<_, ()>(e, "register_callback", (Some(cb.clone()),)).map_err(s)?;
        let got: String = call(e, "trigger", ("later",)).map_err(s)?;
        check(got == "ack:later", || format!("got {got:?}"))
    });

    r.case("oneway post", || {
        let _: Vec<String> = call(e, "posted", ()).map_err(s)?;
        let sent = ["one", "two", "three"];
        for m in sent {
            e.request("post")
                .args(move |out| out.write_string(m))
                .send_oneway()
                .map_err(|e| e.to_string())?;
        }
        let mut got: Vec<String> = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while got.len() < sent.len() && Instant::now() < deadline {
            got.extend(call::<_, Vec<String>>(e, "posted", ()).map_err(s)?);
            std::thread::sleep(Duration::from_millis(50));
        }
        got.sort();
        let mut want: Vec<String> = sent.iter().map(|s| (*s).to_owned()).collect();
        want.sort();
        check(got == want, || format!("got {got:?}"))
    });
}

struct Args {
    naming: String,
    name: String,
    host: Option<String>,
    port: u16,
    timeout: Duration,
    shutdown_server: bool,
    /// The AnyTest suite (`--suite any`) instead of InteropTest.
    any: bool,
    /// The ValueTest suite (`--suite value`).
    value: bool,
}

fn parse_args(mut args: Vec<String>) -> Result<(Args, Vec<String>), String> {
    let mut naming = None;
    let mut name = None;
    let mut host = None;
    let mut port = 0;
    let mut timeout = Duration::from_secs(30);
    let mut shutdown_server = false;
    let mut any = false;
    let mut value_suite = false;
    let mut rest = Vec::new();
    let mut iter = args.drain(..);
    while let Some(arg) = iter.next() {
        let mut value = |opt: &str| iter.next().ok_or_else(|| format!("{opt} needs a value"));
        match arg.as_str() {
            "--naming" => naming = Some(value("--naming")?),
            "--name" => name = Some(value("--name")?),
            "--timeout" => {
                let v = value("--timeout")?;
                timeout = Duration::from_secs_f64(v.parse().map_err(|_| "bad --timeout")?);
            }
            "--host" => host = Some(value("--host")?),
            "--port" => port = value("--port")?.parse().map_err(|_| "bad --port")?,
            "--shutdown-server" => shutdown_server = true,
            "--suite" => {
                (any, value_suite) = match value("--suite")?.as_str() {
                    "basic" => (false, false),
                    "any" => (true, false),
                    "value" => (false, true),
                    other => return Err(format!("bad --suite {other}")),
                }
            }
            _ => rest.push(arg),
        }
    }
    Ok((
        Args {
            naming: naming.ok_or("--naming is required")?,
            name: name.ok_or("--name is required")?,
            host,
            port,
            timeout,
            shutdown_server,
            any,
            value: value_suite,
        },
        rest,
    ))
}

fn run() -> Result<bool, String> {
    let (args, mut orb_args) = parse_args(std::env::args().skip(1).collect())?;
    // run-matrix.sh --giop sets this for omniORB, which reads ORB options
    // from the environment.
    match std::env::var("ORBmaxGIOPVersion") {
        Ok(v) if !v.is_empty() => orb_args.extend(["-ORBmaxGIOPVersion".to_owned(), v]),
        _ => {}
    }
    if args.port != 0 || args.host.is_some() {
        let host = args.host.clone().unwrap_or_default();
        orb_args.extend([
            "-ORBendPoint".to_owned(),
            format!("giop:tcp:{host}:{}", args.port),
        ]);
    }
    let orb = Orb::init(&mut orb_args).map_err(|e| format!("ORB_init: {e}"))?;
    if !orb_args.is_empty() {
        return Err(format!("unknown arguments: {orb_args:?}"));
    }
    let root = orb
        .string_to_object(&args.naming)
        .map_err(|e| format!("{}: {e}", args.naming))?
        .ok_or("nil naming context")?;
    let root = NamingContext::unchecked_narrow(root);
    let name = to_name(&args.name).map_err(|e| format!("--name {}: {e}", args.name))?;
    // NotFound until the server has bound the object.
    let bound = |e: &NamingError| match e {
        NamingError::NotFound { .. } => true,
        NamingError::System(e) => starting(e),
        _ => false,
    };
    if args.any {
        return run_any(&orb, &root, &args, bound);
    }
    if args.value {
        return run_value(&orb, &root, &args, bound);
    }
    let echo = retry("the Echo server", args.timeout, bound, || {
        root.resolve(&name)?.ok_or_else(|| {
            SystemException::new(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No).into()
        })
    })?;
    let is_echo = echo.is_a(ECHO_ID).map_err(|e| format!("_is_a: {e}"))?;
    if !is_echo {
        return Err(format!("{} is not an InteropTest::Echo", args.name));
    }
    let reachable = |e: &Exception| e.as_system().is_some_and(starting);
    retry("the Echo server", args.timeout, reachable, || {
        echo.non_existent()
    })?;

    let cb = serve_callback(&orb)?;
    let mut r = Runner {
        passed: 0,
        failed: Vec::new(),
    };
    run_cases(&mut r, &echo, &cb);

    if args.shutdown_server {
        echo.request("shutdown")
            .send_oneway()
            .map_err(|e| format!("shutdown: {e}"))?;
    }
    println!("\n{} passed, {} failed", r.passed, r.failed.len());
    if !r.failed.is_empty() {
        println!("failed: {}", r.failed.join(", "));
    }
    let _ = orb.shutdown(true);
    Ok(r.failed.is_empty())
}

/// The AnyTest suite against the AnyEcho object at `--name`; the server's
/// Echo object is at NAME-Echo.
fn run_any(
    orb: &Orb,
    root: &NamingContext,
    args: &Args,
    bound: impl Fn(&NamingError) -> bool,
) -> Result<bool, String> {
    let name = to_name(&args.name).map_err(|e| format!("--name {}: {e}", args.name))?;
    let echo = retry("the AnyEcho server", args.timeout, bound, || {
        root.resolve(&name)?.ok_or_else(|| {
            SystemException::new(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No).into()
        })
    })?;
    let is_echo = echo
        .is_a(anytest::ANY_ECHO_ID)
        .map_err(|e| format!("_is_a: {e}"))?;
    if !is_echo {
        return Err(format!("{} is not an AnyTest::AnyEcho", args.name));
    }
    let reachable = |e: &Exception| e.as_system().is_some_and(starting);
    retry("the AnyEcho server", args.timeout, reachable, || {
        echo.non_existent()
    })?;

    let mut r = Runner {
        passed: 0,
        failed: Vec::new(),
    };
    for (case, f) in anytest::cases(&echo) {
        r.case(&case, f);
    }

    if args.shutdown_server {
        let path = format!("{}-Echo", args.name);
        let server = to_name(&path)
            .map_err(|e| e.to_string())
            .and_then(|n| root.resolve(&n).map_err(|e| e.to_string()))?
            .ok_or("nil Echo")?;
        server
            .request("shutdown")
            .send_oneway()
            .map_err(|e| format!("shutdown: {e}"))?;
    }
    println!("\n{} passed, {} failed", r.passed, r.failed.len());
    if !r.failed.is_empty() {
        println!("failed: {}", r.failed.join(", "));
    }
    let _ = orb.shutdown(true);
    Ok(r.failed.is_empty())
}

/// The ValueTest suite against the ValueEcho object at `--name`.
fn run_value(
    orb: &Orb,
    root: &NamingContext,
    args: &Args,
    bound: impl Fn(&NamingError) -> bool,
) -> Result<bool, String> {
    let name = to_name(&args.name).map_err(|e| format!("--name {}: {e}", args.name))?;
    let echo = retry("the ValueEcho server", args.timeout, bound, || {
        root.resolve(&name)?.ok_or_else(|| {
            SystemException::new(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No).into()
        })
    })?;
    let is_echo = echo
        .is_a(valuetest::VALUE_ECHO_ID)
        .map_err(|e| format!("_is_a: {e}"))?;
    if !is_echo {
        return Err(format!("{} is not a ValueTest::ValueEcho", args.name));
    }
    let reachable = |e: &Exception| e.as_system().is_some_and(starting);
    retry("the ValueEcho server", args.timeout, reachable, || {
        echo.non_existent()
    })?;

    let mut r = Runner {
        passed: 0,
        failed: Vec::new(),
    };
    for (case, f) in valuetest::cases(&echo) {
        r.case(&case, f);
    }

    if args.shutdown_server {
        echo.request("shutdown")
            .send_oneway()
            .map_err(|e| format!("shutdown: {e}"))?;
    }
    println!("\n{} passed, {} failed", r.passed, r.failed.len());
    if !r.failed.is_empty() {
        println!("failed: {}", r.failed.join(", "));
    }
    let _ = orb.shutdown(true);
    Ok(r.failed.is_empty())
}

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
