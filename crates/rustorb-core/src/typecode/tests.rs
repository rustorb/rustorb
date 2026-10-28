use rustorb_cdr::{ByteOrder, CdrVersion, Decoder, Encoder};

use super::*;

fn long() -> TypeCode {
    TypeCode::simple(TCKind::Long).unwrap()
}

fn encode(tc: &TypeCode, order: ByteOrder) -> Vec<u8> {
    let mut enc = Encoder::new(order, CdrVersion::Giop1_2);
    tc.encode(&mut enc).unwrap();
    enc.into_bytes()
}

fn decode(bytes: &[u8], order: ByteOrder) -> TypeCode {
    let mut dec = Decoder::new(bytes, order, CdrVersion::Giop1_2);
    let tc = TypeCode::decode(&mut dec).unwrap();
    assert!(dec.is_empty());
    tc
}

/// `struct Node { string name; sequence<Node> children; };`
fn node() -> TypeCode {
    let rec = TypeCode::recursive("IDL:Node:1.0");
    let seq = TypeCode::sequence(&rec, 0).unwrap();
    TypeCode::struct_tc(
        "IDL:Node:1.0",
        "Node",
        &[("name", TypeCode::string(0)), ("children", seq)],
    )
    .unwrap()
}

#[test]
fn simple_kinds() {
    assert_eq!(long().kind(), TCKind::Long);
    assert_eq!(
        TypeCode::simple(TCKind::Struct),
        Err(TypeCodeError::BadKind)
    );
    assert_eq!(long().id(), Err(TypeCodeError::BadKind));
    assert_eq!(encode(&long(), ByteOrder::Big), [0, 0, 0, 3]);
    assert!(decode(&[0, 0, 0, 3], ByteOrder::Big).equal(&long()));
}

#[test]
fn bounded_string_and_fixed() {
    assert_eq!(
        encode(&TypeCode::string(5), ByteOrder::Big),
        [0, 0, 0, 18, 0, 0, 0, 5]
    );
    let f = TypeCode::fixed(5, 2).unwrap();
    assert_eq!(encode(&f, ByteOrder::Big), [0, 0, 0, 28, 0, 5, 0, 2]);
    assert_eq!(f.fixed_digits(), Ok(5));
    assert!(TypeCode::fixed(0, 0).is_err());
    assert!(TypeCode::fixed(3, 4).is_err());
}

#[test]
fn struct_encoding() {
    let tc = TypeCode::struct_tc("IDL:P:1.0", "P", &[("x", long())]).unwrap();
    let bytes = encode(&tc, ByteOrder::Big);
    #[rustfmt::skip]
    let expected: Vec<u8> = vec![
        0, 0, 0, 15,            // tk_struct
        0, 0, 0, 44,            // encapsulation length
        0,                      // big endian
        0, 0, 0,                // padding
        0, 0, 0, 10, b'I', b'D', b'L', b':', b'P', b':', b'1', b'.', b'0', 0,
        0, 0,                   // padding
        0, 0, 0, 2, b'P', 0,
        0, 0,                   // padding
        0, 0, 0, 1,             // one member
        0, 0, 0, 2, b'x', 0,
        0, 0,                   // padding
        0, 0, 0, 3,             // tk_long
    ];
    assert_eq!(bytes, expected);
    let back = decode(&bytes, ByteOrder::Big);
    assert!(back.equal(&tc));
    assert_eq!(back.member_name(0), Ok("x"));
    assert_eq!(back.member_type(1).err(), Some(TypeCodeError::Bounds));
}

#[test]
fn recursive_struct_uses_an_indirection() {
    let tc = node();
    let seq = tc.member_type(1).unwrap();
    assert_eq!(seq.kind(), TCKind::Sequence);
    let back_ref = seq.content_type().unwrap();
    assert_eq!(back_ref.kind(), TCKind::Struct);
    assert_eq!(back_ref.node_key(), tc.node_key());

    for order in [ByteOrder::Big, ByteOrder::Little] {
        let bytes = encode(&tc, order);
        // The sequence's content is an indirection back to offset 0.
        let n = bytes.len();
        let tail = &bytes[n - 12..];
        let kind = match order {
            ByteOrder::Big => u32::from_be_bytes(tail[0..4].try_into().unwrap()),
            ByteOrder::Little => u32::from_le_bytes(tail[0..4].try_into().unwrap()),
        };
        assert_eq!(kind, 0xffff_ffff);
        let offset = match order {
            ByteOrder::Big => i32::from_be_bytes(tail[4..8].try_into().unwrap()),
            ByteOrder::Little => i32::from_le_bytes(tail[4..8].try_into().unwrap()),
        };
        assert_eq!(offset, -((n - 8) as i32));
        let back = decode(&bytes, order);
        assert!(back.equal(&tc));
        let again = back.member_type(1).unwrap().content_type().unwrap();
        assert_eq!(again.node_key(), back.node_key());
    }
}

#[test]
fn nested_in_an_offset_stream() {
    // An indirection must survive being written at an odd offset of a
    // larger stream.
    let tc = node();
    let mut enc = Encoder::new(ByteOrder::Little, CdrVersion::Giop1_2);
    enc.write_octet(7);
    tc.encode(&mut enc).unwrap();
    tc.encode(&mut enc).unwrap();
    let bytes = enc.into_bytes();
    let mut dec = Decoder::new(&bytes, ByteOrder::Little, CdrVersion::Giop1_2);
    dec.read_octet().unwrap();
    assert!(TypeCode::decode(&mut dec).unwrap().equal(&tc));
    assert!(TypeCode::decode(&mut dec).unwrap().equal(&tc));
}

#[test]
fn mutually_recursive() {
    // struct A { sequence<B> b; }; struct B { sequence<A> a; };
    let ra = TypeCode::recursive("IDL:A:1.0");
    let b = TypeCode::struct_tc(
        "IDL:B:1.0",
        "B",
        &[("a", TypeCode::sequence(&ra, 0).unwrap())],
    )
    .unwrap();
    let a = TypeCode::struct_tc(
        "IDL:A:1.0",
        "A",
        &[("b", TypeCode::sequence(&b, 0).unwrap())],
    )
    .unwrap();
    let inner_a = a
        .member_type(0)
        .unwrap()
        .content_type()
        .unwrap()
        .member_type(0)
        .unwrap()
        .content_type()
        .unwrap();
    assert_eq!(inner_a.node_key(), a.node_key());
    let back = decode(&encode(&a, ByteOrder::Big), ByteOrder::Big);
    assert!(back.equal(&a));
    // b on its own still has the placeholder and cannot be written.
    let mut enc = Encoder::new(ByteOrder::Big, CdrVersion::Giop1_2);
    assert!(b.encode(&mut enc).is_err());
}

#[test]
fn union_labels() {
    let color = TypeCode::enum_tc("IDL:Color:1.0", "Color", &["RED", "GREEN"]).unwrap();
    let u = TypeCode::union_tc(
        "IDL:U:1.0",
        "U",
        &long(),
        &[
            (Some(1), "a", long()),
            (Some(2), "b", TypeCode::string(0)),
            (Some(3), "b", TypeCode::string(0)),
            (None, "c", color.clone()),
        ],
    )
    .unwrap();
    assert_eq!(u.default_index(), Ok(3));
    assert_eq!(u.member_label_value(2), Ok(Some(3)));
    assert_eq!(u.member_label(3).unwrap().value(), &Value::Octet(0));
    assert_eq!(u.member_label(0).unwrap().value(), &Value::Long(1));
    assert_eq!(u.union_member_for(&Value::Long(3)), Ok(Some(2)));
    assert_eq!(u.union_member_for(&Value::Long(9)), Ok(Some(3)));
    for order in [ByteOrder::Big, ByteOrder::Little] {
        assert!(decode(&encode(&u, order), order).equal(&u));
    }
    // Duplicate labels, a second default, a bad discriminator.
    assert!(
        TypeCode::union_tc(
            "IDL:V:1.0",
            "V",
            &long(),
            &[(Some(1), "a", long()), (Some(1), "b", long())]
        )
        .is_err()
    );
    assert!(
        TypeCode::union_tc(
            "IDL:V:1.0",
            "V",
            &long(),
            &[(None, "a", long()), (None, "b", long())]
        )
        .is_err()
    );
    assert!(
        TypeCode::union_tc(
            "IDL:V:1.0",
            "V",
            &TypeCode::string(0),
            &[(Some(1), "a", long())]
        )
        .is_err()
    );
    // An enum discriminator.
    let e = TypeCode::union_tc("IDL:E:1.0", "E", &color, &[(Some(1), "g", long())]).unwrap();
    assert!(decode(&encode(&e, ByteOrder::Big), ByteOrder::Big).equal(&e));
    assert!(TypeCode::union_tc("IDL:E:1.0", "E", &color, &[(Some(2), "g", long())]).is_err());
}

#[test]
fn equal_and_equivalent() {
    let alias = TypeCode::alias("IDL:Count:1.0", "Count", &long()).unwrap();
    assert!(!alias.equal(&long()));
    assert!(alias.equivalent(&long()));
    let a = TypeCode::struct_tc("IDL:P:1.0", "P", &[("x", long())]).unwrap();
    let b = TypeCode::struct_tc("IDL:P:1.0", "Q", &[("y", alias.clone())]).unwrap();
    assert!(!a.equal(&b));
    assert!(a.equivalent(&b));
    let anon = TypeCode::struct_tc("", "", &[("", long())]).unwrap();
    assert!(anon.equivalent(&a));
    let c = TypeCode::struct_tc("IDL:R:1.0", "P", &[("x", long())]).unwrap();
    assert!(!c.equivalent(&a));
    assert!(node().equal(&node()));
    let compact = a.compact();
    assert_eq!(compact.name(), Ok(""));
    assert_eq!(compact.member_name(0), Ok(""));
    assert_eq!(compact.id(), Ok("IDL:P:1.0"));
    assert!(compact.equivalent(&a));
    assert!(!compact.equal(&a));
}

#[test]
fn bad_input() {
    for bytes in [
        &[0u8, 0, 0, 99][..],                              // unknown kind
        &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xf8], // indirection to nowhere
        &[0, 0, 0, 15, 0, 0, 0, 1, 0],                     // empty struct encapsulation
    ] {
        let mut dec = Decoder::new(bytes, ByteOrder::Big, CdrVersion::Giop1_2);
        assert!(TypeCode::decode(&mut dec).is_err(), "{bytes:?}");
    }
    // A sequence nested deeper than the limit.
    let mut tc = long();
    for _ in 0..200 {
        tc = TypeCode::sequence(&tc, 0).unwrap();
    }
    let bytes = encode(&tc, ByteOrder::Big);
    let mut dec = Decoder::new(&bytes, ByteOrder::Big, CdrVersion::Giop1_2);
    assert!(TypeCode::decode(&mut dec).is_err());
}

#[test]
fn constructor_checks() {
    assert!(TypeCode::struct_tc("IDL:S:1.0", "S", &[("a", long()), ("A", long())]).is_err());
    assert!(TypeCode::struct_tc("IDL:S:1.0", "S", &[("a", TypeCode::void())]).is_err());
    assert!(TypeCode::struct_tc("IDL:S:1.0", "1S", &[("a", long())]).is_err());
    assert!(TypeCode::array(&long(), 0).is_err());
    assert!(TypeCode::interface(TCKind::Struct, "IDL:I:1.0", "I").is_err());
}

#[test]
fn valuetype_round_trip() {
    let base =
        TypeCode::value_tc::<&str>(TCKind::Value, "IDL:Base:1.0", "Base", 0, None, &[]).unwrap();
    let v = TypeCode::value_tc(
        TCKind::Value,
        "IDL:V:1.0",
        "V",
        0,
        Some(&base),
        &[("x", long(), PUBLIC_MEMBER)],
    )
    .unwrap();
    let back = decode(&encode(&v, ByteOrder::Little), ByteOrder::Little);
    assert!(back.equal(&v));
    assert!(back.concrete_base_type().unwrap().unwrap().equal(&base));
    assert_eq!(back.member_visibility(0), Ok(PUBLIC_MEMBER));
}
