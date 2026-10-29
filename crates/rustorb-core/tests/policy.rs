//! `create_policy`, `register_initial_reference`, the policy overrides of
//! references, threads and the ORB, and `validate_connection`.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rustorb_core::policy::{
    LIFESPAN_POLICY_ID, RELATIVE_RT_TIMEOUT_POLICY_TYPE, THREAD_POLICY_ID,
    set_thread_policy_overrides, thread_policy_overrides,
};
use rustorb_core::{
    Any, CompletionStatus, EndpointAddr, Exception, InStream, Lifespan, Orb, OrbOptions,
    PolicyError, PolicyValue, ResolveError, Servant, ServerReply, SetOverrideType, SystemException,
    SystemExceptionKind, TCKind, ThreadPolicy, TypeCode, Value, minor, set_thread_call_timeout,
};

fn server_orb() -> Orb {
    let options = OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..OrbOptions::default()
    };
    Orb::with_options(options).unwrap()
}

fn enum_any(id: &str, members: &[&str], v: u32) -> Any {
    let tc = TypeCode::enum_tc(id, "E", members).unwrap();
    Any::new(tc, Value::Enum(v))
}

fn time_t(t: u64) -> Any {
    Any::new(
        TypeCode::simple(TCKind::ULongLong).unwrap(),
        Value::ULongLong(t),
    )
}

fn system_of(e: Exception) -> SystemException {
    match e {
        Exception::System(e) => e,
        Exception::User(u) => panic!("unexpected user exception {}", u.repository_id()),
    }
}

struct Nothing;

impl Servant for Nothing {
    fn repository_id(&self) -> &str {
        "IDL:test/Nothing:1.0"
    }

    fn dispatch(
        &self,
        _operation: &str,
        _input: &mut InStream<'_>,
        _reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        Ok(())
    }
}

#[test]
fn create_policy_checks_type_and_value() {
    let orb = Orb::init(&mut Vec::new()).unwrap();
    let lifespan = |v| {
        enum_any(
            "IDL:omg.org/PortableServer/LifespanPolicyValue:1.0",
            &["TRANSIENT", "PERSISTENT"],
            v,
        )
    };
    let p = orb.create_policy(LIFESPAN_POLICY_ID, &lifespan(1)).unwrap();
    assert_eq!(
        p,
        PolicyValue::Poa(rustorb_core::Policy::Lifespan(Lifespan::Persistent))
    );
    assert_eq!((p.policy_type(), p.raw_value()), (17, 1));
    // The version of the enum's repository id does not matter.
    let t = enum_any(
        "IDL:omg.org/PortableServer/ThreadPolicyValue:2.3",
        &["ORB_CTRL_MODEL", "SINGLE_THREAD_MODEL", "MAIN_THREAD_MODEL"],
        2,
    );
    assert_eq!(
        orb.create_policy(THREAD_POLICY_ID, &t),
        Ok(PolicyValue::Poa(rustorb_core::Policy::Thread(
            ThreadPolicy::MainThread
        )))
    );
    let err = |reason| Err(PolicyError { reason });
    // Another enum, or a number, is the wrong type.
    assert_eq!(
        orb.create_policy(LIFESPAN_POLICY_ID, &t),
        err(PolicyError::BAD_POLICY_TYPE)
    );
    let ulong = Any::new(TypeCode::simple(TCKind::ULong).unwrap(), Value::ULong(1));
    assert_eq!(
        orb.create_policy(LIFESPAN_POLICY_ID, &ulong),
        err(PolicyError::BAD_POLICY_TYPE)
    );
    assert_eq!(
        orb.create_policy(LIFESPAN_POLICY_ID, &lifespan(2)),
        err(PolicyError::BAD_POLICY_VALUE)
    );
    assert_eq!(
        orb.create_policy(9999, &ulong),
        err(PolicyError::BAD_POLICY)
    );
    let rt = orb
        .create_policy(RELATIVE_RT_TIMEOUT_POLICY_TYPE, &time_t(15))
        .unwrap();
    assert_eq!(rt, PolicyValue::RelativeRoundtripTimeout(15));
    assert_eq!(rt.relative_expiry(), Some(Duration::from_nanos(1500)));
    assert_eq!(
        orb.create_policy(RELATIVE_RT_TIMEOUT_POLICY_TYPE, &ulong),
        err(PolicyError::BAD_POLICY_TYPE)
    );
    orb.destroy().unwrap();
}

#[test]
fn register_initial_reference() {
    let mut args = vec!["-ORBInitRef".into(), "Foo=corbaloc::127.0.0.1:1/Foo".into()];
    let orb = Orb::init(&mut args).unwrap();
    let foo = orb.resolve_initial_references("Foo").unwrap();
    let bar = orb
        .string_to_object("corbaloc::127.0.0.1:2/Bar")
        .unwrap()
        .unwrap();

    let invalid = |r: Result<(), ResolveError>| matches!(r, Err(ResolveError::InvalidName(_)));
    assert!(invalid(orb.register_initial_reference("", Some(&bar))));
    assert!(invalid(orb.register_initial_reference("Foo", Some(&bar))));
    match orb.register_initial_reference("Nil", None) {
        Err(ResolveError::System(e)) => {
            assert!(e.is(SystemExceptionKind::BAD_PARAM));
            assert_eq!(e.minor, minor::NIL_INITIAL_REFERENCE);
        }
        r => panic!("{r:?}"),
    }
    orb.register_initial_reference("Bar", Some(&bar)).unwrap();
    assert!(invalid(orb.register_initial_reference("Bar", Some(&foo))));
    assert!(
        orb.resolve_initial_references("Bar")
            .unwrap()
            .is_equivalent(&bar)
    );
    // A binding's object without a reference.
    orb.register_local_initial_reference("Local", Arc::new(7u32))
        .unwrap();
    assert!(invalid(
        orb.register_local_initial_reference("Bar", Arc::new(8u32))
    ));
    assert!(invalid(orb.register_initial_reference("Local", Some(&bar))));
    let local = orb.resolve_local_initial_reference("Local").unwrap();
    assert_eq!(local.downcast_ref::<u32>(), Some(&7));
    assert!(orb.resolve_local_initial_reference("Bar").is_none());
    assert!(matches!(
        orb.resolve_initial_references("Local"),
        Err(ResolveError::InvalidName(_))
    ));
    assert_eq!(orb.list_initial_services(), ["Bar", "Foo", "Local"]);
    orb.destroy().unwrap();
    assert!(orb.resolve_local_initial_reference("Local").is_none());
}

#[test]
fn object_overrides_make_a_new_reference() {
    let orb = Orb::init(&mut Vec::new()).unwrap();
    let obj = orb
        .string_to_object("corbaloc::127.0.0.1:1/x")
        .unwrap()
        .unwrap();
    let rt = PolicyValue::RelativeRoundtripTimeout(2_000_000);
    let with = obj
        .set_policy_overrides(&[rt], SetOverrideType::Set)
        .unwrap();
    assert!(with.is_equivalent(&obj));
    assert_eq!(with.call_timeout(), Some(Some(Duration::from_millis(200))));
    assert_eq!(obj.call_timeout(), None);
    assert_eq!(with.policy_overrides(&[]), [rt]);
    assert_eq!(
        with.policy_overrides(&[RELATIVE_RT_TIMEOUT_POLICY_TYPE]),
        [rt]
    );
    assert_eq!(with.policy_overrides(&[LIFESPAN_POLICY_ID]), []);
    assert_eq!(obj.policy_overrides(&[]), []);
    // ADD keeps the timeout, SET with no policies removes it.
    let added = with
        .set_policy_overrides(&[], SetOverrideType::Add)
        .unwrap();
    assert_eq!(added.policy_overrides(&[]), [rt]);
    let none = with
        .set_policy_overrides(&[], SetOverrideType::Set)
        .unwrap();
    assert_eq!(none.call_timeout(), None);

    let lifespan = PolicyValue::Poa(rustorb_core::Policy::Lifespan(Lifespan::Transient));
    let e = obj
        .set_policy_overrides(&[lifespan], SetOverrideType::Set)
        .unwrap_err();
    assert!(e.is(SystemExceptionKind::NO_PERMISSION));
    let e = obj
        .set_policy_overrides(&[rt, rt], SetOverrideType::Set)
        .unwrap_err();
    assert!(e.is(SystemExceptionKind::BAD_PARAM));
    assert_eq!(e.minor, minor::DUPLICATE_POLICY);
    orb.destroy().unwrap();
}

#[test]
fn client_policy_is_the_reference_then_the_thread_then_the_orb() {
    let orb = Orb::init(&mut Vec::new()).unwrap();
    let obj = orb
        .string_to_object("corbaloc::127.0.0.1:1/x")
        .unwrap()
        .unwrap();
    let rt = PolicyValue::RelativeRoundtripTimeout;
    assert_eq!(obj.client_policy(RELATIVE_RT_TIMEOUT_POLICY_TYPE), None);
    assert_eq!(obj.client_policy(LIFESPAN_POLICY_ID), None);

    orb.set_policy_overrides(&[rt(30)], SetOverrideType::Set)
        .unwrap();
    assert_eq!(orb.call_timeout(), Some(Duration::from_micros(3)));
    assert_eq!(orb.policy_overrides(&[]), [rt(30)]);
    assert_eq!(
        obj.client_policy(RELATIVE_RT_TIMEOUT_POLICY_TYPE),
        Some(rt(30))
    );

    set_thread_policy_overrides(&[rt(20)], SetOverrideType::Set).unwrap();
    assert_eq!(thread_policy_overrides(&[]), [rt(20)]);
    assert_eq!(
        obj.client_policy(RELATIVE_RT_TIMEOUT_POLICY_TYPE),
        Some(rt(20))
    );
    // Another thread has its own.
    let other = obj.clone();
    let seen = thread::spawn(move || {
        (
            thread_policy_overrides(&[]),
            other.client_policy(RELATIVE_RT_TIMEOUT_POLICY_TYPE),
        )
    })
    .join()
    .unwrap();
    assert_eq!(seen, (vec![], Some(rt(30))));

    let with = obj
        .set_policy_overrides(&[rt(10)], SetOverrideType::Set)
        .unwrap();
    assert_eq!(
        with.client_policy(RELATIVE_RT_TIMEOUT_POLICY_TYPE),
        Some(rt(10))
    );

    set_thread_policy_overrides(&[], SetOverrideType::Set).unwrap();
    assert_eq!(set_thread_call_timeout(None), None);
    orb.set_policy_overrides(&[], SetOverrideType::Set).unwrap();
    assert_eq!(orb.call_timeout(), None);
    assert_eq!(orb.policy_overrides(&[]), []);
    orb.destroy().unwrap();
}

#[test]
fn a_timeout_override_limits_calls() {
    let orb = Orb::init(&mut Vec::new()).unwrap();
    // A listener that never answers.
    let sink = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = sink.local_addr().unwrap().port();
    let _keep = thread::spawn(move || {
        let mut conns = Vec::new();
        while let Ok((c, _)) = sink.accept() {
            conns.push(c);
        }
    });
    let obj = orb
        .string_to_object(&format!("corbaloc::127.0.0.1:{port}/x"))
        .unwrap()
        .unwrap();
    let rt = PolicyValue::RelativeRoundtripTimeout(1_000_000); // 100 ms
    let with = obj
        .set_policy_overrides(&[rt], SetOverrideType::Set)
        .unwrap();
    let e = system_of(with.request("op").invoke().unwrap_err());
    assert!(e.is(SystemExceptionKind::TIMEOUT));
    assert_eq!(e.completed, CompletionStatus::Maybe);
    orb.destroy().unwrap();
}

#[test]
fn validate_connection() {
    let orb = server_orb();
    let poa = orb.root_poa().unwrap();
    poa.the_poa_manager().activate().unwrap();
    let id = poa.activate_object(Arc::new(Nothing)).unwrap();
    let obj = poa.id_to_reference(&id).unwrap();
    let missing = poa.create_reference("IDL:test/Nothing:1.0").unwrap();

    let client = Orb::init(&mut Vec::new()).unwrap();
    let remote = |o| {
        client
            .string_to_object(&orb.object_to_string(Some(o)).unwrap())
            .unwrap()
            .unwrap()
    };
    assert!(remote(&obj).validate_connection().unwrap());
    let e = system_of(remote(&missing).validate_connection().unwrap_err());
    assert!(e.is(SystemExceptionKind::OBJECT_NOT_EXIST));
    let dead = client
        .string_to_object("corbaloc::127.0.0.1:1/x")
        .unwrap()
        .unwrap();
    let e = system_of(dead.validate_connection().unwrap_err());
    assert!(e.is(SystemExceptionKind::TRANSIENT));
    client.destroy().unwrap();
    orb.destroy().unwrap();
}
