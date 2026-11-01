//! The server side: POAs and servants called by the rustorb client and by
//! hand-built GIOP messages on localhost.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex, OnceLock, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use rustorb_core::cdr::ByteOrder;
use rustorb_core::giop::{
    IiopProfile, Ior, LocateRequestHeader, LocateStatus, Message, MessageEncoder, MessageHeader,
    ReplyStatus, RequestHeader, SystemExceptionBody, TargetAddress, Version, decode_message,
    response_flags,
};
use rustorb_core::{
    CompletionStatus, EndpointAddr, Exception, IdAssignment, IdUniqueness, ImplicitActivation,
    InStream, Lifespan, ObjectRef, Orb, OrbOptions, Poa, PoaError, PoaManagerState, Policy,
    RequestProcessing, Servant, ServantRetention, ServerReply, SystemException,
    SystemExceptionKind, minor,
};

const TEST_ID: &str = "IDL:test/Test:1.0";
const OOPS_ID: &str = "IDL:test/Oops:1.0";

/// A servant with one operation per behaviour under test.
#[derive(Default)]
struct TestServant {
    orb: OnceLock<Orb>,
    this: OnceLock<ObjectRef>,
    barrier: OnceLock<Arc<Barrier>>,
    posted: Mutex<Vec<String>>,
    calls: AtomicUsize,
    threads: Mutex<Vec<thread::ThreadId>>,
}

fn sys(kind: SystemExceptionKind, minor: u32) -> SystemException {
    SystemException::new(kind, minor, CompletionStatus::No)
}

impl Servant for TestServant {
    fn repository_id(&self) -> &str {
        TEST_ID
    }

    fn is_a(&self, id: &str) -> bool {
        id == TEST_ID || id == "IDL:test/Base:1.0"
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.threads.lock().unwrap().push(thread::current().id());
        match operation {
            "echo_long" => {
                let v = input.read::<i32>()?;
                reply.output().write_i32(v);
                Ok(())
            }
            "echo_string" => {
                let s = input.read_string()?;
                reply.output().write_string(&s)
            }
            "echo_wstring" => {
                let s = input.read_wstring()?;
                reply.output().write_wstring(&s)
            }
            "fail_user" => {
                let mut out = reply.user_exception(OOPS_ID)?;
                out.write_i32(7);
                Ok(())
            }
            "fail_system" => Err(sys(SystemExceptionKind::NO_PERMISSION, 5)),
            "panic" => panic!("servant panics"),
            "wait" => {
                self.barrier.get().unwrap().wait();
                Ok(())
            }
            "sleep" => {
                let ms = input.read::<u32>()?;
                thread::sleep(Duration::from_millis(u64::from(ms)));
                reply.output().write_u32(ms);
                Ok(())
            }
            "thread_name" => {
                let name = thread::current().name().unwrap_or("").to_owned();
                reply.output().write_string(&name)
            }
            "call_self" => {
                let v = input.read::<i32>()?;
                let this = self.this.get().unwrap();
                let got = call_long(this, v).map_err(|e| match e {
                    Exception::System(e) => e,
                    Exception::User(_) => sys(SystemExceptionKind::UNKNOWN, 0),
                })?;
                reply.output().write_i32(got);
                Ok(())
            }
            "post" => {
                let s = input.read_string()?;
                self.posted.lock().unwrap().push(s);
                Ok(())
            }
            "stop" => self.orb.get().unwrap().shutdown(false),
            "stop_and_wait" => self.orb.get().unwrap().shutdown(true),
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

/// A PERSISTENT / USER_ID POA named `test` with `servant` active as `obj`,
/// and its manager activated.
fn serve(orb: &Orb, servant: Arc<TestServant>) -> (Poa, ObjectRef) {
    let _ = servant.orb.set(orb.clone());
    let root = orb.root_poa().unwrap();
    let poa = root
        .create_poa(
            "test",
            Some(&root.the_poa_manager()),
            &[
                Policy::Lifespan(Lifespan::Persistent),
                Policy::IdAssignment(IdAssignment::User),
            ],
        )
        .unwrap();
    poa.activate_object_with_id(b"obj", servant.clone())
        .unwrap();
    let obj = poa.id_to_reference(b"obj").unwrap();
    let _ = servant.this.set(obj.clone());
    root.the_poa_manager().activate().unwrap();
    (poa, obj)
}

fn call_long(obj: &ObjectRef, v: i32) -> Result<i32, Exception> {
    let reply = obj
        .request("echo_long")
        .args(move |out| {
            out.write_i32(v);
            Ok(())
        })
        .invoke()?;
    Ok(reply.input().read::<i32>()?)
}

fn call_unit(obj: &ObjectRef, op: &str) -> Result<(), Exception> {
    obj.request(op).invoke().map(|_| ())
}

fn system(result: Result<impl std::fmt::Debug, Exception>) -> SystemException {
    match result {
        Err(Exception::System(e)) => e,
        other => panic!("expected a system exception, got {other:?}"),
    }
}

fn key_of(obj: &ObjectRef) -> Vec<u8> {
    obj.ior()
        .iiop_profiles()
        .next()
        .unwrap()
        .unwrap()
        .object_key
}

/// A reference to the same endpoint as `obj` with another key.
fn with_key(orb: &Orb, obj: &ObjectRef, key: &[u8]) -> ObjectRef {
    let mut profile: IiopProfile = obj.ior().iiop_profiles().next().unwrap().unwrap();
    profile.object_key = key.to_vec();
    let ior = Ior {
        type_id: String::new(),
        profiles: vec![profile.to_tagged_profile(ByteOrder::Big).unwrap()],
    };
    orb.string_to_object(&ior.to_ior_string(ByteOrder::Big).unwrap())
        .unwrap()
        .unwrap()
}

#[test]
fn requests_reach_the_servant() {
    let orb = server_orb();
    let servant = Arc::new(TestServant::default());
    let (_poa, obj) = serve(&orb, servant.clone());
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let remote = client
        .string_to_object(&orb.object_to_string(Some(&obj)).unwrap())
        .unwrap()
        .unwrap();

    assert_eq!(remote.type_id(), TEST_ID);
    assert_eq!(call_long(&remote, -42).unwrap(), -42);
    let reply = remote
        .request("echo_string")
        .args(|out| out.write_string("caf\u{e9} \u{3042}"))
        .invoke()
        .unwrap();
    assert_eq!(reply.input().read_string().unwrap(), "caf\u{e9} \u{3042}");

    assert!(remote.is_a(TEST_ID).unwrap());
    assert!(remote.is_a("IDL:test/Base:1.0").unwrap());
    assert!(remote.is_a("IDL:omg.org/CORBA/Object:1.0").unwrap());
    assert!(!remote.is_a("IDL:test/Other:1.0").unwrap());
    assert!(!remote.non_existent().unwrap());
    assert!(remote.locate().unwrap());
    let reply = remote.request("_repository_id").invoke().unwrap();
    assert_eq!(reply.input().read_string().unwrap(), TEST_ID);
    let e = system(call_unit(&remote, "_interface"));
    assert!(e.is(SystemExceptionKind::INTF_REPOS));

    // Servants run on the upcall threads, not on the I/O threads.
    let reply = remote.request("thread_name").invoke().unwrap();
    assert_eq!(reply.input().read_string().unwrap(), "rustorb-upcall");
    orb.shutdown(true).unwrap();
}

#[test]
fn exceptions_go_back_to_the_client() {
    let orb = server_orb();
    let (_poa, obj) = serve(&orb, Arc::new(TestServant::default()));

    match call_unit(&obj, "fail_user") {
        Err(Exception::User(u)) => {
            assert_eq!(u.repository_id(), OOPS_ID);
            assert_eq!(u.input().unwrap().read::<i32>().unwrap(), 7);
        }
        other => panic!("expected Oops, got {other:?}"),
    }
    let e = system(call_unit(&obj, "fail_system"));
    assert!(e.is(SystemExceptionKind::NO_PERMISSION));
    assert_eq!((e.minor, e.completed), (5, CompletionStatus::No));

    let e = system(call_unit(&obj, "no_such_operation"));
    assert!(e.is(SystemExceptionKind::BAD_OPERATION));
    assert_eq!(e.minor, minor::UNKNOWN_OPERATION);

    // Missing arguments: MARSHAL, not completed.
    let e = system(call_unit(&obj, "echo_long"));
    assert!(e.is(SystemExceptionKind::MARSHAL));
    assert_eq!(e.completed, CompletionStatus::No);

    // A panic is UNKNOWN, and the servant thread goes on.
    let e = system(call_unit(&obj, "panic"));
    assert!(e.is(SystemExceptionKind::UNKNOWN));
    assert_eq!(e.completed, CompletionStatus::Maybe);
    assert_eq!(call_long(&obj, 3).unwrap(), 3);
    orb.shutdown(true).unwrap();
}

#[test]
fn object_keys() {
    let orb = server_orb();
    let (poa, obj) = serve(&orb, Arc::new(TestServant::default()));
    assert_eq!(key_of(&obj), b"\xfftest\x00obj");

    let nested = poa
        .create_poa("nested", None, &[Policy::Lifespan(Lifespan::Persistent)])
        .unwrap();
    let id = nested
        .activate_object(Arc::new(TestServant::default()))
        .unwrap();
    // System ids: the boot id and a serial number.
    assert_eq!(id.len(), 12);
    let key = key_of(&nested.id_to_reference(&id).unwrap());
    let mut want = b"\xfftest\xffnested\x00".to_vec();
    want.extend_from_slice(&id);
    assert_eq!(key, want);

    let root = orb.root_poa().unwrap();
    let transient = root.create_poa("transient", None, &[]).unwrap();
    let id = transient
        .activate_object(Arc::new(TestServant::default()))
        .unwrap();
    let key = key_of(&transient.id_to_reference(&id).unwrap());
    assert!(key.starts_with(b"\xfftransient\xfe"));
    let boot = &key[b"\xfftransient\xfe".len()..][..8];
    assert_eq!(&id[..8], boot);
    assert_eq!(&key[key.len() - 12..], &id[..]);

    // The RootPOA has IMPLICIT_ACTIVATION.
    let servant: Arc<dyn Servant> = Arc::new(TestServant::default());
    let root_ref = root.servant_to_reference(&servant).unwrap();
    let key = key_of(&root_ref);
    assert_eq!(key[0], 0xfe);
    assert_eq!(&key[1..9], boot);
    assert_eq!(call_long(&root_ref, 9).unwrap(), 9);
    // The same servant gets the same id again (UNIQUE_ID).
    assert!(root_ref.is_equivalent(&root.servant_to_reference(&servant).unwrap()));
    assert_eq!(
        root.reference_to_id(&root_ref).unwrap(),
        root.servant_to_id(&servant).unwrap()
    );
    assert_eq!(
        transient.reference_to_id(&root_ref),
        Err(PoaError::WrongAdapter)
    );
    orb.shutdown(true).unwrap();
}

#[test]
fn unknown_objects_do_not_exist() {
    let orb = server_orb();
    let (poa, obj) = serve(&orb, Arc::new(TestServant::default()));
    let unactivated = poa.create_reference_with_id(b"other", TEST_ID).unwrap();
    let transient = orb.root_poa().unwrap().create_poa("t", None, &[]).unwrap();
    transient.the_poa_manager().activate().unwrap();
    let id = transient
        .activate_object(Arc::new(TestServant::default()))
        .unwrap();
    let mut stale = key_of(&transient.id_to_reference(&id).unwrap());
    // Another boot id, as from an earlier run of the server.
    stale[b"\xfft\xfe".len()] ^= 0xff;

    let keys: Vec<Vec<u8>> = vec![
        b"\xffnone\x00obj".to_vec(),
        b"\xfftest\xffnone\x00obj".to_vec(),
        b"\xfftest\xfeobj-without-boot-id".to_vec(),
        b"\xfftest".to_vec(),
        b"NameService".to_vec(),
        Vec::new(),
        stale,
    ];
    let mut targets: Vec<_> = keys.iter().map(|k| with_key(&orb, &obj, k)).collect();
    targets.push(unactivated);
    for target in &targets {
        let e = system(call_long(target, 1));
        assert!(e.is(SystemExceptionKind::OBJECT_NOT_EXIST), "{e}");
        assert_eq!(e.minor, minor::NO_SUCH_OBJECT);
        assert_eq!(e.completed, CompletionStatus::No);
        assert!(!target.locate().unwrap());
        assert!(target.non_existent().unwrap());
    }

    poa.deactivate_object(b"obj").unwrap();
    assert!(system(call_long(&obj, 1)).is(SystemExceptionKind::OBJECT_NOT_EXIST));
    assert_eq!(
        poa.deactivate_object(b"obj"),
        Err(PoaError::ObjectNotActive)
    );
    orb.shutdown(true).unwrap();
}

#[test]
fn destroyed_poas_refuse_requests() {
    let orb = server_orb();
    let (poa, obj) = serve(&orb, Arc::new(TestServant::default()));
    let root = orb.root_poa().unwrap();
    assert_eq!(root.find_poa("test", false).unwrap(), poa);
    poa.destroy(true, true).unwrap();
    assert_eq!(
        root.find_poa("test", false),
        Err(PoaError::AdapterNonExistent)
    );
    assert!(system(call_long(&obj, 1)).is(SystemExceptionKind::OBJECT_NOT_EXIST));
    assert!(!obj.locate().unwrap());

    // The name can be used again, and PERSISTENT keys work with the new POA.
    let (_poa, again) = serve(&orb, Arc::new(TestServant::default()));
    assert_eq!(key_of(&again), key_of(&obj));
    assert_eq!(call_long(&obj, 2).unwrap(), 2);
    orb.shutdown(true).unwrap();
}

#[test]
fn poa_operations_check_their_arguments() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    assert_eq!(root.the_name(), "RootPOA");
    assert!(root.the_parent().is_none());
    let child = root.create_poa("child", None, &[]).unwrap();
    assert_eq!(child.the_parent().unwrap(), root);
    assert_ne!(child.the_poa_manager(), root.the_poa_manager());
    assert_eq!(
        root.create_poa("child", None, &[]),
        Err(PoaError::AdapterAlreadyExists)
    );
    assert!(matches!(
        root.create_poa("bad\0name", None, &[]),
        Err(PoaError::System(_))
    ));
    for (policies, index) in [
        (
            vec![
                Policy::IdAssignment(IdAssignment::User),
                Policy::ImplicitActivation(ImplicitActivation::Implicit),
            ],
            1,
        ),
        (
            vec![Policy::ServantRetention(ServantRetention::NonRetain)],
            0,
        ),
        (
            vec![
                Policy::Lifespan(Lifespan::Persistent),
                Policy::RequestProcessing(RequestProcessing::UseDefaultServant),
            ],
            1,
        ),
    ] {
        assert_eq!(
            root.create_poa("invalid", None, &policies),
            Err(PoaError::InvalidPolicy(index))
        );
    }

    let user = root
        .create_poa("user", None, &[Policy::IdAssignment(IdAssignment::User)])
        .unwrap();
    let servant: Arc<dyn Servant> = Arc::new(TestServant::default());
    assert_eq!(
        user.activate_object(Arc::clone(&servant)),
        Err(PoaError::WrongPolicy)
    );
    user.activate_object_with_id(b"a", Arc::clone(&servant))
        .unwrap();
    assert_eq!(
        user.activate_object_with_id(b"a", Arc::new(TestServant::default())),
        Err(PoaError::ObjectAlreadyActive)
    );
    assert_eq!(
        user.activate_object_with_id(b"b", Arc::clone(&servant)),
        Err(PoaError::ServantAlreadyActive)
    );
    assert_eq!(user.servant_to_id(&servant).unwrap(), b"a");
    assert_eq!(
        user.id_to_reference(b"b").unwrap_err(),
        PoaError::ObjectNotActive
    );
    assert_eq!(
        user.servant_to_id(&(Arc::new(TestServant::default()) as Arc<dyn Servant>)),
        Err(PoaError::ServantNotActive)
    );

    let multiple = root
        .create_poa(
            "multiple",
            None,
            &[
                Policy::IdAssignment(IdAssignment::User),
                Policy::IdUniqueness(IdUniqueness::Multiple),
            ],
        )
        .unwrap();
    multiple
        .activate_object_with_id(b"a", Arc::clone(&servant))
        .unwrap();
    multiple
        .activate_object_with_id(b"b", Arc::clone(&servant))
        .unwrap();

    child.destroy(false, false).unwrap();
    assert!(matches!(
        child.activate_object(Arc::clone(&servant)),
        Err(PoaError::System(_))
    ));
    orb.shutdown(true).unwrap();
}

#[test]
fn poa_manager_states() {
    let orb = server_orb();
    let root = orb.root_poa().unwrap();
    let poa = root
        .create_poa("managed", None, &[Policy::IdAssignment(IdAssignment::User)])
        .unwrap();
    let manager = poa.the_poa_manager();
    assert_eq!(manager.state(), PoaManagerState::Holding);
    poa.activate_object_with_id(b"obj", Arc::new(TestServant::default()))
        .unwrap();
    let obj = poa.id_to_reference(b"obj").unwrap();

    // Holding: the request waits for activate; LocateRequest answers.
    assert!(obj.locate().unwrap());
    let (tx, rx) = mpsc::channel();
    let held = obj.clone();
    thread::spawn(move || tx.send(call_long(&held, 11)).unwrap());
    assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
    manager.activate().unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap(),
        11
    );

    manager.discard_requests(true).unwrap();
    let e = system(call_long(&obj, 1));
    assert!(e.is(SystemExceptionKind::TRANSIENT));
    assert_eq!(e.minor, minor::POA_DISCARDING);
    assert!(obj.locate().unwrap());

    // Requests held when the manager starts discarding are refused too.
    manager.hold_requests(false).unwrap();
    let (tx, rx) = mpsc::channel();
    let held = obj.clone();
    thread::spawn(move || tx.send(call_long(&held, 12)).unwrap());
    thread::sleep(Duration::from_millis(200));
    manager.discard_requests(false).unwrap();
    let e = system(rx.recv_timeout(Duration::from_secs(5)).unwrap());
    assert!(e.is(SystemExceptionKind::TRANSIENT));

    manager.deactivate(false, true).unwrap();
    let e = system(call_long(&obj, 1));
    assert!(e.is(SystemExceptionKind::OBJ_ADAPTER));
    assert_eq!(e.minor, minor::POA_INACTIVE);
    assert!(!obj.locate().unwrap());
    assert_eq!(manager.activate(), Err(PoaError::AdapterInactive));
    assert_eq!(manager.state(), PoaManagerState::Inactive);
    orb.shutdown(true).unwrap();
}

#[test]
fn requests_on_one_connection_run_in_parallel() {
    let orb = server_orb();
    let servant = Arc::new(TestServant::default());
    let _ = servant.barrier.set(Arc::new(Barrier::new(8)));
    let (_poa, obj) = serve(&orb, servant.clone());
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let remote = client
        .string_to_object(&orb.object_to_string(Some(&obj)).unwrap())
        .unwrap()
        .unwrap();
    // The client sends all eight on its one connection; they only finish
    // if they run at the same time.
    let calls: Vec<_> = (0..8)
        .map(|_| {
            let remote = remote.clone();
            thread::spawn(move || {
                remote
                    .request("wait")
                    .timeout(Some(Duration::from_secs(10)))
                    .invoke()
                    .map(|_| ())
            })
        })
        .collect();
    for call in calls {
        call.join().unwrap().unwrap();
    }
    let mut threads = servant.threads.lock().unwrap().clone();
    threads.sort_by_key(|t| format!("{t:?}"));
    threads.dedup();
    assert_eq!(threads.len(), 8);
    orb.shutdown(true).unwrap();
}

#[test]
fn oneway_requests_run_without_a_reply() {
    let orb = server_orb();
    let servant = Arc::new(TestServant::default());
    let (_poa, obj) = serve(&orb, servant.clone());
    for m in ["one", "two"] {
        obj.request("post")
            .args(move |out| out.write_string(m))
            .send_oneway()
            .unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while servant.posted.lock().unwrap().len() < 2 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let mut posted = servant.posted.lock().unwrap().clone();
    posted.sort();
    assert_eq!(posted, ["one", "two"]);
    orb.shutdown(true).unwrap();
}

#[test]
fn servants_can_call_objects_of_their_own_orb() {
    let orb = server_orb();
    let (_poa, obj) = serve(&orb, Arc::new(TestServant::default()));
    let reply = obj
        .request("call_self")
        .args(|out| {
            out.write_i32(77);
            Ok(())
        })
        .invoke()
        .unwrap();
    assert_eq!(reply.input().read::<i32>().unwrap(), 77);
    orb.shutdown(true).unwrap();
}

// --- hand-built GIOP -----------------------------------------------------------

fn connect(obj: &ObjectRef) -> TcpStream {
    let profile = obj.ior().iiop_profiles().next().unwrap().unwrap();
    let stream = TcpStream::connect((profile.host.as_str(), profile.port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
}

fn read_message(stream: &mut TcpStream) -> Option<(MessageHeader, Message)> {
    let mut header = [0u8; MessageHeader::SIZE];
    stream.read_exact(&mut header).ok()?;
    let parsed = MessageHeader::parse(&header).unwrap();
    let mut msg = header.to_vec();
    msg.resize(parsed.total_len(), 0);
    stream.read_exact(&mut msg[MessageHeader::SIZE..]).unwrap();
    Some(decode_message(&msg).unwrap())
}

fn request(
    version: Version,
    order: ByteOrder,
    request_id: u32,
    key: &[u8],
    operation: &str,
    arg: Option<u32>,
) -> Vec<u8> {
    let header = RequestHeader {
        request_id,
        response_flags: response_flags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(key.to_vec()),
        operation: operation.into(),
        service_context: Vec::new(),
        requesting_principal: Vec::new(),
    };
    let mut m = MessageEncoder::request(version, order, &header).unwrap();
    if let Some(v) = arg {
        m.body().write_u32(v);
    }
    m.finish().unwrap()
}

#[test]
fn replies_use_the_version_of_the_request_and_native_byte_order() {
    let orb = server_orb();
    let (_poa, obj) = serve(&orb, Arc::new(TestServant::default()));
    let key = key_of(&obj);
    let mut stream = connect(&obj);
    for (i, version) in [Version::V1_0, Version::V1_1, Version::V1_2]
        .into_iter()
        .enumerate()
    {
        let id = i as u32 + 100;
        stream
            .write_all(&request(
                version,
                ByteOrder::Big,
                id,
                &key,
                "echo_long",
                Some(5),
            ))
            .unwrap();
        let (h, msg) = read_message(&mut stream).unwrap();
        assert_eq!(h.version, version);
        assert_eq!(h.byte_order, ByteOrder::NATIVE);
        let Message::Reply { header, body } = msg else {
            panic!("expected a reply, got {msg:?}")
        };
        assert_eq!(header.request_id, id);
        assert_eq!(header.reply_status, ReplyStatus::NoException);
        assert_eq!(body.decoder().read_u32().unwrap(), 5);

        let locate = MessageEncoder::locate_request(
            version,
            ByteOrder::Big,
            &LocateRequestHeader {
                request_id: id + 10,
                target: TargetAddress::Key(b"\xffmissing\x00x".to_vec()),
            },
        )
        .unwrap();
        stream.write_all(&locate).unwrap();
        let (h, msg) = read_message(&mut stream).unwrap();
        assert_eq!(h.version, version);
        let Message::LocateReply { header, .. } = msg else {
            panic!("expected a locate reply, got {msg:?}")
        };
        assert_eq!(header.locate_status, LocateStatus::UnknownObject);
    }
    // GIOP 1.2 addressing by profile.
    let profile = obj.ior().profiles[0].clone();
    let header = RequestHeader {
        request_id: 7,
        response_flags: response_flags::SYNC_WITH_TARGET,
        target: TargetAddress::Profile(profile),
        operation: "echo_long".into(),
        service_context: Vec::new(),
        requesting_principal: Vec::new(),
    };
    let mut m = MessageEncoder::request(Version::V1_2, ByteOrder::Little, &header).unwrap();
    m.body().write_u32(8);
    stream.write_all(&m.finish().unwrap()).unwrap();
    let (_, msg) = read_message(&mut stream).unwrap();
    let Message::Reply { header, body } = msg else {
        panic!("expected a reply, got {msg:?}")
    };
    assert_eq!(header.reply_status, ReplyStatus::NoException);
    assert_eq!(body.decoder().read_u32().unwrap(), 8);
    orb.shutdown(true).unwrap();
}

#[test]
fn replies_are_sent_as_they_complete_and_cancelled_ones_are_dropped() {
    let orb = server_orb();
    let (_poa, obj) = serve(&orb, Arc::new(TestServant::default()));
    let key = key_of(&obj);
    let mut stream = connect(&obj);
    let v = Version::V1_2;
    let order = ByteOrder::Little;
    stream
        .write_all(&request(v, order, 1, &key, "sleep", Some(400)))
        .unwrap();
    stream
        .write_all(&request(v, order, 2, &key, "sleep", Some(300)))
        .unwrap();
    stream
        .write_all(&request(v, order, 3, &key, "sleep", Some(0)))
        .unwrap();
    let cancel = MessageEncoder::cancel_request(
        v,
        order,
        &rustorb_core::giop::CancelRequestHeader { request_id: 2 },
    )
    .unwrap();
    stream.write_all(&cancel).unwrap();
    let mut ids = Vec::new();
    for _ in 0..2 {
        let (_, msg) = read_message(&mut stream).unwrap();
        let Message::Reply { header, .. } = msg else {
            panic!("expected a reply, got {msg:?}")
        };
        ids.push(header.request_id);
    }
    assert_eq!(ids, [3, 1]);
    // Nothing more: the reply to the cancelled request is not sent.
    stream
        .set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    assert!(read_message(&mut stream).is_none());
    orb.shutdown(true).unwrap();
}

#[test]
fn malformed_messages_get_message_error() {
    let orb = server_orb();
    let (_poa, obj) = serve(&orb, Arc::new(TestServant::default()));
    let mut stream = connect(&obj);
    stream
        .write_all(b"GIOX\x01\x02\x01\x00\x00\x00\x00\x00")
        .unwrap();
    let (_, msg) = read_message(&mut stream).unwrap();
    assert_eq!(msg, Message::MessageError);
    assert!(read_message(&mut stream).is_none());
    orb.shutdown(true).unwrap();
}

#[test]
fn system_exception_replies_carry_the_minor_code() {
    let orb = server_orb();
    let (_poa, obj) = serve(&orb, Arc::new(TestServant::default()));
    let mut stream = connect(&obj);
    stream
        .write_all(&request(
            Version::V1_2,
            ByteOrder::Big,
            1,
            b"\xffnone\x00x",
            "echo_long",
            Some(1),
        ))
        .unwrap();
    let (_, msg) = read_message(&mut stream).unwrap();
    let Message::Reply { header, body } = msg else {
        panic!("expected a reply, got {msg:?}")
    };
    assert_eq!(header.reply_status, ReplyStatus::SystemException);
    let mut dec = body.decoder();
    let e: SystemExceptionBody = rustorb_core::cdr::CdrDecode::decode(&mut dec).unwrap();
    assert_eq!(e.exception_id, "IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0");
    assert_eq!(e.minor_code_value, 0x4f4d_0001);
    assert_eq!(e.completion_status, CompletionStatus::No);
    orb.shutdown(true).unwrap();
}

#[test]
fn shutdown_from_a_servant_replies_and_then_closes_connections() {
    let orb = server_orb();
    let (_poa, obj) = serve(&orb, Arc::new(TestServant::default()));
    let key = key_of(&obj);

    // shutdown(true) in a servant would deadlock.
    let e = system(call_unit(&obj, "stop_and_wait"));
    assert!(e.is(SystemExceptionKind::BAD_INV_ORDER));
    assert_eq!(e.minor, minor::WOULD_DEADLOCK);

    let runner = orb.clone();
    let run = thread::spawn(move || runner.run());
    let mut stream = connect(&obj);
    stream
        .write_all(&request(
            Version::V1_2,
            ByteOrder::Big,
            1,
            &key,
            "stop",
            None,
        ))
        .unwrap();
    let (_, msg) = read_message(&mut stream).unwrap();
    let Message::Reply { header, .. } = msg else {
        panic!("expected a reply, got {msg:?}")
    };
    assert_eq!(header.reply_status, ReplyStatus::NoException);
    let (h, msg) = read_message(&mut stream).unwrap();
    assert_eq!(msg, Message::CloseConnection);
    assert_eq!(h.version, Version::V1_2);
    assert!(read_message(&mut stream).is_none());
    run.join().unwrap();

    // Nothing listens any more, and the ORB refuses new work.
    let profile = obj.ior().iiop_profiles().next().unwrap().unwrap();
    assert!(TcpStream::connect((profile.host.as_str(), profile.port)).is_err());
    assert!(orb.root_poa().is_err());
    orb.destroy().unwrap();
}

#[test]
fn endpoint_options() {
    let mut args: Vec<String> = [
        "prog",
        "-ORBendPoint",
        "giop:tcp:127.0.0.1:0",
        "-ORBendPointPublish",
        "giop:tcp:published.example:2809",
        "-ORBmaxServerThreadPoolSize",
        "7",
        "rest",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let orb = Orb::init(&mut args).unwrap();
    assert_eq!(args, ["prog", "rest"]);
    assert_eq!(orb.options().max_server_threads, 7);
    let root = orb.root_poa().unwrap();
    assert_eq!(orb.endpoints(), [("published.example".to_owned(), 2809)]);
    let servant: Arc<dyn Servant> = Arc::new(TestServant::default());
    let obj = root.servant_to_reference(&servant).unwrap();
    let profile = obj.ior().iiop_profiles().next().unwrap().unwrap();
    assert_eq!(
        (profile.host.as_str(), profile.port),
        ("published.example", 2809)
    );
    assert_eq!(profile.version, Version::V1_2);
    assert!(profile.code_sets().is_some());

    // omniORB's `all(addr)` is the default publication.
    let mut options = rustorb_core::OrbOptions::default();
    let mut args = vec!["-ORBendPointPublish".to_owned(), "all(addr)".to_owned()];
    options.parse_args(&mut args).unwrap();
    assert!(args.is_empty());
    assert_eq!(options.endpoint_publish, None);

    // The leader / follower settings (docs/read-ownership.md).
    let mut args: Vec<String> = ["-ORBfollowLinger=250", "-ORBfollowLimit", "8"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    options.parse_args(&mut args).unwrap();
    assert!(args.is_empty());
    assert_eq!(options.follow_linger, std::time::Duration::from_micros(250));
    assert_eq!(options.follow_limit, 8);
    let mut args = vec!["-ORBfollowLimit".to_owned(), "many".to_owned()];
    assert!(options.parse_args(&mut args).is_err());

    assert_eq!(
        EndpointAddr::parse("giop:tcp::"),
        Some(EndpointAddr {
            host: String::new(),
            port: 0
        })
    );
    assert_eq!(
        EndpointAddr::parse("giop:tcp:[::1]:5000"),
        Some(EndpointAddr {
            host: "::1".into(),
            port: 5000
        })
    );
    assert_eq!(EndpointAddr::parse("giop:unix:/tmp/x"), None);
    orb.shutdown(true).unwrap();
}

#[test]
fn wide_strings_need_a_negotiated_code_set() {
    let orb = server_orb();
    let (_poa, obj) = serve(&orb, Arc::new(TestServant::default()));
    // The rustorb client announces UTF-16 from the IOR.
    let reply = obj
        .request("echo_wstring")
        .args(|out| out.write_wstring("\u{3053}\u{1F980}"))
        .invoke()
        .unwrap();
    assert_eq!(reply.input().read_wstring().unwrap(), "\u{3053}\u{1F980}");

    // A GIOP 1.1 request without a code set context.
    let header = RequestHeader {
        request_id: 1,
        response_flags: response_flags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(key_of(&obj)),
        operation: "echo_wstring".into(),
        service_context: Vec::new(),
        requesting_principal: Vec::new(),
    };
    let mut m = MessageEncoder::request(Version::V1_1, ByteOrder::Big, &header).unwrap();
    m.body().write_u32(0);
    let mut stream = connect(&obj);
    stream.write_all(&m.finish().unwrap()).unwrap();
    let (_, msg) = read_message(&mut stream).unwrap();
    let Message::Reply { header, body } = msg else {
        panic!("expected a reply, got {msg:?}")
    };
    assert_eq!(header.reply_status, ReplyStatus::SystemException);
    let e: SystemExceptionBody = rustorb_core::cdr::CdrDecode::decode(&mut body.decoder()).unwrap();
    assert_eq!(e.exception_id, "IDL:omg.org/CORBA/BAD_PARAM:1.0");
    assert_eq!(e.minor_code_value, minor::WCHAR_TCS_NOT_KNOWN);
    orb.shutdown(true).unwrap();
}
