//! The valuetype tests (`idl/ValueTest.idl`): the TypeCodes, the ValueEcho
//! servant and the client cases, the same as those of
//! `impls/omniorbpy/valuetest.py`. Values are the core's [`Value`] and
//! [`ValueRef`], with no generated code. Shared by the client and the
//! server.

#![allow(dead_code)]

use std::sync::LazyLock;

use rustorb_core::{
    Any, CompletionStatus, Exception, InStream, ObjectRef, Orb, Servant, ServerReply,
    SystemException, SystemExceptionKind, TCKind, TypeCode, Value, ValueRef, minor,
};

pub const VALUE_ECHO_ID: &str = "IDL:ValueTest/ValueEcho:1.0";

const PUBLIC: i16 = 1;
const PRIVATE: i16 = 0;

/// The TypeCodes of ValueTest.idl.
pub struct Types {
    pub point: TypeCode,
    pub point3: TypeCode,
    pub node: TypeCode,
    pub pair: TypeCode,
    pub point_seq: TypeCode,
    pub shape: TypeCode,
    pub long_box: TypeCode,
    pub string_box: TypeCode,
    pub pair_box: TypeCode,
}

fn simple(kind: TCKind) -> TypeCode {
    TypeCode::simple(kind).expect("a simple kind")
}

pub static TYPES: LazyLock<Types> = LazyLock::new(|| {
    let long = simple(TCKind::Long);
    let string = TypeCode::string(0);
    let point = TypeCode::value_tc(
        TCKind::Value,
        "IDL:ValueTest/Point:1.0",
        "Point",
        0,
        None,
        &[("x", long.clone(), PUBLIC), ("y", long.clone(), PUBLIC)],
    )
    .unwrap();
    let point3 = TypeCode::value_tc(
        TCKind::Value,
        "IDL:ValueTest/Point3:1.0",
        "Point3",
        3,
        Some(&point),
        &[
            ("z", long.clone(), PUBLIC),
            ("label", string.clone(), PRIVATE),
        ],
    )
    .unwrap();
    let node = TypeCode::value_tc(
        TCKind::Value,
        "IDL:ValueTest/Node:1.0",
        "Node",
        0,
        None,
        &[
            ("id", long.clone(), PUBLIC),
            (
                "next",
                TypeCode::recursive("IDL:ValueTest/Node:1.0"),
                PUBLIC,
            ),
        ],
    )
    .unwrap();
    let pair = TypeCode::struct_tc(
        "IDL:ValueTest/Pair:1.0",
        "Pair",
        &[("first", point.clone()), ("second", point.clone())],
    )
    .unwrap();
    let point_seq = TypeCode::alias(
        "IDL:ValueTest/PointSeq:1.0",
        "PointSeq",
        &TypeCode::sequence(&point, 0).unwrap(),
    )
    .unwrap();
    let shape = TypeCode::value_tc(
        TCKind::Value,
        "IDL:ValueTest/Shape:1.0",
        "Shape",
        0,
        None,
        &[
            ("name", string.clone(), PUBLIC),
            ("points", point_seq.clone(), PUBLIC),
            ("center", point.clone(), PUBLIC),
            ("area", simple(TCKind::Double), PUBLIC),
        ],
    )
    .unwrap();
    let long_box = TypeCode::value_box("IDL:ValueTest/LongBox:1.0", "LongBox", &long).unwrap();
    let string_box =
        TypeCode::value_box("IDL:ValueTest/StringBox:1.0", "StringBox", &string).unwrap();
    let pair_box = TypeCode::value_box("IDL:ValueTest/PairBox:1.0", "PairBox", &pair).unwrap();
    // Point3 arrives where Point is declared.
    rustorb_core::register_value_type(&point3);
    Types {
        point,
        point3,
        node,
        pair,
        point_seq,
        shape,
        long_box,
        string_box,
        pair_box,
    }
});

fn instance(tc: &TypeCode, members: Vec<Value>) -> ValueRef {
    ValueRef::new(tc.clone(), members)
}

fn some(r: &ValueRef) -> Value {
    Value::Valuetype(Some(r.clone()))
}

fn point(x: i32, y: i32) -> ValueRef {
    instance(&TYPES.point, vec![Value::Long(x), Value::Long(y)])
}

fn point3() -> ValueRef {
    instance(
        &TYPES.point3,
        vec![
            Value::Long(1),
            Value::Long(2),
            Value::Long(3),
            Value::String("three".into()),
        ],
    )
}

/// A list of n nodes, ids 0 to n - 1.
fn chain(n: i32) -> ValueRef {
    let mut next = Value::Valuetype(None);
    let mut head = None;
    for i in (0..n).rev() {
        let node = instance(&TYPES.node, vec![Value::Long(i), next]);
        next = some(&node);
        head = Some(node);
    }
    head.expect("n > 0")
}

/// Two nodes that refer to each other.
fn ring() -> ValueRef {
    let a = instance(&TYPES.node, Vec::new());
    let b = instance(&TYPES.node, vec![Value::Long(2), some(&a)]);
    a.set_members(vec![Value::Long(1), some(&b)]);
    a
}

fn bad_operation() -> SystemException {
    SystemException::new(
        SystemExceptionKind::BAD_OPERATION,
        minor::UNKNOWN_OPERATION,
        CompletionStatus::No,
    )
}

pub struct ValueEcho {
    pub orb: Orb,
}

impl Servant for ValueEcho {
    fn repository_id(&self) -> &str {
        VALUE_ECHO_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        let t = &*TYPES;
        let echo = |tc: &TypeCode, input: &mut InStream<'_>, reply: &mut ServerReply| {
            let v = input.read_value(tc)?;
            let result = reply.output().write_value(tc, &v);
            v.break_cycles();
            result
        };
        match operation {
            "echo_point" => echo(&t.point, input, reply),
            "echo_pair" => echo(&t.pair, input, reply),
            "echo_node" => echo(&t.node, input, reply),
            "echo_shape" => echo(&t.shape, input, reply),
            "echo_points" => echo(&t.point_seq, input, reply),
            "echo_long_box" => echo(&t.long_box, input, reply),
            "echo_string_box" => echo(&t.string_box, input, reply),
            "echo_pair_box" => echo(&t.pair_box, input, reply),
            "same" => {
                let a = input.read_value(&t.point)?;
                let b = input.read_value(&t.point)?;
                let same = match (&a, &b) {
                    (Value::Valuetype(Some(a)), Value::Valuetype(Some(b))) => a.ptr_eq(b),
                    _ => false,
                };
                reply.output().write_bool(same);
                Ok(())
            }
            "count_nodes" => {
                let n = input.read_value(&t.node)?;
                let count = count_nodes(&n);
                n.break_cycles();
                reply.output().write_i32(count);
                Ok(())
            }
            "echo_any" => {
                let a = input.read_any()?;
                let result = reply.output().write_any(&a);
                a.into_parts().1.break_cycles();
                result
            }
            "shutdown" => self.orb.shutdown(false),
            _ => Err(bad_operation()),
        }
    }
}

/// The number of nodes reachable from `n`, each counted once.
fn count_nodes(n: &Value) -> i32 {
    let mut seen: Vec<ValueRef> = Vec::new();
    let mut n = n.clone();
    while let Value::Valuetype(Some(r)) = n {
        if seen.iter().any(|s| s.ptr_eq(&r)) {
            break;
        }
        n = r
            .members()
            .get(1)
            .cloned()
            .unwrap_or(Value::Valuetype(None));
        seen.push(r);
    }
    seen.len() as i32
}

// --- client ----------------------------------------------------------------------

type Case<'a> = (String, Box<dyn Fn() -> Result<(), String> + 'a>);

fn check(cond: bool, what: impl FnOnce() -> String) -> Result<(), String> {
    if cond { Ok(()) } else { Err(what()) }
}

fn err(e: Exception) -> String {
    e.to_string()
}

/// Calls `op` with the values `args` (each with its type) and reads a
/// result of type `result`.
fn call(
    echo: &ObjectRef,
    op: &str,
    args: &[(&TypeCode, Value)],
    result: &TypeCode,
) -> Result<Value, String> {
    let args: Vec<(TypeCode, Value)> = args
        .iter()
        .map(|(t, v)| ((*t).clone(), v.clone()))
        .collect();
    let reply = echo
        .request(op)
        .args(move |out| {
            for (t, v) in &args {
                out.write_value(t, v)?;
            }
            Ok(())
        })
        .invoke()
        .map_err(err)?;
    reply.input().read_value(result).map_err(|e| e.to_string())
}

fn get(v: &Value) -> Result<ValueRef, String> {
    match v {
        Value::Valuetype(Some(r)) => Ok(r.clone()),
        other => Err(format!("got {other:?}")),
    }
}

fn is_point(v: &Value, x: i32, y: i32) -> Result<(), String> {
    let r = get(v)?;
    check(r.id() == "IDL:ValueTest/Point:1.0", || {
        format!("got {}", r.id())
    })?;
    check(r.members() == [Value::Long(x), Value::Long(y)], || {
        format!("got {:?}", r.members())
    })
}

fn is_point3(v: &Value) -> Result<(), String> {
    let r = get(v)?;
    check(r.id() == "IDL:ValueTest/Point3:1.0", || {
        format!("got {}", r.id())
    })?;
    check(r.members() == point3().members(), || {
        format!("got {:?}", r.members())
    })
}

fn members(v: &Value) -> Result<Vec<Value>, String> {
    match v {
        Value::List(items) => Ok(items.clone()),
        other => Err(format!("got {other:?}")),
    }
}

fn same_instance(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Valuetype(Some(a)), Value::Valuetype(Some(b))) if a.ptr_eq(b))
}

fn node_ids(v: &Value) -> Vec<i32> {
    let mut ids = Vec::new();
    let mut v = v.clone();
    while let Value::Valuetype(Some(r)) = v {
        let m = r.members();
        if let Some(Value::Long(id)) = m.first() {
            ids.push(*id);
        }
        if ids.len() > 1000 {
            break;
        }
        v = m.get(1).cloned().unwrap_or(Value::Valuetype(None));
    }
    ids
}

fn add<'a>(cases: &mut Vec<Case<'a>>, name: &str, f: impl Fn() -> Result<(), String> + 'a) {
    cases.push((name.to_owned(), Box::new(f)));
}

/// The valuetype client cases against the ValueEcho object `echo`.
pub fn cases(echo: &ObjectRef) -> Vec<Case<'_>> {
    let t = &*TYPES;
    let null = || Value::Valuetype(None);
    let mut cases = Vec::new();
    add(&mut cases, "echo_point[plain]", move || {
        let got = call(
            echo,
            "echo_point",
            &[(&t.point, some(&point(1, -2)))],
            &t.point,
        )?;
        is_point(&got, 1, -2)
    });
    add(&mut cases, "echo_point[null]", move || {
        let got = call(echo, "echo_point", &[(&t.point, null())], &t.point)?;
        check(got == null(), || format!("got {got:?}"))
    });
    add(&mut cases, "echo_point[derived]", move || {
        let got = call(echo, "echo_point", &[(&t.point, some(&point3()))], &t.point)?;
        is_point3(&got)
    });
    add(&mut cases, "echo_pair[shared]", move || {
        let p = point(5, 6);
        let got = call(
            echo,
            "echo_pair",
            &[(&t.pair, Value::List(vec![some(&p), some(&p)]))],
            &t.pair,
        )?;
        let m = members(&got)?;
        is_point(&m[0], 5, 6)?;
        check(same_instance(&m[0], &m[1]), || "not one instance".into())
    });
    add(&mut cases, "echo_pair[distinct]", move || {
        let pair = Value::List(vec![some(&point(5, 6)), some(&point(5, 6))]);
        let got = call(echo, "echo_pair", &[(&t.pair, pair)], &t.pair)?;
        let m = members(&got)?;
        is_point(&m[0], 5, 6)?;
        is_point(&m[1], 5, 6)?;
        check(!same_instance(&m[0], &m[1]), || "one instance".into())
    });
    add(&mut cases, "echo_pair[null]", move || {
        let pair = Value::List(vec![null(), some(&point3())]);
        let got = call(echo, "echo_pair", &[(&t.pair, pair)], &t.pair)?;
        let m = members(&got)?;
        check(m[0] == null(), || format!("got {:?}", m[0]))?;
        is_point3(&m[1])
    });
    add(&mut cases, "same[shared]", move || {
        let p = some(&point(0, 0));
        let got = call(
            echo,
            "same",
            &[(&t.point, p.clone()), (&t.point, p)],
            &simple(TCKind::Boolean),
        )?;
        check(got == Value::Bool(true), || format!("got {got:?}"))
    });
    add(&mut cases, "same[distinct]", move || {
        let got = call(
            echo,
            "same",
            &[
                (&t.point, some(&point(0, 0))),
                (&t.point, some(&point(0, 0))),
            ],
            &simple(TCKind::Boolean),
        )?;
        check(got == Value::Bool(false), || format!("got {got:?}"))
    });
    add(&mut cases, "echo_node[list]", move || {
        let got = call(echo, "echo_node", &[(&t.node, some(&chain(20)))], &t.node)?;
        let ids = node_ids(&got);
        check(ids == (0..20).collect::<Vec<_>>(), || {
            format!("got {ids:?}")
        })
    });
    add(&mut cases, "echo_node[ring]", move || {
        let ring = some(&ring());
        let got = call(echo, "echo_node", &[(&t.node, ring.clone())], &t.node)?;
        ring.break_cycles();
        let a = get(&got)?;
        let b = get(&a.members()[1])?;
        let result = check(
            a.members()[0] == Value::Long(1) && b.members()[0] == Value::Long(2),
            || format!("got {:?} {:?}", a.members()[0], b.members()[0]),
        )
        .and_then(|()| {
            check(same_instance(&b.members()[1], &got), || {
                "the cycle is lost".into()
            })
        });
        got.break_cycles();
        result
    });
    add(&mut cases, "count_nodes[list]", move || {
        let got = call(
            echo,
            "count_nodes",
            &[(&t.node, some(&chain(20)))],
            &simple(TCKind::Long),
        )?;
        check(got == Value::Long(20), || format!("got {got:?}"))
    });
    add(&mut cases, "count_nodes[ring]", move || {
        let ring = some(&ring());
        let got = call(
            echo,
            "count_nodes",
            &[(&t.node, ring.clone())],
            &simple(TCKind::Long),
        );
        ring.break_cycles();
        let got = got?;
        check(got == Value::Long(2), || format!("got {got:?}"))
    });
    add(&mut cases, "echo_shape", move || {
        let c = point(0, 0);
        let shape = instance(
            &t.shape,
            vec![
                Value::String("square".into()),
                Value::List(vec![some(&point(1, 1)), some(&c)]),
                some(&c),
                Value::Double(1.5),
            ],
        );
        let got = call(echo, "echo_shape", &[(&t.shape, some(&shape))], &t.shape)?;
        let m = get(&got)?.members();
        check(
            m[0] == Value::String("square".into()) && m[3] == Value::Double(1.5),
            || format!("got {m:?}"),
        )?;
        let points = members(&m[1])?;
        check(points.len() == 2, || format!("got {points:?}"))?;
        is_point(&points[0], 1, 1)?;
        is_point(&m[2], 0, 0)?;
        check(same_instance(&points[1], &m[2]), || {
            "not one instance".into()
        })
    });
    add(&mut cases, "echo_points", move || {
        let p = some(&point(7, 8));
        let q = some(&point3());
        let s = Value::List(vec![p.clone(), q.clone(), p, null(), q]);
        let got = call(echo, "echo_points", &[(&t.point_seq, s)], &t.point_seq)?;
        let m = members(&got)?;
        check(m.len() == 5, || format!("got {m:?}"))?;
        is_point(&m[0], 7, 8)?;
        is_point3(&m[1])?;
        check(
            same_instance(&m[0], &m[2]) && same_instance(&m[1], &m[4]),
            || "sharing lost".into(),
        )?;
        check(m[3] == null(), || format!("got {:?}", m[3]))
    });
    let boxed = |tc: &TypeCode, v: Value| some(&instance(tc, vec![v]));
    add(&mut cases, "echo_long_box[value]", move || {
        let b = boxed(&t.long_box, Value::Long(7));
        let got = call(
            echo,
            "echo_long_box",
            &[(&t.long_box, b.clone())],
            &t.long_box,
        )?;
        check(got == b, || format!("got {got:?}"))
    });
    add(&mut cases, "echo_long_box[null]", move || {
        let got = call(echo, "echo_long_box", &[(&t.long_box, null())], &t.long_box)?;
        check(got == null(), || format!("got {got:?}"))
    });
    add(&mut cases, "echo_string_box[value]", move || {
        let b = boxed(&t.string_box, Value::String("boxed".into()));
        let got = call(
            echo,
            "echo_string_box",
            &[(&t.string_box, b.clone())],
            &t.string_box,
        )?;
        check(got == b, || format!("got {got:?}"))
    });
    add(&mut cases, "echo_string_box[null]", move || {
        let got = call(
            echo,
            "echo_string_box",
            &[(&t.string_box, null())],
            &t.string_box,
        )?;
        check(got == null(), || format!("got {got:?}"))
    });
    add(&mut cases, "echo_pair_box", move || {
        let b = boxed(&t.pair_box, Value::List(vec![some(&point(1, 2)), null()]));
        let got = call(echo, "echo_pair_box", &[(&t.pair_box, b)], &t.pair_box)?;
        let pair = get(&got)?.members();
        let m = members(&pair[0])?;
        is_point(&m[0], 1, 2)?;
        check(m[1] == null(), || format!("got {:?}", m[1]))
    });
    let call_any = move |a: Any| -> Result<Any, String> {
        let reply = echo
            .request("echo_any")
            .args(move |out| out.write_any(&a))
            .invoke()
            .map_err(err)?;
        reply.input().read_any().map_err(|e| e.to_string())
    };
    add(&mut cases, "echo_any[ring]", move || {
        let ring = some(&ring());
        let got = call_any(Any::new(t.node.clone(), ring.clone()));
        ring.break_cycles();
        let got = got?;
        check(got.typecode().equal(&t.node), || {
            format!("got {:?}", got.typecode())
        })?;
        let ids = node_ids(got.value());
        let a = get(got.value())?;
        let b = get(&a.members()[1])?;
        let result = check(
            ids.len() > 2 && ids[..2] == [1, 2] && same_instance(&b.members()[1], got.value()),
            || format!("got {:?}", &ids[..ids.len().min(4)]),
        );
        got.into_parts().1.break_cycles();
        result
    });
    add(&mut cases, "echo_any[derived]", move || {
        let got = call_any(Any::new(t.point.clone(), some(&point3())))?;
        check(got.typecode().equal(&t.point), || {
            format!("got {:?}", got.typecode())
        })?;
        is_point3(got.value())
    });
    add(&mut cases, "echo_any[box]", move || {
        let b = boxed(&t.string_box, Value::String("in an any".into()));
        let got = call_any(Any::new(t.string_box.clone(), b.clone()))?;
        check(got.typecode().equal(&t.string_box), || {
            format!("got {:?}", got.typecode())
        })?;
        check(*got.value() == b, || format!("got {:?}", got.value()))
    });
    cases
}
