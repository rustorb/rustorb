//! rustorb client for the interop tests.
//!
//! Resolves the Echo object from the naming service and runs the same cases
//! as the omniORB clients (see `interop/README.md`), with hand-written stubs
//! until the code generator exists. The callback cases need a server side
//! ORB and are skipped for now.
//!
//! Options: `--naming CORBALOC --name PATH [--timeout S] [--shutdown-server]`,
//! plus ORB options such as `-ORBmaxGIOPVersion 1.1` (also read from the
//! `ORBmaxGIOPVersion` environment variable, as omniORB does). `--host` and `--port`
//! are accepted and ignored, as the client has no objects of its own yet.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use rustorb_core::cdr::CdrEncode;
use rustorb_core::{
    CompletionStatus, Exception, InStream, ObjectRef, Orb, OutStream, SystemException,
    SystemExceptionKind,
};

type Sys<T> = Result<T, SystemException>;

/// Writes an IDL value to a request.
trait Put {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()>;
}

/// Reads an IDL value from a reply.
trait Get: Sized {
    fn get(input: &mut InStream<'_>) -> Sys<Self>;
}

macro_rules! primitive {
    ($($ty:ty),*) => {
        $(
            impl Put for $ty {
                fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
                    Ok(self.encode(out)?)
                }
            }
            impl Get for $ty {
                fn get(input: &mut InStream<'_>) -> Sys<Self> {
                    input.read::<$ty>()
                }
            }
        )*
    };
}

primitive!(bool, u8, i16, u16, i32, u32, i64, u64, f32, f64);

impl Put for () {
    fn put(&self, _: &mut OutStream<'_>) -> Sys<()> {
        Ok(())
    }
}

impl Get for () {
    fn get(_: &mut InStream<'_>) -> Sys<Self> {
        Ok(())
    }
}

impl Put for String {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_string(self)
    }
}

impl Put for &str {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_string(self)
    }
}

impl Get for String {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        input.read_string()
    }
}

/// IDL `char`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Char(u8);

impl Put for Char {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_char(self.0);
        Ok(())
    }
}

impl Get for Char {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        Ok(Char(input.read::<u8>()?))
    }
}

/// IDL `wchar`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct WChar(u16);

impl Put for WChar {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_wchar(self.0)
    }
}

impl Get for WChar {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        Ok(WChar(input.read_wchar()?))
    }
}

/// IDL `wstring`.
#[derive(Debug, Clone, PartialEq)]
struct WString(String);

impl Put for WString {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_wstring(&self.0)
    }
}

impl Get for WString {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        Ok(WString(input.read_wstring()?))
    }
}

impl<T: Put> Put for Vec<T> {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_len(self.len())?;
        self.iter().try_for_each(|v| v.put(out))
    }
}

impl<T: Get> Get for Vec<T> {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        let len = input.read::<u32>()? as usize;
        if len > input.remaining() {
            return Err(marshal());
        }
        (0..len).map(|_| T::get(input)).collect()
    }
}

impl<T: Put, const N: usize> Put for [T; N] {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        self.iter().try_for_each(|v| v.put(out))
    }
}

impl<T: Get + Default + Copy, const N: usize> Get for [T; N] {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        let mut a = [T::default(); N];
        for v in &mut a {
            *v = T::get(input)?;
        }
        Ok(a)
    }
}

impl Put for Option<ObjectRef> {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        out.write_object(self.as_ref())
    }
}

impl Get for Option<ObjectRef> {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        input.read_object()
    }
}

macro_rules! tuple {
    ($($name:ident),*) => {
        impl<$($name: Put),*> Put for ($($name,)*) {
            #[allow(non_snake_case)]
            fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
                let ($($name,)*) = self;
                $($name.put(out)?;)*
                Ok(())
            }
        }
        impl<$($name: Get),*> Get for ($($name,)*) {
            fn get(input: &mut InStream<'_>) -> Sys<Self> {
                Ok(($($name::get(input)?,)*))
            }
        }
    };
}

tuple!(A);
tuple!(A, B);
tuple!(A, B, C);

fn marshal() -> SystemException {
    SystemException::new(SystemExceptionKind::MARSHAL, 0, CompletionStatus::Yes)
}

// --- InteropTest types -----------------------------------------------------

const MAGIC: i32 = 0x5255_5354;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Color {
    Red,
    Green,
    Blue,
}

impl Put for Color {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        (*self as u32).put(out)
    }
}

impl Get for Color {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        match u32::get(input)? {
            0 => Ok(Color::Red),
            1 => Ok(Color::Green),
            2 => Ok(Color::Blue),
            _ => Err(marshal()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

impl Put for Point {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        (self.x, self.y).put(out)
    }
}

impl Get for Point {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        let (x, y) = Get::get(input)?;
        Ok(Point { x, y })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Record {
    id: i32,
    name: String,
    hue: Color,
    pos: Point,
    values: Vec<i32>,
    active: bool,
}

impl Put for Record {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        self.id.put(out)?;
        self.name.put(out)?;
        self.hue.put(out)?;
        self.pos.put(out)?;
        self.values.put(out)?;
        self.active.put(out)
    }
}

impl Get for Record {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        Ok(Record {
            id: Get::get(input)?,
            name: Get::get(input)?,
            hue: Get::get(input)?,
            pos: Get::get(input)?,
            values: Get::get(input)?,
            active: Get::get(input)?,
        })
    }
}

/// `union Value switch (long)`.
#[derive(Debug, Clone, PartialEq)]
enum Value {
    L(i32),
    S(String),
    P(Point),
    /// Discriminator 4 or 5.
    Seq(i32, Vec<i32>),
    /// The default member, with its discriminator.
    Flag(i32, bool),
}

impl Put for Value {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        match self {
            Value::L(v) => (1i32, *v).put(out),
            Value::S(v) => (2i32, v.clone()).put(out),
            Value::P(v) => (3i32, *v).put(out),
            Value::Seq(d, v) => (*d, v.clone()).put(out),
            Value::Flag(d, v) => (*d, *v).put(out),
        }
    }
}

impl Get for Value {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        Ok(match i32::get(input)? {
            1 => Value::L(Get::get(input)?),
            2 => Value::S(Get::get(input)?),
            3 => Value::P(Get::get(input)?),
            d @ (4 | 5) => Value::Seq(d, Get::get(input)?),
            d => Value::Flag(d, Get::get(input)?),
        })
    }
}

/// `union Shade switch (Color)`, with an implicit default for BLUE.
#[derive(Debug, Clone, PartialEq)]
enum Shade {
    Level(u8),
    Label(String),
    Blue,
}

impl Put for Shade {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        match self {
            Shade::Level(v) => (Color::Red, *v).put(out),
            Shade::Label(v) => (Color::Green, v.clone()).put(out),
            Shade::Blue => Color::Blue.put(out),
        }
    }
}

impl Get for Shade {
    fn get(input: &mut InStream<'_>) -> Sys<Self> {
        Ok(match Color::get(input)? {
            Color::Red => Shade::Level(Get::get(input)?),
            Color::Green => Shade::Label(Get::get(input)?),
            Color::Blue => Shade::Blue,
        })
    }
}

const OOPS_ID: &str = "IDL:rustorb.org/InteropTest/Oops:1.0";
const DIVIDE_BY_ZERO_ID: &str = "IDL:rustorb.org/InteropTest/DivideByZero:1.0";
const ECHO_ID: &str = "IDL:rustorb.org/InteropTest/Echo:1.0";

// --- invocation helpers ----------------------------------------------------

fn call<A: Put, T: Get>(obj: &ObjectRef, op: &str, args: A) -> Result<T, Exception> {
    let reply = obj.request(op).args(|out| args.put(out)).invoke()?;
    let mut input = reply.input();
    let value = T::get(&mut input)?;
    Ok(value)
}

/// `CosNaming::NamingContext::resolve`, with the name given as `a/b/c`.
fn resolve(root: &ObjectRef, path: &str) -> Result<Option<ObjectRef>, Exception> {
    let name: Vec<(String, String)> = path
        .split('/')
        .map(|id| (id.to_owned(), String::new()))
        .collect();
    call(root, "resolve", (name,))
}

/// Retries `f` while it fails with TRANSIENT, COMM_FAILURE or a user
/// exception (NotFound while the server is starting).
fn retry<T>(
    what: &str,
    timeout: Duration,
    mut f: impl FnMut() -> Result<T, Exception>,
) -> Result<T, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                let retriable = match &e {
                    Exception::User(_) => true,
                    Exception::System(s) => {
                        s.is(SystemExceptionKind::TRANSIENT)
                            || s.is(SystemExceptionKind::COMM_FAILURE)
                    }
                };
                if !retriable || Instant::now() >= deadline {
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

    fn skip(&mut self, name: &str, why: &str) {
        println!("skip {name}: {why}");
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

impl<T: Put> Put for &T {
    fn put(&self, out: &mut OutStream<'_>) -> Sys<()> {
        (*self).put(out)
    }
}

fn s(e: Exception) -> String {
    e.to_string()
}

fn run_cases(r: &mut Runner, e: &ObjectRef) {
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

    r.skip("call_back", "rustorb has no POA yet");
    r.skip("register_callback/trigger", "rustorb has no POA yet");

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
    timeout: Duration,
    shutdown_server: bool,
}

fn parse_args(mut args: Vec<String>) -> Result<(Args, Vec<String>), String> {
    let mut naming = None;
    let mut name = None;
    let mut timeout = Duration::from_secs(30);
    let mut shutdown_server = false;
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
            "--host" | "--port" => {
                value(&arg)?;
            }
            "--shutdown-server" => shutdown_server = true,
            _ => rest.push(arg),
        }
    }
    Ok((
        Args {
            naming: naming.ok_or("--naming is required")?,
            name: name.ok_or("--name is required")?,
            timeout,
            shutdown_server,
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
    let orb = Orb::init(&mut orb_args).map_err(|e| format!("ORB_init: {e}"))?;
    if !orb_args.is_empty() {
        return Err(format!("unknown arguments: {orb_args:?}"));
    }
    let root = orb
        .string_to_object(&args.naming)
        .map_err(|e| format!("{}: {e}", args.naming))?
        .ok_or("nil naming context")?;
    let echo = retry("the Echo server", args.timeout, || {
        resolve(&root, &args.name)?.ok_or_else(|| {
            SystemException::new(SystemExceptionKind::TRANSIENT, 0, CompletionStatus::No).into()
        })
    })?;
    let is_echo = echo.is_a(ECHO_ID).map_err(|e| format!("_is_a: {e}"))?;
    if !is_echo {
        return Err(format!("{} is not an InteropTest::Echo", args.name));
    }
    retry("the Echo server", args.timeout, || echo.non_existent())?;

    let mut r = Runner {
        passed: 0,
        failed: Vec::new(),
    };
    run_cases(&mut r, &echo);

    if args.shutdown_server {
        echo.request("shutdown")
            .send_oneway()
            .map_err(|e| format!("shutdown: {e}"))?;
    }
    println!("\n{} passed, {} failed", r.passed, r.failed.len());
    if !r.failed.is_empty() {
        println!("failed: {}", r.failed.join(", "));
    }
    orb.shutdown();
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
