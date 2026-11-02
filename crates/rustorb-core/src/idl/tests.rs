use std::any::{Any as StdAny, TypeId};
use std::sync::Arc;

use rustorb_cdr::{ByteOrder, Decoder, Encoder, Encoding};

use super::*;
use crate::{TCKind, Value};

fn round_trip<T: IdlType>(v: &T::Borrowed) -> Sys<T::Rust> {
    let mut enc = Encoder::new(ByteOrder::Little, Encoding::Giop1_2);
    T::write(v, &mut OutStream::standalone(&mut enc))?;
    let bytes = enc.into_bytes();
    let mut input = InStream::standalone(
        Decoder::new(&bytes, ByteOrder::Little, Encoding::Giop1_2),
        None,
    );
    let r = T::read(&mut input)?;
    assert!(input.is_empty());
    Ok(r)
}

fn kind(e: SystemException) -> Option<SystemExceptionKind> {
    e.kind()
}

#[test]
fn basic_and_anonymous_types() {
    assert_eq!(round_trip::<Long>(&-5).unwrap(), -5);
    assert_eq!(round_trip::<Octet>(&7).unwrap(), 7);
    assert_eq!(round_trip::<Char>(&'A').unwrap(), 'A');
    assert_eq!(round_trip::<WChar>(&'あ').unwrap(), 'あ');
    assert_eq!(round_trip::<String>("hé").unwrap(), "hé");
    assert_eq!(round_trip::<WString>("wide ∀").unwrap(), "wide ∀");
    assert_eq!(
        round_trip::<Sequence<Octet>>(&[1, 2, 3]).unwrap(),
        vec![1, 2, 3]
    );
    assert_eq!(
        round_trip::<Sequence<String>>(&["a".to_owned(), "b".to_owned()]).unwrap(),
        vec!["a", "b"]
    );
    assert_eq!(
        round_trip::<Array<Array<Double, 3>, 2>>(&[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]).unwrap(),
        [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]
    );
    let f: crate::Fixed = "-12.3".parse().unwrap();
    assert_eq!(round_trip::<Fixed<5, 2>>(&f).unwrap().to_string(), "-12.30");
}

#[test]
fn bounds_and_conversions() {
    assert_eq!(round_trip::<BoundedString<3>>("abc").unwrap(), "abc");
    assert_eq!(
        kind(round_trip::<BoundedString<3>>("abcd").unwrap_err()),
        Some(SystemExceptionKind::BAD_PARAM)
    );
    assert_eq!(
        kind(round_trip::<BoundedSequence<Long, 1>>(&[1, 2]).unwrap_err()),
        Some(SystemExceptionKind::BAD_PARAM)
    );
    assert_eq!(
        kind(round_trip::<Char>(&'é').unwrap_err()),
        Some(SystemExceptionKind::DATA_CONVERSION)
    );
    assert_eq!(
        kind(round_trip::<WChar>(&'😀').unwrap_err()),
        Some(SystemExceptionKind::DATA_CONVERSION)
    );
    let big: crate::Fixed = "1234".parse().unwrap();
    assert_eq!(
        kind(round_trip::<Fixed<5, 2>>(&big).unwrap_err()),
        Some(SystemExceptionKind::DATA_CONVERSION)
    );

    // A sequence longer than its bound is MARSHAL when read.
    let mut enc = Encoder::new(ByteOrder::Big, Encoding::Giop1_2);
    Sequence::<Long>::write(&[1, 2], &mut OutStream::standalone(&mut enc)).unwrap();
    let bytes = enc.into_bytes();
    let mut input = InStream::standalone(
        Decoder::new(&bytes, ByteOrder::Big, Encoding::Giop1_2),
        None,
    );
    assert_eq!(
        kind(BoundedSequence::<Long, 1>::read(&mut input).unwrap_err()),
        Some(SystemExceptionKind::MARSHAL)
    );
}

#[test]
fn anys() {
    let a = crate::Any::insert::<WString>("w").unwrap();
    assert_eq!(a.typecode().kind(), TCKind::WString);
    assert_eq!(a.value(), &Value::String("w".into()));
    assert_eq!(a.extract::<WString>().as_deref(), Some("w"));
    assert_eq!(a.extract::<String>(), None);

    let a = crate::Any::insert::<Sequence<Long>>(&[1, 2]).unwrap();
    assert_eq!(a.extract::<Sequence<Long>>(), Some(vec![1, 2]));
    let a = crate::Any::insert::<WChar>(&'x').unwrap();
    assert_eq!(a.value(), &Value::WChar(u16::from(b'x')));
    assert_eq!(a.extract::<WChar>(), Some('x'));
}

// A valuetype by hand, as the generator writes them: `valuetype Node {
// public long id; public Node next; }`.

#[derive(Default)]
struct NodeState {
    id: i32,
    next: Option<ValueHandle>,
}

static NODE: ValueInfo = ValueInfo {
    id: "IDL:Test/Node:1.0",
    sent_ids: &["IDL:Test/Node:1.0"],
    ancestors: &["IDL:Test/Node:1.0"],
    custom: false,
    make: || ValueHandle::new(NodeState::default()),
    type_code: || {
        tc(TypeCode::value_tc(
            TCKind::Value,
            "IDL:Test/Node:1.0",
            "Node",
            0,
            None,
            &[
                ("id", simple_tc(TCKind::Long), 1),
                ("next", TypeCode::recursive("IDL:Test/Node:1.0"), 1),
            ],
        ))
    },
};

impl ValueState for NodeState {
    fn info(&self) -> &'static ValueInfo {
        &NODE
    }

    fn write_state(&self, out: &mut OutStream<'_>) -> Sys<()> {
        Long::write(&self.id, out)?;
        ValueHandle::write(self.next.as_ref(), out)
    }

    fn read_state(&mut self, input: &mut InStream<'_>) -> Sys<()> {
        self.id = Long::read(input)?;
        self.next = ValueHandle::read(input, &NODE)?;
        Ok(())
    }

    fn view(&self, ty: TypeId) -> Option<&dyn StdAny> {
        (ty == TypeId::of::<NodeState>()).then_some(self as &dyn StdAny)
    }

    fn view_mut(&mut self, ty: TypeId) -> Option<&mut dyn StdAny> {
        (ty == TypeId::of::<NodeState>()).then_some(self as &mut dyn StdAny)
    }
}

#[derive(Debug)]
struct NodeType;

impl IdlType for NodeType {
    type Rust = Option<ValueHandle>;
    type Borrowed = Option<ValueHandle>;
    const MIN_SIZE: usize = 4;

    fn type_code() -> TypeCode {
        (NODE.type_code)()
    }

    fn write(v: &Option<ValueHandle>, out: &mut OutStream<'_>) -> Sys<()> {
        ValueHandle::write(v.as_ref(), out)
    }

    fn read(input: &mut InStream<'_>) -> Sys<Option<ValueHandle>> {
        ValueHandle::read(input, &NODE)
    }
}

#[test]
fn values_keep_sharing_and_cycles() {
    let a = ValueHandle::new(NodeState { id: 1, next: None });
    let b = ValueHandle::new(NodeState {
        id: 2,
        next: Some(a.clone()),
    });
    a.with_mut(|s: &mut NodeState| s.next = Some(b.clone()));

    let back = round_trip::<Sequence<NodeType>>(&[Some(a.clone()), Some(b.clone()), None]).unwrap();
    let (a2, b2) = (back[0].clone().unwrap(), back[1].clone().unwrap());
    assert!(back[2].is_none());
    assert_eq!(a2.with(|s: &NodeState| s.id), 1);
    let next = a2.with(|s: &NodeState| s.next.clone()).unwrap();
    assert!(next.ptr_eq(&b2));
    let next = b2.with(|s: &NodeState| s.next.clone()).unwrap();
    assert!(next.ptr_eq(&a2));

    // Through an any and back.
    let any = crate::Any::insert::<NodeType>(&Some(a.clone())).unwrap();
    let a3 = any.extract::<NodeType>().unwrap().unwrap();
    let b3 = a3.with(|s: &NodeState| s.next.clone()).unwrap();
    let a4 = b3.with(|s: &NodeState| s.next.clone()).unwrap();
    assert!(a4.ptr_eq(&a3));
    assert_eq!(b3.with(|s: &NodeState| s.id), 2);

    // Break the cycles.
    for v in [a, a2, a3] {
        v.with_mut(|s: &mut NodeState| s.next = None);
    }
}

#[test]
fn value_boxes() {
    #[derive(Debug)]
    struct LongBox;
    impl IdlType for LongBox {
        type Rust = Option<Arc<i32>>;
        type Borrowed = Option<Arc<i32>>;
        const MIN_SIZE: usize = 4;
        fn type_code() -> TypeCode {
            tc(TypeCode::value_box(
                "IDL:Test/LongBox:1.0",
                "LongBox",
                &simple_tc(TCKind::Long),
            ))
        }
        fn write(v: &Option<Arc<i32>>, out: &mut OutStream<'_>) -> Sys<()> {
            write_box::<Long>(v.as_ref(), "IDL:Test/LongBox:1.0", out)
        }
        fn read(input: &mut InStream<'_>) -> Sys<Option<Arc<i32>>> {
            read_box::<Long>(input, "IDL:Test/LongBox:1.0")
        }
    }
    let v = Arc::new(5);
    let back = round_trip::<Sequence<LongBox>>(&[Some(v.clone()), Some(v), None]).unwrap();
    assert_eq!(back[0].as_deref(), Some(&5));
    assert!(Arc::ptr_eq(
        back[0].as_ref().unwrap(),
        back[1].as_ref().unwrap()
    ));
    assert!(back[2].is_none());
}
