//! Valuetypes written and read back: sharing, cycles, truncation, boxes,
//! custom values and codebase URLs.

use std::sync::Arc;

use rustorb_core::cdr::{ByteOrder, Decoder, Encoder, Encoding};
use rustorb_core::{
    Any, CompletionStatus, EndpointAddr, InStream, Orb, OrbOptions, OutStream, Servant,
    ServerReply, SystemException, SystemExceptionKind, TCKind, TypeCode, Value, ValueRef, minor,
};

fn long() -> TypeCode {
    TypeCode::simple(TCKind::Long).unwrap()
}

fn string() -> TypeCode {
    TypeCode::string(0)
}

/// `valuetype Base { public long a; };`
fn base_tc(id: &str, modifier: i16) -> TypeCode {
    TypeCode::value_tc(
        TCKind::Value,
        id,
        "Base",
        modifier,
        None,
        &[("a", long(), 1)],
    )
    .unwrap()
}

/// `valuetype Derived : truncatable Base { public string s; public Base other; };`
fn derived_tc(base: &TypeCode, id: &str, modifier: i16) -> TypeCode {
    TypeCode::value_tc(
        TCKind::Value,
        id,
        "Derived",
        modifier,
        Some(base),
        &[("s", string(), 1), ("other", base.clone(), 1)],
    )
    .unwrap()
}

/// `valuetype Node { public long n; public Node next; };`
fn node_tc() -> TypeCode {
    let id = "IDL:test/Node:1.0";
    TypeCode::value_tc(
        TCKind::Value,
        id,
        "Node",
        0,
        None,
        &[("n", long(), 1), ("next", TypeCode::recursive(id), 1)],
    )
    .unwrap()
}

fn roundtrip(order: ByteOrder, tc: &TypeCode, v: &Value) -> Result<Value, SystemException> {
    let mut enc = Encoder::new(order, Encoding::Giop1_2);
    let mut out = OutStream::standalone(&mut enc);
    // Something before, so that alignment and offsets are not trivial.
    out.write_octet(1);
    out.write_value(tc, v)?;
    out.write_u16(0xbeef);
    let bytes = enc.into_bytes();
    let mut input = InStream::standalone(Decoder::new(&bytes, order, Encoding::Giop1_2), None);
    assert_eq!(input.read_octet().unwrap(), 1);
    let back = input.read_value(tc)?;
    assert_eq!(input.read_u16().unwrap(), 0xbeef);
    assert!(input.is_empty());
    Ok(back)
}

fn get(v: &Value) -> ValueRef {
    match v {
        Value::Valuetype(Some(r)) => r.clone(),
        other => panic!("not a value: {other:?}"),
    }
}

#[test]
fn shared_values_stay_shared() {
    let base = base_tc("IDL:test/Shared/Base:1.0", 0);
    let pair = TypeCode::struct_tc(
        "IDL:test/Pair:1.0",
        "Pair",
        &[
            ("x", base.clone()),
            ("y", base.clone()),
            ("z", base.clone()),
        ],
    )
    .unwrap();
    let shared = ValueRef::new(base.clone(), vec![Value::Long(5)]);
    let v = Value::List(vec![
        Value::Valuetype(Some(shared.clone())),
        Value::Valuetype(Some(shared)),
        Value::Valuetype(None),
    ]);
    for order in [ByteOrder::Big, ByteOrder::Little] {
        let back = roundtrip(order, &pair, &v).unwrap();
        assert_eq!(back, v);
        let Value::List(items) = back else { panic!() };
        assert!(get(&items[0]).ptr_eq(&get(&items[1])));
        assert_eq!(get(&items[0]).members(), vec![Value::Long(5)]);
        assert_eq!(items[2], Value::Valuetype(None));
    }
}

#[test]
fn cycles_come_back_as_cycles() {
    let tc = node_tc();
    let a = ValueRef::new(tc.clone(), Vec::new());
    let b = ValueRef::new(
        tc.clone(),
        vec![Value::Long(2), Value::Valuetype(Some(a.clone()))],
    );
    a.set_members(vec![Value::Long(1), Value::Valuetype(Some(b))]);
    let v = Value::Valuetype(Some(a));
    let back = roundtrip(ByteOrder::Little, &tc, &v).unwrap();
    assert_eq!(back, v);
    let a2 = get(&back);
    let b2 = get(&a2.members()[1]);
    assert_eq!(b2.members()[0], Value::Long(2));
    assert!(get(&b2.members()[1]).ptr_eq(&a2));
    back.break_cycles();
    v.break_cycles();
}

/// Counts the drops of the instances it is set on, as their native.
struct Tracker(Arc<std::sync::atomic::AtomicUsize>);

impl Drop for Tracker {
    fn drop(&mut self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// A ring of two nodes, tracked by `drops`.
fn ring(drops: &Arc<std::sync::atomic::AtomicUsize>) -> (ValueRef, ValueRef) {
    let tc = node_tc();
    let a = ValueRef::new(tc.clone(), Vec::new());
    let b = ValueRef::new(tc, vec![Value::Long(2), Value::Valuetype(Some(a.clone()))]);
    a.set_members(vec![Value::Long(1), Value::Valuetype(Some(b.clone()))]);
    for r in [&a, &b] {
        r.set_native(Some(Arc::new(Tracker(drops.clone()))));
    }
    (a, b)
}

#[test]
fn release_frees_cycles_nothing_else_refers_to() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let drops = Arc::new(AtomicUsize::new(0));

    // Only in the value (twice, and inside an any): freed.
    let (a, b) = ring(&drops);
    let any = Any::new(node_tc(), Value::Valuetype(Some(b)));
    Value::List(vec![
        Value::Valuetype(Some(a.clone())),
        Value::Valuetype(Some(a)),
        Value::Any(Box::new(any)),
    ])
    .release();
    assert_eq!(drops.load(Ordering::SeqCst), 2);

    // An instance referred to from elsewhere keeps what it reaches.
    drops.store(0, Ordering::SeqCst);
    let (a, b) = ring(&drops);
    Value::Valuetype(Some(a)).release();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let a = get(&b.members()[1]);
    assert_eq!(a.members()[0], Value::Long(1));
    assert!(get(&a.members()[1]).ptr_eq(&b));
    drop(a);
    Value::Valuetype(Some(b)).release();
    assert_eq!(drops.load(Ordering::SeqCst), 2);

    // A value held from elsewhere, referring into a ring: the ring stays.
    drops.store(0, Ordering::SeqCst);
    let (a, b) = ring(&drops);
    let holder = ValueRef::new(node_tc(), vec![Value::Long(0), Value::Valuetype(Some(a))]);
    Value::List(vec![
        Value::Valuetype(Some(holder.clone())),
        Value::Valuetype(Some(b)),
    ])
    .release();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    Value::Valuetype(Some(holder)).release();
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

#[test]
fn truncatable_values_are_read_as_a_known_base() {
    let base = base_tc("IDL:test/Trunc/Base:1.0", 0);
    let derived = derived_tc(&base, "IDL:test/Trunc/Derived:1.0", 3);
    let inner = ValueRef::new(base.clone(), vec![Value::Long(7)]);
    let v = Value::Valuetype(Some(ValueRef::new(
        derived.clone(),
        vec![
            Value::Long(1),
            Value::String("derived state".into()),
            Value::Valuetype(Some(inner)),
        ],
    )));
    // The derived type is not registered: the value is truncated.
    let back = roundtrip(ByteOrder::Big, &base, &v).unwrap();
    let r = get(&back);
    assert_eq!(r.id(), "IDL:test/Trunc/Base:1.0");
    assert_eq!(r.members(), vec![Value::Long(1)]);

    // Registered, it is read whole.
    rustorb_core::register_value_type(&derived);
    let back = roundtrip(ByteOrder::Big, &base, &v).unwrap();
    rustorb_core::unregister_value_type("IDL:test/Trunc/Derived:1.0");
    assert_eq!(back, v);
    assert_eq!(get(&back).id(), "IDL:test/Trunc/Derived:1.0");
}

#[test]
fn unknown_types_that_cannot_be_truncated_are_marshal() {
    let base = base_tc("IDL:test/Plain/Base:1.0", 0);
    let derived = derived_tc(&base, "IDL:test/Plain/Derived:1.0", 0);
    let v = Value::Valuetype(Some(ValueRef::new(
        derived,
        vec![
            Value::Long(1),
            Value::String("x".into()),
            Value::Valuetype(None),
        ],
    )));
    let e = roundtrip(ByteOrder::Big, &base, &v).unwrap_err();
    assert_eq!(e.kind(), Some(SystemExceptionKind::MARSHAL));
}

/// `custom valuetype Custom { public long a; };`, marshaled as a long and
/// a string.
fn custom_tc() -> TypeCode {
    TypeCode::value_tc(
        TCKind::Value,
        "IDL:test/Custom:1.0",
        "Custom",
        1,
        None,
        &[("a", long(), 1)],
    )
    .unwrap()
}

#[test]
fn custom_values_are_chunked_and_read_by_their_reader() {
    let tc = custom_tc();
    let items = vec![
        Value::Any(Box::new(Any::new(long(), Value::Long(7)))),
        Value::Any(Box::new(Any::new(string(), Value::String("seven".into())))),
    ];
    let v = Value::Valuetype(Some(ValueRef::new(tc.clone(), items)));

    // Chunked, with a single repository id.
    let mut enc = Encoder::new(ByteOrder::Big, Encoding::Giop1_2);
    OutStream::standalone(&mut enc)
        .write_value(&tc, &v)
        .unwrap();
    let bytes = enc.into_bytes();
    assert_eq!(bytes[..4], 0x7fff_ff0a_u32.to_be_bytes());

    // Without a reader for the type, its state cannot be read.
    let e = roundtrip(ByteOrder::Big, &tc, &v).unwrap_err();
    assert_eq!(e.kind(), Some(SystemExceptionKind::MARSHAL));

    rustorb_core::register_custom_reader(
        "IDL:test/Custom:1.0",
        Arc::new(|v: &ValueRef, input: &mut InStream<'_>| {
            let a = input.read::<i32>()?;
            let s = input.read_string()?;
            v.set_members(vec![Value::Long(a), Value::String(s)]);
            Ok(())
        }),
    );
    let back = roundtrip(ByteOrder::Little, &tc, &v);
    rustorb_core::unregister_custom_reader("IDL:test/Custom:1.0");
    let back = get(&back.unwrap());
    assert_eq!(
        back.members(),
        vec![Value::Long(7), Value::String("seven".into())]
    );
}

#[test]
fn codebase_urls_are_skipped() {
    // Two values with a codebase URL (the second an indirection to the
    // first's), as a struct of two Base.
    let base = base_tc("IDL:test/Codebase/Base:1.0", 0);
    let pair = TypeCode::struct_tc(
        "IDL:test/CodebasePair:1.0",
        "CodebasePair",
        &[("x", base.clone()), ("y", base.clone())],
    )
    .unwrap();
    let mut enc = Encoder::new(ByteOrder::Big, Encoding::Giop1_2);
    enc.write_u32(0x7fff_ff03); // a codebase URL and a single repository id
    let url_at = enc.position();
    enc.write_string("http://example.com/classes/").unwrap();
    enc.write_string("IDL:test/Codebase/Base:1.0").unwrap();
    enc.write_i32(1);
    enc.write_u32(0x7fff_ff03);
    enc.write_u32(0xffff_ffff);
    let offset = url_at as i64 - enc.position() as i64;
    enc.write_i32(offset as i32);
    enc.write_string("IDL:test/Codebase/Base:1.0").unwrap();
    enc.write_i32(2);
    let bytes = enc.into_bytes();
    let mut input = InStream::standalone(
        Decoder::new(&bytes, ByteOrder::Big, Encoding::Giop1_2),
        None,
    );
    let back = input.read_value(&pair).unwrap();
    assert!(input.is_empty());
    let Value::List(items) = back else { panic!() };
    assert_eq!(get(&items[0]).members(), vec![Value::Long(1)]);
    assert_eq!(get(&items[1]).members(), vec![Value::Long(2)]);
}

#[test]
fn abstract_interfaces_are_a_reference_or_a_value() {
    let named =
        TypeCode::interface(TCKind::AbstractInterface, "IDL:test/Named:1.0", "Named").unwrap();
    let base = base_tc("IDL:test/Abstract/Base:1.0", 0);
    rustorb_core::register_value_type(&base);
    let shared = ValueRef::new(base.clone(), vec![Value::Long(3)]);
    let seq = TypeCode::sequence(&named, 0).unwrap();
    let v = Value::List(vec![
        Value::Valuetype(Some(shared.clone())),
        Value::Objref(None),
        Value::Valuetype(None),
        Value::Valuetype(Some(shared)),
    ]);
    let back = roundtrip(ByteOrder::Big, &seq, &v);
    rustorb_core::unregister_value_type("IDL:test/Abstract/Base:1.0");
    let Value::List(items) = back.unwrap() else {
        panic!()
    };
    assert_eq!(get(&items[0]).members(), vec![Value::Long(3)]);
    assert!(get(&items[0]).ptr_eq(&get(&items[3])));
    assert!(matches!(items[1], Value::Objref(None)));
    assert!(matches!(items[2], Value::Valuetype(None)));

    // The discriminator, then the reference or the value.
    let mut enc = Encoder::new(ByteOrder::Big, Encoding::Giop1_2);
    OutStream::standalone(&mut enc)
        .write_value(&named, &Value::Valuetype(None))
        .unwrap();
    assert_eq!(enc.into_bytes(), [0, 0, 0, 0, 0, 0, 0, 0]);
    let e = roundtrip(ByteOrder::Big, &named, &Value::Long(1)).unwrap_err();
    assert_eq!(e.kind(), Some(SystemExceptionKind::BAD_PARAM));
}

#[test]
fn values_that_do_not_fit_are_bad_param() {
    let base = base_tc("IDL:test/Fit/Base:1.0", 0);
    let other = base_tc("IDL:test/Fit/Other:1.0", 0);
    let wrong_type = Value::Valuetype(Some(ValueRef::new(other, vec![Value::Long(1)])));
    let wrong_state = Value::Valuetype(Some(ValueRef::new(base.clone(), vec![])));
    for v in [wrong_type, wrong_state, Value::Long(1)] {
        let e = roundtrip(ByteOrder::Big, &base, &v).unwrap_err();
        assert_eq!(e.kind(), Some(SystemExceptionKind::BAD_PARAM));
    }
    // The state of a custom value is the anys its marshal wrote.
    let custom = base_tc("IDL:test/Fit/Custom:1.0", 1);
    let v = Value::Valuetype(Some(ValueRef::new(custom.clone(), vec![Value::Long(1)])));
    let e = roundtrip(ByteOrder::Big, &custom, &v).unwrap_err();
    assert_eq!(e.kind(), Some(SystemExceptionKind::BAD_PARAM));
}

#[test]
fn value_boxes() {
    let boxed = TypeCode::value_box("IDL:test/LongBox:1.0", "LongBox", &long()).unwrap();
    let seq = TypeCode::sequence(&boxed, 0).unwrap();
    let one = ValueRef::new(boxed.clone(), vec![Value::Long(42)]);
    let v = Value::List(vec![
        Value::Valuetype(Some(one.clone())),
        Value::Valuetype(None),
        Value::Valuetype(Some(one)),
    ]);
    let back = roundtrip(ByteOrder::Little, &seq, &v).unwrap();
    assert_eq!(back, v);
}

const ID: &str = "IDL:test/ValueEcho:1.0";

struct ValueEcho;

impl Servant for ValueEcho {
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
                let b = input.read_any()?;
                let mut out = reply.output();
                out.write_any(&a)?;
                out.write_any(&b)
            }
            _ => Err(SystemException::new(
                SystemExceptionKind::BAD_OPERATION,
                minor::UNKNOWN_OPERATION,
                CompletionStatus::No,
            )),
        }
    }
}

/// Two arguments are one message: a value in both is sent once.
#[test]
fn values_in_anys_between_orbs() {
    let server = Orb::with_options(OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..OrbOptions::default()
    })
    .unwrap();
    let root = server.root_poa().unwrap();
    let id = root.activate_object(Arc::new(ValueEcho)).unwrap();
    let local = root.id_to_reference(&id).unwrap();
    root.the_poa_manager().activate().unwrap();
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let ior = local.ior().to_ior_string(ByteOrder::Big).unwrap();
    let obj = client.string_to_object(&ior).unwrap().unwrap();

    let tc = node_tc();
    let a = ValueRef::new(tc.clone(), Vec::new());
    a.set_members(vec![Value::Long(1), Value::Valuetype(Some(a.clone()))]);
    let any = Any::new(tc.clone(), Value::Valuetype(Some(a.clone())));
    let arg = any.clone();
    let reply = obj
        .request("echo")
        .args(move |out| {
            out.write_any(&arg)?;
            out.write_any(&arg)
        })
        .invoke()
        .unwrap();
    let mut input = reply.input();
    let (x, y) = (input.read_any().unwrap(), input.read_any().unwrap());
    assert_eq!(x, any);
    let (x, y) = (get(x.value()), get(y.value()));
    assert!(x.ptr_eq(&y));
    assert!(get(&x.members()[1]).ptr_eq(&x));
    Value::Valuetype(Some(x)).break_cycles();
    Value::Valuetype(Some(a)).break_cycles();
}
