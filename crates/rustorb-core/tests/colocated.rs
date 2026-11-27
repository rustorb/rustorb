//! Calls between a client and a servant of the same ORB without GIOP
//! (docs/colocated-calls.md): what the servant sees and what the caller
//! gets back is the same as on the general path, and the general path is
//! still taken where it is needed (a holding manager, SINGLE_THREAD_MODEL,
//! server interceptors, GIOP before 1.2).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread;
use std::time::Duration;

use rustorb_core::giop::Version;
use rustorb_core::pi::sync_scope::{SYNC_NONE, SYNC_WITH_SERVER, SYNC_WITH_TARGET};
use rustorb_core::pi::{
    InterceptError, Interceptor, OrbInitInfo, OrbInitializer, ServerRequestInfo,
    ServerRequestInterceptor,
};
use rustorb_core::stats::{self, Counter};
use rustorb_core::{
    Any, CompletionStatus, EndpointAddr, Exception, IdAssignment, InStream, Lifespan, ObjectRef,
    Orb, OrbOptions, Poa, Policy, PolicyValue, RequestProcessing, Servant, ServantLocator,
    ServantManager, ServantManagerError, ServantRetention, ServerReply, SetOverrideType,
    SystemException, SystemExceptionKind, TCKind, ThreadPolicy, TypeCode, Value, minor,
};

const ECHO_ID: &str = "IDL:test/Echo:1.0";
const OOPS_ID: &str = "IDL:test/Oops:1.0";

/// The counters are per process: one test at a time.
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    stats::set_enabled(true);
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// The number of colocated calls that ran without GIOP and on the general
/// path while `f` ran.
fn paths<T>(f: impl FnOnce() -> T) -> (T, u64, u64) {
    let before = stats::snapshot();
    let out = f();
    let used = stats::snapshot().since(&before);
    (
        out,
        used.get(Counter::local_direct),
        used.get(Counter::local_general),
    )
}

#[derive(Default)]
struct Echo {
    oneways: AtomicUsize,
    /// Where `note` sends what it got.
    notes: Mutex<Option<mpsc::Sender<String>>>,
}

impl Servant for Echo {
    fn repository_id(&self) -> &str {
        ECHO_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        match operation {
            "ping" => Ok(()),
            // in octet, in double, in string, in wstring, in char,
            // in sequence<octet>, inout long -> returns all, the long + 1
            "mix" => {
                let o = input.read::<u8>()?;
                let d = input.read::<f64>()?;
                let s = input.read_string()?;
                let w = input.read_wstring()?;
                let c = input.read_char_value()?;
                let bytes = input.read::<Vec<u8>>()?;
                let n = input.read::<i32>()?;
                let mut out = reply.output();
                out.write_octet(o);
                out.write_f64(d);
                out.write_string(&s)?;
                out.write_wstring(&w)?;
                out.write_char(c as u8);
                out.write_octet_seq(&bytes).unwrap();
                out.write_i32(n + 1);
                Ok(())
            }
            "any" => {
                let any = input.read_any()?;
                reply.output().write_any(&any)
            }
            "oops" => {
                let code = input.read::<i32>()?;
                let mut out = reply.user_exception(OOPS_ID)?;
                out.write_i32(code);
                Ok(())
            }
            "fail" => Err(SystemException::new(
                SystemExceptionKind::NO_RESOURCES,
                7,
                CompletionStatus::Yes,
            )),
            "panic" => panic!("servant panics"),
            "oneway" => {
                self.oneways.fetch_add(1, Ordering::SeqCst);
                Err(SystemException::new(
                    SystemExceptionKind::NO_RESOURCES,
                    1,
                    CompletionStatus::Yes,
                ))
            }
            "note" => {
                let s = input.read_string()?;
                if let Some(tx) = &*self.notes.lock().unwrap() {
                    let _ = tx.send(s);
                }
                Ok(())
            }
            "thread" => {
                let name = thread::current().name().unwrap_or("").to_owned();
                reply.output().write_string(&name)
            }
            _ => Err(SystemException::new(
                SystemExceptionKind::BAD_OPERATION,
                minor::UNKNOWN_OPERATION,
                CompletionStatus::No,
            )),
        }
    }
}

fn orb_with(options: OrbOptions) -> Orb {
    Orb::with_options(OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..options
    })
    .unwrap()
}

/// An `Echo` activated in a POA with `policies` (under the RootPOA and its
/// manager, which is activated).
fn echo_in(orb: &Orb, name: &str, policies: &[Policy]) -> (Poa, Arc<Echo>, ObjectRef) {
    let root = orb.root_poa().unwrap();
    let mut all = vec![
        Policy::Lifespan(Lifespan::Persistent),
        Policy::IdAssignment(IdAssignment::User),
    ];
    all.extend_from_slice(policies);
    let poa = root
        .create_poa(name, Some(&root.the_poa_manager()), &all)
        .unwrap();
    let echo = Arc::new(Echo::default());
    poa.activate_object_with_id(b"echo", echo.clone()).unwrap();
    root.the_poa_manager().activate().unwrap();
    let obj = poa.id_to_reference(b"echo").unwrap();
    (poa, echo, obj)
}

fn ping(obj: &ObjectRef) -> Result<(), Exception> {
    obj.request("ping").invoke().map(|_| ())
}

fn system(r: Result<impl Sized, Exception>) -> SystemException {
    match r {
        Err(Exception::System(e)) => e,
        Err(Exception::User(u)) => panic!("user exception {}", u.repository_id()),
        Ok(_) => panic!("no exception"),
    }
}

fn mix(obj: &ObjectRef) -> Result<(), Exception> {
    let reply = obj
        .request("mix")
        .args(|out| {
            out.write_octet(0xa5);
            out.write_f64(2.5);
            out.write_string("héllo")?;
            out.write_wstring("wïde ✓")?;
            out.write_char(b'z');
            out.write_octet_seq(&[1, 2, 3, 4, 5]).unwrap();
            out.write_i32(41);
            Ok(())
        })
        .invoke()?;
    let mut input = reply.input();
    assert_eq!(input.read::<u8>().unwrap(), 0xa5);
    assert_eq!(input.read::<f64>().unwrap(), 2.5);
    assert_eq!(input.read_string().unwrap(), "héllo");
    assert_eq!(input.read_wstring().unwrap(), "wïde ✓");
    assert_eq!(input.read_char_value().unwrap(), 'z');
    assert_eq!(input.read::<Vec<u8>>().unwrap(), vec![1, 2, 3, 4, 5]);
    assert_eq!(input.read::<i32>().unwrap(), 42);
    Ok(())
}

/// Every kind of outcome, on whichever path the ORB takes.
fn outcomes(orb: &Orb, obj: &ObjectRef, echo: &Echo) {
    ping(obj).unwrap();
    mix(obj).unwrap();

    let any = Any::new(TypeCode::simple(TCKind::Long).unwrap(), Value::Long(-9));
    let reply = obj
        .request("any")
        .args(|out| out.write_any(&any))
        .invoke()
        .unwrap();
    assert_eq!(reply.input().read_any().unwrap(), any);

    match obj
        .request("oops")
        .args(|out| {
            out.write_i32(17);
            Ok(())
        })
        .invoke()
    {
        Err(Exception::User(u)) => {
            assert_eq!(u.repository_id(), OOPS_ID);
            let mut input = u.input().unwrap();
            assert_eq!(input.read::<i32>().unwrap(), 17);
        }
        other => panic!("{:?}", other.err()),
    }

    let e = system(obj.request("fail").invoke());
    assert!(e.is(SystemExceptionKind::NO_RESOURCES));
    assert_eq!((e.minor, e.completed), (7, CompletionStatus::Yes));

    let e = system(obj.request("panic").invoke());
    assert!(e.is(SystemExceptionKind::UNKNOWN));
    assert_eq!(e.completed, CompletionStatus::Maybe);

    let e = system(obj.request("nothing").invoke());
    assert!(e.is(SystemExceptionKind::BAD_OPERATION));

    // A oneway request runs, and what it raises does not come back.
    let before = echo.oneways.load(Ordering::SeqCst);
    obj.request("oneway").send_oneway().unwrap();
    assert_eq!(echo.oneways.load(Ordering::SeqCst), before + 1);

    // The ORB's own operations.
    assert!(obj.is_a(ECHO_ID).unwrap());
    assert!(!obj.is_a("IDL:test/Other:1.0").unwrap());
    assert!(!obj.non_existent().unwrap());

    // An object that is not there.
    let poa = orb.root_poa().unwrap();
    let gone = poa.create_reference(ECHO_ID).unwrap();
    let e = system(ping(&gone));
    assert!(e.is(SystemExceptionKind::OBJECT_NOT_EXIST));
}

#[test]
fn calls_run_without_giop_with_the_same_outcomes() {
    let _serial = serial();
    let orb = orb_with(OrbOptions::default());
    let (_poa, echo, obj) = echo_in(&orb, "plain", &[]);
    let ((), direct, general) = paths(|| outcomes(&orb, &obj, &echo));
    assert!(direct >= 12, "{direct} calls without GIOP");
    assert_eq!(general, 0);

    // On the calling thread.
    let o = obj.clone();
    let name = thread::Builder::new()
        .name("caller".into())
        .spawn(move || {
            let reply = o.request("thread").invoke().unwrap();
            reply.input().read_string().unwrap()
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(name, "caller");
    orb.shutdown(true).unwrap();
}

#[test]
fn the_general_path_gives_the_same_outcomes() {
    let _serial = serial();
    // SINGLE_THREAD_MODEL always takes the general path.
    let orb = orb_with(OrbOptions::default());
    let (_poa, echo, obj) = echo_in(
        &orb,
        "single",
        &[Policy::Thread(ThreadPolicy::SingleThread)],
    );
    let ((), direct, general) = paths(|| outcomes(&orb, &obj, &echo));
    // `is_a` and the missing object do not get to the POA's threading.
    assert!(general >= 10, "{general} on the general path");
    assert!(direct <= 4, "{direct} without GIOP");
    orb.shutdown(true).unwrap();

    // So does GIOP 1.1.
    let orb = orb_with(OrbOptions {
        max_giop_version: Version::V1_1,
        ..OrbOptions::default()
    });
    let (_poa, echo, obj) = echo_in(&orb, "old", &[]);
    let ((), direct, general) = paths(|| outcomes(&orb, &obj, &echo));
    assert!(general >= 12);
    assert_eq!(direct, 0);
    orb.shutdown(true).unwrap();
}

#[test]
fn oneway_sync_scopes() {
    let _serial = serial();
    let orb = orb_with(OrbOptions::default());
    let (_poa, echo, obj) = echo_in(&orb, "sync", &[]);
    for scope in [SYNC_NONE, SYNC_WITH_SERVER, SYNC_WITH_TARGET] {
        let with = obj
            .set_policy_overrides(&[PolicyValue::SyncScope(scope)], SetOverrideType::Set)
            .unwrap();
        let before = echo.oneways.load(Ordering::SeqCst);
        let (r, direct, _) = paths(|| with.request("oneway").send_oneway());
        assert_eq!(direct, 1);
        // SYNC_WITH_TARGET waits for the servant, and gets what it raised.
        if scope == SYNC_WITH_TARGET {
            assert!(r.unwrap_err().is(SystemExceptionKind::NO_RESOURCES));
        } else {
            r.unwrap();
        }
        assert_eq!(echo.oneways.load(Ordering::SeqCst), before + 1);
    }
    // SYNC_WITH_SERVER: an object that is not there is still reported.
    let gone = orb.root_poa().unwrap().create_reference(ECHO_ID).unwrap();
    let gone = gone
        .set_policy_overrides(
            &[PolicyValue::SyncScope(SYNC_WITH_SERVER)],
            SetOverrideType::Set,
        )
        .unwrap();
    let e = gone.request("oneway").send_oneway().unwrap_err();
    assert!(e.is(SystemExceptionKind::OBJECT_NOT_EXIST));
    orb.shutdown(true).unwrap();
}

#[test]
fn a_holding_manager_makes_the_call_wait() {
    let _serial = serial();
    let orb = orb_with(OrbOptions::default());
    let root = orb.root_poa().unwrap();
    let (_poa, echo, obj) = echo_in(&orb, "held", &[]);
    let (tx, rx) = mpsc::channel();
    *echo.notes.lock().unwrap() = Some(tx);
    root.the_poa_manager().hold_requests(false).unwrap();
    let before = stats::snapshot();
    let held = obj.clone();
    let call = thread::spawn(move || {
        held.request("note")
            .args(|out| out.write_string("held"))
            .invoke()
            .map(|_| ())
    });
    assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    root.the_poa_manager().activate().unwrap();
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), "held");
    call.join().unwrap().unwrap();
    let used = stats::snapshot().since(&before);
    assert_eq!(used.get(Counter::local_general), 1);
    assert_eq!(used.get(Counter::local_direct), 0);

    // Discarding: TRANSIENT, as for a remote caller.
    root.the_poa_manager().discard_requests(false).unwrap();
    let e = system(ping(&obj));
    assert!(e.is(SystemExceptionKind::TRANSIENT));
    root.the_poa_manager().activate().unwrap();
    ping(&obj).unwrap();
    orb.shutdown(true).unwrap();
}

#[test]
fn a_call_timeout_still_holds_on_the_general_path() {
    let _serial = serial();
    let orb = orb_with(OrbOptions::default());
    let root = orb.root_poa().unwrap();
    let (_poa, _echo, obj) = echo_in(&orb, "timeout", &[]);
    root.the_poa_manager().hold_requests(false).unwrap();
    let e = system(
        obj.request("ping")
            .timeout(Some(Duration::from_millis(100)))
            .invoke(),
    );
    assert!(e.is(SystemExceptionKind::TIMEOUT));
    root.the_poa_manager().activate().unwrap();
    obj.request("ping")
        .timeout(Some(Duration::from_millis(100)))
        .invoke()
        .unwrap();
    orb.shutdown(true).unwrap();
}

/// A servant locator that sends "away" to another object.
struct Locator {
    to: ObjectRef,
    echo: Arc<Echo>,
}

impl ServantLocator for Locator {
    fn preinvoke(
        &self,
        id: &[u8],
        _adapter: &Poa,
        _operation: &str,
    ) -> Result<(Arc<dyn Servant>, rustorb_core::Cookie), ServantManagerError> {
        match id {
            b"away" => Err(ServantManagerError::ForwardRequest(self.to.clone())),
            b"denied" => Err(ServantManagerError::System(SystemException::new(
                SystemExceptionKind::NO_PERMISSION,
                3,
                CompletionStatus::No,
            ))),
            _ => Ok((self.echo.clone(), Box::new(()))),
        }
    }
}

#[test]
fn servant_managers_forward_and_refuse() {
    let _serial = serial();
    let orb = orb_with(OrbOptions::default());
    let (_poa, echo, target) = echo_in(&orb, "target", &[]);
    let root = orb.root_poa().unwrap();
    let located = root
        .create_poa(
            "located",
            Some(&root.the_poa_manager()),
            &[
                Policy::Lifespan(Lifespan::Persistent),
                Policy::IdAssignment(IdAssignment::User),
                Policy::ServantRetention(ServantRetention::NonRetain),
                Policy::RequestProcessing(RequestProcessing::UseServantManager),
            ],
        )
        .unwrap();
    located
        .set_servant_manager(ServantManager::Locator(Arc::new(Locator {
            to: target.clone(),
            echo: echo.clone(),
        })))
        .unwrap();
    let away = located.create_reference_with_id(b"away", ECHO_ID).unwrap();
    let denied = located
        .create_reference_with_id(b"denied", ECHO_ID)
        .unwrap();
    let here = located.create_reference_with_id(b"here", ECHO_ID).unwrap();

    let ((), direct, general) = paths(|| {
        mix(&here).unwrap();
        // LOCATION_FORWARD: the call goes on to the target.
        mix(&away).unwrap();
        let e = system(ping(&denied));
        assert!(e.is(SystemExceptionKind::NO_PERMISSION));
        assert_eq!(e.minor, 3);
    });
    assert_eq!((direct, general), (4, 0));
    orb.shutdown(true).unwrap();
}

/// Registers a server interceptor that counts requests.
struct CountingInit(Arc<AtomicUsize>);

struct Counting(Arc<AtomicUsize>);

impl Interceptor for Counting {
    fn name(&self) -> &str {
        "counting"
    }
}

impl ServerRequestInterceptor for Counting {
    fn receive_request(&self, _ri: &ServerRequestInfo) -> Result<(), InterceptError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl OrbInitializer for CountingInit {
    fn pre_init(&self, _info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        Ok(())
    }

    fn post_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        info.add_server_request_interceptor(Arc::new(Counting(self.0.clone())))
            .unwrap();
        Ok(())
    }
}

#[test]
fn server_interceptors_see_colocated_calls() {
    let _serial = serial();
    let seen = Arc::new(AtomicUsize::new(0));
    let orb = orb_with(OrbOptions {
        initializers: vec![Arc::new(CountingInit(seen.clone()))],
        ..OrbOptions::default()
    });
    let (_poa, _echo, obj) = echo_in(&orb, "intercepted", &[]);
    let ((), direct, general) = paths(|| {
        ping(&obj).unwrap();
        mix(&obj).unwrap();
    });
    assert_eq!((direct, general), (0, 2));
    assert_eq!(seen.load(Ordering::SeqCst), 2);
    orb.shutdown(true).unwrap();
}

#[test]
fn calls_from_many_threads() {
    let _serial = serial();
    let orb = orb_with(OrbOptions::default());
    let (_poa, _echo, obj) = echo_in(&orb, "threads", &[]);
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let obj = obj.clone();
            thread::spawn(move || {
                for _ in 0..500 {
                    mix(&obj).unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    // Shutting down waits for nothing left behind.
    orb.shutdown(true).unwrap();
}
