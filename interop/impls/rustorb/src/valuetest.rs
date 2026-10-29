//! The valuetype tests (`idl/ValueTest.idl`): the ValueEcho servant and the
//! client cases, the same as those of `impls/omniorbpy/valuetest.py`, with
//! the valuetypes `rustorb-idl -brust` generates. Shared by the client and
//! the server.
//!
//! Instances are reference counted, so the cycles of a ring are broken by
//! hand when it is no longer used.

#![allow(dead_code)]

use rustorb_core::idl::{IdlType as _, Sys};
use rustorb_core::{Any, Orb};

use crate::idl::value_test::{
    LongBox, Node, NodeState, Pair, PairBox, Point, Point3, Point3State, PointSeq, PointState,
    Shape, ShapeState, StringBox, ValueEcho, ValueEchoServant,
};

fn point(x: i32, y: i32) -> Point {
    Point::new(PointState { x, y })
}

fn point3() -> Point {
    Point3::new(Point3State {
        base: PointState { x: 1, y: 2 },
        z: 3,
        label: "three".into(),
    })
    .into()
}

/// A list of n nodes, ids 0 to n - 1.
fn chain(n: i32) -> Node {
    let mut next = None;
    for id in (0..n).rev() {
        next = Some(Node::new(NodeState { id, next }));
    }
    next.expect("n > 0")
}

/// Two nodes that refer to each other.
fn ring() -> Node {
    let a = Node::new(NodeState { id: 1, next: None });
    let b = Node::new(NodeState {
        id: 2,
        next: Some(a.clone()),
    });
    a.with_mut(|s| s.next = Some(b));
    a
}

/// Breaks the cycles of the nodes reachable from `n`.
fn break_ring(n: Option<&Node>) {
    let mut n = n.cloned();
    while let Some(node) = n {
        n = node.with_mut(|s| s.next.take());
    }
}

/// The nodes reachable from `n`, each once.
fn nodes(n: Option<&Node>) -> Vec<Node> {
    let mut seen: Vec<Node> = Vec::new();
    let mut n = n.cloned();
    while let Some(node) = n {
        if seen.iter().any(|s| s.ptr_eq(&node)) {
            break;
        }
        n = node.with(|s| s.next.clone());
        seen.push(node);
    }
    seen
}

/// `ValueTest::ValueEcho`.
pub struct ValueEchoImpl {
    pub orb: Orb,
}

impl ValueEchoServant for ValueEchoImpl {
    fn echo_point(&self, p: Option<&Point>) -> Sys<Option<Point>> {
        Ok(p.cloned())
    }

    fn echo_pair(&self, p: &Pair) -> Sys<Pair> {
        Ok(p.clone())
    }

    fn echo_node(&self, n: Option<&Node>) -> Sys<Option<Node>> {
        Ok(n.cloned())
    }

    fn echo_shape(&self, s: Option<&Shape>) -> Sys<Option<Shape>> {
        Ok(s.cloned())
    }

    fn echo_points(&self, s: &[Option<Point>]) -> Sys<PointSeq> {
        Ok(s.to_vec())
    }

    fn same(&self, a: Option<&Point>, b: Option<&Point>) -> Sys<bool> {
        Ok(matches!((a, b), (Some(a), Some(b)) if a.ptr_eq(b)))
    }

    fn count_nodes(&self, n: Option<&Node>) -> Sys<i32> {
        let count = nodes(n).len() as i32;
        break_ring(n);
        Ok(count)
    }

    fn echo_long_box(&self, b: Option<&LongBox>) -> Sys<Option<LongBox>> {
        Ok(b.cloned())
    }

    fn echo_string_box(&self, b: Option<&StringBox>) -> Sys<Option<StringBox>> {
        Ok(b.cloned())
    }

    fn echo_pair_box(&self, b: Option<&PairBox>) -> Sys<Option<PairBox>> {
        Ok(b.cloned())
    }

    fn echo_any(&self, a: &Any) -> Sys<Any> {
        Ok(a.clone())
    }

    fn shutdown(&self) -> Sys<()> {
        self.orb.shutdown(false)
    }
}

// --- client ----------------------------------------------------------------------

type Case<'a> = (String, Box<dyn Fn() -> Result<(), String> + 'a>);

fn check(cond: bool, what: impl FnOnce() -> String) -> Result<(), String> {
    if cond { Ok(()) } else { Err(what()) }
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn get<T: std::fmt::Debug>(v: &Option<T>) -> Result<&T, String> {
    v.as_ref().ok_or_else(|| "got null".to_owned())
}

fn is_point(v: &Option<Point>, x: i32, y: i32) -> Result<(), String> {
    let p = get(v)?;
    let id = p.handle().info().id;
    check(id == Point::REPOSITORY_ID, || format!("got {id}"))?;
    let s = p.get();
    check((s.x, s.y) == (x, y), || format!("got {s:?}"))
}

fn is_point3(v: &Option<Point>) -> Result<(), String> {
    let p = get(v)?;
    let p3 = Point3::try_from(p.clone()).map_err(|_| format!("got {}", p.handle().info().id))?;
    let (got, want) = (p3.get(), Point3::try_from(point3()).unwrap().get());
    check(got == want, || format!("got {got:?}"))
}

fn same_instance(a: &Option<Point>, b: &Option<Point>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if a.ptr_eq(b))
}

fn node_ids(n: Option<&Node>) -> Vec<i32> {
    let mut ids = Vec::new();
    let mut n = n.cloned();
    while let Some(node) = n {
        let s = node.get();
        ids.push(s.id);
        if ids.len() > 1000 {
            break;
        }
        n = s.next;
    }
    ids
}

/// Whether the node after the one after `a` is `a` again.
fn is_ring(a: &Node) -> Result<(), String> {
    let s = a.get();
    let b = get(&s.next)?.get();
    check(s.id == 1 && b.id == 2, || format!("got {} {}", s.id, b.id))?;
    check(b.next.as_ref().is_some_and(|n| n.ptr_eq(a)), || {
        "the cycle is lost".into()
    })
}

fn add<'a>(cases: &mut Vec<Case<'a>>, name: &str, f: impl Fn() -> Result<(), String> + 'a) {
    cases.push((name.to_owned(), Box::new(f)));
}

/// The valuetype client cases against the ValueEcho object `echo`.
pub fn cases(echo: &ValueEcho) -> Vec<Case<'_>> {
    let mut cases = Vec::new();
    add(&mut cases, "echo_point[plain]", move || {
        let got = echo.echo_point(Some(&point(1, -2))).map_err(err)?;
        is_point(&got, 1, -2)
    });
    add(&mut cases, "echo_point[null]", move || {
        let got = echo.echo_point(None).map_err(err)?;
        check(got.is_none(), || format!("got {got:?}"))
    });
    add(&mut cases, "echo_point[derived]", move || {
        let got = echo.echo_point(Some(&point3())).map_err(err)?;
        is_point3(&got)
    });
    add(&mut cases, "echo_pair[shared]", move || {
        let p = Some(point(5, 6));
        let got = echo
            .echo_pair(&Pair {
                first: p.clone(),
                second: p,
            })
            .map_err(err)?;
        is_point(&got.first, 5, 6)?;
        check(same_instance(&got.first, &got.second), || {
            "not one instance".into()
        })
    });
    add(&mut cases, "echo_pair[distinct]", move || {
        let pair = Pair {
            first: Some(point(5, 6)),
            second: Some(point(5, 6)),
        };
        let got = echo.echo_pair(&pair).map_err(err)?;
        is_point(&got.first, 5, 6)?;
        is_point(&got.second, 5, 6)?;
        check(!same_instance(&got.first, &got.second), || {
            "one instance".into()
        })
    });
    add(&mut cases, "echo_pair[null]", move || {
        let pair = Pair {
            first: None,
            second: Some(point3()),
        };
        let got = echo.echo_pair(&pair).map_err(err)?;
        check(got.first.is_none(), || format!("got {:?}", got.first))?;
        is_point3(&got.second)
    });
    add(&mut cases, "same[shared]", move || {
        let p = point(0, 0);
        let got = echo.same(Some(&p), Some(&p)).map_err(err)?;
        check(got, || format!("got {got:?}"))
    });
    add(&mut cases, "same[distinct]", move || {
        let got = echo
            .same(Some(&point(0, 0)), Some(&point(0, 0)))
            .map_err(err)?;
        check(!got, || format!("got {got:?}"))
    });
    add(&mut cases, "echo_node[list]", move || {
        let got = echo.echo_node(Some(&chain(20))).map_err(err)?;
        let ids = node_ids(got.as_ref());
        check(ids == (0..20).collect::<Vec<_>>(), || {
            format!("got {ids:?}")
        })
    });
    add(&mut cases, "echo_node[ring]", move || {
        let ring = ring();
        let got = echo.echo_node(Some(&ring));
        break_ring(Some(&ring));
        let got = got.map_err(err)?;
        let result = is_ring(get(&got)?);
        break_ring(got.as_ref());
        result
    });
    add(&mut cases, "count_nodes[list]", move || {
        let got = echo.count_nodes(Some(&chain(20))).map_err(err)?;
        check(got == 20, || format!("got {got:?}"))
    });
    add(&mut cases, "count_nodes[ring]", move || {
        let ring = ring();
        let got = echo.count_nodes(Some(&ring));
        break_ring(Some(&ring));
        let got = got.map_err(err)?;
        check(got == 2, || format!("got {got:?}"))
    });
    add(&mut cases, "echo_shape", move || {
        let c = Some(point(0, 0));
        let shape = Shape::new(ShapeState {
            name: "square".into(),
            points: vec![Some(point(1, 1)), c.clone()],
            center: c,
            area: 1.5,
        });
        let got = echo.echo_shape(Some(&shape)).map_err(err)?;
        let m = get(&got)?.get();
        check(m.name == "square" && m.area == 1.5, || format!("got {m:?}"))?;
        check(m.points.len() == 2, || format!("got {:?}", m.points))?;
        is_point(&m.points[0], 1, 1)?;
        is_point(&m.center, 0, 0)?;
        check(same_instance(&m.points[1], &m.center), || {
            "not one instance".into()
        })
    });
    add(&mut cases, "echo_points", move || {
        let p = Some(point(7, 8));
        let q = Some(point3());
        let s = vec![p.clone(), q.clone(), p, None, q];
        let m = echo.echo_points(&s).map_err(err)?;
        check(m.len() == 5, || format!("got {m:?}"))?;
        is_point(&m[0], 7, 8)?;
        is_point3(&m[1])?;
        check(
            same_instance(&m[0], &m[2]) && same_instance(&m[1], &m[4]),
            || "sharing lost".into(),
        )?;
        check(m[3].is_none(), || format!("got {:?}", m[3]))
    });
    add(&mut cases, "echo_long_box[value]", move || {
        let got = echo.echo_long_box(Some(&LongBox::new(7))).map_err(err)?;
        check(got.as_deref() == Some(&7), || format!("got {got:?}"))
    });
    add(&mut cases, "echo_long_box[null]", move || {
        let got = echo.echo_long_box(None).map_err(err)?;
        check(got.is_none(), || format!("got {got:?}"))
    });
    add(&mut cases, "echo_string_box[value]", move || {
        let got = echo
            .echo_string_box(Some(&StringBox::new("boxed".into())))
            .map_err(err)?;
        check(got.as_deref().map(String::as_str) == Some("boxed"), || {
            format!("got {got:?}")
        })
    });
    add(&mut cases, "echo_string_box[null]", move || {
        let got = echo.echo_string_box(None).map_err(err)?;
        check(got.is_none(), || format!("got {got:?}"))
    });
    add(&mut cases, "echo_pair_box", move || {
        let b = PairBox::new(Pair {
            first: Some(point(1, 2)),
            second: None,
        });
        let got = echo.echo_pair_box(Some(&b)).map_err(err)?;
        let pair = get(&got)?;
        is_point(&pair.first, 1, 2)?;
        check(pair.second.is_none(), || format!("got {:?}", pair.second))
    });
    add(&mut cases, "echo_any[ring]", move || {
        let ring = ring();
        let a = Any::insert::<Node>(&Some(ring.clone())).map_err(err);
        break_ring(Some(&ring));
        // An any holds the core's values, whose cycles are broken with
        // break_cycles.
        let a = a?;
        let got = echo.echo_any(&a);
        a.into_parts().1.break_cycles();
        let got = got.map_err(err)?;
        check(got.typecode().equal(&Node::type_code()), || {
            format!("got {:?}", got.typecode())
        })?;
        let n = got.extract::<Node>();
        got.into_parts().1.break_cycles();
        let n = n.ok_or("not a Node")?;
        let result = is_ring(get(&n)?);
        break_ring(n.as_ref());
        result
    });
    add(&mut cases, "echo_any[derived]", move || {
        let a = Any::insert::<Point>(&Some(point3())).map_err(err)?;
        let got = echo.echo_any(&a).map_err(err)?;
        check(got.typecode().equal(&Point::type_code()), || {
            format!("got {:?}", got.typecode())
        })?;
        is_point3(&got.extract::<Point>().ok_or("not a Point")?)
    });
    add(&mut cases, "echo_any[box]", move || {
        let a = Any::insert::<StringBox>(&Some(StringBox::new("in an any".into()))).map_err(err)?;
        let got = echo.echo_any(&a).map_err(err)?;
        check(got.typecode().equal(&StringBox::type_code()), || {
            format!("got {:?}", got.typecode())
        })?;
        let b = got.extract::<StringBox>().ok_or("not a StringBox")?;
        check(
            b.as_deref().map(String::as_str) == Some("in an any"),
            || format!("got {b:?}"),
        )
    });
    cases
}
