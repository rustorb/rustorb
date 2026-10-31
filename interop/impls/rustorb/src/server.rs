//! rustorb server for the interop tests.
//!
//! Serves `InteropTest::Echo` with the skeleton `rustorb-idl -brust`
//! generates (`src/idl/`) from a PERSISTENT / USER_ID POA named `InteropTest`,
//! with the ObjectId `Echo`, and binds it in the naming service, like the
//! omniORB servers (see `interop/README.md`).
//!
//! Options: `--naming CORBALOC --name PATH [--host HOST] [--port N]
//! [--timeout S]`, plus ORB options.

mod anytest;
mod customtest;
mod idl;
mod ssl;
mod valuetest;

use std::process::ExitCode;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use idl::interop_test::{
    Callback, Color, Echo, EchoPoa, EchoServant, LongArray, LongSeq, Matrix, OctetSeq, Oops, Point,
    Record, RecordSeq, Shade, StringSeq, Value, echo,
};
use rustorb_core::idl::bad_param;
use rustorb_core::{
    CompletionStatus, IdAssignment, Lifespan, ObjectRef, Orb, Policy, SystemException,
    SystemExceptionKind,
};
use rustorb_naming::{NamingContext, NamingError, to_name};

type Sys<T> = Result<T, SystemException>;

#[derive(Default)]
struct State {
    posted: Vec<String>,
    counter: i32,
    callback: Option<Callback>,
}

/// `InteropTest::Echo`.
#[derive(Clone)]
struct EchoImpl {
    orb: Orb,
    this: Arc<OnceLock<Echo>>,
    state: Arc<Mutex<State>>,
}

impl EchoImpl {
    fn this(&self) -> Option<Echo> {
        self.this.get().cloned()
    }
}

impl EchoServant for EchoImpl {
    fn echo_boolean(&self, v: bool) -> Sys<bool> {
        Ok(v)
    }
    fn echo_octet(&self, v: u8) -> Sys<u8> {
        Ok(v)
    }
    fn echo_char(&self, v: char) -> Sys<char> {
        Ok(v)
    }
    fn echo_wchar(&self, v: char) -> Sys<char> {
        Ok(v)
    }
    fn echo_short(&self, v: i16) -> Sys<i16> {
        Ok(v)
    }
    fn echo_ushort(&self, v: u16) -> Sys<u16> {
        Ok(v)
    }
    fn echo_long(&self, v: i32) -> Sys<i32> {
        Ok(v)
    }
    fn echo_ulong(&self, v: u32) -> Sys<u32> {
        Ok(v)
    }
    fn echo_longlong(&self, v: i64) -> Sys<i64> {
        Ok(v)
    }
    fn echo_ulonglong(&self, v: u64) -> Sys<u64> {
        Ok(v)
    }
    fn echo_float(&self, v: f32) -> Sys<f32> {
        Ok(v)
    }
    fn echo_double(&self, v: f64) -> Sys<f64> {
        Ok(v)
    }
    fn echo_string(&self, v: &str) -> Sys<String> {
        Ok(v.to_owned())
    }
    fn echo_wstring(&self, v: &str) -> Sys<String> {
        Ok(v.to_owned())
    }
    fn echo_color(&self, v: Color) -> Sys<Color> {
        Ok(v)
    }
    fn echo_point(&self, v: &Point) -> Sys<Point> {
        Ok(*v)
    }
    fn echo_record(&self, v: &Record) -> Sys<Record> {
        Ok(v.clone())
    }
    fn echo_records(&self, v: &[Record]) -> Sys<RecordSeq> {
        Ok(v.to_vec())
    }
    fn echo_value(&self, v: &Value) -> Sys<Value> {
        Ok(v.clone())
    }
    fn echo_shade(&self, v: &Shade) -> Sys<Shade> {
        Ok(v.clone())
    }
    fn echo_long_array(&self, v: &LongArray) -> Sys<LongArray> {
        Ok(*v)
    }
    fn echo_matrix(&self, v: &Matrix) -> Sys<Matrix> {
        Ok(*v)
    }
    fn echo_long_seq(&self, v: &[i32]) -> Sys<LongSeq> {
        Ok(v.to_vec())
    }
    fn echo_string_seq(&self, v: &[String]) -> Sys<StringSeq> {
        Ok(v.to_vec())
    }
    fn echo_octets(&self, v: &[u8]) -> Sys<OctetSeq> {
        Ok(v.to_vec())
    }

    fn swap(&self, a: &mut i32, b: &mut i32) -> Sys<i32> {
        let sum = a.wrapping_add(*b);
        std::mem::swap(a, b);
        Ok(sum)
    }

    fn divide(&self, a: i32, b: i32) -> Result<(i32, i32), echo::DivideError> {
        if b == 0 {
            return Err(idl::interop_test::DivideByZero {}.into());
        }
        Ok((a.wrapping_div(b), a.wrapping_rem(b)))
    }

    fn make(&self, x: i32) -> Sys<(Point, Record)> {
        let p = Point {
            x: f64::from(x),
            y: -f64::from(x),
        };
        let r = Record {
            id: x,
            name: format!("r{x}"),
            hue: Color::Blue,
            pos: p,
            values: vec![x, x.wrapping_add(1)],
            active: true,
        };
        Ok((p, r))
    }

    fn fail(&self, code: i32, reason: &str) -> Result<(), echo::FailError> {
        Err(Oops {
            code,
            reason: reason.to_owned(),
        }
        .into())
    }

    fn fail_system(&self) -> Sys<()> {
        Err(SystemException::new(
            SystemExceptionKind::BAD_PARAM,
            42,
            CompletionStatus::No,
        ))
    }

    fn counter(&self) -> Sys<i32> {
        Ok(self.state.lock().unwrap().counter)
    }

    fn set_counter(&self, value: i32) -> Sys<()> {
        self.state.lock().unwrap().counter = value;
        Ok(())
    }

    fn name(&self) -> Sys<String> {
        Ok("rustorb-interop-echo".to_owned())
    }

    fn post(&self, message: &str) -> Sys<()> {
        self.state.lock().unwrap().posted.push(message.to_owned());
        Ok(())
    }

    fn posted(&self) -> Sys<StringSeq> {
        Ok(std::mem::take(&mut self.state.lock().unwrap().posted))
    }

    fn self_ref(&self) -> Sys<Option<Echo>> {
        Ok(self.this())
    }

    fn echo_ref(&self, other: Option<&Echo>) -> Sys<Option<Echo>> {
        Ok(other.cloned().or_else(|| self.this()))
    }

    fn is_nil(&self, other: Option<&Echo>) -> Sys<bool> {
        Ok(other.is_none())
    }

    // A failure of notify is passed on to the client as the system
    // exception it is.
    fn call_back(&self, cb: Option<&Callback>, message: &str) -> Sys<String> {
        cb.ok_or_else(bad_param)?.notify(message)
    }

    fn register_callback(&self, cb: Option<&Callback>) -> Sys<()> {
        self.state.lock().unwrap().callback = cb.cloned();
        Ok(())
    }

    fn trigger(&self, message: &str) -> Result<String, echo::TriggerError> {
        let cb = self.state.lock().unwrap().callback.clone();
        match cb {
            Some(cb) => Ok(cb.notify(message)?),
            None => Err(Oops {
                code: 1,
                reason: "no callback".to_owned(),
            }
            .into()),
        }
    }

    fn shutdown(&self) -> Sys<()> {
        // Without waiting, so the reply (if any) still goes out.
        self.orb.shutdown(false)
    }
}

struct Args {
    naming: String,
    name: String,
    host: Option<String>,
    port: u16,
    timeout: Duration,
    /// The suite: `basic` (InteropTest), `any` (AnyTest), `value`
    /// (ValueTest).
    suite: String,
}

fn parse_args(args: Vec<String>) -> Result<(Args, Vec<String>), String> {
    let mut naming = None;
    let mut name = None;
    let mut host = None;
    let mut port = 0;
    let mut timeout = Duration::from_secs(30);
    let mut suite = "basic".to_owned();
    let mut rest = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let mut value = |opt: &str| iter.next().ok_or_else(|| format!("{opt} needs a value"));
        match arg.as_str() {
            "--naming" => naming = Some(value("--naming")?),
            "--name" => name = Some(value("--name")?),
            "--host" => host = Some(value("--host")?),
            "--port" => port = value("--port")?.parse().map_err(|_| "bad --port")?,
            "--timeout" => {
                let v = value("--timeout")?;
                timeout = Duration::from_secs_f64(v.parse().map_err(|_| "bad --timeout")?);
            }
            "--suite" => {
                suite = value("--suite")?;
                if !matches!(suite.as_str(), "basic" | "any" | "value" | "custom") {
                    return Err(format!("bad --suite {suite}"));
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
            suite,
        },
        rest,
    ))
}

/// Binds `obj` at `path`, creating the intermediate contexts.
fn bind(root: &NamingContext, path: &str, obj: &ObjectRef) -> Result<(), NamingError> {
    let name = to_name(path).map_err(|_| NamingError::InvalidName)?;
    let (last, contexts) = name.split_last().ok_or(NamingError::InvalidName)?;
    let mut ctx = root.clone();
    for component in contexts {
        let one = std::slice::from_ref(component);
        ctx = match ctx.bind_new_context(one) {
            Ok(c) => c,
            Err(NamingError::AlreadyBound) => {
                let obj = ctx.resolve(one)?.ok_or(NamingError::InvalidName)?;
                NamingContext::unchecked_narrow(obj)
            }
            Err(e) => return Err(e),
        };
    }
    ctx.rebind(std::slice::from_ref(last), obj)
}

fn run() -> Result<(), String> {
    let (args, mut orb_args) = parse_args(std::env::args().skip(1).collect())?;
    if args.port != 0 || args.host.is_some() {
        let host = args.host.clone().unwrap_or_default();
        orb_args.extend(ssl::endpoint_args(&host, args.port));
    }
    let orb = Orb::init(&mut orb_args).map_err(|e| format!("ORB_init: {e}"))?;
    if !orb_args.is_empty() {
        return Err(format!("unknown arguments: {orb_args:?}"));
    }
    let root_poa = orb.root_poa().map_err(|e| format!("RootPOA: {e}"))?;
    let manager = root_poa.the_poa_manager();
    let poa = root_poa
        .create_poa(
            match args.suite.as_str() {
                "any" => "AnyTest",
                "value" => "ValueTest",
                "custom" => "CustomTest",
                _ => "InteropTest",
            },
            Some(&manager),
            &[
                Policy::Lifespan(Lifespan::Persistent),
                Policy::IdAssignment(IdAssignment::User),
            ],
        )
        .map_err(|e| format!("create_POA: {e}"))?;
    let this = Arc::new(OnceLock::new());
    let echo = EchoPoa(EchoImpl {
        orb: orb.clone(),
        this: this.clone(),
        state: Arc::default(),
    })
    .activate_with_id(&poa, b"Echo")
    .map_err(|e| format!("activate_object_with_id: {e}"))?;
    let _ = this.set(echo.clone());
    let obj = echo.into_object();
    // The AnyTest suite serves AnyEcho at --name, and Echo next to it (at
    // NAME-Echo) for the client to shut the server down.
    let mut bindings = vec![(args.name.clone(), obj.clone())];
    if args.suite == "value" {
        // Point3 arrives where Point is declared.
        idl::register_value_types();
        // ValueEcho stops the server itself.
        let value_echo =
            idl::value_test::ValueEchoPoa(valuetest::ValueEchoImpl { orb: orb.clone() })
                .activate_with_id(&poa, b"ValueEcho")
                .map_err(|e| format!("activate_object_with_id: {e}"))?;
        bindings = vec![(args.name.clone(), value_echo.into_object())];
    }
    if args.suite == "custom" {
        // Point3 arrives where Point is declared.
        idl::register_value_types();
        idl::custom::register_value_types();
        // CustomEcho stops the server itself.
        let custom_echo = idl::custom::custom_test::CustomEchoPoa(customtest::CustomEchoImpl {
            orb: orb.clone(),
        })
        .activate_with_id(&poa, b"CustomEcho")
        .map_err(|e| format!("activate_object_with_id: {e}"))?;
        bindings = vec![(args.name.clone(), custom_echo.into_object())];
    }
    if args.suite == "any" {
        let this = Arc::new(OnceLock::new());
        let any_echo = idl::any_test::AnyEchoPoa(anytest::AnyEchoImpl { this: this.clone() })
            .activate_with_id(&poa, b"AnyEcho")
            .map_err(|e| format!("activate_object_with_id: {e}"))?;
        let _ = this.set(any_echo.clone());
        bindings = vec![
            (args.name.clone(), any_echo.into_object()),
            (format!("{}-Echo", args.name), obj.clone()),
        ];
    }
    manager.activate().map_err(|e| format!("activate: {e}"))?;

    let root = orb
        .string_to_object(&args.naming)
        .map_err(|e| format!("{}: {e}", args.naming))?
        .ok_or("nil naming context")?;
    let root = NamingContext::unchecked_narrow(root);
    let deadline = Instant::now() + args.timeout;
    for (path, obj) in &bindings {
        loop {
            match bind(&root, path, obj) {
                Ok(()) => break,
                Err(NamingError::System(e))
                    if (e.is(SystemExceptionKind::TRANSIENT)
                        || e.is(SystemExceptionKind::COMM_FAILURE))
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(200));
                }
                Err(e) => return Err(format!("binding {path}: {e}")),
            }
        }
    }
    println!("server ready: {}", args.name);

    orb.run();
    orb.destroy().map_err(|e| format!("destroy: {e}"))?;
    println!("server stopped");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
