//! The DynAny probe of `impls/omniorb-cpp/dynany_probe.cc` with rustorb's
//! DynAny: the same cases, printed the same way, so that
//! `run-dynany-probe.sh` can compare both with their expected output (the
//! differences are in docs/dynany.md).

use rustorb_core::{Any, DynAny, DynAnyError, DynAnyFactory, TCKind, TypeCode, Value};

type R = Result<String, DynAnyError>;

fn case(name: &str, f: impl FnOnce() -> R) {
    let out = match f() {
        Ok(s) => s,
        Err(DynAnyError::TypeMismatch) => "TypeMismatch".into(),
        Err(DynAnyError::InvalidValue) => "InvalidValue".into(),
        Err(DynAnyError::InconsistentTypeCode) => "InconsistentTypeCode".into(),
        Err(DynAnyError::MustTruncate) => "MustTruncate".into(),
        Err(DynAnyError::System(e)) => e
            .kind()
            .map_or_else(|| e.repository_id.clone(), |k| k.name().to_owned()),
    };
    println!("{name}: {out}");
}

fn kind(k: TCKind) -> String {
    match k {
        TCKind::TypeCode => "tk_TypeCode".into(),
        TCKind::Principal => "tk_Principal".into(),
        TCKind::ValueBox => "tk_value_box".into(),
        TCKind::AbstractInterface => "tk_abstract_interface".into(),
        TCKind::LocalInterface => "tk_local_interface".into(),
        TCKind::LongDouble => "tk_longdouble".into(),
        TCKind::LongLong => "tk_longlong".into(),
        TCKind::ULongLong => "tk_ulonglong".into(),
        TCKind::WString => "tk_wstring".into(),
        TCKind::WChar => "tk_wchar".into(),
        TCKind::UShort => "tk_ushort".into(),
        TCKind::ULong => "tk_ulong".into(),
        TCKind::Objref => "tk_objref".into(),
        TCKind::Except => "tk_except".into(),
        k => format!("tk_{}", format!("{k:?}").to_ascii_lowercase()),
    }
}

fn q(s: &str) -> String {
    format!("\"{s}\"")
}

/// The current position, found by counting the components after it (this
/// moves the position to -1, so it comes last in a case).
fn position(d: &DynAny) -> Result<i32, DynAnyError> {
    if d.current_component()?.is_none() {
        return Ok(-1);
    }
    let mut after = 0;
    while d.next()? {
        after += 1;
    }
    Ok(d.component_count()? as i32 - 1 - after)
}

fn main() {
    let f = DynAnyFactory;
    let tc = |k| TypeCode::simple(k).unwrap();
    let long = tc(TCKind::Long);
    let string = TypeCode::string(0);
    let count = TypeCode::alias("IDL:P/Count:1.0", "Count", &long).unwrap();
    let s_tc = TypeCode::struct_tc(
        "IDL:P/S:1.0",
        "S",
        &[
            ("a", long.clone()),
            ("b", string.clone()),
            ("c", count.clone()),
        ],
    )
    .unwrap();
    let e_tc = TypeCode::exception::<&str>("IDL:P/E:1.0", "E", &[]).unwrap();
    let color = TypeCode::enum_tc("IDL:P/Color:1.0", "Color", &["RED", "GREEN", "BLUE"]).unwrap();
    let u_tc = TypeCode::union_tc(
        "IDL:P/U:1.0",
        "U",
        &long,
        &[
            (Some(1), "x", long.clone()),
            (Some(2), "y", string.clone()),
            (Some(3), "y", string.clone()),
            (None, "z", tc(TCKind::Boolean)),
        ],
    )
    .unwrap();
    let d_tc = TypeCode::union_tc(
        "IDL:P/D:1.0",
        "D",
        &long,
        &[
            (None, "z", tc(TCKind::Boolean)),
            (Some(0), "x", long.clone()),
        ],
    )
    .unwrap();
    let n_tc = TypeCode::union_tc(
        "IDL:P/N:1.0",
        "N",
        &tc(TCKind::Short),
        &[(Some(5), "x", long.clone()), (Some(2), "y", string.clone())],
    )
    .unwrap();
    let b_tc = TypeCode::union_tc(
        "IDL:P/B:1.0",
        "B",
        &tc(TCKind::Boolean),
        &[(Some(1), "t", long.clone()), (Some(0), "f", string.clone())],
    )
    .unwrap();
    let seq_tc = TypeCode::sequence(&long, 0).unwrap();
    let bseq_tc = TypeCode::sequence(&long, 2).unwrap();
    let arr_tc = TypeCode::array(&long, 3).unwrap();
    let money = TypeCode::fixed(5, 2).unwrap();
    let bstr = TypeCode::string(3);
    let v_tc = TypeCode::value_tc(
        TCKind::Value,
        "IDL:P/V:1.0",
        "V",
        0,
        None,
        &[("n", long.clone(), 1)],
    )
    .unwrap();
    let box_tc = TypeCode::value_box("IDL:P/LBox:1.0", "LBox", &long).unwrap();
    let new = |t: &TypeCode| f.create_dyn_any_from_type_code(t);
    let b = |v: bool| v.to_string();

    // -- creation from TypeCodes and positions
    case("long.count", || {
        Ok(new(&long)?.component_count()?.to_string())
    });
    case("long.get", || Ok(new(&long)?.get_long()?.to_string()));
    case("long.seek0", || Ok(b(new(&long)?.seek(0)?)));
    case("long.next", || Ok(b(new(&long)?.next()?)));
    case("long.current_component", || {
        Ok(b(new(&long)?.current_component()?.is_none()))
    });
    case("long.get_short", || {
        new(&long)?.get_short().map(|_| String::new())
    });
    case("long.insert_short", || {
        new(&long)?.insert_short(1).map(|_| String::new())
    });
    case("alias.insert_long", || {
        let d = new(&count)?;
        d.insert_long(7)?;
        Ok(d.get_long()?.to_string())
    });
    case("alias.type", || Ok(kind(new(&count)?.type_code()?.kind())));
    case("string.get", || Ok(q(&new(&string)?.get_string()?)));
    case("bstring.insert_long", || {
        new(&bstr)?.insert_string("abcd").map(|_| String::new())
    });
    case("bstring.insert_ok", || {
        new(&bstr)?.insert_string("abc")?;
        Ok(b(true))
    });
    case("typecode.get", || {
        Ok(kind(new(&tc(TCKind::TypeCode))?.get_typecode()?.kind()))
    });
    case("any.get", || {
        Ok(kind(new(&tc(TCKind::Any))?.get_any()?.typecode().kind()))
    });
    case("any.count", || {
        Ok(new(&tc(TCKind::Any))?.component_count()?.to_string())
    });
    case("objref.get", || {
        Ok(b(new(&TypeCode::object())?.get_reference()?.is_none()))
    });
    case("principal", || {
        Ok(kind(
            new(&tc(TCKind::Principal))?.to_any()?.typecode().kind(),
        ))
    });
    case("longdouble", || {
        Ok(kind(
            new(&tc(TCKind::LongDouble))?.to_any()?.typecode().kind(),
        ))
    });

    // -- get_dyn_any: shared with the any or a copy
    case("any.get_dyn_any_shared", || {
        let d = new(&tc(TCKind::Any))?;
        d.insert_any(&Any::from(5i32))?;
        d.get_dyn_any()?.insert_long(9)?;
        match d.get_any()?.value() {
            Value::Long(v) => Ok(v.to_string()),
            _ => Ok("0".into()),
        }
    });

    // -- struct
    case("struct.count", || {
        Ok(new(&s_tc)?.component_count()?.to_string())
    });
    case(
        "struct.position",
        || Ok(position(&new(&s_tc)?)?.to_string()),
    );
    case("struct.insert_long", || {
        let d = new(&s_tc)?;
        d.insert_long(3)?;
        Ok(q(&d.current_member_name()?))
    });
    case("struct.insert_long_moves", || {
        let d = new(&s_tc)?;
        d.insert_long(3)?;
        let s = b(d.seek(0)?);
        Ok(s + &d.current_component()?.unwrap().get_long()?.to_string())
    });
    case("struct.insert_string_at_0", || {
        new(&s_tc)?.insert_string("x").map(|_| String::new())
    });
    case("struct.member_kind_alias", || {
        let d = new(&s_tc)?;
        d.seek(2)?;
        Ok(kind(d.current_member_kind()?))
    });
    case("struct.member_name_end", || {
        let d = new(&s_tc)?;
        d.seek(5)?;
        Ok(q(&d.current_member_name()?))
    });
    case("struct.insert_at_end", || {
        let d = new(&s_tc)?;
        d.seek(5)?;
        d.insert_long(1).map(|_| String::new())
    });
    case("struct.next_past_end", || {
        let d = new(&s_tc)?;
        d.seek(2)?;
        let s = b(d.next()?);
        Ok(s + &b(d.current_component()?.is_none()))
    });
    case("struct.next_from_minus1", || {
        let d = new(&s_tc)?;
        d.seek(-1)?;
        let s = b(d.next()?);
        Ok(s + &position(&d)?.to_string())
    });
    case("struct.seek_minus1", || Ok(b(new(&s_tc)?.seek(-1)?)));
    case("struct.component_shared", || {
        let d = new(&s_tc)?;
        d.current_component()?.unwrap().insert_long(11)?;
        match d.get_members()?[0].1.value() {
            Value::Long(v) => Ok(v.to_string()),
            _ => Ok("0".into()),
        }
    });
    case("struct.members_as_dyn_any_shared", || {
        let d = new(&s_tc)?;
        d.get_members_as_dyn_any()?[0].1.insert_long(12)?;
        Ok(d.current_component()?.unwrap().get_long()?.to_string())
    });
    case("struct.set_members_wrong_name", || {
        let d = new(&s_tc)?;
        let mut m = d.get_members()?;
        m[1].0 = "zz".into();
        d.set_members(&m).map(|_| String::new())
    });
    case("struct.set_members_empty_names", || {
        let d = new(&s_tc)?;
        let mut m = d.get_members()?;
        for x in &mut m {
            x.0.clear();
        }
        d.set_members(&m)?;
        Ok(b(true))
    });
    case("struct.set_members_short", || {
        let d = new(&s_tc)?;
        let mut m = d.get_members()?;
        m.truncate(2);
        d.set_members(&m).map(|_| String::new())
    });
    case("struct.set_members_wrong_type", || {
        let d = new(&s_tc)?;
        let mut m = d.get_members()?;
        m[0].1 = Any::from(1i16);
        d.set_members(&m).map(|_| String::new())
    });
    case("struct.set_members_unaliased", || {
        let d = new(&s_tc)?;
        let mut m = d.get_members()?;
        m[2].1 = Any::from(4i32);
        d.set_members(&m)?;
        Ok(b(true))
    });
    case("struct.set_members_position", || {
        let d = new(&s_tc)?;
        d.seek(2)?;
        let m = d.get_members()?;
        d.set_members(&m)?;
        Ok(position(&d)?.to_string())
    });
    case("struct.from_any_wrong", || {
        new(&s_tc)?
            .from_any(&Any::from(1i32))
            .map(|_| String::new())
    });
    case("struct.from_any_position", || {
        let d = new(&s_tc)?;
        let a = d.to_any()?;
        d.seek(2)?;
        d.from_any(&a)?;
        Ok(position(&d)?.to_string())
    });
    case("struct.assign_wrong", || {
        new(&s_tc)?.assign(&new(&long)?).map(|_| String::new())
    });
    case("struct.destroy_component", || {
        let d = new(&s_tc)?;
        let c = d.current_component()?.unwrap();
        c.destroy()?;
        c.insert_long(2)?;
        Ok(c.get_long()?.to_string())
    });
    case("struct.destroyed", || {
        let d = new(&s_tc)?;
        let c = d.current_component()?.unwrap();
        d.destroy()?;
        c.get_long().map(|_| String::new())
    });
    case("struct.destroyed_top", || {
        let d = new(&s_tc)?;
        d.destroy()?;
        d.component_count().map(|_| String::new())
    });
    case("struct.copy_independent", || {
        let d = new(&s_tc)?;
        d.copy()?.insert_long(5)?;
        Ok(d.current_component()?.unwrap().get_long()?.to_string())
    });
    case("struct.equal", || {
        let d = new(&s_tc)?;
        let e = d.copy()?;
        let s = b(d.equal(&e)?);
        e.insert_long(1)?;
        Ok(s + &b(d.equal(&e)?))
    });
    case("except.count", || {
        Ok(new(&e_tc)?.component_count()?.to_string())
    });
    case("except.current_component", || {
        Ok(b(new(&e_tc)?.current_component()?.is_none()))
    });
    case("except.member_name", || {
        Ok(q(&new(&e_tc)?.current_member_name()?))
    });

    // -- enum
    case("enum.get", || Ok(q(&new(&color)?.get_enum_as_string()?)));
    case("enum.set_as_string_bad", || {
        new(&color)?
            .set_enum_as_string("PINK")
            .map(|_| String::new())
    });
    case("enum.set_as_ulong_bad", || {
        new(&color)?.set_enum_as_ulong(3).map(|_| String::new())
    });
    case("enum.set_as_ulong", || {
        let d = new(&color)?;
        d.set_enum_as_ulong(2)?;
        Ok(q(&d.get_enum_as_string()?))
    });
    case("enum.current_component", || {
        Ok(b(new(&color)?.current_component()?.is_none()))
    });
    case("enum.count", || {
        Ok(new(&color)?.component_count()?.to_string())
    });
    case("enum.insert_ulong", || {
        new(&color)?.insert_ulong(1).map(|_| String::new())
    });

    // -- fixed
    let fixed = |lit: &str| -> R {
        let d = new(&money)?;
        let exact = d.set_fixed_value(lit)?;
        Ok(b(exact) + &q(&d.get_fixed_value()?))
    };
    case("fixed.get", || Ok(q(&new(&money)?.get_fixed_value()?)));
    case("fixed.set", || fixed("-12.5"));
    case("fixed.set_d", || fixed("1.25d"));
    case("fixed.set_blank", || fixed(" 1.25 "));
    case("fixed.set_truncates", || fixed("1.239"));
    case("fixed.set_zero_tail", || {
        Ok(b(new(&money)?.set_fixed_value("1.230")?))
    });
    case("fixed.set_too_big", || {
        new(&money)?
            .set_fixed_value("1234.5")
            .map(|_| String::new())
    });
    case("fixed.set_bad", || {
        new(&money)?.set_fixed_value("abc").map(|_| String::new())
    });
    case("fixed.set_int", || fixed("7"));

    // -- union
    let disc_of = |t: &TypeCode, v: Value| -> Result<DynAny, DynAnyError> {
        let d = new(t)?;
        d.from_any(&Any::new(t.clone(), v))?;
        Ok(d)
    };
    case("union.initial", || {
        let d = new(&u_tc)?;
        Ok(format!(
            "{} {} {}",
            d.get_discriminator()?.get_long()?,
            b(d.has_no_active_member()?),
            q(&d.member_name()?)
        ))
    });
    case("union.count", || {
        Ok(new(&u_tc)?.component_count()?.to_string())
    });
    case("union.position", || Ok(position(&new(&u_tc)?)?.to_string()));
    case("union.default_first", || {
        let d = new(&d_tc)?;
        Ok(format!(
            "{} {}",
            d.get_discriminator()?.get_long()?,
            q(&d.member_name()?)
        ))
    });
    case("union.no_default_initial", || {
        let d = new(&n_tc)?;
        Ok(format!(
            "{} {}",
            d.get_discriminator()?.get_short()?,
            q(&d.member_name()?)
        ))
    });
    case("union.set_discriminator", || {
        let d = new(&u_tc)?;
        d.set_discriminator(&disc_of(&long, Value::Long(3))?)?;
        Ok(format!("{} {}", q(&d.member_name()?), position(&d)?))
    });
    case("union.set_discriminator_keeps", || {
        let d = new(&u_tc)?;
        let disc = disc_of(&long, Value::Long(2))?;
        d.set_discriminator(&disc)?;
        d.member()?.insert_string("kept")?;
        disc.insert_long(3)?;
        d.set_discriminator(&disc)?;
        Ok(q(&d.member()?.get_string()?))
    });
    case("union.set_discriminator_default", || {
        let d = new(&u_tc)?;
        d.set_discriminator(&disc_of(&long, Value::Long(42))?)?;
        Ok(q(&d.member_name()?))
    });
    case("union.set_discriminator_wrong", || {
        new(&u_tc)?
            .set_discriminator(&new(&tc(TCKind::Short))?)
            .map(|_| String::new())
    });
    case("union.disc_component", || {
        let d = new(&u_tc)?;
        d.get_discriminator()?.insert_long(2)?;
        Ok(format!("{} {}", q(&d.member_name()?), d.component_count()?))
    });
    case("union.disc_component_to_none", || {
        let d = new(&n_tc)?;
        d.get_discriminator()?.insert_short(9)?;
        Ok(format!(
            "{} {}",
            b(d.has_no_active_member()?),
            d.component_count()?
        ))
    });
    case("union.set_to_default_member", || {
        let d = new(&u_tc)?;
        d.set_to_default_member()?;
        Ok(format!(
            "{} {} {}",
            q(&d.member_name()?),
            d.get_discriminator()?.get_long()?,
            position(&d)?
        ))
    });
    case("union.set_to_default_member_none", || {
        new(&n_tc)?.set_to_default_member().map(|_| String::new())
    });
    case("union.set_to_no_active_member", || {
        let d = new(&n_tc)?;
        d.set_to_no_active_member()?;
        Ok(format!(
            "{} {} {} {}",
            b(d.has_no_active_member()?),
            d.get_discriminator()?.get_short()?,
            d.component_count()?,
            position(&d)?
        ))
    });
    case("union.set_to_no_active_member_default", || {
        new(&u_tc)?.set_to_no_active_member().map(|_| String::new())
    });
    case("union.set_to_no_active_member_full", || {
        new(&b_tc)?.set_to_no_active_member().map(|_| String::new())
    });
    case("union.member_none", || {
        let d = new(&n_tc)?;
        d.set_to_no_active_member()?;
        d.member().map(|_| String::new())
    });
    case("union.member_name_none", || {
        let d = new(&n_tc)?;
        d.set_to_no_active_member()?;
        d.member_name().map(|_| String::new())
    });
    case("union.discriminator_kind", || {
        Ok(kind(new(&n_tc)?.discriminator_kind()?))
    });
    case("union.member_kind", || Ok(kind(new(&u_tc)?.member_kind()?)));
    case("union.insert_long", || {
        let d = new(&u_tc)?;
        d.insert_long(1)?;
        Ok(position(&d)?.to_string())
    });
    case("union.boolean_initial", || {
        let d = new(&b_tc)?;
        Ok(format!(
            "{} {}",
            b(d.get_discriminator()?.get_boolean()?),
            q(&d.member_name()?)
        ))
    });

    // -- sequence and array
    case("seq.count", || {
        Ok(new(&seq_tc)?.component_count()?.to_string())
    });
    case("seq.current_component", || {
        Ok(b(new(&seq_tc)?.current_component()?.is_none()))
    });
    case("seq.insert_empty", || {
        new(&seq_tc)?.insert_long(1).map(|_| String::new())
    });
    case("seq.grow_position", || {
        let d = new(&seq_tc)?;
        d.set_length(3)?;
        Ok(position(&d)?.to_string())
    });
    case("seq.grow_keeps_position", || {
        let d = new(&seq_tc)?;
        d.set_length(2)?;
        d.seek(1)?;
        d.insert_long(4)?;
        d.set_length(4)?;
        Ok(d.current_component()?.unwrap().get_long()?.to_string())
    });
    case("seq.grow_from_minus1", || {
        let d = new(&seq_tc)?;
        d.set_length(2)?;
        d.seek(-1)?;
        d.set_length(4)?;
        let s = b(d.current_component()?.is_none());
        d.insert_long(8)?;
        let e = d.copy()?;
        e.seek(2)?;
        Ok(format!("{s} {}", e.get_long()?))
    });
    case("seq.shrink_position", || {
        let d = new(&seq_tc)?;
        d.set_length(4)?;
        d.seek(3)?;
        d.set_length(2)?;
        Ok(b(d.current_component()?.is_none()))
    });
    case("seq.shrink_keeps", || {
        let d = new(&seq_tc)?;
        d.set_length(4)?;
        d.seek(1)?;
        d.insert_long(6)?;
        d.set_length(2)?;
        Ok(d.current_component()?.unwrap().get_long()?.to_string())
    });
    case("seq.shrink_to_0", || {
        let d = new(&seq_tc)?;
        d.set_length(4)?;
        d.set_length(0)?;
        Ok(b(d.current_component()?.is_none()))
    });
    case("seq.bound", || {
        new(&bseq_tc)?.set_length(3).map(|_| String::new())
    });
    let longs = |n: i32| (0..n).map(Any::from).collect::<Vec<_>>();
    case("seq.set_elements_bound", || {
        new(&bseq_tc)?
            .set_elements(&longs(3))
            .map(|_| String::new())
    });
    case("seq.set_elements_type", || {
        new(&seq_tc)?
            .set_elements(&[Any::from(1i16)])
            .map(|_| String::new())
    });
    case("seq.set_elements_position", || {
        let d = new(&seq_tc)?;
        d.set_elements(&longs(2))?;
        Ok(position(&d)?.to_string())
    });
    case("seq.insert_long_seq", || {
        let d = new(&seq_tc)?;
        d.insert_long_seq(&[1, 2])?;
        Ok(format!("{} {}", d.component_count()?, position(&d)?))
    });
    case("seq.get_long_seq", || {
        let d = new(&seq_tc)?;
        d.set_length(2)?;
        Ok(d.get_long_seq()?.len().to_string())
    });
    case("seq.insert_short_seq", || {
        new(&seq_tc)?.insert_short_seq(&[1]).map(|_| String::new())
    });
    case("seq.insert_long_seq_bound", || {
        new(&bseq_tc)?
            .insert_long_seq(&[0, 0, 0])
            .map(|_| String::new())
    });
    case("struct.insert_long_seq", || {
        new(&s_tc)?.insert_long_seq(&[0]).map(|_| String::new())
    });
    case("arr.count", || {
        let d = new(&arr_tc)?;
        Ok(format!("{} {}", d.component_count()?, position(&d)?))
    });
    case("arr.set_elements_count", || {
        new(&arr_tc)?.set_elements(&longs(2)).map(|_| String::new())
    });
    case("arr.insert_long_seq_count", || {
        new(&arr_tc)?
            .insert_long_seq(&[0, 0])
            .map(|_| String::new())
    });

    // -- valuetypes
    case("value.null", || {
        let d = new(&v_tc)?;
        Ok(format!("{} {}", b(d.is_null()?), d.component_count()?))
    });
    case("value.set_to_value", || {
        let d = new(&v_tc)?;
        d.set_to_value()?;
        Ok(format!(
            "{} {} {}",
            b(d.is_null()?),
            d.component_count()?,
            position(&d)?
        ))
    });
    case("value.get_members_null", || {
        Ok(new(&v_tc)?.get_members()?.len().to_string())
    });
    case("value.member_name_null", || {
        Ok(q(&new(&v_tc)?.current_member_name()?))
    });
    case("value.current_component_null", || {
        Ok(b(new(&v_tc)?.current_component()?.is_none()))
    });
    case("value.set_members", || {
        let d = new(&v_tc)?;
        d.set_members(&[("n".into(), Any::from(3i32))])?;
        Ok(format!(
            "{} {}",
            b(d.is_null()?),
            d.current_component()?.unwrap().get_long()?
        ))
    });
    case("box.null", || {
        let d = new(&box_tc)?;
        Ok(format!("{} {}", b(d.is_null()?), d.component_count()?))
    });
    case("box.get_null", || {
        new(&box_tc)?.get_boxed_value().map(|_| String::new())
    });
    case("box.set", || {
        let d = new(&box_tc)?;
        d.set_boxed_value(&Any::from(4i32))?;
        Ok(format!(
            "{} {} {}",
            b(d.is_null()?),
            d.component_count()?,
            position(&d)?
        ))
    });
    case("box.set_wrong", || {
        new(&box_tc)?
            .set_boxed_value(&Any::from(4i16))
            .map(|_| String::new())
    });
    case("box.set_to_value", || {
        let d = new(&box_tc)?;
        d.set_to_value()?;
        Ok(d.current_component()?.unwrap().get_long()?.to_string())
    });
}
