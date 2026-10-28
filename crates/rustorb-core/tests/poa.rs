//! The rest of the POA: servant managers, default servants, adapter
//! activators, POACurrent, the SINGLE_THREAD and MAIN_THREAD models, and
//! calls between objects of the same ORB.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use rustorb_core::{
    AdapterActivator, CompletionStatus, Cookie, EndpointAddr, Exception, IdAssignment,
    IdUniqueness, InStream, Lifespan, ObjectRef, Orb, OrbOptions, Poa, PoaCurrent, PoaError,
    Policy, RequestProcessing, Servant, ServantActivator, ServantLocator, ServantManager,
    ServantManagerError, ServantRetention, ServerReply, SystemException, SystemExceptionKind,
    ThreadPolicy, minor,
};

const PROBE_ID: &str = "IDL:test/Probe:1.0";

/// Records what happened, in order, across threads.
#[derive(Default)]
struct Log(Mutex<Vec<String>>);

impl Log {
    fn push(&self, s: impl Into<String>) {
        self.0.lock().unwrap().push(s.into());
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }

    /// Waits until `n` entries are there.
    fn wait_for(&self, n: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.0.lock().unwrap().len() < n && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        self.take()
    }
}

/// A servant that reports where and for which object it runs.
#[derive(Default)]
struct Probe {
    name: String,
    /// Requests running now, and the most seen at once.
    running: AtomicUsize,
    most: AtomicUsize,
    orb: OnceLock<Orb>,
}

impl Probe {
    fn new(name: &str) -> Arc<Probe> {
        Arc::new(Probe {
            name: name.into(),
            ..Probe::default()
        })
    }
}

fn sys(kind: SystemExceptionKind, minor: u32) -> SystemException {
    SystemException::new(kind, minor, CompletionStatus::No)
}

fn system_of(e: Exception) -> SystemException {
    match e {
        Exception::System(e) => e,
        Exception::User(u) => panic!("unexpected user exception {}", u.repository_id()),
    }
}

impl Servant for Probe {
    fn repository_id(&self) -> &str {
        PROBE_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        match operation {
            // "servant name/ObjectId/thread name"
            "who" => {
                let id = PoaCurrent::get_object_id().unwrap();
                let poa = PoaCurrent::get_poa().unwrap();
                let thread = thread::current().name().unwrap_or("").to_owned();
                let s = format!(
                    "{}/{}/{}/{}",
                    self.name,
                    poa.the_name(),
                    String::from_utf8_lossy(&id),
                    thread
                );
                reply.output().write_string(&s)
            }
            "sleep" => {
                let ms = input.read::<u32>()?;
                let now = self.running.fetch_add(1, Ordering::SeqCst) + 1;
                self.most.fetch_max(now, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(u64::from(ms)));
                self.running.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            }
            // Calls `who` on the object given, and returns its answer.
            "relay" => {
                let target = input.read_object()?.unwrap();
                let answer = who(&target).map_err(system_of)?;
                reply.output().write_string(&answer)
            }
            "reference" => {
                let me = PoaCurrent::get_reference().unwrap();
                reply.output().write_object(Some(&me))
            }
            "stop" => self.orb.get().unwrap().shutdown(false),
            _ => Err(sys(
                SystemExceptionKind::BAD_OPERATION,
                minor::UNKNOWN_OPERATION,
            )),
        }
    }
}

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

/// The same reference through another ORB, so that calls go over TCP.
fn remote(client: &Orb, server: &Orb, obj: &ObjectRef) -> ObjectRef {
    client
        .string_to_object(&server.object_to_string(Some(obj)).unwrap())
        .unwrap()
        .unwrap()
}

fn who(obj: &ObjectRef) -> Result<String, Exception> {
    let reply = obj.request("who").invoke()?;
    Ok(reply.input().read_string()?)
}

fn relay(obj: &ObjectRef, to: &ObjectRef) -> Result<String, Exception> {
    let reply = obj
        .request("relay")
        .args(|out| out.write_object(Some(to)))
        .invoke()?;
    Ok(reply.input().read_string()?)
}

fn sleep(obj: &ObjectRef, ms: u32) -> Result<(), Exception> {
    obj.request("sleep")
        .args(move |out| {
            out.write_u32(ms);
            Ok(())
        })
        .invoke()
        .map(|_| ())
}

fn user_poa(root: &Poa, name: &str, more: &[Policy]) -> Poa {
    let mut policies = vec![
        Policy::Lifespan(Lifespan::Persistent),
        Policy::IdAssignment(IdAssignment::User),
    ];
    policies.extend_from_slice(more);
    root.create_poa(name, Some(&root.the_poa_manager()), &policies)
        .unwrap()
}

fn named<T: Send + 'static>(name: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
    thread::Builder::new()
        .name(name.into())
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn calls_within_the_orb_run_on_the_calling_thread() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    let poa = user_poa(&root, "p", &[]);
    poa.activate_object_with_id(b"a", Probe::new("A")).unwrap();
    poa.activate_object_with_id(b"b", Probe::new("B")).unwrap();
    root.the_poa_manager().activate().unwrap();
    let a = poa.id_to_reference(b"a").unwrap();
    let b = poa.id_to_reference(b"b").unwrap();

    let local = a.clone();
    assert_eq!(
        named("caller", move || who(&local).unwrap()),
        "A/p/a/caller"
    );
    // A servant calling another object of its ORB: on the servant's thread.
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let ra = remote(&client, &orb, &a);
    assert_eq!(who(&ra).unwrap(), "A/p/a/rustorb-upcall");
    assert_eq!(relay(&ra, &b).unwrap(), "B/p/b/rustorb-upcall");
    let (la, lb) = (a.clone(), b.clone());
    assert_eq!(
        named("caller", move || relay(&la, &lb).unwrap()),
        "B/p/b/caller"
    );
    // Holding: the colocated call waits for the manager like a remote one.
    root.the_poa_manager().hold_requests(false).unwrap();
    let (tx, rx) = mpsc::channel();
    let held = a.clone();
    thread::spawn(move || tx.send(who(&held)).unwrap());
    assert!(rx.recv_timeout(Duration::from_millis(200)).is_err());
    root.the_poa_manager().activate().unwrap();
    assert!(rx.recv_timeout(Duration::from_secs(5)).unwrap().is_ok());

    // `PoaCurrent::get_reference` gives the target, of the same ORB.
    let reply = a.request("reference").invoke().unwrap();
    let me = reply.input().read_object().unwrap().unwrap();
    assert!(me.is_equivalent(&a));
    assert_eq!(PoaCurrent::get_poa().unwrap_err(), PoaError::NoContext);
    orb.shutdown(true).unwrap();
}

/// A servant activator that logs its calls and forwards the id "away".
struct Activator {
    log: Arc<Log>,
    forward_to: OnceLock<ObjectRef>,
    delay: Duration,
}

impl ServantActivator for Activator {
    fn incarnate(
        &self,
        id: &[u8],
        _adapter: &Poa,
    ) -> Result<Arc<dyn Servant>, ServantManagerError> {
        let id = String::from_utf8_lossy(id).into_owned();
        self.log.push(format!("incarnate {id}"));
        thread::sleep(self.delay);
        match id.as_str() {
            "away" => Err(ServantManagerError::ForwardRequest(
                self.forward_to.get().unwrap().clone(),
            )),
            "none" => Err(sys(SystemExceptionKind::OBJECT_NOT_EXIST, 0).into()),
            _ => Ok(Probe::new(&format!("inc-{id}"))),
        }
    }

    fn etherealize(
        &self,
        id: &[u8],
        _adapter: &Poa,
        _servant: Arc<dyn Servant>,
        cleanup_in_progress: bool,
        remaining_activations: bool,
    ) {
        self.log.push(format!(
            "etherealize {} {cleanup_in_progress} {remaining_activations}",
            String::from_utf8_lossy(id)
        ));
    }
}

#[test]
fn servant_activators_incarnate_once_and_etherealize_after_deactivation() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    let poa = user_poa(
        &root,
        "activated",
        &[Policy::RequestProcessing(
            RequestProcessing::UseServantManager,
        )],
    );
    let log = Arc::new(Log::default());
    let activator = Arc::new(Activator {
        log: Arc::clone(&log),
        forward_to: OnceLock::new(),
        delay: Duration::from_millis(100),
    });
    // Without a servant manager: OBJ_ADAPTER.
    let x = poa.create_reference_with_id(b"x", PROBE_ID).unwrap();
    root.the_poa_manager().activate().unwrap();
    let e = system_of(who(&x).unwrap_err());
    assert!(e.is(SystemExceptionKind::OBJ_ADAPTER));
    assert_eq!(e.minor, minor::NO_SERVANT_MANAGER);

    poa.set_servant_manager(ServantManager::Activator(activator.clone()))
        .unwrap();
    let again = poa.set_servant_manager(ServantManager::Activator(activator.clone()));
    assert!(
        matches!(again, Err(PoaError::System(e)) if e.minor == minor::SERVANT_MANAGER_ALREADY_SET)
    );

    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let rx_ = remote(&client, &orb, &x);
    // LocateRequest does not incarnate.
    assert!(rx_.locate().unwrap());
    assert!(log.take().is_empty());

    // Four requests at once: one incarnate, and all run on its servant.
    let calls: Vec<_> = (0..4)
        .map(|_| {
            let r = rx_.clone();
            thread::spawn(move || who(&r).unwrap())
        })
        .collect();
    for call in calls {
        assert!(call.join().unwrap().starts_with("inc-x/activated/x/"));
    }
    assert_eq!(log.take(), ["incarnate x"]);
    assert!(poa.id_to_servant(b"x").is_ok());

    // Deactivation while a request runs: etherealize once it is done.
    let busy = rx_.clone();
    let running = thread::spawn(move || sleep(&busy, 300));
    thread::sleep(Duration::from_millis(100));
    poa.deactivate_object(b"x").unwrap();
    assert!(log.take().is_empty());
    running.join().unwrap().unwrap();
    assert_eq!(log.wait_for(1), ["etherealize x false false"]);
    // The next request incarnates again.
    assert!(who(&rx_).unwrap().starts_with("inc-x/"));
    assert_eq!(log.take(), ["incarnate x"]);

    // ForwardRequest is LOCATION_FORWARD, and system exceptions pass.
    let other = user_poa(&root, "other", &[]);
    other
        .activate_object_with_id(b"o", Probe::new("O"))
        .unwrap();
    let _ = activator
        .forward_to
        .set(other.id_to_reference(b"o").unwrap());
    let away = remote(
        &client,
        &orb,
        &poa.create_reference_with_id(b"away", PROBE_ID).unwrap(),
    );
    assert!(who(&away).unwrap().starts_with("O/other/o/"));
    let none = poa.create_reference_with_id(b"none", PROBE_ID).unwrap();
    let e = system_of(who(&remote(&client, &orb, &none)).unwrap_err());
    assert!(e.is(SystemExceptionKind::OBJECT_NOT_EXIST));
    assert_eq!(e.minor, 0);
    log.take();

    // destroy(etherealize, wait): every active object, cleanup in progress.
    let y = poa.create_reference_with_id(b"y", PROBE_ID).unwrap();
    who(&y).unwrap();
    log.take();
    poa.destroy(true, true).unwrap();
    assert_eq!(
        log.take(),
        ["etherealize x true false", "etherealize y true false"]
    );
    orb.shutdown(true).unwrap();
}

/// A servant locator with one servant per ObjectId, which refuses "bad".
struct Locator {
    log: Arc<Log>,
}

impl ServantLocator for Locator {
    fn preinvoke(
        &self,
        id: &[u8],
        _adapter: &Poa,
        operation: &str,
    ) -> Result<(Arc<dyn Servant>, Cookie), ServantManagerError> {
        let id = String::from_utf8_lossy(id).into_owned();
        self.log.push(format!("preinvoke {id} {operation}"));
        if id == "bad" {
            return Err(sys(SystemExceptionKind::NO_PERMISSION, 9).into());
        }
        Ok((Probe::new(&format!("loc-{id}")), Box::new(id.len())))
    }

    fn postinvoke(
        &self,
        id: &[u8],
        _adapter: &Poa,
        operation: &str,
        cookie: Cookie,
        _servant: Arc<dyn Servant>,
    ) {
        let cookie = cookie.downcast::<usize>().unwrap();
        self.log.push(format!(
            "postinvoke {} {operation} {cookie}",
            String::from_utf8_lossy(id)
        ));
    }
}

#[test]
fn servant_locators_are_asked_for_every_request() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    let poa = user_poa(
        &root,
        "located",
        &[
            Policy::ServantRetention(ServantRetention::NonRetain),
            Policy::RequestProcessing(RequestProcessing::UseServantManager),
        ],
    );
    let log = Arc::new(Log::default());
    // An activator does not fit NON_RETAIN.
    let wrong = Arc::new(Activator {
        log: Arc::clone(&log),
        forward_to: OnceLock::new(),
        delay: Duration::ZERO,
    });
    assert!(matches!(
        poa.set_servant_manager(ServantManager::Activator(wrong)),
        Err(PoaError::System(e)) if e.is(SystemExceptionKind::OBJ_ADAPTER)
    ));
    poa.set_servant_manager(ServantManager::Locator(Arc::new(Locator {
        log: Arc::clone(&log),
    })))
    .unwrap();
    assert_eq!(
        poa.activate_object_with_id(b"z", Probe::new("Z")),
        Err(PoaError::WrongPolicy)
    );
    root.the_poa_manager().activate().unwrap();

    let obj = poa.create_reference_with_id(b"abc", PROBE_ID).unwrap();
    for _ in 0..2 {
        assert!(who(&obj).unwrap().starts_with("loc-abc/located/abc/"));
    }
    assert_eq!(
        log.take(),
        [
            "preinvoke abc who",
            "postinvoke abc who 3",
            "preinvoke abc who",
            "postinvoke abc who 3"
        ]
    );
    // An exception from preinvoke goes to the client; no postinvoke.
    let bad = poa.create_reference_with_id(b"bad", PROBE_ID).unwrap();
    let e = system_of(who(&bad).unwrap_err());
    assert!(e.is(SystemExceptionKind::NO_PERMISSION));
    assert_eq!(e.minor, 9);
    assert_eq!(log.take(), ["preinvoke bad who"]);
    // Nor does LocateRequest ask the locator.
    assert!(bad.locate().unwrap());
    assert!(log.take().is_empty());
    orb.shutdown(true).unwrap();
}

#[test]
fn default_servants_serve_any_object_id() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    let poa = user_poa(
        &root,
        "defaulted",
        &[
            Policy::RequestProcessing(RequestProcessing::UseDefaultServant),
            Policy::IdUniqueness(IdUniqueness::Multiple),
        ],
    );
    root.the_poa_manager().activate().unwrap();
    let obj = poa.create_reference_with_id(b"any", PROBE_ID).unwrap();
    assert!(matches!(poa.get_servant(), Err(PoaError::NoServant)));
    let e = system_of(who(&obj).unwrap_err());
    assert!(e.is(SystemExceptionKind::OBJ_ADAPTER));
    assert_eq!(e.minor, minor::NO_DEFAULT_SERVANT);

    let servant = Probe::new("D");
    poa.set_servant(servant.clone()).unwrap();
    assert!(who(&obj).unwrap().starts_with("D/defaulted/any/"));
    let other = poa.create_reference_with_id(b"other", PROBE_ID).unwrap();
    assert!(who(&other).unwrap().starts_with("D/defaulted/other/"));
    // The active object map comes first.
    poa.activate_object_with_id(b"other", Probe::new("E"))
        .unwrap();
    assert!(who(&other).unwrap().starts_with("E/"));
    let as_dyn: Arc<dyn Servant> = servant;
    assert!(Arc::ptr_eq(&poa.id_to_servant(b"zzz").unwrap(), &as_dyn));
    assert_eq!(poa.servant_to_id(&as_dyn), Err(PoaError::ServantNotActive));
    orb.shutdown(true).unwrap();
}

/// Creates the child "made" (and refuses any other name), logging calls.
struct Adapters {
    log: Arc<Log>,
}

impl AdapterActivator for Adapters {
    fn unknown_adapter(&self, parent: &Poa, name: &str) -> bool {
        self.log.push(format!("unknown_adapter {name}"));
        if name != "made" {
            return false;
        }
        let poa = user_poa(parent, name, &[]);
        poa.activate_object_with_id(b"m", Probe::new("M")).unwrap();
        true
    }
}

#[test]
fn adapter_activators_create_child_poas_on_demand() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    let parent = user_poa(&root, "parent", &[]);
    let log = Arc::new(Log::default());
    parent
        .set_the_activator(Some(Arc::new(Adapters {
            log: Arc::clone(&log),
        })))
        .unwrap();
    root.the_poa_manager().activate().unwrap();
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let port = orb.endpoints()[0].1;
    let url = |key: &str| format!("corbaloc::127.0.0.1:{port}/{key}");

    // Over the network: the request waits for the new POA.
    let made = client
        .string_to_object(&url("%FFparent%FFmade%00m"))
        .unwrap()
        .unwrap();
    assert!(made.locate().unwrap());
    assert_eq!(log.take(), ["unknown_adapter made"]);
    assert!(who(&made).unwrap().starts_with("M/made/m/"));
    assert!(log.take().is_empty());

    let missing = client
        .string_to_object(&url("%FFparent%FFnope%00m"))
        .unwrap()
        .unwrap();
    let e = system_of(who(&missing).unwrap_err());
    assert!(e.is(SystemExceptionKind::OBJECT_NOT_EXIST));
    assert_eq!(e.minor, minor::NO_SUCH_OBJECT);
    assert!(!missing.locate().unwrap());
    assert_eq!(log.take(), ["unknown_adapter nope", "unknown_adapter nope"]);

    // A destroyed child is created again, here through find_poa.
    parent
        .find_poa("made", false)
        .unwrap()
        .destroy(false, true)
        .unwrap();
    assert_eq!(
        parent.find_poa("made", false).unwrap_err(),
        PoaError::AdapterNonExistent
    );
    assert_eq!(parent.find_poa("made", true).unwrap().the_name(), "made");
    assert_eq!(log.take(), ["unknown_adapter made"]);
    orb.shutdown(true).unwrap();
}

#[test]
fn single_thread_poas_run_one_request_at_a_time_and_allow_reentry() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    let single = user_poa(
        &root,
        "single",
        &[Policy::Thread(ThreadPolicy::SingleThread)],
    );
    let probe = Probe::new("S");
    single.activate_object_with_id(b"s", probe.clone()).unwrap();
    single
        .activate_object_with_id(b"t", Probe::new("T"))
        .unwrap();
    root.the_poa_manager().activate().unwrap();
    let s = single.id_to_reference(b"s").unwrap();
    let t = single.id_to_reference(b"t").unwrap();

    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let rs = remote(&client, &orb, &s);
    let started = Instant::now();
    let calls: Vec<_> = (0..4)
        .map(|_| {
            let r = rs.clone();
            thread::spawn(move || sleep(&r, 100).unwrap())
        })
        .collect();
    for call in calls {
        call.join().unwrap();
    }
    assert_eq!(probe.most.load(Ordering::SeqCst), 1);
    assert!(started.elapsed() >= Duration::from_millis(400));

    // A servant of the POA calling another object of the same POA.
    assert!(
        relay(&rs, &t)
            .unwrap()
            .starts_with("T/single/t/rustorb-upcall")
    );
    // Colocated calls from two threads are still one at a time.
    probe.most.store(0, Ordering::SeqCst);
    let calls: Vec<_> = (0..3)
        .map(|_| {
            let s = s.clone();
            thread::spawn(move || sleep(&s, 50).unwrap())
        })
        .collect();
    for call in calls {
        call.join().unwrap();
    }
    assert_eq!(probe.most.load(Ordering::SeqCst), 1);
    orb.shutdown(true).unwrap();
}

#[test]
fn main_thread_poas_run_on_the_thread_in_orb_run() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    let main = user_poa(&root, "main", &[Policy::Thread(ThreadPolicy::MainThread)]);
    let probe = Probe::new("M");
    let _ = probe.orb.set(orb.clone());
    main.activate_object_with_id(b"m", probe.clone()).unwrap();
    main.activate_object_with_id(b"n", Probe::new("N")).unwrap();
    root.the_poa_manager().activate().unwrap();
    let m = main.id_to_reference(b"m").unwrap();
    let n = main.id_to_reference(b"n").unwrap();
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let rm = remote(&client, &orb, &m);

    // Nobody runs the main loop yet: the request waits.
    let (tx, rx) = mpsc::channel();
    let waiting = rm.clone();
    thread::spawn(move || tx.send(who(&waiting).unwrap()).unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    while !orb.work_pending().unwrap() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(5));
    }
    assert!(rx.try_recv().is_err());
    // perform_work runs it on this thread.
    let me = thread::current().name().unwrap_or("").to_owned();
    orb.perform_work().unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        format!("M/main/m/{me}")
    );
    assert!(!orb.work_pending().unwrap());

    // With a thread in run(): remote requests, and colocated calls from a
    // main thread servant, run there.
    let runner = {
        let orb = orb.clone();
        thread::Builder::new()
            .name("main-loop".into())
            .spawn(move || orb.run())
            .unwrap()
    };
    assert_eq!(who(&rm).unwrap(), "M/main/m/main-loop");
    assert_eq!(relay(&rm, &n).unwrap(), "N/main/n/main-loop");
    let local = m.clone();
    assert_eq!(
        named("other", move || who(&local).unwrap()),
        "M/main/m/main-loop"
    );
    rm.request("stop").invoke().unwrap();
    runner.join().unwrap();
    assert!(orb.work_pending().is_err());
}
