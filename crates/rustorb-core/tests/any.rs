//! Any and TypeCode values sent between two ORBs on localhost.

use std::sync::Arc;

use rustorb_core::cdr::ByteOrder;
use rustorb_core::{
    Any, CompletionStatus, EndpointAddr, Exception, InStream, ObjectRef, Orb, OrbOptions, Servant,
    ServerReply, SystemException, SystemExceptionKind, TCKind, TypeCode, Value, minor,
};

const ID: &str = "IDL:test/AnyEcho:1.0";

struct AnyEcho;

impl Servant for AnyEcho {
    fn repository_id(&self) -> &str {
        ID
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
            "echo_tc" => {
                let tc = input.read_typecode()?;
                reply.output().write_typecode(&tc)
            }
            // The kind and id of the TypeCode of an Any, as the server read
            // them.
            "describe" => {
                let a = input.read_any()?;
                let tc = a.typecode();
                let text = format!("{} {}", tc.kind().name(), tc.id().unwrap_or("-"));
                reply.output().write_string(&text)
            }
            _ => Err(SystemException::new(
                SystemExceptionKind::BAD_OPERATION,
                minor::UNKNOWN_OPERATION,
                CompletionStatus::No,
            )),
        }
    }
}

struct Fixture {
    _server: Orb,
    _client: Orb,
    obj: ObjectRef,
}

fn fixture() -> Fixture {
    let server = Orb::with_options(OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..OrbOptions::default()
    })
    .unwrap();
    let root = server.root_poa().unwrap();
    let id = root.activate_object(Arc::new(AnyEcho)).unwrap();
    let local = root.id_to_reference(&id).unwrap();
    root.the_poa_manager().activate().unwrap();
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let ior = local.ior().to_ior_string(ByteOrder::Big).unwrap();
    let obj = client.string_to_object(&ior).unwrap().unwrap();
    Fixture {
        _server: server,
        _client: client,
        obj,
    }
}

fn echo(obj: &ObjectRef, a: &Any) -> Result<Any, Exception> {
    let a = a.clone();
    let reply = obj
        .request("echo")
        .args(move |out| out.write_any(&a))
        .invoke()?;
    Ok(reply.input().read_any()?)
}

fn long() -> TypeCode {
    TypeCode::simple(TCKind::Long).unwrap()
}

fn node_tc() -> TypeCode {
    let rec = TypeCode::recursive("IDL:test/Node:1.0");
    TypeCode::struct_tc(
        "IDL:test/Node:1.0",
        "Node",
        &[
            ("name", TypeCode::string(0)),
            ("children", TypeCode::sequence(&rec, 0).unwrap()),
        ],
    )
    .unwrap()
}

fn node(name: &str, children: Vec<Value>) -> Value {
    Value::List(vec![Value::String(name.into()), Value::List(children)])
}

fn values() -> Vec<Any> {
    let color =
        TypeCode::enum_tc("IDL:test/Color:1.0", "Color", &["RED", "GREEN", "BLUE"]).unwrap();
    let union = TypeCode::union_tc(
        "IDL:test/U:1.0",
        "U",
        &long(),
        &[
            (Some(1), "l", long()),
            (Some(2), "s", TypeCode::string(0)),
            (None, "b", TypeCode::simple(TCKind::Boolean).unwrap()),
        ],
    )
    .unwrap();
    let point = TypeCode::struct_tc(
        "IDL:test/Point:1.0",
        "Point",
        &[
            ("x", long()),
            ("y", TypeCode::simple(TCKind::Double).unwrap()),
        ],
    )
    .unwrap();
    let octet_seq = TypeCode::sequence(&TypeCode::simple(TCKind::Octet).unwrap(), 0).unwrap();
    let char_array = TypeCode::array(&TypeCode::simple(TCKind::Char).unwrap(), 3).unwrap();
    let points = TypeCode::sequence(&point, 0).unwrap();
    let count = TypeCode::alias("IDL:test/Count:1.0", "Count", &long()).unwrap();
    vec![
        Any::from(42i32),
        Any::from(-1i16),
        Any::from(u64::MAX),
        Any::from(1.5f64),
        Any::from(true),
        Any::from('x'),
        Any::from(0xffu8),
        Any::from("hello"),
        Any::new(TypeCode::wstring(0), Value::String("wide \u{3042}".into())),
        Any::new(TypeCode::string(5), Value::String("short".into())),
        Any::new(count, Value::Long(3)),
        Any::new(color, Value::Enum(2)),
        Any::new(
            union.clone(),
            Value::Union {
                disc: Box::new(Value::Long(2)),
                member: Some((1, Box::new(Value::String("two".into())))),
            },
        ),
        Any::new(
            union,
            Value::Union {
                disc: Box::new(Value::Long(7)),
                member: Some((2, Box::new(Value::Bool(true)))),
            },
        ),
        Any::new(
            points,
            Value::List(vec![
                Value::List(vec![Value::Long(1), Value::Double(0.5)]),
                Value::List(vec![Value::Long(-2), Value::Double(2.0)]),
            ]),
        ),
        Any::new(octet_seq, Value::Octets(vec![0, 1, 255].into())),
        Any::new(char_array, Value::Chars("abc".into())),
        Any::new(
            node_tc(),
            node(
                "root",
                vec![node("a", vec![]), node("b", vec![node("c", vec![])])],
            ),
        ),
        Any::new(
            TypeCode::fixed(5, 2).unwrap(),
            Value::Fixed {
                digits: vec![1, 2, 3, 4, 5],
                negative: true,
            },
        ),
        Any::new(
            TypeCode::simple(TCKind::Any).unwrap(),
            Value::Any(Box::new(Any::from("inner"))),
        ),
        Any::from(node_tc()),
        Any::default(),
    ]
}

#[test]
fn values_come_back_unchanged() {
    let f = fixture();
    for a in values() {
        let back = echo(&f.obj, &a).unwrap();
        assert_eq!(back, a, "{:?}", a.typecode());
    }
}

#[test]
fn the_server_sees_the_typecode() {
    let f = fixture();
    let describe = |a: Any| {
        let reply = f
            .obj
            .request("describe")
            .args(move |out| out.write_any(&a))
            .invoke()
            .unwrap();
        reply.input().read_string().unwrap()
    };
    assert_eq!(describe(Any::from(1i32)), "long -");
    assert_eq!(
        describe(Any::new(node_tc(), node("n", vec![]))),
        "struct IDL:test/Node:1.0"
    );
}

#[test]
fn typecodes_come_back_equal() {
    let f = fixture();
    for a in values() {
        let tc = a.typecode().clone();
        let sent = tc.clone();
        let reply = f
            .obj
            .request("echo_tc")
            .args(move |out| out.write_typecode(&sent))
            .invoke()
            .unwrap();
        let back = reply.input().read_typecode().unwrap();
        assert!(back.equal(&tc), "{tc:?}");
    }
}

#[test]
fn object_references_in_an_any() {
    let f = fixture();
    let tc = TypeCode::objref(ID, "AnyEcho").unwrap();
    let a = Any::new(tc.clone(), Value::Objref(Some(f.obj.clone())));
    let back = echo(&f.obj, &a).unwrap();
    match back.value() {
        Value::Objref(Some(obj)) => {
            assert!(obj.is_equivalent(&f.obj));
        }
        other => panic!("{other:?}"),
    }
    let nil = Any::new(tc, Value::Objref(None));
    assert_eq!(echo(&f.obj, &nil).unwrap(), nil);
}

#[test]
fn values_that_do_not_fit_are_bad_param() {
    let f = fixture();
    let bad = [
        Any::new(long(), Value::String("x".into())),
        Any::new(TypeCode::string(2), Value::String("too long".into())),
        Any::new(
            TypeCode::array(&long(), 2).unwrap(),
            Value::List(vec![Value::Long(1)]),
        ),
        Any::new(
            TypeCode::enum_tc("IDL:E:1.0", "E", &["A"]).unwrap(),
            Value::Enum(1),
        ),
    ];
    for a in bad {
        match echo(&f.obj, &a) {
            Err(Exception::System(e)) => {
                assert_eq!(e.kind(), Some(SystemExceptionKind::BAD_PARAM), "{a:?}");
                assert_eq!(e.completed, CompletionStatus::No);
            }
            other => panic!("{other:?}"),
        }
    }
    // A recursive placeholder cannot be sent.
    let a = Any::new(TypeCode::recursive("IDL:X:1.0"), Value::Void);
    match echo(&f.obj, &a) {
        Err(Exception::System(e)) => assert_eq!(e.kind(), Some(SystemExceptionKind::BAD_TYPECODE)),
        other => panic!("{other:?}"),
    }
}
