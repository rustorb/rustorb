//! The stage 2 tests (`idl/AnyTest.idl`): the TypeCodes, the table of
//! values, the AnyEcho servant and the client cases, the same as those of
//! `impls/omniorbpy/anytest.py`. TypeCodes and values are built with the
//! core's [`TypeCode`] and [`Value`], with no generated code. Shared by
//! the client and the server.

#![allow(dead_code)]

use std::sync::{LazyLock, OnceLock};

use rustorb_core::{
    Any, CompletionStatus, Exception, InStream, ObjectRef, Servant, ServerReply, SystemException,
    SystemExceptionKind, TCKind, TypeCode, Value, minor,
};

pub const ANY_ECHO_ID: &str = "IDL:AnyTest/AnyEcho:1.0";

/// The names of the table of AnyTest.idl, in the order the client runs
/// them.
pub const NAMES: &[&str] = &[
    "long",
    "short",
    "ulonglong",
    "float",
    "double",
    "boolean",
    "char",
    "octet",
    "string",
    "bounded",
    "count",
    "money",
    "fixed",
    "point",
    "shade",
    "choice_l",
    "choice_s",
    "choice_default",
    "longseq",
    "octets",
    "shortarray",
    "node",
    "nvlist",
    "any",
    "typecode",
    "objref",
    "nil",
    "oops",
    "empty",
];

/// Values of the types only the clients know (AnyTestHidden.idl), with what
/// `describe` answers for them.
pub const HIDDEN: &[(&str, &str)] = &[
    ("secret", "struct IDL:AnyTestHidden/Secret:1.0"),
    ("pick", "union IDL:AnyTestHidden/Pick:1.0"),
    ("moods", "alias IDL:AnyTestHidden/Moods:1.0"),
];

/// The TypeCodes of AnyTest.idl and AnyTestHidden.idl.
pub struct Types {
    pub point: TypeCode,
    pub shade: TypeCode,
    pub choice: TypeCode,
    pub count: TypeCode,
    pub short5: TypeCode,
    pub money: TypeCode,
    pub fixed: TypeCode,
    pub longseq: TypeCode,
    pub octets: TypeCode,
    pub shortarray: TypeCode,
    pub node: TypeCode,
    pub nvlist: TypeCode,
    pub oops: TypeCode,
    pub any_echo: TypeCode,
    pub secret: TypeCode,
    pub pick: TypeCode,
    pub moods: TypeCode,
}

fn simple(kind: TCKind) -> TypeCode {
    TypeCode::simple(kind).expect("a simple kind")
}

pub static TYPES: LazyLock<Types> = LazyLock::new(|| {
    let long = simple(TCKind::Long);
    let string = TypeCode::string(0);
    let point = TypeCode::struct_tc(
        "IDL:AnyTest/Point:1.0",
        "Point",
        &[("x", long.clone()), ("y", simple(TCKind::Double))],
    )
    .unwrap();
    let shade =
        TypeCode::enum_tc("IDL:AnyTest/Shade:1.0", "Shade", &["LIGHT", "DARK", "DIM"]).unwrap();
    let choice = TypeCode::union_tc(
        "IDL:AnyTest/Choice:1.0",
        "Choice",
        &long,
        &[
            (Some(1), "l", long.clone()),
            (Some(2), "s", string.clone()),
            (Some(3), "s", string.clone()),
            (None, "b", simple(TCKind::Boolean)),
        ],
    )
    .unwrap();
    let alias = |name: &str, tc: &TypeCode| {
        TypeCode::alias(&format!("IDL:AnyTest/{name}:1.0"), name, tc).unwrap()
    };
    let node_seq = alias(
        "NodeSeq",
        &TypeCode::sequence(&TypeCode::recursive("IDL:AnyTest/Node:1.0"), 0).unwrap(),
    );
    let node = TypeCode::struct_tc(
        "IDL:AnyTest/Node:1.0",
        "Node",
        &[("name", string.clone()), ("children", node_seq)],
    )
    .unwrap();
    let name_value = TypeCode::struct_tc(
        "IDL:AnyTest/NameValue:1.0",
        "NameValue",
        &[("name", string.clone()), ("value", simple(TCKind::Any))],
    )
    .unwrap();
    let point_seq = TypeCode::alias(
        "IDL:AnyTestHidden/PointSeq:1.0",
        "PointSeq",
        &TypeCode::sequence(&point, 0).unwrap(),
    )
    .unwrap();
    let secret = TypeCode::struct_tc(
        "IDL:AnyTestHidden/Secret:1.0",
        "Secret",
        &[
            ("text", string.clone()),
            ("points", point_seq),
            ("shade", shade.clone()),
        ],
    )
    .unwrap();
    let mood = TypeCode::enum_tc("IDL:AnyTestHidden/Mood:1.0", "Mood", &["HAPPY", "SAD"]).unwrap();
    Types {
        count: alias("Count", &long),
        short5: alias("Short5", &TypeCode::string(5)),
        money: alias("Money", &TypeCode::fixed(5, 2).unwrap()),
        fixed: TypeCode::fixed(31, 4).unwrap(),
        longseq: alias("LongSeq", &TypeCode::sequence(&long, 0).unwrap()),
        octets: alias(
            "Octets",
            &TypeCode::sequence(&simple(TCKind::Octet), 0).unwrap(),
        ),
        shortarray: alias(
            "ShortArray",
            &TypeCode::array(&simple(TCKind::Short), 3).unwrap(),
        ),
        nvlist: alias("NVList", &TypeCode::sequence(&name_value, 0).unwrap()),
        oops: TypeCode::exception(
            "IDL:AnyTest/Oops:1.0",
            "Oops",
            &[("why", string.clone()), ("code", long.clone())],
        )
        .unwrap(),
        any_echo: TypeCode::objref(ANY_ECHO_ID, "AnyEcho").unwrap(),
        pick: TypeCode::union_tc(
            "IDL:AnyTestHidden/Pick:1.0",
            "Pick",
            &simple(TCKind::Char),
            &[
                (Some(i128::from(b'a')), "n", long.clone()),
                (Some(i128::from(b'b')), "s", secret.clone()),
            ],
        )
        .unwrap(),
        moods: TypeCode::alias(
            "IDL:AnyTestHidden/Moods:1.0",
            "Moods",
            &TypeCode::sequence(&mood, 0).unwrap(),
        )
        .unwrap(),
        point,
        shade,
        choice,
        node,
        secret,
    }
});

fn s(v: &str) -> Value {
    Value::String(v.to_owned())
}

/// A fixed of the digits of `text` (all of them, without the point).
fn fixed(text: &str) -> Value {
    Value::Fixed {
        digits: text
            .bytes()
            .filter(u8::is_ascii_digit)
            .map(|b| b - b'0')
            .collect(),
        negative: text.starts_with('-'),
    }
}

fn point(x: i32, y: f64) -> Value {
    Value::List(vec![Value::Long(x), Value::Double(y)])
}

fn node(name: &str, children: Vec<Value>) -> Value {
    Value::List(vec![s(name), Value::List(children)])
}

fn union(disc: Value, member: u32, v: Value) -> Value {
    Value::Union {
        disc: Box::new(disc),
        member: Some((member, Box::new(v))),
    }
}

fn secret(text: &str, points: Vec<Value>, shade: u32) -> Value {
    Value::List(vec![s(text), Value::List(points), Value::Enum(shade)])
}

/// The value named `name`; `echo` is the AnyEcho object (for "objref").
pub fn value(name: &str, echo: &ObjectRef) -> Option<Any> {
    let t = &*TYPES;
    let simple_any = |kind, v| Any::new(simple(kind), v);
    Some(match name {
        "long" => Any::from(42i32),
        "short" => Any::from(-7i16),
        "ulonglong" => Any::from(u64::MAX),
        "float" => Any::from(0.25f32),
        "double" => Any::from(1.5f64),
        "boolean" => Any::from(true),
        "char" => Any::from('x'),
        "octet" => Any::from(255u8),
        "string" => Any::from("hello any"),
        "bounded" => Any::new(t.short5.clone(), s("abc")),
        "count" => Any::new(t.count.clone(), Value::Long(7)),
        "money" => Any::new(t.money.clone(), fixed("-123.45")),
        "fixed" => Any::new(t.fixed.clone(), fixed("123456789012345678901234567.8901")),
        "point" => Any::new(t.point.clone(), point(1, 2.5)),
        "shade" => Any::new(t.shade.clone(), Value::Enum(2)),
        "choice_l" => Any::new(t.choice.clone(), union(Value::Long(1), 0, Value::Long(10))),
        "choice_s" => Any::new(t.choice.clone(), union(Value::Long(3), 2, s("three"))),
        "choice_default" => Any::new(
            t.choice.clone(),
            union(Value::Long(9), 3, Value::Bool(true)),
        ),
        "longseq" => Any::new(
            t.longseq.clone(),
            Value::List(vec![Value::Long(1), Value::Long(2), Value::Long(3)]),
        ),
        "octets" => Any::new(t.octets.clone(), Value::Octets(vec![0, 1, 255])),
        "shortarray" => Any::new(
            t.shortarray.clone(),
            Value::List(vec![Value::Short(1), Value::Short(-2), Value::Short(3)]),
        ),
        "node" => Any::new(
            t.node.clone(),
            node(
                "root",
                vec![node("a", vec![]), node("b", vec![node("c", vec![])])],
            ),
        ),
        "nvlist" => Any::new(
            t.nvlist.clone(),
            Value::List(vec![
                Value::List(vec![s("a"), Value::Any(Box::new(Any::from(1i32)))]),
                Value::List(vec![s("b"), Value::Any(Box::new(Any::from("x")))]),
                Value::List(vec![
                    s("p"),
                    Value::Any(Box::new(Any::new(t.point.clone(), point(3, -0.5)))),
                ]),
            ]),
        ),
        "any" => simple_any(TCKind::Any, Value::Any(Box::new(Any::from(5i32)))),
        "typecode" => Any::from(t.node.clone()),
        "objref" => Any::new(t.any_echo.clone(), Value::Objref(Some(echo.clone()))),
        "nil" => Any::new(t.any_echo.clone(), Value::Objref(None)),
        "oops" => Any::new(t.oops.clone(), Value::List(vec![s("bad"), Value::Long(3)])),
        "empty" => Any::default(),
        "secret" => Any::new(t.secret.clone(), secret("s", vec![point(1, 2.0)], 1)),
        "pick" => Any::new(
            t.pick.clone(),
            union(Value::Char('b'), 1, secret("p", vec![], 0)),
        ),
        "moods" => Any::new(
            t.moods.clone(),
            Value::List(vec![Value::Enum(1), Value::Enum(0)]),
        ),
        _ => return None,
    })
}

/// Whether `a` is the value `want`: equal TypeCodes and the same value.
pub fn same(a: &Any, want: &Any) -> bool {
    a.typecode().equal(want.typecode()) && a.value() == want.value()
}

/// What `describe` answers for an Any of type `tc`.
pub fn describe(tc: &TypeCode) -> String {
    format!("{} {}", tc.kind().name(), tc.id().unwrap_or(""))
}

fn bad_param() -> SystemException {
    SystemException::new(SystemExceptionKind::BAD_PARAM, 0, CompletionStatus::No)
}

/// `AnyTest::AnyEcho`.
#[derive(Default)]
pub struct AnyEcho {
    /// The reference of the object, for "objref".
    pub this: OnceLock<ObjectRef>,
}

impl AnyEcho {
    fn value(&self, name: &str) -> Result<Any, SystemException> {
        let this = self.this.get().ok_or_else(bad_param)?;
        value(name, this).ok_or_else(bad_param)
    }
}

impl Servant for AnyEcho {
    fn repository_id(&self) -> &str {
        ANY_ECHO_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        match operation {
            "echo" => {
                let a = input.read_any()?;
                reply.output().write_any(&a)
            }
            "make" => {
                let name = input.read_string()?;
                let v = self.value(&name)?;
                reply.output().write_any(&v)
            }
            "check" => {
                let name = input.read_string()?;
                let a = input.read_any()?;
                let ok = same(&a, &self.value(&name)?);
                reply.output().write_bool(ok);
                Ok(())
            }
            "out_inout" => {
                let name = input.read_string()?;
                let c = input.read_any()?;
                let want = self.value(&name)?;
                if !same(&c, &want) {
                    return Err(bad_param());
                }
                let mut out = reply.output();
                out.write_any(&want)?;
                out.write_any(&self.value("long")?)
            }
            "echo_tc" => {
                let tc = input.read_typecode()?;
                reply.output().write_typecode(&tc)
            }
            "make_tc" => {
                let name = input.read_string()?;
                let v = self.value(&name)?;
                reply.output().write_typecode(v.typecode())
            }
            "check_tc" => {
                let name = input.read_string()?;
                let tc = input.read_typecode()?;
                let ok = tc.equal(self.value(&name)?.typecode());
                reply.output().write_bool(ok);
                Ok(())
            }
            "echo_nvlist" => {
                let l = input.read_value(&TYPES.nvlist)?;
                reply.output().write_value(&TYPES.nvlist, &l)
            }
            "describe" => {
                let a = input.read_any()?;
                reply.output().write_string(&describe(a.typecode()))
            }
            _ => Err(SystemException::new(
                SystemExceptionKind::BAD_OPERATION,
                minor::UNKNOWN_OPERATION,
                CompletionStatus::No,
            )),
        }
    }
}

// --- the client side ---------------------------------------------------------------

type Case<'a> = (String, Box<dyn Fn() -> Result<(), String> + 'a>);

fn err(e: Exception) -> String {
    e.to_string()
}

fn check(cond: bool, what: impl FnOnce() -> String) -> Result<(), String> {
    if cond { Ok(()) } else { Err(what()) }
}

fn call_any(
    echo: &ObjectRef,
    op: &str,
    name: Option<&str>,
    a: Option<&Any>,
) -> Result<Any, String> {
    let reply = echo
        .request(op)
        .args(|out| {
            if let Some(name) = name {
                out.write_string(name)?;
            }
            if let Some(a) = a {
                out.write_any(a)?;
            }
            Ok(())
        })
        .invoke()
        .map_err(err)?;
    reply.input().read_any().map_err(|e| e.to_string())
}

fn call_bool(
    echo: &ObjectRef,
    op: &str,
    name: &str,
    a: Option<&Any>,
    tc: Option<&TypeCode>,
) -> Result<bool, String> {
    let reply = echo
        .request(op)
        .args(|out| {
            out.write_string(name)?;
            if let Some(a) = a {
                out.write_any(a)?;
            }
            if let Some(tc) = tc {
                out.write_typecode(tc)?;
            }
            Ok(())
        })
        .invoke()
        .map_err(err)?;
    reply.input().read::<bool>().map_err(|e| e.to_string())
}

fn call_tc(
    echo: &ObjectRef,
    op: &str,
    name: Option<&str>,
    tc: Option<&TypeCode>,
) -> Result<TypeCode, String> {
    let reply = echo
        .request(op)
        .args(|out| {
            if let Some(name) = name {
                out.write_string(name)?;
            }
            if let Some(tc) = tc {
                out.write_typecode(tc)?;
            }
            Ok(())
        })
        .invoke()
        .map_err(err)?;
    reply.input().read_typecode().map_err(|e| e.to_string())
}

fn call_describe(echo: &ObjectRef, a: &Any) -> Result<String, String> {
    let reply = echo
        .request("describe")
        .args(|out| out.write_any(a))
        .invoke()
        .map_err(err)?;
    reply.input().read_string().map_err(|e| e.to_string())
}

fn add<'a>(cases: &mut Vec<Case<'a>>, name: String, f: Box<dyn Fn() -> Result<(), String> + 'a>) {
    cases.push((name, f));
}

/// The stage 2 client cases against the AnyEcho object `echo`.
pub fn cases(echo: &ObjectRef) -> Vec<Case<'_>> {
    let v = move |name: &str| value(name, echo).expect("a name of the table");
    let mut cases: Vec<Case<'_>> = Vec::new();
    for &n in NAMES {
        add(
            &mut cases,
            format!("make[{n}]"),
            Box::new(move || {
                let got = call_any(echo, "make", Some(n), None)?;
                check(same(&got, &v(n)), || format!("got {got:?}"))
            }),
        );
        add(
            &mut cases,
            format!("check[{n}]"),
            Box::new(move || {
                let ok = call_bool(echo, "check", n, Some(&v(n)), None)?;
                check(ok, || "server says no".into())
            }),
        );
        add(
            &mut cases,
            format!("echo[{n}]"),
            Box::new(move || {
                let got = call_any(echo, "echo", None, Some(&v(n)))?;
                check(same(&got, &v(n)), || format!("got {got:?}"))
            }),
        );
    }
    for &n in NAMES {
        add(
            &mut cases,
            format!("make_tc[{n}]"),
            Box::new(move || {
                let got = call_tc(echo, "make_tc", Some(n), None)?;
                check(got.equal(v(n).typecode()), || format!("got {got:?}"))
            }),
        );
        add(
            &mut cases,
            format!("check_tc[{n}]"),
            Box::new(move || {
                let ok = call_bool(echo, "check_tc", n, None, Some(v(n).typecode()))?;
                check(ok, || "server says no".into())
            }),
        );
        add(
            &mut cases,
            format!("echo_tc[{n}]"),
            Box::new(move || {
                let got = call_tc(echo, "echo_tc", None, Some(v(n).typecode()))?;
                check(got.equal(v(n).typecode()), || format!("got {got:?}"))
            }),
        );
    }
    for n in ["point", "nvlist", "node"] {
        add(
            &mut cases,
            format!("out_inout[{n}]"),
            Box::new(move || {
                let reply = echo
                    .request("out_inout")
                    .args(|out| {
                        out.write_string(n)?;
                        out.write_any(&v(n))
                    })
                    .invoke()
                    .map_err(err)?;
                let mut input = reply.input();
                let b = input.read_any().map_err(|e| e.to_string())?;
                let c = input.read_any().map_err(|e| e.to_string())?;
                check(same(&b, &v(n)), || format!("out {b:?}"))?;
                check(same(&c, &v("long")), || format!("inout {c:?}"))
            }),
        );
    }
    add(
        &mut cases,
        "echo_nvlist".into(),
        Box::new(move || {
            let want = v("nvlist");
            let reply = echo
                .request("echo_nvlist")
                .args(|out| out.write_value(&TYPES.nvlist, want.value()))
                .invoke()
                .map_err(err)?;
            let got = reply
                .input()
                .read_value(&TYPES.nvlist)
                .map_err(|e| e.to_string())?;
            check(&got == want.value(), || format!("got {got:?}"))
        }),
    );
    for &(n, description) in HIDDEN {
        add(
            &mut cases,
            format!("echo[{n}]"),
            Box::new(move || {
                let got = call_any(echo, "echo", None, Some(&v(n)))?;
                check(same(&got, &v(n)), || format!("got {got:?}"))
            }),
        );
        add(
            &mut cases,
            format!("describe[{n}]"),
            Box::new(move || {
                let got = call_describe(echo, &v(n))?;
                check(got == description, || got.clone())
            }),
        );
    }
    add(
        &mut cases,
        "describe[long]".into(),
        Box::new(move || {
            let got = call_describe(echo, &v("long"))?;
            check(got == "long ", || got.clone())
        }),
    );
    add(
        &mut cases,
        "describe[node]".into(),
        Box::new(move || {
            let got = call_describe(echo, &v("node"))?;
            check(got == "struct IDL:AnyTest/Node:1.0", || got.clone())
        }),
    );
    add(
        &mut cases,
        "check_wrong_type".into(),
        Box::new(move || {
            let ok = call_bool(echo, "check", "long", Some(&Any::from(42i16)), None)?;
            check(!ok, || "server says yes".into())
        }),
    );
    add(
        &mut cases,
        "check_unaliased".into(),
        Box::new(move || {
            let ok = call_bool(echo, "check", "count", Some(&Any::from(7i32)), None)?;
            check(!ok, || "server says yes".into())
        }),
    );
    add(
        &mut cases,
        "check_tc_unaliased".into(),
        Box::new(move || {
            let ok = call_bool(echo, "check_tc", "count", None, Some(&simple(TCKind::Long)))?;
            check(!ok, || "server says yes".into())
        }),
    );
    add(
        &mut cases,
        "out_inout_wrong".into(),
        Box::new(move || {
            let result = echo
                .request("out_inout")
                .args(|out| {
                    out.write_string("point")?;
                    out.write_any(&v("long"))
                })
                .invoke();
            match result {
                Err(Exception::System(e)) if e.is(SystemExceptionKind::BAD_PARAM) => Ok(()),
                Err(e) => Err(format!("got {e}")),
                Ok(_) => Err("no BAD_PARAM".into()),
            }
        }),
    );
    cases
}
