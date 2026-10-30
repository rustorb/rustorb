use super::*;
use crate::SystemExceptionKind as K;

fn long() -> TypeCode {
    TypeCode::simple(TCKind::Long).unwrap()
}

fn f() -> DynAnyFactory {
    DynAnyFactory
}

fn is_sys(e: DynAnyError, kind: K) -> bool {
    matches!(e, DynAnyError::System(s) if s.is(kind))
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

fn node_value(name: &str, children: Vec<Value>) -> Value {
    Value::List(vec![Value::String(name.into()), Value::List(children)])
}

/// `valuetype V { public long n; public V next; };`
fn v_tc() -> TypeCode {
    let rec = TypeCode::recursive("IDL:V:1.0");
    TypeCode::value_tc(
        TCKind::Value,
        "IDL:V:1.0",
        "V",
        0,
        None,
        &[("n", long(), 1), ("next", rec, 1)],
    )
    .unwrap()
}

/// `valuetype W : V { public short extra; };`
fn w_tc() -> TypeCode {
    TypeCode::value_tc(
        TCKind::Value,
        "IDL:W:1.0",
        "W",
        0,
        Some(&v_tc()),
        &[("extra", TypeCode::simple(TCKind::Short).unwrap(), 1)],
    )
    .unwrap()
}

#[test]
fn round_trip_recursive_struct() {
    let tc = node();
    let value = node_value(
        "root",
        vec![
            node_value("a", vec![]),
            node_value("b", vec![node_value("c", vec![])]),
        ],
    );
    let any = Any::new(tc.clone(), value);
    let d = f().create_dyn_any(&any).unwrap();
    assert_eq!(d.kind(), TCKind::Struct);
    assert_eq!(d.component_count().unwrap(), 2);
    assert_eq!(d.to_any().unwrap(), any);
    // Through the components: the name of the second child.
    d.seek(1).unwrap();
    let children = d.current_component().unwrap().unwrap();
    assert_eq!(children.get_length().unwrap(), 2);
    children.seek(1).unwrap();
    let b = children.current_component().unwrap().unwrap();
    assert_eq!(b.get_string().unwrap(), "b");
    b.insert_string("B").unwrap();
    let Value::List(items) = d.to_any().unwrap().into_parts().1 else {
        panic!()
    };
    assert_eq!(
        items[1],
        Value::List(vec![
            node_value("a", vec![]),
            node_value("B", vec![node_value("c", vec![])])
        ])
    );
}

#[test]
fn build_from_typecode() {
    let d = f().create_dyn_any_from_type_code(&node()).unwrap();
    d.insert_string("root").unwrap();
    d.next().unwrap();
    let children = d.current_component().unwrap().unwrap();
    children.set_length(1).unwrap();
    let child = children.current_component().unwrap().unwrap();
    child.insert_string("leaf").unwrap();
    assert_eq!(
        d.to_any().unwrap(),
        Any::new(node(), node_value("root", vec![node_value("leaf", vec![])]))
    );
    assert_eq!(
        f().create_dyn_any_from_type_code(&TypeCode::recursive("IDL:X:1.0"))
            .unwrap_err(),
        DynAnyError::InconsistentTypeCode
    );
}

#[test]
fn octets_and_chars() {
    let octets = TypeCode::sequence(&TypeCode::simple(TCKind::Octet).unwrap(), 0).unwrap();
    let d = f()
        .create_dyn_any(&Any::new(
            octets.clone(),
            Value::Octets(vec![1, 2, 255].into()),
        ))
        .unwrap();
    assert_eq!(d.component_count().unwrap(), 3);
    assert_eq!(d.get_octet().unwrap(), 1);
    assert_eq!(d.get_octet_seq().unwrap(), vec![1, 2, 255]);
    d.insert_octet_seq(&[9]).unwrap();
    assert!(matches!(d.to_any().unwrap().value(), Value::Octets(b) if b.as_slice() == [9]));
    let chars = TypeCode::array(&TypeCode::simple(TCKind::Char).unwrap(), 2).unwrap();
    let d = f().create_dyn_any_from_type_code(&chars).unwrap();
    d.insert_char_seq(&['a', 'b']).unwrap();
    assert!(matches!(d.to_any().unwrap().value(), Value::Chars(s) if s == "ab"));
    assert_eq!(d.insert_char_seq(&['a']), Err(DynAnyError::InvalidValue));
}

#[test]
fn union_from_value() {
    let tc = TypeCode::union_tc(
        "IDL:U:1.0",
        "U",
        &long(),
        &[(Some(1), "x", long()), (None, "s", TypeCode::string(0))],
    )
    .unwrap();
    let value = Value::Union {
        disc: Box::new(Value::Long(7)),
        member: Some((1, Box::new(Value::String("seven".into())))),
    };
    let d = f()
        .create_dyn_any(&Any::new(tc.clone(), value.clone()))
        .unwrap();
    assert_eq!(d.member_name().unwrap(), "s");
    assert_eq!(d.member().unwrap().get_string().unwrap(), "seven");
    assert_eq!(d.to_any().unwrap().value(), &value);
    // A value that does not fit the type.
    let bad = Value::Union {
        disc: Box::new(Value::Long(1)),
        member: Some((1, Box::new(Value::String("x".into())))),
    };
    assert_eq!(
        d.from_any(&Any::new(tc.clone(), bad.clone())),
        Err(DynAnyError::InvalidValue)
    );
    assert!(is_sys(
        f().create_dyn_any(&Any::new(tc, bad)).unwrap_err(),
        K::BAD_PARAM
    ));
}

#[test]
fn shared_and_cyclic_valuetypes() {
    let tc = v_tc();
    let a = ValueRef::new(tc.clone(), vec![Value::Long(1), Value::Valuetype(None)]);
    // a.next = a
    a.set_members(vec![Value::Long(1), Value::Valuetype(Some(a.clone()))]);
    let d = f()
        .create_dyn_any(&Any::new(tc.clone(), Value::Valuetype(Some(a.clone()))))
        .unwrap();
    assert_eq!(d.component_count().unwrap(), 2);
    d.seek(1).unwrap();
    let next = d.current_component().unwrap().unwrap();
    assert!(next.ptr_eq(&d));
    assert_eq!(d.current_member_name().unwrap(), "next");
    d.rewind().unwrap();
    d.insert_long(5).unwrap();
    let out = d.to_any().unwrap();
    let Value::Valuetype(Some(r)) = out.value() else {
        panic!()
    };
    let members = r.members();
    assert_eq!(members[0], Value::Long(5));
    let Value::Valuetype(Some(next)) = &members[1] else {
        panic!()
    };
    assert!(next.ptr_eq(r));
    Value::Valuetype(Some(r.clone())).break_cycles();
    d.destroy().unwrap();
    assert!(is_sys(
        d.component_count().unwrap_err(),
        K::OBJECT_NOT_EXIST
    ));
    Value::Valuetype(Some(a)).break_cycles();
}

#[test]
fn truncation() {
    let w = ValueRef::new(
        w_tc(),
        vec![Value::Long(3), Value::Valuetype(None), Value::Short(9)],
    );
    let any = Any::new(v_tc(), Value::Valuetype(Some(w.clone())));
    let d = f().create_dyn_any(&any).unwrap();
    assert_eq!(d.component_count().unwrap(), 2);
    let Value::Valuetype(Some(r)) = d.to_any().unwrap().into_parts().1 else {
        panic!()
    };
    assert_eq!(r.id(), "IDL:V:1.0");
    assert_eq!(
        f().create_dyn_any_without_truncation(&any).unwrap_err(),
        DynAnyError::MustTruncate
    );
    assert_eq!(
        f().create_multiple_dyn_anys(std::slice::from_ref(&any), false)
            .unwrap_err(),
        DynAnyError::MustTruncate
    );
    assert_eq!(f().create_multiple_dyn_anys(&[any], true).unwrap().len(), 1);
    // The same type is not truncated.
    let v = ValueRef::new(v_tc(), vec![Value::Long(1), Value::Valuetype(None)]);
    let any = Any::new(v_tc(), Value::Valuetype(Some(v)));
    assert!(f().create_dyn_any_without_truncation(&any).is_ok());
}

#[test]
fn insert_val() {
    let holder = TypeCode::struct_tc("IDL:H:1.0", "H", &[("v", v_tc())]).unwrap();
    let d = f().create_dyn_any_from_type_code(&holder).unwrap();
    assert!(d.get_val().unwrap().is_none());
    let w = ValueRef::new(
        w_tc(),
        vec![Value::Long(3), Value::Valuetype(None), Value::Short(9)],
    );
    d.insert_val(Some(w)).unwrap();
    let c = d.current_component().unwrap().unwrap();
    assert!(!c.is_null().unwrap());
    assert_eq!(c.component_count().unwrap(), 2);
    let other = TypeCode::value_tc::<&str>(TCKind::Value, "IDL:O:1.0", "O", 0, None, &[]).unwrap();
    assert_eq!(
        d.insert_val(Some(ValueRef::new(other, vec![]))),
        Err(DynAnyError::TypeMismatch)
    );
    c.set_to_null().unwrap();
    assert!(d.get_val().unwrap().is_none());
}

#[test]
fn narrowing_and_kinds() {
    let d = f().create_dyn_any_from_type_code(&long()).unwrap();
    assert!(is_sys(d.get_members().unwrap_err(), K::BAD_OPERATION));
    assert!(is_sys(d.set_length(1).unwrap_err(), K::BAD_OPERATION));
    assert!(is_sys(
        d.get_enum_as_string().unwrap_err(),
        K::BAD_OPERATION
    ));
    assert_eq!(d.get_seq(TCKind::Long), Err(DynAnyError::TypeMismatch));
    assert!(is_sys(d.no_implement().unwrap_err(), K::NO_IMPLEMENT));
    let ld = TypeCode::simple(TCKind::LongDouble).unwrap();
    assert_eq!(
        f().create_dyn_any_from_type_code(&ld).unwrap_err(),
        DynAnyError::InconsistentTypeCode
    );
}

#[test]
fn nested_any() {
    let any_tc = TypeCode::simple(TCKind::Any).unwrap();
    let d = f().create_dyn_any_from_type_code(&any_tc).unwrap();
    let inner = f().create_dyn_any_from_type_code(&node()).unwrap();
    inner.insert_string("n").unwrap();
    d.insert_dyn_any(&inner).unwrap();
    let got = d.get_dyn_any().unwrap();
    assert!(got.equal(&inner).unwrap());
    assert!(!got.ptr_eq(&inner));
    assert_eq!(d.get_any().unwrap(), inner.to_any().unwrap());
}

#[test]
fn equal_and_assign() {
    let tc = node();
    let a = f().create_dyn_any_from_type_code(&tc).unwrap();
    let b = f().create_dyn_any_from_type_code(&tc).unwrap();
    assert!(a.equal(&b).unwrap());
    b.insert_string("x").unwrap();
    assert!(!a.equal(&b).unwrap());
    a.assign(&b).unwrap();
    assert!(a.equal(&b).unwrap());
    // An alias is equivalent to the type it stands for.
    let alias = TypeCode::alias("IDL:N:1.0", "N", &tc).unwrap();
    let c = f().create_dyn_any_from_type_code(&alias).unwrap();
    c.assign(&b).unwrap();
    assert!(c.equal(&b).unwrap());
    assert_eq!(c.type_code().unwrap().kind(), TCKind::Alias);
    assert_eq!(c.kind(), TCKind::Struct);
    let anys = f().create_multiple_anys(&[a, c]).unwrap();
    assert_eq!(anys[1].typecode().kind(), TCKind::Alias);
}

#[test]
fn union_member_with_several_labels() {
    let tc = TypeCode::union_tc(
        "IDL:U:1.0",
        "U",
        &long(),
        &[
            (Some(1), "x", long()),
            (Some(2), "y", TypeCode::string(0)),
            (Some(3), "y", TypeCode::string(0)),
        ],
    )
    .unwrap();
    let d = f().create_dyn_any_from_type_code(&tc).unwrap();
    let disc = d.get_discriminator().unwrap();
    disc.insert_long(2).unwrap();
    d.member().unwrap().insert_string("kept").unwrap();
    disc.insert_long(3).unwrap();
    assert_eq!(d.member().unwrap().get_string().unwrap(), "kept");
    assert_eq!(
        d.to_any().unwrap().value(),
        &Value::Union {
            disc: Box::new(Value::Long(3)),
            member: Some((2, Box::new(Value::String("kept".into())))),
        }
    );
    disc.insert_long(1).unwrap();
    assert_eq!(d.member().unwrap().get_long().unwrap(), 0);
}

#[test]
fn fixed_values() {
    let tc = TypeCode::fixed(5, 2).unwrap();
    let d = f()
        .create_dyn_any(&Any::new(
            tc,
            Value::Fixed {
                digits: vec![0, 1, 2, 3, 0],
                negative: true,
            },
        ))
        .unwrap();
    assert_eq!(d.get_fixed_value().unwrap(), "-12.3");
    assert!(d.set_fixed_value("+000.5").unwrap());
    assert_eq!(d.get_fixed_value().unwrap(), "0.5");
    assert!(d.set_fixed_value("-0.001").is_ok_and(|exact| !exact));
    assert_eq!(d.get_fixed_value().unwrap(), "0");
}
