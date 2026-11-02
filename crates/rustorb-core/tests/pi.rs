//! Portable Interceptors: initializers, client and server request
//! interceptors, PICurrent, IOR interceptors and the Codec, between two
//! ORBs over TCP and within one (docs/portable-interceptors.md).

use std::sync::{Arc, Mutex};

use rustorb_core::dii::{
    ArgMode, DynamicImplementation, DynamicServant, NamedValue, ServerRequest,
};
use rustorb_core::giop::Version;
use rustorb_core::iop::{Encoding, ServiceContext, TaggedComponent};
use rustorb_core::pi::{
    ClientRequestInfo, ClientRequestInterceptor, DuplicateName, InterceptError, Interceptor,
    IorInfo, IorInterceptor, OrbInitInfo, OrbInitializer, ServerRequestInfo,
    ServerRequestInterceptor, SlotId, reply_status,
};
use rustorb_core::{
    Any, CompletionStatus, EndpointAddr, Exception, InStream, ObjectRef, Orb, OrbOptions, Servant,
    ServerReply, SystemException, SystemExceptionKind, TCKind, TypeCode, Value,
};

const ECHO_ID: &str = "IDL:test/Echo:1.0";
const OOPS_ID: &str = "IDL:test/Oops:1.0";
/// The service context of the tests.
const CTX: u32 = 0x5254_0001;
/// The reply service context of the tests.
const REPLY_CTX: u32 = 0x5254_0002;
/// The tagged component of the tests.
const TAG: u32 = 0x5254_0003;

type Log = Arc<Mutex<Vec<String>>>;

fn log(log: &Log, s: impl Into<String>) {
    log.lock().unwrap().push(s.into());
}

fn take(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.lock().unwrap())
}

fn string_any(s: &str) -> Any {
    Any::new(TypeCode::string(0), Value::String(s.into()))
}

fn any_string(a: &Any) -> Option<String> {
    match a.value() {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn sys(kind: SystemExceptionKind, minor: u32) -> SystemException {
    SystemException::new(kind, minor, CompletionStatus::No)
}

// --- the servant -----------------------------------------------------------------

/// ```idl
/// exception Oops {};
/// interface Echo {
///   string slot();          // slot 0 of PICurrent, as the servant sees it
///   void user() raises (Oops);
///   void system();          // NO_PERMISSION
///   oneway void post();
///   void sleep();           // 300 ms
/// };
/// ```
struct Echo {
    orb: Mutex<Option<Orb>>,
    log: Log,
}

impl Servant for Echo {
    fn repository_id(&self) -> &str {
        ECHO_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        _input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        log(&self.log, format!("servant {operation}"));
        match operation {
            "slot" => {
                let orb = self.orb.lock().unwrap().clone().unwrap();
                let value = orb.pi_current().get_slot(0).unwrap();
                // What the servant sets stays in its thread scope.
                orb.pi_current().set_slot(0, string_any("servant")).unwrap();
                reply
                    .output()
                    .write_string(&any_string(&value).unwrap_or_default())
            }
            "user" => {
                reply.user_exception(OOPS_ID)?;
                Ok(())
            }
            "system" => Err(sys(SystemExceptionKind::NO_PERMISSION, 7)),
            "post" => Ok(()),
            "sleep" => {
                std::thread::sleep(std::time::Duration::from_millis(300));
                Ok(())
            }
            _ => Err(sys(SystemExceptionKind::BAD_OPERATION, 0)),
        }
    }
}

// --- interceptors ----------------------------------------------------------------

/// What a test interceptor does at its points.
#[derive(Default, Clone)]
struct Behavior {
    /// Raise this in `send_request` / `receive_request_service_contexts`.
    fail_start: Option<SystemException>,
    /// Forward in `send_request` / `receive_request`.
    forward: Option<ObjectRef>,
    /// Raise this in `receive_reply` / `send_reply`.
    fail_end: Option<SystemException>,
}

/// A client and server interceptor that logs what it sees, carries slot 0
/// in the service context `CTX` and answers with `REPLY_CTX`.
struct Recorder {
    name: String,
    log: Log,
    slot: SlotId,
    behavior: Mutex<Behavior>,
}

impl Recorder {
    fn behavior(&self) -> Behavior {
        self.behavior.lock().unwrap().clone()
    }

    fn client(&self, point: &str, ri: &ClientRequestInfo<'_>) {
        let mut line = format!("{} {point} {}", self.name, ri.operation());
        if let Ok(status) = ri.reply_status() {
            line += &format!(" status={status}");
        }
        if let Ok(id) = ri.received_exception_id() {
            line += &format!(" ex={id}");
        }
        if let Ok(sc) = ri.get_reply_service_context(REPLY_CTX) {
            line += &format!(" reply_ctx={}", String::from_utf8_lossy(&sc.context_data));
        }
        log(&self.log, line);
    }

    fn server(&self, point: &str, ri: &ServerRequestInfo<'_>) {
        let mut line = format!("{} {point} {}", self.name, ri.operation());
        if let Ok(status) = ri.reply_status() {
            line += &format!(" status={status}");
        }
        if let Ok(id) = ri.sending_exception_id() {
            line += &format!(" ex={id}");
        }
        log(&self.log, line);
    }
}

impl Interceptor for Recorder {
    fn name(&self) -> &str {
        &self.name
    }

    fn destroy(&self) {
        log(&self.log, format!("{} destroy", self.name));
    }
}

impl ClientRequestInterceptor for Recorder {
    fn send_request(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        self.client("send_request", ri);
        let b = self.behavior();
        if let Some(e) = b.fail_start {
            return Err(e.into());
        }
        if let Some(to) = b.forward {
            self.behavior.lock().unwrap().forward = None;
            return Err(InterceptError::ForwardRequest(to));
        }
        if self.name == "A" {
            let data = ri
                .get_slot(self.slot)
                .ok()
                .and_then(|a| any_string(&a))
                .unwrap_or_else(|| "none".into());
            ri.add_request_service_context(
                ServiceContext {
                    context_id: CTX,
                    context_data: data.into_bytes(),
                },
                false,
            )?;
        }
        Ok(())
    }

    fn receive_reply(&self, ri: &ClientRequestInfo<'_>) -> Result<(), SystemException> {
        self.client("receive_reply", ri);
        // Only send_request may add contexts.
        let e = ri
            .add_request_service_context(
                ServiceContext {
                    context_id: CTX + 9,
                    context_data: Vec::new(),
                },
                false,
            )
            .unwrap_err();
        assert!(e.is(SystemExceptionKind::BAD_INV_ORDER));
        match self.behavior().fail_end {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn receive_exception(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        self.client("receive_exception", ri);
        Ok(())
    }

    fn receive_other(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        self.client("receive_other", ri);
        Ok(())
    }
}

impl ServerRequestInterceptor for Recorder {
    fn receive_request_service_contexts(
        &self,
        ri: &ServerRequestInfo<'_>,
    ) -> Result<(), InterceptError> {
        self.server("rrsc", ri);
        // Not known yet at this point.
        assert!(ri.object_id().is_err());
        if let Some(e) = self.behavior().fail_start {
            return Err(e.into());
        }
        if self.name == "A" {
            if let Ok(sc) = ri.get_request_service_context(CTX) {
                let text = String::from_utf8_lossy(&sc.context_data).into_owned();
                ri.set_slot(self.slot, string_any(&format!("from {text}")))
                    .unwrap();
            }
        }
        Ok(())
    }

    fn receive_request(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        self.server("receive_request", ri);
        assert_eq!(ri.target_most_derived_interface().unwrap(), ECHO_ID);
        assert!(ri.target_is_a(ECHO_ID).unwrap());
        assert_eq!(ri.adapter_name().unwrap(), vec!["RootPOA".to_owned()]);
        if let Some(to) = self.behavior().forward {
            self.behavior.lock().unwrap().forward = None;
            return Err(InterceptError::ForwardRequest(to));
        }
        Ok(())
    }

    fn send_reply(&self, ri: &ServerRequestInfo<'_>) -> Result<(), SystemException> {
        self.server("send_reply", ri);
        if self.name == "A" {
            // The servant's own set_slot does not come back here.
            let slot = ri.get_slot(self.slot).ok().and_then(|a| any_string(&a));
            ri.add_reply_service_context(
                ServiceContext {
                    context_id: REPLY_CTX,
                    context_data: slot.unwrap_or_default().into_bytes(),
                },
                false,
            )?;
        }
        match self.behavior().fail_end {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn send_exception(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        self.server("send_exception", ri);
        Ok(())
    }

    fn send_other(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        self.server("send_other", ri);
        Ok(())
    }
}

/// Adds the tagged component `TAG`.
struct Tagger;

impl Interceptor for Tagger {
    fn name(&self) -> &str {
        "tagger"
    }
}

impl IorInterceptor for Tagger {
    fn establish_components(&self, info: &IorInfo<'_>) -> Result<(), SystemException> {
        let name = info.adapter_name().join("/");
        info.add_ior_component(TaggedComponent {
            tag: TAG,
            component_data: name.into_bytes(),
        });
        assert!(
            info.add_ior_component_to_profile(
                TaggedComponent {
                    tag: TAG,
                    component_data: Vec::new()
                },
                99
            )
            .is_err()
        );
        Ok(())
    }
}

/// Adds two recorders, `A` and `B`, of each kind, with one slot.
struct Init {
    log: Log,
    recorders: Mutex<Vec<Arc<Recorder>>>,
    fail: bool,
}

impl OrbInitializer for Init {
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        // Initial references can be resolved only in post_init.
        assert!(info.resolve_initial_references("NameService").is_err());
        let slot = info.allocate_slot_id();
        for name in ["A", "B"] {
            let r = Arc::new(Recorder {
                name: name.into(),
                log: Arc::clone(&self.log),
                slot,
                behavior: Mutex::default(),
            });
            info.add_client_request_interceptor(r.clone()).unwrap();
            info.add_server_request_interceptor(r.clone()).unwrap();
            self.recorders.lock().unwrap().push(r);
        }
        let dup = Arc::new(Recorder {
            name: "A".into(),
            log: Arc::clone(&self.log),
            slot,
            behavior: Mutex::default(),
        });
        assert_eq!(
            info.add_client_request_interceptor(dup).unwrap_err(),
            DuplicateName("A".into())
        );
        info.add_ior_interceptor(Arc::new(Tagger)).unwrap();
        Ok(())
    }

    fn post_init(&self, _info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        if self.fail {
            return Err(sys(SystemExceptionKind::INITIALIZE, 1));
        }
        Ok(())
    }
}

struct Fixture {
    log: Log,
    init: Arc<Init>,
    orb: Orb,
}

impl Fixture {
    fn new(options: OrbOptions) -> Fixture {
        let log = Log::default();
        let init = Arc::new(Init {
            log: Arc::clone(&log),
            recorders: Mutex::default(),
            fail: false,
        });
        let orb = Orb::with_options(OrbOptions {
            endpoints: vec![EndpointAddr {
                host: "127.0.0.1".into(),
                port: 0,
            }],
            initializers: vec![init.clone()],
            ..options
        })
        .unwrap();
        Fixture { log, init, orb }
    }

    fn recorder(&self, name: &str) -> Arc<Recorder> {
        let recorders = self.init.recorders.lock().unwrap();
        Arc::clone(recorders.iter().find(|r| r.name == name).unwrap())
    }

    fn echo(&self) -> ObjectRef {
        let root = self.orb.root_poa().unwrap();
        root.the_poa_manager().activate().unwrap();
        let echo = Arc::new(Echo {
            orb: Mutex::new(Some(self.orb.clone())),
            log: Arc::clone(&self.log),
        });
        let id = root.activate_object(echo).unwrap();
        root.id_to_reference(&id).unwrap()
    }
}

fn remote(client: &Orb, server: &Orb, obj: &ObjectRef) -> ObjectRef {
    client
        .string_to_object(&server.object_to_string(Some(obj)).unwrap())
        .unwrap()
        .unwrap()
}

fn call_slot(obj: &ObjectRef) -> Result<String, Exception> {
    let reply = obj.request("slot").invoke()?;
    Ok(reply.input().read_string()?)
}

/// Client and server on two ORBs, or both on one.
fn pair(colocated: bool) -> (Fixture, Option<Fixture>, ObjectRef) {
    let server = Fixture::new(OrbOptions::default());
    let echo = server.echo();
    if colocated {
        return (server, None, echo);
    }
    let client = Fixture::new(OrbOptions::default());
    let obj = remote(&client.orb, &server.orb, &echo);
    (server, Some(client), obj)
}

fn client_of<'f>(server: &'f Fixture, client: &'f Option<Fixture>) -> &'f Fixture {
    client.as_ref().unwrap_or(server)
}

fn round_trip(colocated: bool) {
    let (server, client, obj) = pair(colocated);
    let c = client_of(&server, &client);
    take(&server.log);
    take(&c.log);

    c.orb.pi_current().set_slot(0, string_any("app")).unwrap();
    assert_eq!(call_slot(&obj).unwrap(), "from app");
    // The servant's set_slot was in its own thread scope, put back after.
    assert_eq!(
        any_string(&c.orb.pi_current().get_slot(0).unwrap()).unwrap(),
        "app"
    );

    let mut events = take(&server.log);
    if !colocated {
        events.extend(take(&c.log));
        events.sort_by_key(|e| order_of(e));
    }
    assert_eq!(
        events,
        vec![
            "A send_request slot",
            "B send_request slot",
            "A rrsc slot",
            "B rrsc slot",
            "A receive_request slot",
            "B receive_request slot",
            "servant slot",
            "B send_reply slot status=0",
            "A send_reply slot status=0",
            "B receive_reply slot status=0 reply_ctx=from app",
            "A receive_reply slot status=0 reply_ctx=from app",
        ]
    );
}

/// Where a line goes in a call, to merge the logs of two ORBs.
fn order_of(e: &str) -> usize {
    let points = [
        "send_request",
        "rrsc",
        "receive_request",
        "servant",
        "send_reply",
        "send_exception",
        "send_other",
        "receive_reply",
        "receive_exception",
        "receive_other",
    ];
    let point = e.split(' ').nth(1).unwrap_or("");
    let point = if e.starts_with("servant") {
        "servant"
    } else {
        point
    };
    points.iter().position(|p| *p == point).unwrap_or(99)
}

#[test]
fn service_contexts_and_slots_over_tcp() {
    round_trip(false);
}

#[test]
fn service_contexts_and_slots_colocated() {
    round_trip(true);
}

#[test]
fn failing_send_request_ends_the_ones_before() {
    let (server, client, obj) = pair(false);
    let c = client.as_ref().unwrap();
    c.recorder("B").behavior.lock().unwrap().fail_start =
        Some(sys(SystemExceptionKind::NO_PERMISSION, 3));
    take(&server.log);
    take(&c.log);
    let e = match call_slot(&obj) {
        Err(Exception::System(e)) => e,
        other => panic!("{other:?}"),
    };
    assert!(e.is(SystemExceptionKind::NO_PERMISSION));
    assert_eq!(e.minor, 3);
    assert_eq!(
        take(&c.log),
        vec![
            "A send_request slot",
            "B send_request slot",
            "A receive_exception slot status=1 ex=IDL:omg.org/CORBA/NO_PERMISSION:1.0",
        ]
    );
    assert!(take(&server.log).is_empty());
}

#[test]
fn failing_receive_request_service_contexts() {
    let (server, _client, obj) = pair(false);
    server.recorder("B").behavior.lock().unwrap().fail_start =
        Some(sys(SystemExceptionKind::NO_PERMISSION, 4));
    take(&server.log);
    let e = match call_slot(&obj) {
        Err(Exception::System(e)) => e,
        other => panic!("{other:?}"),
    };
    assert_eq!(e.minor, 4);
    assert_eq!(
        take(&server.log),
        vec![
            "A rrsc slot",
            "B rrsc slot",
            "A send_exception slot status=1 ex=IDL:omg.org/CORBA/NO_PERMISSION:1.0",
        ]
    );
}

#[test]
fn exceptions_of_the_servant() {
    let (server, client, obj) = pair(false);
    let c = client.as_ref().unwrap();
    take(&server.log);
    take(&c.log);
    match obj.request("user").invoke() {
        Err(Exception::User(e)) => assert_eq!(e.repository_id(), OOPS_ID),
        other => panic!("{other:?}"),
    }
    let server_log = take(&server.log);
    assert!(server_log.contains(&format!("A send_exception user status=2 ex={OOPS_ID}")));
    assert!(take(&c.log).contains(&format!("A receive_exception user status=2 ex={OOPS_ID}")));

    match obj.request("system").invoke() {
        Err(Exception::System(e)) => assert_eq!(e.minor, 7),
        other => panic!("{other:?}"),
    }
    assert!(take(&server.log).contains(
        &"B send_exception system status=1 ex=IDL:omg.org/CORBA/NO_PERMISSION:1.0".to_owned()
    ));
}

#[test]
fn failing_send_reply_replaces_the_reply() {
    let (server, client, obj) = pair(false);
    let c = client.as_ref().unwrap();
    server.recorder("B").behavior.lock().unwrap().fail_end = Some(SystemException::new(
        SystemExceptionKind::NO_RESOURCES,
        5,
        CompletionStatus::Yes,
    ));
    take(&server.log);
    take(&c.log);
    match call_slot(&obj) {
        Err(Exception::System(e)) => {
            assert!(e.is(SystemExceptionKind::NO_RESOURCES));
            assert_eq!(e.completed, CompletionStatus::Yes);
        }
        other => panic!("{other:?}"),
    }
    let log = take(&server.log);
    assert!(log.contains(&"B send_reply slot status=0".to_owned()));
    assert!(log.contains(
        &"A send_exception slot status=1 ex=IDL:omg.org/CORBA/NO_RESOURCES:1.0".to_owned()
    ));
}

#[test]
fn failing_receive_reply_gives_the_rest_an_exception() {
    let (server, client, obj) = pair(false);
    let c = client.as_ref().unwrap();
    c.recorder("B").behavior.lock().unwrap().fail_end =
        Some(sys(SystemExceptionKind::NO_PERMISSION, 6));
    take(&server.log);
    take(&c.log);
    match call_slot(&obj) {
        Err(Exception::System(e)) => assert_eq!(e.minor, 6),
        other => panic!("{other:?}"),
    }
    let log = take(&c.log);
    assert_eq!(
        &log[2..],
        [
            "B receive_reply slot status=0 reply_ctx=from none",
            "A receive_exception slot status=1 ex=IDL:omg.org/CORBA/NO_PERMISSION:1.0 reply_ctx=from none",
        ]
    );
}

#[test]
fn forward_from_send_request() {
    let (server, client, obj) = pair(false);
    let c = client.as_ref().unwrap();
    let other = Fixture::new(OrbOptions::default());
    let other_echo = remote(&c.orb, &other.orb, &other.echo());
    c.recorder("A").behavior.lock().unwrap().forward = Some(other_echo);
    take(&server.log);
    take(&other.log);
    take(&c.log);
    call_slot(&obj).unwrap();
    // The first attempt ends at once (A raised, so it gets no ending point
    // and B never starts); the second goes to the other ORB.
    let log = take(&c.log);
    assert_eq!(log[0], "A send_request slot");
    assert_eq!(log[1], "A send_request slot");
    assert_eq!(log[2], "B send_request slot");
    assert!(take(&server.log).is_empty());
    assert!(take(&other.log).contains(&"servant slot".to_owned()));
}

#[test]
fn forward_from_receive_request() {
    let (server, client, obj) = pair(false);
    let c = client.as_ref().unwrap();
    let other = Fixture::new(OrbOptions::default());
    let other_echo = remote(&server.orb, &other.orb, &other.echo());
    server.recorder("B").behavior.lock().unwrap().forward = Some(other_echo);
    take(&server.log);
    take(&other.log);
    take(&c.log);
    call_slot(&obj).unwrap();
    let log = take(&server.log);
    assert!(!log.contains(&"servant slot".to_owned()));
    assert!(log.contains(&"A send_other slot status=3".to_owned()));
    assert!(take(&c.log).contains(&"A receive_other slot status=3".to_owned()));
    assert!(take(&other.log).contains(&"servant slot".to_owned()));
}

#[test]
fn oneway_ends_with_receive_other() {
    let (server, client, obj) = pair(false);
    let c = client.as_ref().unwrap();
    take(&c.log);
    obj.request("post").send_oneway().unwrap();
    let log = take(&c.log);
    assert_eq!(
        log,
        vec![
            "A send_request post",
            "B send_request post",
            "B receive_other post status=0",
            "A receive_other post status=0",
        ]
    );
    drop(server);
}

#[test]
fn refused_requests_go_through_the_server_interceptors() {
    let (server, client, obj) = pair(false);
    let c = client.as_ref().unwrap();
    let root = server.orb.root_poa().unwrap();
    let id = root.reference_to_id(&server.echo()).unwrap();
    root.deactivate_object(&id).unwrap();
    let gone = remote(
        &c.orb,
        &server.orb,
        &root.create_reference_with_id(&id, ECHO_ID).unwrap(),
    );
    take(&server.log);
    match call_slot(&gone) {
        Err(Exception::System(e)) => assert!(e.is(SystemExceptionKind::OBJECT_NOT_EXIST)),
        other => panic!("{other:?}"),
    }
    let log = take(&server.log);
    assert_eq!(
        log,
        vec![
            "A rrsc slot",
            "B rrsc slot",
            "B send_exception slot status=1 ex=IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0",
            "A send_exception slot status=1 ex=IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0",
        ]
    );
    drop(obj);
}

#[test]
fn timeout_ends_with_receive_exception() {
    let (server, client, obj) = pair(false);
    let c = client.as_ref().unwrap();
    take(&server.log);
    take(&c.log);
    let e = obj
        .request("sleep")
        .timeout(Some(std::time::Duration::from_millis(50)))
        .invoke()
        .unwrap_err();
    assert!(
        matches!(&e, Exception::System(e) if e.is(SystemExceptionKind::TIMEOUT)),
        "{e:?}"
    );
    assert_eq!(
        take(&c.log),
        [
            "A send_request sleep",
            "B send_request sleep",
            "B receive_exception sleep status=1 ex=IDL:omg.org/CORBA/TIMEOUT:1.0",
            "A receive_exception sleep status=1 ex=IDL:omg.org/CORBA/TIMEOUT:1.0",
        ]
    );
}

/// The contexts the ORB writes itself (CodeSets) are not the
/// interceptors': they cannot see them in `send_request` nor add one.
#[test]
fn core_contexts_are_hidden_from_client_interceptors() {
    struct Core(Mutex<Vec<String>>);
    impl Interceptor for Core {
        fn name(&self) -> &str {
            "core"
        }
    }
    impl ClientRequestInterceptor for Core {
        fn send_request(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
            let id = rustorb_core::iop::service_id::CODE_SETS;
            let mut seen = self.0.lock().unwrap();
            let e = ri.get_request_service_context(id).unwrap_err();
            seen.push(format!("get {} {:x}", e.repository_id, e.minor));
            let e = ri
                .add_request_service_context(
                    ServiceContext {
                        context_id: id,
                        context_data: Vec::new(),
                    },
                    true,
                )
                .unwrap_err();
            seen.push(format!("add {} {:x}", e.repository_id, e.minor));
            Ok(())
        }
    }
    struct CoreInit(Arc<Core>);
    impl OrbInitializer for CoreInit {
        fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
            info.add_client_request_interceptor(self.0.clone()).unwrap();
            Ok(())
        }
    }
    let server = Fixture::new(OrbOptions::default());
    let core = Arc::new(Core(Mutex::default()));
    let client = Orb::with_options(OrbOptions {
        initializers: vec![Arc::new(CoreInit(core.clone()))],
        ..OrbOptions::default()
    })
    .unwrap();
    let obj = remote(&client, &server.orb, &server.echo());
    call_slot(&obj).unwrap();
    assert_eq!(
        *core.0.lock().unwrap(),
        [
            "get IDL:omg.org/CORBA/BAD_PARAM:1.0 4f4d001a",
            "add IDL:omg.org/CORBA/BAD_INV_ORDER:1.0 4f4d000f",
        ]
    );
    // The server still gets the code sets: the call above worked.
    let _ = client.destroy();
}

#[test]
fn ior_interceptor_adds_components() {
    let (server, client, obj) = pair(false);
    let _ = &client;
    let profile = obj.ior().profiles[0].iiop().unwrap();
    let found: Vec<_> = profile
        .components
        .iter()
        .filter(|tc| tc.tag == TAG)
        .collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].component_data, b"RootPOA");

    let root = server.orb.root_poa().unwrap();
    let child = root.create_poa("child", None, &[]).unwrap();
    child.the_poa_manager().activate().unwrap();
    let echo = Arc::new(Echo {
        orb: Mutex::new(Some(server.orb.clone())),
        log: Arc::clone(&server.log),
    });
    let id = child.activate_object(echo).unwrap();
    let r = child.id_to_reference(&id).unwrap();
    let profile = r.ior().profiles[0].iiop().unwrap();
    let data: Vec<_> = profile
        .components
        .iter()
        .filter(|tc| tc.tag == TAG)
        .map(|tc| tc.component_data.clone())
        .collect();
    assert_eq!(data, vec![b"RootPOA/child".to_vec()]);
    drop(client);
}

/// Checks `get_effective_component` in the client.
struct ComponentCheck {
    seen: Mutex<Vec<Vec<u8>>>,
}

impl Interceptor for ComponentCheck {
    fn name(&self) -> &str {
        ""
    }
}

impl ClientRequestInterceptor for ComponentCheck {
    fn send_request(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        if let Ok(c) = ri.get_effective_component(TAG) {
            self.seen.lock().unwrap().push(c.component_data);
        }
        assert!(ri.get_effective_component(0x7777_0000).is_err());
        assert!(ri.response_expected());
        assert_eq!(ri.effective_target().ior(), ri.target().ior());
        Ok(())
    }
}

struct CheckInit(Arc<ComponentCheck>);

impl OrbInitializer for CheckInit {
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        // Anonymous interceptors may share their name.
        info.add_client_request_interceptor(self.0.clone()).unwrap();
        info.add_client_request_interceptor(Arc::new(ComponentCheck {
            seen: Mutex::default(),
        }))
        .unwrap();
        Ok(())
    }
}

#[test]
fn effective_components() {
    let server = Fixture::new(OrbOptions::default());
    let echo = server.echo();
    let check = Arc::new(ComponentCheck {
        seen: Mutex::default(),
    });
    let client = Orb::with_options(OrbOptions {
        initializers: vec![Arc::new(CheckInit(check.clone()))],
        ..OrbOptions::default()
    })
    .unwrap();
    let obj = remote(&client, &server.orb, &echo);
    call_slot(&obj).unwrap();
    assert_eq!(*check.seen.lock().unwrap(), vec![b"RootPOA".to_vec()]);
}

#[test]
fn failing_initializer_fails_the_orb() {
    let log = Log::default();
    let init = Arc::new(Init {
        log: Arc::clone(&log),
        recorders: Mutex::default(),
        fail: true,
    });
    let e = Orb::with_options(OrbOptions {
        initializers: vec![init],
        ..OrbOptions::default()
    })
    .unwrap_err();
    assert!(e.is(SystemExceptionKind::INITIALIZE));
    // The interceptors added are destroyed.
    let mut log = take(&log);
    log.sort();
    assert_eq!(
        log,
        vec!["A destroy", "A destroy", "B destroy", "B destroy"]
    );
}

#[test]
fn destroy_destroys_the_interceptors() {
    let f = Fixture::new(OrbOptions::default());
    take(&f.log);
    f.orb.destroy().unwrap();
    let mut log = take(&f.log);
    log.sort();
    assert_eq!(
        log,
        vec!["A destroy", "A destroy", "B destroy", "B destroy"]
    );
}

#[test]
fn pi_current_slots() {
    let f = Fixture::new(OrbOptions::default());
    let current = f.orb.pi_current();
    assert_eq!(current.get_slot(0).unwrap().typecode().kind(), TCKind::Null);
    current.set_slot(0, string_any("x")).unwrap();
    assert_eq!(any_string(&current.get_slot(0).unwrap()).unwrap(), "x");
    assert!(current.get_slot(1).is_err());
    // Another thread has its own.
    let c = current.clone();
    std::thread::spawn(move || {
        assert_eq!(c.get_slot(0).unwrap().typecode().kind(), TCKind::Null);
    })
    .join()
    .unwrap();
    // Another ORB has its own.
    let g = Fixture::new(OrbOptions::default());
    assert_eq!(
        g.orb.pi_current().get_slot(0).unwrap().typecode().kind(),
        TCKind::Null
    );
    // An ORB without initializers has no slots.
    let plain = Orb::with_options(OrbOptions::default()).unwrap();
    assert!(plain.pi_current().get_slot(0).is_err());
}

#[test]
fn reply_contexts_with_giop_1_1() {
    for version in [Version::V1_0, Version::V1_1, Version::V1_2] {
        let server = Fixture::new(OrbOptions::default());
        let echo = server.echo();
        let client = Fixture::new(OrbOptions {
            max_giop_version: version,
            ..OrbOptions::default()
        });
        let obj = remote(&client.orb, &server.orb, &echo);
        client
            .orb
            .pi_current()
            .set_slot(0, string_any("app"))
            .unwrap();
        // "from app" is 8 bytes: the context is 4 + 4 + 8 + 4 (count)
        // bytes, a multiple of 8, so it can be sent in every version.
        assert_eq!(call_slot(&obj).unwrap(), "from app");
        let log = take(&client.log);
        assert!(
            log.contains(&"A receive_reply slot status=0 reply_ctx=from app".to_owned()),
            "{version:?}: {log:?}"
        );
        client
            .orb
            .pi_current()
            .set_slot(0, string_any("app1"))
            .unwrap();
        assert_eq!(call_slot(&obj).unwrap(), "from app1");
        let log = take(&client.log);
        let sent = log.contains(&"A receive_reply slot status=0 reply_ctx=from app1".to_owned());
        // 9 bytes of data move a GIOP 1.0/1.1 body: not sent there.
        assert_eq!(sent, version == Version::V1_2, "{version:?}: {log:?}");
    }
}

// --- DII and DSI -------------------------------------------------------------------

fn long() -> TypeCode {
    TypeCode::simple(TCKind::Long).unwrap()
}

struct Adder;

impl DynamicImplementation for Adder {
    fn primary_interface(&self) -> &str {
        ECHO_ID
    }

    fn invoke(&self, request: &mut ServerRequest) -> Result<(), SystemException> {
        let args = request.arguments(vec![
            NamedValue::new("a", Any::new(long(), Value::Void), ArgMode::In),
            NamedValue::out("b", long()),
        ])?;
        let Value::Long(a) = *args[0].value.value() else {
            unreachable!()
        };
        args[1].value = Any::new(long(), Value::Long(a * 2));
        request.set_result(Any::new(long(), Value::Long(a + 1)))
    }
}

/// Logs the arguments and the result the DII and DSI give.
struct ArgsLog(Log);

impl Interceptor for ArgsLog {
    fn name(&self) -> &str {
        "args"
    }
}

fn values(params: &[rustorb_core::pi::Parameter]) -> String {
    params
        .iter()
        .map(|p| format!("{:?}:{:?}", p.mode, p.argument.value()))
        .collect::<Vec<_>>()
        .join(",")
}

impl ClientRequestInterceptor for ArgsLog {
    fn send_request(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        let args = ri
            .arguments()
            .map(|a| values(&a))
            .unwrap_or_else(|e| e.to_string());
        log(&self.0, format!("client send {args}"));
        assert!(ri.result().is_err());
        Ok(())
    }

    fn receive_reply(&self, ri: &ClientRequestInfo<'_>) -> Result<(), SystemException> {
        let args = ri
            .arguments()
            .map(|a| values(&a))
            .unwrap_or_else(|e| e.to_string());
        let result = ri
            .result()
            .map(|r| format!("{:?}", r.value()))
            .unwrap_or_else(|e| e.to_string());
        log(&self.0, format!("client reply {args} -> {result}"));
        Ok(())
    }
}

impl ServerRequestInterceptor for ArgsLog {
    fn send_reply(&self, ri: &ServerRequestInfo<'_>) -> Result<(), SystemException> {
        let args = ri
            .arguments()
            .map(|a| values(&a))
            .unwrap_or_else(|e| e.to_string());
        let result = ri
            .result()
            .map(|r| format!("{:?}", r.value()))
            .unwrap_or_else(|e| e.to_string());
        log(&self.0, format!("server reply {args} -> {result}"));
        Ok(())
    }
}

struct ArgsInit(Log);

impl OrbInitializer for ArgsInit {
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        let i = Arc::new(ArgsLog(Arc::clone(&self.0)));
        info.add_client_request_interceptor(i.clone()).unwrap();
        info.add_server_request_interceptor(i).unwrap();
        Ok(())
    }
}

#[test]
fn dii_and_dsi_arguments() {
    let log = Log::default();
    let options = || OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        initializers: vec![Arc::new(ArgsInit(Arc::clone(&log)))],
        ..OrbOptions::default()
    };
    let server = Orb::with_options(options()).unwrap();
    let client = Orb::with_options(options()).unwrap();
    let root = server.root_poa().unwrap();
    root.the_poa_manager().activate().unwrap();
    let id = root
        .activate_object(Arc::new(DynamicServant(Adder)))
        .unwrap();
    let obj = remote(&client, &server, &root.id_to_reference(&id).unwrap());

    let mut req = obj.create_request("add");
    req.add_in_arg("a", Any::new(long(), Value::Long(20)))
        .add_out_arg("b", long())
        .set_return_type(long());
    req.invoke().unwrap();
    assert_eq!(req.result().value(), &Value::Long(21));
    let mut seen = take(&log);
    seen.sort();
    assert_eq!(
        seen,
        vec![
            "client reply In:Long(20),Out:Long(40) -> Long(21)",
            "client send In:Long(20),Out:Void",
            "server reply In:Long(20),Out:Long(40) -> Long(21)",
        ]
    );

    // A static call has no arguments to show.
    let reply = obj
        .request("add")
        .args(|out| {
            out.write_i32(1);
            Ok(())
        })
        .invoke()
        .unwrap();
    assert_eq!(reply.input().read::<i32>().unwrap(), 2);
    let log = take(&log);
    assert!(
        log[0].starts_with("client send ") && log[0].contains("NO_RESOURCES"),
        "{log:?}"
    );
}

// --- Codec -------------------------------------------------------------------------

#[test]
fn codec_round_trip() {
    let orb = Orb::with_options(OrbOptions::default()).unwrap();
    let factory = orb.codec_factory();
    assert!(factory.create_codec(Encoding::cdr(1, 3)).is_err());
    let value = Any::new(
        TypeCode::struct_tc(
            "IDL:test/S:1.0",
            "S",
            &[("a", long()), ("s", TypeCode::string(0))],
        )
        .unwrap(),
        Value::List(vec![Value::Long(5), Value::String("x".into())]),
    );
    for minor in 0..=2 {
        let codec = factory.create_codec(Encoding::cdr(1, minor)).unwrap();
        let data = codec.encode(&value).unwrap();
        assert_eq!(codec.decode(&data).unwrap(), value);
        let data = codec.encode_value(&value).unwrap();
        assert_eq!(codec.decode_value(&data, value.typecode()).unwrap(), value);
        assert!(codec.decode(&[]).is_err());
    }
    let wide = Any::new(TypeCode::wstring(0), Value::String("ω".into()));
    let codec10 = factory.create_codec(Encoding::cdr(1, 0)).unwrap();
    assert!(codec10.encode(&wide).is_err());
    let codec12 = factory.create_codec(Encoding::cdr(1, 2)).unwrap();
    let data = codec12.encode(&wide).unwrap();
    assert_eq!(codec12.decode(&data).unwrap(), wide);
}

#[test]
fn orb_id_and_arguments() {
    struct IdCheck(Mutex<Option<(String, Vec<String>)>>);
    impl OrbInitializer for IdCheck {
        fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
            *self.0.lock().unwrap() = Some((info.orb_id().to_owned(), info.arguments().to_vec()));
            Ok(())
        }
    }
    let check = Arc::new(IdCheck(Mutex::default()));
    rustorb_core::pi::register_orb_initializer(check.clone());
    let mut args: Vec<String> = ["prog", "-ORBid", "myorb", "x"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let orb = Orb::init(&mut args).unwrap();
    assert_eq!(args, vec!["prog", "x"]);
    let (id, seen) = check.0.lock().unwrap().clone().unwrap();
    assert_eq!(id, "myorb");
    assert_eq!(seen, vec!["prog", "-ORBid", "myorb", "x"]);
    drop(orb);
    let _ = reply_status::SUCCESSFUL;
}
