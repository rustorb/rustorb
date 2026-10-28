//! rustorb server for the interop tests.
//!
//! Serves `InteropTest::Echo` with a hand-written skeleton (until the code
//! generator exists) from a PERSISTENT / USER_ID POA named `InteropTest`,
//! with the ObjectId `Echo`, and binds it in the naming service, like the
//! omniORB servers (see `interop/README.md`).
//!
//! Options: `--naming CORBALOC --name PATH [--host HOST] [--port N]
//! [--timeout S]`, plus ORB options.

mod anytest;
mod idl;

use std::process::ExitCode;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use idl::{
    Char, Color, DIVIDE_BY_ZERO_ID, ECHO_ID, Get, OOPS_ID, Point, Put, Record, Shade, Sys, Value,
    WChar, WString,
};
use rustorb_core::{
    CompletionStatus, Exception, IdAssignment, InStream, Lifespan, ObjectRef, Orb, Policy, Servant,
    ServerReply, SystemException, SystemExceptionKind, minor,
};
use rustorb_naming::{NamingContext, NamingError, to_name};

#[derive(Default)]
struct State {
    posted: Vec<String>,
    counter: i32,
    callback: Option<ObjectRef>,
}

struct Echo {
    orb: Orb,
    this: OnceLock<ObjectRef>,
    state: Mutex<State>,
}

/// Reads the arguments, runs `f` and writes its result.
fn op<A: Get, R: Put>(
    input: &mut InStream<'_>,
    reply: &mut ServerReply,
    f: impl FnOnce(A) -> Sys<R>,
) -> Sys<()> {
    let args = A::get(input)?;
    let result = f(args)?;
    result.put(&mut reply.output())
}

fn bad_param() -> SystemException {
    SystemException::new(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No)
}

/// Calls `Callback::notify` on `cb`; a failure is passed on to the client
/// as the system exception, or `UNKNOWN` for a user exception.
fn notify(cb: &ObjectRef, message: &str) -> Sys<String> {
    let reply = cb
        .request("notify")
        .args(|out| out.write_string(message))
        .invoke()
        .map_err(|e| match e {
            Exception::System(e) => e,
            Exception::User(_) => SystemException::new(
                SystemExceptionKind::UNKNOWN,
                minor::UNLISTED_USER_EXCEPTION,
                CompletionStatus::Maybe,
            ),
        })?;
    reply.input().read_string()
}

impl Echo {
    fn this(&self) -> Option<ObjectRef> {
        self.this.get().cloned()
    }
}

impl Servant for Echo {
    fn repository_id(&self) -> &str {
        ECHO_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        match operation {
            "echo_boolean" => op(input, reply, |(v,): (bool,)| Ok(v)),
            "echo_octet" => op(input, reply, |(v,): (u8,)| Ok(v)),
            "echo_char" => op(input, reply, |(v,): (Char,)| Ok(v)),
            "echo_wchar" => op(input, reply, |(v,): (WChar,)| Ok(v)),
            "echo_short" => op(input, reply, |(v,): (i16,)| Ok(v)),
            "echo_ushort" => op(input, reply, |(v,): (u16,)| Ok(v)),
            "echo_long" => op(input, reply, |(v,): (i32,)| Ok(v)),
            "echo_ulong" => op(input, reply, |(v,): (u32,)| Ok(v)),
            "echo_longlong" => op(input, reply, |(v,): (i64,)| Ok(v)),
            "echo_ulonglong" => op(input, reply, |(v,): (u64,)| Ok(v)),
            "echo_float" => op(input, reply, |(v,): (f32,)| Ok(v)),
            "echo_double" => op(input, reply, |(v,): (f64,)| Ok(v)),
            "echo_string" => op(input, reply, |(v,): (String,)| Ok(v)),
            "echo_wstring" => op(input, reply, |(v,): (WString,)| Ok(v)),
            "echo_color" => op(input, reply, |(v,): (Color,)| Ok(v)),
            "echo_point" => op(input, reply, |(v,): (Point,)| Ok(v)),
            "echo_record" => op(input, reply, |(v,): (Record,)| Ok(v)),
            "echo_records" => op(input, reply, |(v,): (Vec<Record>,)| Ok(v)),
            "echo_value" => op(input, reply, |(v,): (Value,)| Ok(v)),
            "echo_shade" => op(input, reply, |(v,): (Shade,)| Ok(v)),
            "echo_long_array" => op(input, reply, |(v,): ([i32; 4],)| Ok(v)),
            "echo_matrix" => op(input, reply, |(v,): ([[f64; 3]; 2],)| Ok(v)),
            "echo_long_seq" => op(input, reply, |(v,): (Vec<i32>,)| Ok(v)),
            "echo_string_seq" => op(input, reply, |(v,): (Vec<String>,)| Ok(v)),
            "echo_octets" => op(input, reply, |(v,): (Vec<u8>,)| Ok(v)),
            "swap" => op(input, reply, |(a, b): (i32, i32)| {
                Ok((a.wrapping_add(b), b, a))
            }),
            "divide" => {
                let (a, b): (i32, i32) = Get::get(input)?;
                if b == 0 {
                    reply.user_exception(DIVIDE_BY_ZERO_ID)?;
                    return Ok(());
                }
                (a.wrapping_div(b), a.wrapping_rem(b)).put(&mut reply.output())
            }
            "make" => op(input, reply, |(x,): (i32,)| {
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
            }),
            "fail" => {
                let (code, reason): (i32, String) = Get::get(input)?;
                let mut out = reply.user_exception(OOPS_ID)?;
                (code, reason).put(&mut out)
            }
            "fail_system" => Err(SystemException::new(
                SystemExceptionKind::BAD_PARAM,
                42,
                CompletionStatus::No,
            )),
            "_get_counter" => op(input, reply, |(): ()| {
                Ok(self.state.lock().unwrap().counter)
            }),
            "_set_counter" => op(input, reply, |(v,): (i32,)| {
                self.state.lock().unwrap().counter = v;
                Ok(())
            }),
            "_get_name" => op(input, reply, |(): ()| Ok("rustorb-interop-echo")),
            "post" => op(input, reply, |(m,): (String,)| {
                self.state.lock().unwrap().posted.push(m);
                Ok(())
            }),
            "posted" => op(input, reply, |(): ()| {
                Ok(std::mem::take(&mut self.state.lock().unwrap().posted))
            }),
            "self_ref" => op(input, reply, |(): ()| Ok(self.this())),
            "echo_ref" => op(input, reply, |(other,): (Option<ObjectRef>,)| {
                Ok(other.or_else(|| self.this()))
            }),
            "is_nil" => op(input, reply, |(other,): (Option<ObjectRef>,)| {
                Ok(other.is_none())
            }),
            "call_back" => op(
                input,
                reply,
                |(cb, message): (Option<ObjectRef>, String)| {
                    notify(&cb.ok_or_else(bad_param)?, &message)
                },
            ),
            "register_callback" => op(input, reply, |(cb,): (Option<ObjectRef>,)| {
                self.state.lock().unwrap().callback = cb;
                Ok(())
            }),
            "trigger" => {
                let (message,): (String,) = Get::get(input)?;
                let cb = self.state.lock().unwrap().callback.clone();
                match cb {
                    Some(cb) => notify(&cb, &message)?.put(&mut reply.output()),
                    None => {
                        let mut out = reply.user_exception(OOPS_ID)?;
                        (1i32, "no callback").put(&mut out)
                    }
                }
            }
            "shutdown" => {
                // Without waiting, so the reply (if any) still goes out.
                self.orb.shutdown(false)
            }
            _ => Err(SystemException::new(
                SystemExceptionKind::BAD_OPERATION,
                minor::UNKNOWN_OPERATION,
                CompletionStatus::No,
            )),
        }
    }
}

struct Args {
    naming: String,
    name: String,
    host: Option<String>,
    port: u16,
    timeout: Duration,
    /// The AnyTest suite (`--suite any`) instead of InteropTest.
    any: bool,
}

fn parse_args(args: Vec<String>) -> Result<(Args, Vec<String>), String> {
    let mut naming = None;
    let mut name = None;
    let mut host = None;
    let mut port = 0;
    let mut timeout = Duration::from_secs(30);
    let mut any = false;
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
            "--suite" => any = suite_is_any(&value("--suite")?)?,
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
            any,
        },
        rest,
    ))
}

fn suite_is_any(suite: &str) -> Result<bool, String> {
    match suite {
        "basic" => Ok(false),
        "any" => Ok(true),
        _ => Err(format!("bad --suite {suite}")),
    }
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
        orb_args.extend([
            "-ORBendPoint".to_owned(),
            format!("giop:tcp:{host}:{}", args.port),
        ]);
    }
    let orb = Orb::init(&mut orb_args).map_err(|e| format!("ORB_init: {e}"))?;
    if !orb_args.is_empty() {
        return Err(format!("unknown arguments: {orb_args:?}"));
    }
    let root_poa = orb.root_poa().map_err(|e| format!("RootPOA: {e}"))?;
    let manager = root_poa.the_poa_manager();
    let poa = root_poa
        .create_poa(
            if args.any { "AnyTest" } else { "InteropTest" },
            Some(&manager),
            &[
                Policy::Lifespan(Lifespan::Persistent),
                Policy::IdAssignment(IdAssignment::User),
            ],
        )
        .map_err(|e| format!("create_POA: {e}"))?;
    let servant = Arc::new(Echo {
        orb: orb.clone(),
        this: OnceLock::new(),
        state: Mutex::default(),
    });
    poa.activate_object_with_id(b"Echo", servant.clone())
        .map_err(|e| format!("activate_object_with_id: {e}"))?;
    let obj = poa
        .id_to_reference(b"Echo")
        .map_err(|e| format!("id_to_reference: {e}"))?;
    let _ = servant.this.set(obj.clone());
    // The AnyTest suite serves AnyEcho at --name, and Echo next to it (at
    // NAME-Echo) for the client to shut the server down.
    let mut bindings = vec![(args.name.clone(), obj.clone())];
    if args.any {
        let any_echo = Arc::new(anytest::AnyEcho::default());
        poa.activate_object_with_id(b"AnyEcho", any_echo.clone())
            .map_err(|e| format!("activate_object_with_id: {e}"))?;
        let any_obj = poa
            .id_to_reference(b"AnyEcho")
            .map_err(|e| format!("id_to_reference: {e}"))?;
        let _ = any_echo.this.set(any_obj.clone());
        bindings = vec![
            (args.name.clone(), any_obj),
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
