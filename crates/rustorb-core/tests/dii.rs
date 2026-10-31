//! DII requests and DSI servants, between two ORBs over TCP and within one.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rustorb_core::dii::{
    ArgMode, DynamicImplementation, DynamicServant, NamedValue, ServerRequest,
};
use rustorb_core::{
    Any, CompletionStatus, EndpointAddr, Exception, InStream, ObjectRef, Orb, OrbOptions, Servant,
    ServerReply, SystemException, SystemExceptionKind, TCKind, TypeCode, Value, minor,
};

const CALC_ID: &str = "IDL:test/Calc:1.0";
const OOPS_ID: &str = "IDL:test/Oops:1.0";

fn long() -> TypeCode {
    TypeCode::simple(TCKind::Long).unwrap()
}

fn double() -> TypeCode {
    TypeCode::simple(TCKind::Double).unwrap()
}

fn oops() -> TypeCode {
    TypeCode::exception(OOPS_ID, "Oops", &[("code", long())]).unwrap()
}

fn sys(kind: SystemExceptionKind) -> SystemException {
    SystemException::new(kind, 0, CompletionStatus::No)
}

/// The DSI servant of
///
/// ```idl
/// exception Oops { long code; };
/// interface Calc {
///   long add(in long a, in long b);
///   void scale(inout double v, in double by, out string text);
///   void fail(in long code) raises (Oops);
///   void deny();
///   oneway void post(in long v);
///   long slow(in long ms);
/// };
/// ```
#[derive(Default)]
struct Calc {
    posted: Mutex<Vec<i32>>,
}

fn long_of(nv: &NamedValue) -> i32 {
    match nv.value.value() {
        Value::Long(v) => *v,
        other => panic!("not a long: {other:?}"),
    }
}

fn double_of(any: &Any) -> f64 {
    match any.value() {
        Value::Double(v) => *v,
        other => panic!("not a double: {other:?}"),
    }
}

fn long_in(name: &str) -> NamedValue {
    NamedValue::new(name, Any::new(long(), Value::Void), ArgMode::In)
}

impl DynamicImplementation for Calc {
    fn primary_interface(&self) -> &str {
        CALC_ID
    }

    fn invoke(&self, request: &mut ServerRequest) -> Result<(), SystemException> {
        match request.operation() {
            "add" => {
                let args = request.arguments(vec![long_in("a"), long_in("b")])?;
                let sum = long_of(&args[0]) + long_of(&args[1]);
                request.set_result(Any::new(long(), Value::Long(sum)))
            }
            "scale" => {
                let args = request.arguments(vec![
                    NamedValue::new("v", Any::new(double(), Value::Void), ArgMode::InOut),
                    NamedValue::new("by", Any::new(double(), Value::Void), ArgMode::In),
                    NamedValue::out("text", TypeCode::string(0)),
                ])?;
                let v = double_of(&args[0].value) * double_of(&args[1].value);
                args[0].value = Any::new(double(), Value::Double(v));
                args[2].value = Any::new(TypeCode::string(0), Value::String(format!("{v}")));
                Ok(())
            }
            "fail" => {
                let args = request.arguments(vec![long_in("code")])?;
                let code = long_of(&args[0]);
                request.set_exception(Any::new(oops(), Value::List(vec![Value::Long(code)])))
            }
            "deny" => {
                request.arguments(Vec::new())?;
                Err(SystemException::new(
                    SystemExceptionKind::NO_PERMISSION,
                    3,
                    CompletionStatus::No,
                ))
            }
            "post" => {
                let args = request.arguments(vec![long_in("v")])?;
                let v = long_of(&args[0]);
                self.posted.lock().unwrap().push(v);
                Ok(())
            }
            "slow" => {
                let args = request.arguments(vec![long_in("ms")])?;
                let ms = long_of(&args[0]);
                thread::sleep(Duration::from_millis(ms as u64));
                request.set_result(Any::new(long(), Value::Long(ms)))
            }
            _ => Err(SystemException::new(
                SystemExceptionKind::BAD_OPERATION,
                minor::UNKNOWN_OPERATION,
                CompletionStatus::No,
            )),
        }
    }
}

/// The same `add` as a static servant, for DII to an ordinary servant.
struct StaticCalc;

impl Servant for StaticCalc {
    fn repository_id(&self) -> &str {
        CALC_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        match operation {
            "add" => {
                let a = input.read::<i32>()?;
                let b = input.read::<i32>()?;
                reply.output().write_i32(a + b);
                Ok(())
            }
            _ => Err(sys(SystemExceptionKind::BAD_OPERATION)),
        }
    }
}

fn server_options() -> OrbOptions {
    OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..OrbOptions::default()
    }
}

fn activate(orb: &Orb, servant: Arc<dyn Servant>) -> ObjectRef {
    let root = orb.root_poa().unwrap();
    let id = root.activate_object(servant).unwrap();
    root.the_poa_manager().activate().unwrap();
    root.id_to_reference(&id).unwrap()
}

fn remote(client: &Orb, server: &Orb, obj: &ObjectRef) -> ObjectRef {
    client
        .string_to_object(&server.object_to_string(Some(obj)).unwrap())
        .unwrap()
        .unwrap()
}

fn add_request(obj: &ObjectRef, a: i32, b: i32) -> rustorb_core::dii::DiiRequest {
    let mut req = obj.create_request("add");
    req.add_in_arg("a", Any::new(long(), Value::Long(a)))
        .add_in_arg("b", Any::new(long(), Value::Long(b)))
        .set_return_type(long());
    req
}

fn system<T: std::fmt::Debug>(result: Result<T, Exception>) -> SystemException {
    match result {
        Err(Exception::System(e)) => e,
        other => panic!("expected a system exception, got {other:?}"),
    }
}

/// Every kind of call of `obj`, a `Calc` of the DSI servant `calc`.
fn exercise(obj: &ObjectRef, calc: &Calc) {
    assert_eq!(obj.type_id(), CALC_ID);
    assert!(obj.is_a(CALC_ID).unwrap());
    assert!(!obj.is_a("IDL:test/Other:1.0").unwrap());

    let mut req = add_request(obj, 2, 40);
    req.invoke().unwrap();
    assert_eq!(req.result().value(), &Value::Long(42));
    // Again, with other arguments.
    req.arguments_mut()[0].value = Any::new(long(), Value::Long(-5));
    req.invoke().unwrap();
    assert_eq!(req.result().value(), &Value::Long(35));

    let mut req = obj.create_request("scale");
    req.add_inout_arg("v", Any::new(double(), Value::Double(1.5)))
        .add_in_arg("by", Any::new(double(), Value::Double(4.0)))
        .add_out_arg("text", TypeCode::string(0));
    req.invoke().unwrap();
    assert_eq!(req.result().typecode().kind(), TCKind::Void);
    assert_eq!(req.arguments()[0].value.value(), &Value::Double(6.0));
    assert_eq!(req.arguments()[1].value.value(), &Value::Double(4.0));
    assert_eq!(req.arguments()[2].value.value(), &Value::String("6".into()));
    assert_eq!(req.arguments()[2].mode, ArgMode::Out);

    let mut req = obj.create_request("fail");
    req.add_in_arg("code", Any::new(long(), Value::Long(7)));
    let e = match req.invoke() {
        Err(Exception::User(e)) => e,
        other => panic!("expected Oops, got {other:?}"),
    };
    assert_eq!(e.repository_id(), OOPS_ID);
    // Not listed: UNKNOWN.
    let unknown = req.exception_value(&e).unwrap_err();
    assert!(unknown.is(SystemExceptionKind::UNKNOWN));
    req.add_exception(oops());
    let value = req.exception_value(&e).unwrap();
    assert_eq!(value.value(), &Value::List(vec![Value::Long(7)]));

    let e = system(obj.create_request("deny").invoke());
    assert!(e.is(SystemExceptionKind::NO_PERMISSION));
    assert_eq!(e.minor, 3);
    let e = system(obj.create_request("nope").invoke());
    assert!(e.is(SystemExceptionKind::BAD_OPERATION));
    // Arguments that do not fit the operation.
    let mut req = obj.create_request("add");
    req.add_in_arg("a", Any::new(long(), Value::Long(1)));
    let e = system(req.invoke());
    assert!(e.is(SystemExceptionKind::MARSHAL), "{e:?}");

    let mut req = obj.create_request("post");
    req.add_in_arg("v", Any::new(long(), Value::Long(9)));
    req.send_oneway().unwrap();
    let posted = (0..200).find_map(|_| {
        let posted = calc.posted.lock().unwrap().clone();
        if posted.is_empty() {
            thread::sleep(Duration::from_millis(10));
            None
        } else {
            Some(posted)
        }
    });
    assert_eq!(posted, Some(vec![9]));

    // Deferred: several at once, collected in another order.
    let mut slow = obj.create_request("slow");
    slow.add_in_arg("ms", Any::new(long(), Value::Long(300)))
        .set_return_type(long());
    slow.send_deferred().unwrap();
    let mut quick = add_request(obj, 1, 1);
    quick.send_deferred().unwrap();
    // A request waiting for its response is not sent again.
    assert!(system(quick.invoke()).is(SystemExceptionKind::BAD_INV_ORDER));
    quick.get_response().unwrap();
    assert_eq!(quick.result().value(), &Value::Long(2));
    assert!(!slow.poll_response().unwrap());
    let done = (0..300).any(|_| {
        thread::sleep(Duration::from_millis(10));
        slow.poll_response().unwrap()
    });
    assert!(done);
    assert!(slow.poll_response().unwrap());
    slow.get_response().unwrap();
    assert_eq!(slow.result().value(), &Value::Long(300));
    // Nothing is pending any more.
    assert!(
        slow.poll_response()
            .unwrap_err()
            .is(SystemExceptionKind::BAD_INV_ORDER)
    );
    assert!(system(slow.get_response()).is(SystemExceptionKind::BAD_INV_ORDER));

    // A deferred call that fails gives its exception at get_response.
    let mut req = obj.create_request("deny");
    req.send_deferred().unwrap();
    assert!(system(req.get_response()).is(SystemExceptionKind::NO_PERMISSION));
}

#[test]
fn dii_to_a_dsi_servant_over_tcp() {
    let server = Orb::with_options(server_options()).unwrap();
    let calc = Arc::new(DynamicServant(Calc::default()));
    let obj = activate(&server, calc.clone());
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    exercise(&remote(&client, &server, &obj), &calc.0);
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn dii_to_a_dsi_servant_of_the_same_orb() {
    let orb = Orb::with_options(server_options()).unwrap();
    let calc = Arc::new(DynamicServant(Calc::default()));
    let obj = activate(&orb, calc.clone());
    exercise(&obj, &calc.0);
    orb.destroy().unwrap();
}

#[test]
fn dii_to_a_static_servant_and_async() {
    let server = Orb::with_options(server_options()).unwrap();
    let obj = activate(&server, Arc::new(StaticCalc));
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let target = remote(&client, &server, &obj);
    let mut req = add_request(&target, 20, 22);
    req.invoke().unwrap();
    assert_eq!(req.result().value(), &Value::Long(42));
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    rt.block_on(async {
        let mut req = add_request(&target, 1, 2);
        req.invoke_async().await.unwrap();
        assert_eq!(req.result().value(), &Value::Long(3));
    });
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn server_request_rules() {
    struct Rules;
    impl DynamicImplementation for Rules {
        fn primary_interface(&self) -> &str {
            CALC_ID
        }
        fn invoke(&self, request: &mut ServerRequest) -> Result<(), SystemException> {
            // The result before the arguments, the arguments twice, and an
            // exception that is not one.
            let early = request.set_result(Any::new(long(), Value::Long(1)));
            assert!(early.unwrap_err().is(SystemExceptionKind::BAD_INV_ORDER));
            request.arguments(Vec::new())?;
            let again = request.arguments(Vec::new()).unwrap_err();
            assert!(again.is(SystemExceptionKind::BAD_INV_ORDER));
            let not_except = request.set_exception(Any::new(long(), Value::Long(1)));
            assert!(not_except.unwrap_err().is(SystemExceptionKind::BAD_PARAM));
            // An out argument left without a value.
            request.parameters_mut().unwrap();
            let _ = request.arguments(vec![NamedValue::out("x", long())]);
            request.set_result(Any::new(long(), Value::Long(5)))
        }
    }
    let orb = Orb::with_options(server_options()).unwrap();
    let obj = activate(&orb, Arc::new(DynamicServant(Rules)));
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let mut req = remote(&client, &orb, &obj).create_request("any");
    req.set_return_type(long());
    req.invoke().unwrap();
    assert_eq!(req.result().value(), &Value::Long(5));
    client.destroy().unwrap();
    orb.destroy().unwrap();
}

#[allow(dead_code)]
fn invoke_async_is_send(req: &mut rustorb_core::dii::DiiRequest) -> impl Send + '_ {
    req.invoke_async()
}
