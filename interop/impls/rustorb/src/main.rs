//! rustorb client for the interop tests.
//!
//! Resolves the Echo object from the naming service (with the
//! `rustorb-naming` client) and runs the same cases
//! as the omniORB clients (see `interop/README.md`), with the stubs
//! `rustorb-idl -brust` generates (`src/idl/`). For the callback cases it serves an
//! `InteropTest::Callback` object from a PERSISTENT / USER_ID POA named
//! `InteropTestClient` (ObjectId `Callback`), as the omniORB clients do.
//!
//! Options: `--naming CORBALOC --name PATH [--host HOST] [--port N]
//! [--timeout S] [--shutdown-server]`, plus ORB options such as
//! `-ORBmaxGIOPVersion 1.1` (also read from the `ORBmaxGIOPVersion`
//! environment variable, as omniORB does). `--host` and `--port` give the
//! endpoint of the callback object.

mod anytest;
mod customtest;
mod idl;
mod valuetest;

use std::fmt;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use idl::interop_test::{
    Callback, CallbackPoa, CallbackServant, Color, Echo, MAGIC, Point, Record, Shade, Value, echo,
};
use rustorb_core::{
    CompletionStatus, Exception, IdAssignment, Lifespan, Orb, Policy, SystemException,
    SystemExceptionKind,
};
use rustorb_naming::{NamingContext, NamingError, to_name};

/// `InteropTest::Callback`: `notify` returns `"ack:" + message`.
struct Acknowledger;

impl CallbackServant for Acknowledger {
    fn notify(&self, message: &str) -> Result<String, SystemException> {
        Ok(format!("ack:{message}"))
    }
}

/// Activates the Callback object and returns its reference.
fn serve_callback(orb: &Orb) -> Result<Callback, String> {
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
    let cb = CallbackPoa(Acknowledger)
        .activate_with_id(&poa, b"Callback")
        .map_err(|e| format!("activate_object_with_id: {e}"))?;
    manager.activate().map_err(|e| format!("activate: {e}"))?;
    Ok(cb)
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

/// Checks that an echo operation returned what was sent.
fn same<T, E>(sent: &T, got: Result<T, E>) -> Result<(), String>
where
    T: PartialEq + fmt::Debug,
    E: fmt::Display,
{
    let got = got.map_err(|e| e.to_string())?;
    check(&got == sent, || format!("sent {sent:?}, got {got:?}"))
}

fn s(e: impl fmt::Display) -> String {
    e.to_string()
}

fn run_cases(r: &mut Runner, e: &Echo, cb: &Callback) {
    for (i, v) in [true, false].into_iter().enumerate() {
        r.case(&format!("echo_boolean[{i}]"), || {
            same(&v, e.echo_boolean(v))
        });
    }
    for (i, v) in [0u8, 0x7f, 0xff].into_iter().enumerate() {
        r.case(&format!("echo_octet[{i}]"), || same(&v, e.echo_octet(v)));
    }
    for (i, v) in ['a', '\0', '~'].into_iter().enumerate() {
        r.case(&format!("echo_char[{i}]"), || same(&v, e.echo_char(v)));
    }
    for (i, v) in ['a', 'あ', 'é'].into_iter().enumerate() {
        r.case(&format!("echo_wchar[{i}]"), || same(&v, e.echo_wchar(v)));
    }
    for (i, v) in [0i16, i16::MIN, i16::MAX].into_iter().enumerate() {
        r.case(&format!("echo_short[{i}]"), || same(&v, e.echo_short(v)));
    }
    for (i, v) in [0u16, u16::MAX].into_iter().enumerate() {
        r.case(&format!("echo_ushort[{i}]"), || same(&v, e.echo_ushort(v)));
    }
    for (i, v) in [0i32, i32::MIN, i32::MAX, MAGIC].into_iter().enumerate() {
        r.case(&format!("echo_long[{i}]"), || same(&v, e.echo_long(v)));
    }
    for (i, v) in [0u32, u32::MAX].into_iter().enumerate() {
        r.case(&format!("echo_ulong[{i}]"), || same(&v, e.echo_ulong(v)));
    }
    for (i, v) in [0i64, i64::MIN, i64::MAX].into_iter().enumerate() {
        r.case(&format!("echo_longlong[{i}]"), || {
            same(&v, e.echo_longlong(v))
        });
    }
    for (i, v) in [0u64, u64::MAX].into_iter().enumerate() {
        r.case(&format!("echo_ulonglong[{i}]"), || {
            same(&v, e.echo_ulonglong(v))
        });
    }
    for (i, v) in [0.0f64, -1.5, 1e300, f64::from_bits(1)]
        .into_iter()
        .enumerate()
    {
        r.case(&format!("echo_double[{i}]"), || same(&v, e.echo_double(v)));
    }
    for (i, v) in ["", "hello", &"x".repeat(10000), "caf\u{e9}"]
        .into_iter()
        .enumerate()
    {
        r.case(&format!("echo_string[{i}]"), || {
            same(&v.to_owned(), e.echo_string(v))
        });
    }
    for (i, v) in ["", "hello", "こんにちは", "\u{1F980}"]
        .into_iter()
        .enumerate()
    {
        r.case(&format!("echo_wstring[{i}]"), || {
            same(&v.to_owned(), e.echo_wstring(v))
        });
    }
    for (i, v) in [0.0f32, -1.5, f32::MAX, f32::MIN_POSITIVE]
        .into_iter()
        .enumerate()
    {
        r.case(&format!("echo_float[{i}]"), || {
            let got = e.echo_float(v).map_err(s)?;
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
        Color::VALUES
            .into_iter()
            .try_for_each(|c| same(&c, e.echo_color(c)))
    });
    r.case("echo_point", || same(&p, e.echo_point(&p)));
    r.case("echo_record", || same(&rec, e.echo_record(&rec)));
    r.case("echo_records", || {
        let v = vec![rec.clone(), rec2.clone(), rec.clone()];
        same(&v, e.echo_records(&v))
    });
    r.case("echo_records[empty]", || same(&vec![], e.echo_records(&[])));
    let large: Vec<Record> = (0..400)
        .map(|i| Record {
            id: i,
            name: format!("r{i}"),
            hue: Color::VALUES[(i % 3) as usize],
            pos: Point {
                x: f64::from(i) + 0.5,
                y: -0.25 * f64::from(i),
            },
            values: vec![i, i + 1],
            active: i % 2 == 0,
        })
        .collect();
    r.case("echo_records[large]", || {
        same(&large, e.echo_records(&large))
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
        r.case(&format!("echo_value[{i}]"), || same(&v, e.echo_value(&v)));
    }
    for (i, v) in [
        Shade::Level(200),
        Shade::Label("leaf".into()),
        Shade::Default(Color::Blue),
    ]
    .into_iter()
    .enumerate()
    {
        r.case(&format!("echo_shade[{i}]"), || same(&v, e.echo_shade(&v)));
    }
    r.case("echo_long_array", || {
        let v = [1, -1, i32::MAX, 0];
        same(&v, e.echo_long_array(&v))
    });
    r.case("echo_matrix", || {
        let v = [[1.0, 2.0, 3.0], [-4.0, 0.5, 1e-9]];
        same(&v, e.echo_matrix(&v))
    });
    r.case("echo_long_seq", || {
        let v: Vec<i32> = (-500..500).collect();
        same(&v, e.echo_long_seq(&v))
    });
    r.case("echo_long_seq[empty]", || {
        same(&vec![], e.echo_long_seq(&[]))
    });
    r.case("echo_string_seq", || {
        let v = vec!["a".to_owned(), String::new(), "ccc".to_owned()];
        same(&v, e.echo_string_seq(&v))
    });
    r.case("echo_octets", || {
        let v: Vec<u8> = (0..=255).collect();
        same(&v, e.echo_octets(&v))
    });

    r.case("swap", || {
        let (mut a, mut b) = (3, -10);
        let ret = e.swap(&mut a, &mut b).map_err(s)?;
        check((ret, a, b) == (-7, -10, 3), || format!("got {ret} {a} {b}"))
    });
    r.case("divide", || {
        let got = e.divide(17, 5).map_err(s)?;
        check(got == (3, 2), || format!("got {got:?}"))?;
        let got = e.divide(-17, 5).map_err(s)?;
        check(got == (-3, -2), || format!("got {got:?}"))?;
        match e.divide(1, 0) {
            Err(echo::DivideError::DivideByZero(_)) => Ok(()),
            other => Err(format!("expected DivideByZero, got {other:?}")),
        }
    });
    r.case("make", || {
        let (pt, rc) = e.make(12).map_err(s)?;
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

    r.case("fail", || match e.fail(7, "boom") {
        Err(echo::FailError::Oops(o)) => check(o.code == 7 && o.reason == "boom", || {
            format!("got {} {:?}", o.code, o.reason)
        }),
        other => Err(format!("expected Oops, got {other:?}")),
    });
    r.case("fail_system", || match e.fail_system() {
        Err(x)
            if x.is(SystemExceptionKind::BAD_PARAM)
                && x.minor == 42
                && x.completed == CompletionStatus::No =>
        {
            Ok(())
        }
        other => Err(format!("expected BAD_PARAM 42, got {other:?}")),
    });

    r.case("attribute counter", || {
        for v in [41, -1] {
            e.set_counter(v).map_err(s)?;
            let got = e.counter().map_err(s)?;
            check(got == v, || format!("set {v}, got {got}"))?;
        }
        Ok(())
    });
    r.case("attribute name", || {
        let got = e.name().map_err(s)?;
        check(got == "rustorb-interop-echo", || format!("got {got:?}"))
    });

    r.case("self_ref", || {
        let got = e.self_ref().map_err(s)?.ok_or("nil reference")?;
        check(got.object().is_equivalent(e.object()), || {
            "not equivalent".into()
        })?;
        let v = got.echo_long(5).map_err(s)?;
        check(v == 5, || format!("got {v}"))
    });
    r.case("echo_ref", || {
        for arg in [Some(e), None] {
            let got = e.echo_ref(arg).map_err(s)?.ok_or("nil reference")?;
            check(got.object().is_equivalent(e.object()), || {
                "not equivalent".into()
            })?;
        }
        Ok(())
    });
    r.case("is_nil", || {
        let nil = e.is_nil(None).map_err(s)?;
        let not_nil = e.is_nil(Some(e)).map_err(s)?;
        check(nil && !not_nil, || format!("got {nil} {not_nil}"))
    });

    r.case("call_back", || {
        let got = e.call_back(Some(cb), "hi").map_err(s)?;
        check(got == "ack:hi", || format!("got {got:?}"))
    });
    r.case("register_callback/trigger", || {
        e.register_callback(Some(cb)).map_err(s)?;
        let got = e.trigger("later").map_err(s)?;
        check(got == "ack:later", || format!("got {got:?}"))
    });

    r.case("oneway post", || {
        e.posted().map_err(s)?;
        let sent = ["one", "two", "three"];
        for m in sent {
            e.post(m).map_err(s)?;
        }
        let mut got: Vec<String> = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while got.len() < sent.len() && Instant::now() < deadline {
            got.extend(e.posted().map_err(s)?);
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
    /// The CustomTest suite (`--suite custom`).
    custom: bool,
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
    let mut custom = false;
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
                (any, value_suite, custom) = match value("--suite")?.as_str() {
                    "basic" => (false, false, false),
                    "any" => (true, false, false),
                    "value" => (false, true, false),
                    "custom" => (false, false, true),
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
            custom,
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
    // Point3 arrives where Point is declared.
    idl::register_value_types();
    idl::custom::register_value_types();
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
    if args.custom {
        return run_custom(&orb, &root, &args, bound);
    }
    let echo = retry("the Echo server", args.timeout, bound, || {
        root.resolve(&name)?.ok_or_else(|| {
            SystemException::new(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No).into()
        })
    })?;
    let is_echo = echo
        .is_a(Echo::REPOSITORY_ID)
        .map_err(|e| format!("_is_a: {e}"))?;
    if !is_echo {
        return Err(format!("{} is not an InteropTest::Echo", args.name));
    }
    let reachable = |e: &Exception| e.as_system().is_some_and(starting);
    retry("the Echo server", args.timeout, reachable, || {
        echo.non_existent()
    })?;

    let echo = Echo::unchecked_narrow(echo);

    let cb = serve_callback(&orb)?;
    let mut r = Runner {
        passed: 0,
        failed: Vec::new(),
    };
    run_cases(&mut r, &echo, &cb);

    if args.shutdown_server {
        echo.shutdown().map_err(|e| format!("shutdown: {e}"))?;
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
        .is_a(idl::any_test::AnyEcho::REPOSITORY_ID)
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
    let echo = idl::any_test::AnyEcho::unchecked_narrow(echo);
    for (case, f) in anytest::cases(&echo) {
        r.case(&case, f);
    }

    if args.shutdown_server {
        let path = format!("{}-Echo", args.name);
        let server = to_name(&path)
            .map_err(|e| e.to_string())
            .and_then(|n| root.resolve(&n).map_err(|e| e.to_string()))?
            .ok_or("nil Echo")?;
        Echo::unchecked_narrow(server)
            .shutdown()
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
        .is_a(idl::value_test::ValueEcho::REPOSITORY_ID)
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
    let echo = idl::value_test::ValueEcho::unchecked_narrow(echo);
    for (case, f) in valuetest::cases(&echo) {
        r.case(&case, f);
    }

    if args.shutdown_server {
        echo.shutdown().map_err(|e| format!("shutdown: {e}"))?;
    }
    println!("\n{} passed, {} failed", r.passed, r.failed.len());
    if !r.failed.is_empty() {
        println!("failed: {}", r.failed.join(", "));
    }
    let _ = orb.shutdown(true);
    Ok(r.failed.is_empty())
}

/// The CustomTest suite against the CustomEcho object at `--name`.
fn run_custom(
    orb: &Orb,
    root: &NamingContext,
    args: &Args,
    bound: impl Fn(&NamingError) -> bool,
) -> Result<bool, String> {
    use idl::custom::custom_test::CustomEcho;
    let name = to_name(&args.name).map_err(|e| format!("--name {}: {e}", args.name))?;
    let echo = retry("the CustomEcho server", args.timeout, bound, || {
        root.resolve(&name)?.ok_or_else(|| {
            SystemException::new(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No).into()
        })
    })?;
    let is_echo = echo
        .is_a(CustomEcho::REPOSITORY_ID)
        .map_err(|e| format!("_is_a: {e}"))?;
    if !is_echo {
        return Err(format!("{} is not a CustomTest::CustomEcho", args.name));
    }
    let reachable = |e: &Exception| e.as_system().is_some_and(starting);
    retry("the CustomEcho server", args.timeout, reachable, || {
        echo.non_existent()
    })?;

    let mut r = Runner {
        passed: 0,
        failed: Vec::new(),
    };
    let echo = CustomEcho::unchecked_narrow(echo);
    for (case, f) in customtest::cases(&echo) {
        r.case(&case, f);
    }

    if args.shutdown_server {
        echo.shutdown().map_err(|e| format!("shutdown: {e}"))?;
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
