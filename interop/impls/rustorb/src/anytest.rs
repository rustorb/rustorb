//! The stage 2 tests (`idl/AnyTest.idl`): the table of values, the AnyEcho
//! servant and the client cases, the same as those of
//! `impls/omniorbpy/anytest.py`. Values are put into anys from the types
//! `rustorb-idl -brust` generates. Shared by the client and the server.

#![allow(dead_code)]

use std::sync::{Arc, OnceLock};

use rustorb_core::idl::{self as rt, IdlType, Sys, bad_param};
use rustorb_core::{Any, SystemExceptionKind, TypeCode};

use crate::idl::any_test::{
    AnyEcho, AnyEchoServant, Choice, NameValue, Node, NvList, Oops, Point, Shade, alias,
};
use crate::idl::any_test_hidden as hidden;

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

/// An any of type `T` that holds `v`.
fn any<T: IdlType>(v: &T::Borrowed) -> Any {
    Any::insert::<T>(v).expect("a value of the table")
}

fn node(name: &str, children: Vec<Node>) -> Node {
    Node {
        name: name.to_owned(),
        children,
    }
}

fn secret(text: &str, points: Vec<Point>, shade: Shade) -> hidden::Secret {
    hidden::Secret {
        text: text.to_owned(),
        points,
        shade,
    }
}

/// The value named `name`; `echo` is the AnyEcho object (for "objref").
pub fn value(name: &str, echo: &AnyEcho) -> Option<Any> {
    let point = |x, y| Point { x, y };
    let nv = |name: &str, value| NameValue {
        name: name.to_owned(),
        value,
    };
    Some(match name {
        "long" => any::<rt::Long>(&42),
        "short" => any::<rt::Short>(&-7),
        "ulonglong" => any::<rt::ULongLong>(&u64::MAX),
        "float" => any::<rt::Float>(&0.25),
        "double" => any::<rt::Double>(&1.5),
        "boolean" => any::<rt::Boolean>(&true),
        "char" => any::<rt::Char>(&'x'),
        "octet" => any::<rt::Octet>(&255),
        "string" => any::<rt::String>("hello any"),
        "bounded" => any::<alias::Short5>("abc"),
        "count" => any::<alias::Count>(&7),
        "money" => any::<alias::Money>(&"-123.45".parse().unwrap()),
        "fixed" => any::<rt::Fixed<31, 4>>(&"123456789012345678901234567.8901".parse().unwrap()),
        "point" => any::<Point>(&point(1, 2.5)),
        "shade" => any::<Shade>(&Shade::Dim),
        "choice_l" => any::<Choice>(&Choice::L(10)),
        "choice_s" => any::<Choice>(&Choice::S(3, "three".into())),
        "choice_default" => any::<Choice>(&Choice::B(9, true)),
        "longseq" => any::<alias::LongSeq>(&[1, 2, 3]),
        "octets" => any::<alias::Octets>(&[0, 1, 255]),
        "shortarray" => any::<alias::ShortArray>(&[1, -2, 3]),
        "node" => any::<Node>(&node(
            "root",
            vec![node("a", vec![]), node("b", vec![node("c", vec![])])],
        )),
        "nvlist" => any::<alias::NvList>(&[
            nv("a", any::<rt::Long>(&1)),
            nv("b", any::<rt::String>("x")),
            nv("p", any::<Point>(&point(3, -0.5))),
        ]),
        "any" => any::<rt::Any>(&any::<rt::Long>(&5)),
        "typecode" => any::<rt::TypeCodeType>(&Node::type_code()),
        "objref" => any::<AnyEcho>(&Some(echo.clone())),
        "nil" => any::<AnyEcho>(&None),
        "oops" => any::<Oops>(&Oops {
            why: "bad".into(),
            code: 3,
        }),
        "empty" => Any::default(),
        "secret" => any::<hidden::Secret>(&secret("s", vec![point(1, 2.0)], Shade::Dark)),
        "pick" => any::<hidden::Pick>(&hidden::Pick::S(secret("p", vec![], Shade::Light))),
        "moods" => any::<hidden::alias::Moods>(&[hidden::Mood::Sad, hidden::Mood::Happy]),
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

/// `AnyTest::AnyEcho`.
pub struct AnyEchoImpl {
    /// The reference of the object, for "objref".
    pub this: Arc<OnceLock<AnyEcho>>,
}

impl AnyEchoImpl {
    fn value(&self, name: &str) -> Sys<Any> {
        let this = self.this.get().ok_or_else(bad_param)?;
        value(name, this).ok_or_else(bad_param)
    }
}

impl AnyEchoServant for AnyEchoImpl {
    fn echo(&self, a: &Any) -> Sys<Any> {
        Ok(a.clone())
    }

    fn make(&self, name: &str) -> Sys<Any> {
        self.value(name)
    }

    fn check(&self, name: &str, a: &Any) -> Sys<bool> {
        Ok(same(a, &self.value(name)?))
    }

    fn out_inout(&self, name: &str, c: &mut Any) -> Sys<Any> {
        let want = self.value(name)?;
        if !same(c, &want) {
            return Err(bad_param());
        }
        *c = self.value("long")?;
        Ok(want)
    }

    fn echo_tc(&self, tc: &TypeCode) -> Sys<TypeCode> {
        Ok(tc.clone())
    }

    fn make_tc(&self, name: &str) -> Sys<TypeCode> {
        Ok(self.value(name)?.typecode().clone())
    }

    fn check_tc(&self, name: &str, tc: &TypeCode) -> Sys<bool> {
        Ok(tc.equal(self.value(name)?.typecode()))
    }

    fn echo_nvlist(&self, l: &[NameValue]) -> Sys<NvList> {
        Ok(l.to_vec())
    }

    fn describe(&self, a: &Any) -> Sys<String> {
        Ok(describe(a.typecode()))
    }
}

// --- the client side ---------------------------------------------------------------

type Case<'a> = (String, Box<dyn Fn() -> Result<(), String> + 'a>);

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn check(cond: bool, what: impl FnOnce() -> String) -> Result<(), String> {
    if cond { Ok(()) } else { Err(what()) }
}

fn add<'a>(cases: &mut Vec<Case<'a>>, name: String, f: impl Fn() -> Result<(), String> + 'a) {
    cases.push((name, Box::new(f)));
}

/// The stage 2 client cases against the AnyEcho object `echo`.
pub fn cases(echo: &AnyEcho) -> Vec<Case<'_>> {
    let v = move |name: &str| value(name, echo).expect("a name of the table");
    let mut cases: Vec<Case<'_>> = Vec::new();
    for &n in NAMES {
        add(&mut cases, format!("make[{n}]"), move || {
            let got = echo.make(n).map_err(err)?;
            check(same(&got, &v(n)), || format!("got {got:?}"))
        });
        add(&mut cases, format!("check[{n}]"), move || {
            let ok = echo.check(n, &v(n)).map_err(err)?;
            check(ok, || "server says no".into())
        });
        add(&mut cases, format!("echo[{n}]"), move || {
            let got = echo.echo(&v(n)).map_err(err)?;
            check(same(&got, &v(n)), || format!("got {got:?}"))
        });
    }
    for &n in NAMES {
        add(&mut cases, format!("make_tc[{n}]"), move || {
            let got = echo.make_tc(n).map_err(err)?;
            check(got.equal(v(n).typecode()), || format!("got {got:?}"))
        });
        add(&mut cases, format!("check_tc[{n}]"), move || {
            let ok = echo.check_tc(n, v(n).typecode()).map_err(err)?;
            check(ok, || "server says no".into())
        });
        add(&mut cases, format!("echo_tc[{n}]"), move || {
            let got = echo.echo_tc(v(n).typecode()).map_err(err)?;
            check(got.equal(v(n).typecode()), || format!("got {got:?}"))
        });
    }
    for n in ["point", "nvlist", "node"] {
        add(&mut cases, format!("out_inout[{n}]"), move || {
            let mut c = v(n);
            let b = echo.out_inout(n, &mut c).map_err(err)?;
            check(same(&b, &v(n)), || format!("out {b:?}"))?;
            check(same(&c, &v("long")), || format!("inout {c:?}"))
        });
    }
    add(&mut cases, "echo_nvlist".into(), move || {
        let want = v("nvlist")
            .extract::<alias::NvList>()
            .ok_or("not an NVList")?;
        let got = echo.echo_nvlist(&want).map_err(err)?;
        check(got == want, || format!("got {got:?}"))
    });
    for &(n, description) in HIDDEN {
        add(&mut cases, format!("echo[{n}]"), move || {
            let got = echo.echo(&v(n)).map_err(err)?;
            check(same(&got, &v(n)), || format!("got {got:?}"))
        });
        add(&mut cases, format!("describe[{n}]"), move || {
            let got = echo.describe(&v(n)).map_err(err)?;
            check(got == description, || got.clone())
        });
    }
    add(&mut cases, "describe[long]".into(), move || {
        let got = echo.describe(&v("long")).map_err(err)?;
        check(got == "long ", || got.clone())
    });
    add(&mut cases, "describe[node]".into(), move || {
        let got = echo.describe(&v("node")).map_err(err)?;
        check(got == "struct IDL:AnyTest/Node:1.0", || got.clone())
    });
    add(&mut cases, "check_wrong_type".into(), move || {
        let ok = echo.check("long", &any::<rt::Short>(&42)).map_err(err)?;
        check(!ok, || "server says yes".into())
    });
    add(&mut cases, "check_unaliased".into(), move || {
        let ok = echo.check("count", &any::<rt::Long>(&7)).map_err(err)?;
        check(!ok, || "server says yes".into())
    });
    add(&mut cases, "check_tc_unaliased".into(), move || {
        let ok = echo
            .check_tc("count", &rt::Long::type_code())
            .map_err(err)?;
        check(!ok, || "server says yes".into())
    });
    add(&mut cases, "out_inout_wrong".into(), move || {
        match echo.out_inout("point", &mut v("long")) {
            Err(e) if e.is(SystemExceptionKind::BAD_PARAM) => Ok(()),
            Err(e) => Err(format!("got {e}")),
            Ok(_) => Err("no BAD_PARAM".into()),
        }
    });
    cases
}
