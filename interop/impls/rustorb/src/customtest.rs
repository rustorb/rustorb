//! The custom valuetype tests (`idl/CustomTest.idl`): the marshaling of
//! Blob, the CustomEcho servant and the client cases, the same as those of
//! `impls/rustorbpy/customtest.py`. Shared by the client and the server.

#![allow(dead_code)]

use rustorb_core::Orb;
use rustorb_core::idl::Sys;

use crate::idl::custom::custom_test::{
    Blob, BlobPair, BlobSeq, BlobState, CustomEcho, CustomEchoServant,
};
use crate::idl::value_test::{Point, Point3, Point3State, PointState};

/// `CustomTest::CustomEcho`.
pub struct CustomEchoImpl {
    pub orb: Orb,
}

impl CustomEchoServant for CustomEchoImpl {
    fn echo_blob(&self, b: Option<&Blob>) -> Sys<Option<Blob>> {
        Ok(b.cloned())
    }

    fn echo_blob_pair(&self, p: &BlobPair) -> Sys<BlobPair> {
        Ok(p.clone())
    }

    fn echo_blobs(&self, s: &[Option<Blob>]) -> Sys<BlobSeq> {
        Ok(s.to_vec())
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

fn blob(n: i32, label: &str, pt: Option<Point>, values: Vec<i32>) -> Blob {
    Blob::new(BlobState {
        n,
        label: label.into(),
        pt,
        values,
    })
}

fn point(x: i32, y: i32) -> Point {
    Point::new(PointState { x, y })
}

fn is_blob(b: &Option<Blob>, n: i32, label: &str, values: &[i32]) -> Result<BlobState, String> {
    let s = b.as_ref().ok_or("got null")?.get();
    check(
        (s.n, s.label.as_str(), s.values.as_slice()) == (n, label, values),
        || format!("got {} {:?} {:?}", s.n, s.label, s.values),
    )?;
    Ok(s)
}

fn is_point(p: &Option<Point>, x: i32, y: i32) -> Result<(), String> {
    let p = p.as_ref().ok_or("got a null point")?;
    let id = p.handle().info().id;
    check(id == Point::REPOSITORY_ID, || format!("got {id}"))?;
    let s = p.get();
    check((s.x, s.y) == (x, y), || format!("got {s:?}"))
}

fn same(a: &Option<Point>, b: &Option<Point>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if a.ptr_eq(b))
}

fn add<'a>(cases: &mut Vec<Case<'a>>, name: &str, f: impl Fn() -> Result<(), String> + 'a) {
    cases.push((name.to_owned(), Box::new(f)));
}

/// The custom valuetype client cases against the CustomEcho object `echo`.
pub fn cases(echo: &CustomEcho) -> Vec<Case<'_>> {
    let mut cases = Vec::new();
    add(&mut cases, "echo_blob[plain]", move || {
        let got = echo
            .echo_blob(Some(&blob(7, "seven", Some(point(1, -2)), vec![1, 2, 3])))
            .map_err(err)?;
        let s = is_blob(&got, 7, "seven", &[1, 2, 3])?;
        is_point(&s.pt, 1, -2)
    });
    add(&mut cases, "echo_blob[null]", move || {
        let got = echo.echo_blob(None).map_err(err)?;
        check(got.is_none(), || "not null".into())
    });
    add(&mut cases, "echo_blob[empty]", move || {
        let got = echo
            .echo_blob(Some(&blob(0, "", None, vec![])))
            .map_err(err)?;
        let s = is_blob(&got, 0, "", &[])?;
        check(s.pt.is_none(), || "a point".into())
    });
    add(&mut cases, "echo_blob[derived]", move || {
        let p3 = Point3::new(Point3State {
            base: PointState { x: 1, y: 2 },
            z: 3,
            label: "three".into(),
        });
        let got = echo
            .echo_blob(Some(&blob(3, "three", Some(p3.clone().into()), vec![-1])))
            .map_err(err)?;
        let s = is_blob(&got, 3, "three", &[-1])?;
        let p = s.pt.ok_or("got a null point")?;
        let got =
            Point3::try_from(p.clone()).map_err(|_| format!("got {}", p.handle().info().id))?;
        let (got, want) = (got.get(), p3.get());
        check(got == want, || format!("got {got:?}"))
    });
    add(&mut cases, "echo_blob_pair[shared]", move || {
        let p = point(5, 6);
        let got = echo
            .echo_blob_pair(&BlobPair {
                data: Some(blob(1, "one", Some(p.clone()), vec![4, 5])),
                pt: Some(p),
            })
            .map_err(err)?;
        let s = is_blob(&got.data, 1, "one", &[4, 5])?;
        check(same(&s.pt, &got.pt), || "not one instance".into())?;
        is_point(&got.pt, 5, 6)
    });
    add(&mut cases, "echo_blob_pair[distinct]", move || {
        let got = echo
            .echo_blob_pair(&BlobPair {
                data: Some(blob(1, "one", Some(point(5, 6)), vec![])),
                pt: Some(point(5, 6)),
            })
            .map_err(err)?;
        let s = is_blob(&got.data, 1, "one", &[])?;
        check(!same(&s.pt, &got.pt), || "one instance".into())
    });
    add(&mut cases, "echo_blobs", move || {
        let b = blob(2, "two", None, (0..100).collect());
        let c = blob(9, "nine", Some(point(9, 9)), vec![]);
        let got = echo
            .echo_blobs(&[Some(b.clone()), Some(c), Some(b), None])
            .map_err(err)?;
        check(got.len() == 4, || format!("got {} elements", got.len()))?;
        is_blob(&got[0], 2, "two", &(0..100).collect::<Vec<_>>())?;
        is_blob(&got[1], 9, "nine", &[])?;
        let (a, b, c) = (got[0].as_ref(), got[2].as_ref(), got[1].as_ref());
        check(matches!((a, b), (Some(a), Some(b)) if a.ptr_eq(b)), || {
            "sharing lost".into()
        })?;
        check(!matches!((a, c), (Some(a), Some(c)) if a.ptr_eq(c)), || {
            "one instance".into()
        })?;
        check(got[3].is_none(), || "not null".into())
    });
    cases
}
