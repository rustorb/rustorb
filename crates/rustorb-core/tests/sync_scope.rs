//! `Messaging::SyncScopePolicy`: `create_policy`, the overrides of
//! references, threads and the ORB, the `response_flags` of a oneway
//! request and when the call returns (docs/sync-scope.md).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use rustorb_core::pi::sync_scope::{
    SYNC_NONE, SYNC_WITH_SERVER, SYNC_WITH_TARGET, SYNC_WITH_TRANSPORT,
};
use rustorb_core::pi::{
    ClientRequestInfo, ClientRequestInterceptor, InterceptError, Interceptor, OrbInitInfo,
    OrbInitializer, ServerRequestInfo, ServerRequestInterceptor,
};
use rustorb_core::policy::{
    RELATIVE_RT_TIMEOUT_POLICY_TYPE, SYNC_SCOPE_POLICY_TYPE, set_thread_policy_overrides,
    thread_policy_overrides,
};
use rustorb_core::{
    Any, EndpointAddr, InStream, ObjectRef, Orb, OrbOptions, PolicyError, PolicyValue, Servant,
    ServerReply, SetOverrideType, SystemException, SystemExceptionKind, TCKind, TypeCode, Value,
};

fn server_orb(options: OrbOptions) -> Orb {
    Orb::with_options(OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..options
    })
    .unwrap()
}

fn short(v: i16) -> Any {
    Any::new(TypeCode::simple(TCKind::Short).unwrap(), Value::Short(v))
}

fn sync(v: i16) -> PolicyValue {
    PolicyValue::SyncScope(v)
}

#[test]
fn create_policy_takes_a_sync_scope() {
    let orb = Orb::init(&mut Vec::new()).unwrap();
    for v in [
        SYNC_NONE,
        SYNC_WITH_TRANSPORT,
        SYNC_WITH_SERVER,
        SYNC_WITH_TARGET,
    ] {
        let p = orb
            .create_policy(SYNC_SCOPE_POLICY_TYPE, &short(v))
            .unwrap();
        assert_eq!(p, sync(v));
        assert_eq!(p.synchronization(), Some(v));
        assert_eq!((p.policy_type(), p.raw_value()), (24, v as u64));
        assert_eq!(PolicyValue::from_raw(24, v as u64), Some(p));
        assert_eq!(
            p.repository_id(),
            "IDL:omg.org/Messaging/SyncScopePolicy:1.0"
        );
    }
    // Messaging::SyncScope, the alias of short.
    let alias = TypeCode::alias(
        "IDL:omg.org/Messaging/SyncScope:1.0",
        "SyncScope",
        &TypeCode::simple(TCKind::Short).unwrap(),
    )
    .unwrap();
    assert_eq!(
        orb.create_policy(SYNC_SCOPE_POLICY_TYPE, &Any::new(alias, Value::Short(2))),
        Ok(sync(SYNC_WITH_SERVER))
    );
    let err = |reason| Err(PolicyError { reason });
    for v in [-1, 4] {
        assert_eq!(
            orb.create_policy(SYNC_SCOPE_POLICY_TYPE, &short(v)),
            err(PolicyError::BAD_POLICY_VALUE)
        );
    }
    let ushort = Any::new(TypeCode::simple(TCKind::UShort).unwrap(), Value::UShort(1));
    assert_eq!(
        orb.create_policy(SYNC_SCOPE_POLICY_TYPE, &ushort),
        err(PolicyError::BAD_POLICY_TYPE)
    );
    assert_eq!(PolicyValue::from_raw(24, 4), None);
}

#[test]
fn overrides_of_the_reference_the_thread_and_the_orb() {
    let orb = Orb::init(&mut Vec::new()).unwrap();
    let obj = orb
        .string_to_object("corbaloc::127.0.0.1:1/x")
        .unwrap()
        .unwrap();
    assert_eq!(obj.client_policy(SYNC_SCOPE_POLICY_TYPE), None);

    orb.set_policy_overrides(&[sync(SYNC_NONE)], SetOverrideType::Add)
        .unwrap();
    assert_eq!(orb.policy_overrides(&[]), vec![sync(SYNC_NONE)]);
    assert_eq!(
        obj.client_policy(SYNC_SCOPE_POLICY_TYPE),
        Some(sync(SYNC_NONE))
    );

    set_thread_policy_overrides(&[sync(SYNC_WITH_SERVER)], SetOverrideType::Add).unwrap();
    assert_eq!(
        thread_policy_overrides(&[SYNC_SCOPE_POLICY_TYPE]),
        vec![sync(SYNC_WITH_SERVER)]
    );
    assert_eq!(
        obj.client_policy(SYNC_SCOPE_POLICY_TYPE),
        Some(sync(SYNC_WITH_SERVER))
    );
    // Another thread has its own.
    let other = obj.clone();
    let seen = thread::spawn(move || other.client_policy(SYNC_SCOPE_POLICY_TYPE))
        .join()
        .unwrap();
    assert_eq!(seen, Some(sync(SYNC_NONE)));

    let rt = PolicyValue::RelativeRoundtripTimeout(10_000_000);
    let with = obj
        .set_policy_overrides(&[sync(SYNC_WITH_TARGET), rt], SetOverrideType::Set)
        .unwrap();
    assert_eq!(with.policy_overrides(&[]), vec![rt, sync(SYNC_WITH_TARGET)]);
    assert_eq!(
        with.policy_overrides(&[SYNC_SCOPE_POLICY_TYPE]),
        vec![sync(SYNC_WITH_TARGET)]
    );
    assert_eq!(
        with.policy_overrides(&[RELATIVE_RT_TIMEOUT_POLICY_TYPE]),
        vec![rt]
    );
    assert_eq!(
        with.client_policy(SYNC_SCOPE_POLICY_TYPE),
        Some(sync(SYNC_WITH_TARGET))
    );
    assert!(obj.policy_overrides(&[]).is_empty());
    // ADD keeps the others; SET drops them.
    let added = with
        .set_policy_overrides(&[sync(SYNC_NONE)], SetOverrideType::Add)
        .unwrap();
    assert_eq!(added.policy_overrides(&[]), vec![rt, sync(SYNC_NONE)]);
    let set = with
        .set_policy_overrides(&[rt], SetOverrideType::Set)
        .unwrap();
    assert_eq!(set.policy_overrides(&[]), vec![rt]);
    // Two of the same type.
    let e = obj
        .set_policy_overrides(&[sync(SYNC_NONE), sync(SYNC_NONE)], SetOverrideType::Set)
        .unwrap_err();
    assert!(e.is(SystemExceptionKind::BAD_PARAM));

    set_thread_policy_overrides(&[], SetOverrideType::Set).unwrap();
    assert!(thread_policy_overrides(&[]).is_empty());
    orb.set_policy_overrides(&[], SetOverrideType::Set).unwrap();
    assert!(orb.policy_overrides(&[]).is_empty());
    assert_eq!(obj.client_policy(SYNC_SCOPE_POLICY_TYPE), None);
    orb.destroy().unwrap();
}

/// A GIOP server on a socket that records the `response_flags` of each
/// request and answers those with the low bit set after `delay`.
fn fake_server(delay: Duration) -> (u16, mpsc::Receiver<u8>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        while let Ok((conn, _)) = listener.accept() {
            let tx = tx.clone();
            thread::spawn(move || serve(conn, delay, tx));
        }
    });
    (port, rx)
}

fn serve(mut conn: TcpStream, delay: Duration, tx: mpsc::Sender<u8>) {
    loop {
        let mut header = [0u8; 12];
        if conn.read_exact(&mut header).is_err() {
            return;
        }
        let little = header[6] & 0x01 != 0;
        let size = if little {
            u32::from_le_bytes(header[8..12].try_into().unwrap())
        } else {
            u32::from_be_bytes(header[8..12].try_into().unwrap())
        };
        let mut body = vec![0u8; size as usize];
        if conn.read_exact(&mut body).is_err() {
            return;
        }
        // GIOP 1.2 Request: request_id, response_flags.
        assert_eq!(&header[..6], b"GIOP\x01\x02");
        assert_eq!(header[7], 0, "a Request");
        let flags = body[4];
        let _ = tx.send(flags);
        if flags & 0x01 == 0 {
            continue;
        }
        thread::sleep(delay);
        // A Reply: request_id, NO_EXCEPTION, no service contexts.
        let mut reply = Vec::new();
        reply.extend_from_slice(b"GIOP\x01\x02");
        reply.push(header[6] & 0x01);
        reply.push(1);
        let u32_bytes = |v: u32| {
            if little {
                v.to_le_bytes()
            } else {
                v.to_be_bytes()
            }
        };
        reply.extend_from_slice(&u32_bytes(12));
        reply.extend_from_slice(&body[..4]);
        reply.extend_from_slice(&u32_bytes(0));
        reply.extend_from_slice(&u32_bytes(0));
        if conn.write_all(&reply).is_err() {
            return;
        }
    }
}

fn with_sync(obj: &ObjectRef, v: i16) -> ObjectRef {
    obj.set_policy_overrides(&[sync(v)], SetOverrideType::Set)
        .unwrap()
}

#[test]
fn response_flags_and_when_the_call_returns() {
    let delay = Duration::from_millis(300);
    let (port, flags) = fake_server(delay);
    let orb = Orb::init(&mut Vec::new()).unwrap();
    let obj = orb
        .string_to_object(&format!("corbaloc::1.2@127.0.0.1:{port}/x"))
        .unwrap()
        .unwrap();
    let next = || flags.recv_timeout(Duration::from_secs(5)).unwrap();

    // The default: SYNC_WITH_TRANSPORT.
    let started = Instant::now();
    obj.request("op").send_oneway().unwrap();
    assert!(started.elapsed() < delay);
    assert_eq!(next(), 0x00);

    for (v, wire, waits) in [
        (SYNC_NONE, 0x00, false),
        (SYNC_WITH_TRANSPORT, 0x00, false),
        (SYNC_WITH_SERVER, 0x01, true),
        (SYNC_WITH_TARGET, 0x03, true),
    ] {
        let target = with_sync(&obj, v);
        let started = Instant::now();
        target.request("op").send_oneway().unwrap();
        let took = started.elapsed();
        assert_eq!(took >= delay, waits, "sync scope {v}: took {took:?}");
        assert_eq!(next(), wire, "sync scope {v}");
        // The async form too.
        let started = Instant::now();
        block_on(target.request_async("op").send_oneway()).unwrap();
        assert_eq!(started.elapsed() >= delay, waits, "async, sync scope {v}");
        assert_eq!(next(), wire);
    }

    // A two-way call is SYNC_WITH_TARGET whatever the policy.
    let none = with_sync(&obj, SYNC_NONE);
    none.request("op").invoke().unwrap();
    assert_eq!(next(), 0x03);

    // The thread's override, and the ORB's.
    set_thread_policy_overrides(&[sync(SYNC_WITH_SERVER)], SetOverrideType::Set).unwrap();
    obj.request("op").send_oneway().unwrap();
    assert_eq!(next(), 0x01);
    set_thread_policy_overrides(&[], SetOverrideType::Set).unwrap();
    orb.set_policy_overrides(&[sync(SYNC_WITH_TARGET)], SetOverrideType::Set)
        .unwrap();
    obj.request("op").send_oneway().unwrap();
    assert_eq!(next(), 0x03);
    // The reference's beats the ORB's.
    with_sync(&obj, SYNC_WITH_TRANSPORT)
        .request("op")
        .send_oneway()
        .unwrap();
    assert_eq!(next(), 0x00);
    orb.destroy().unwrap();
}

/// Runs an async request from a plain thread: the future can be polled by
/// any executor.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

#[test]
fn sync_none_returns_before_the_request_is_written() {
    // A peer that accepts but never reads: a large request fills the
    // socket buffers.
    let sink = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = sink.local_addr().unwrap().port();
    thread::spawn(move || {
        let mut conns = Vec::new();
        while let Ok((c, _)) = sink.accept() {
            conns.push(c);
        }
    });
    let orb = Orb::init(&mut Vec::new()).unwrap();
    let obj = orb
        .string_to_object(&format!("corbaloc::1.2@127.0.0.1:{port}/x"))
        .unwrap()
        .unwrap();
    let rt = PolicyValue::RelativeRoundtripTimeout(3_000_000); // 300 ms
    let big = vec![0u8; 32 << 20];
    let send = |v: i16| {
        let target = obj
            .set_policy_overrides(&[sync(v), rt], SetOverrideType::Set)
            .unwrap();
        let started = Instant::now();
        let result = target
            .request("op")
            .args(|out| {
                out.write_octet_seq(&big).unwrap();
                Ok(())
            })
            .send_oneway();
        (result, started.elapsed())
    };
    let (result, _) = send(SYNC_WITH_TRANSPORT);
    assert!(result.unwrap_err().is(SystemExceptionKind::TIMEOUT));
    let (result, took) = send(SYNC_NONE);
    result.unwrap();
    assert!(took < Duration::from_millis(300), "took {took:?}");
    orb.destroy().unwrap();
}

/// A servant whose `slow` takes 300 ms, and that records its calls.
struct Slow {
    calls: mpsc::Sender<&'static str>,
}

impl Servant for Slow {
    fn repository_id(&self) -> &str {
        "IDL:test/Slow:1.0"
    }

    fn dispatch(
        &self,
        operation: &str,
        _input: &mut InStream<'_>,
        _reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        thread::sleep(Duration::from_millis(300));
        let _ = self
            .calls
            .send(if operation == "slow" { "slow" } else { "other" });
        Ok(())
    }
}

#[test]
fn a_rustorb_server_answers_sync_with_server_before_the_upcall() {
    let server = server_orb(OrbOptions::default());
    let poa = server.root_poa().unwrap();
    poa.the_poa_manager().activate().unwrap();
    let (tx, calls) = mpsc::channel();
    let id = poa.activate_object(Arc::new(Slow { calls: tx })).unwrap();
    let slow = poa.id_to_reference(&id).unwrap();
    let missing = poa.create_reference("IDL:test/Slow:1.0").unwrap();

    let client = Orb::init(&mut Vec::new()).unwrap();
    let remote = |o: &ObjectRef, v: i16| {
        let obj = client
            .string_to_object(&server.object_to_string(Some(o)).unwrap())
            .unwrap()
            .unwrap();
        with_sync(&obj, v)
    };
    let delay = Duration::from_millis(300);

    // SYNC_WITH_SERVER: the reply comes before the servant runs.
    let started = Instant::now();
    remote(&slow, SYNC_WITH_SERVER)
        .request("slow")
        .send_oneway()
        .unwrap();
    assert!(started.elapsed() < delay, "took {:?}", started.elapsed());
    assert_eq!(calls.recv_timeout(Duration::from_secs(5)), Ok("slow"));

    // SYNC_WITH_TARGET: after it.
    let started = Instant::now();
    remote(&slow, SYNC_WITH_TARGET)
        .request("slow")
        .send_oneway()
        .unwrap();
    assert!(started.elapsed() >= delay);
    assert_eq!(calls.try_recv(), Ok("slow"));

    // The server's answer can be an exception.
    for v in [SYNC_WITH_SERVER, SYNC_WITH_TARGET] {
        let e = remote(&missing, v)
            .request("slow")
            .send_oneway()
            .unwrap_err();
        assert!(e.is(SystemExceptionKind::OBJECT_NOT_EXIST), "{v}: {e}");
    }
    // Without one, nothing comes back.
    for v in [SYNC_NONE, SYNC_WITH_TRANSPORT] {
        remote(&missing, v).request("slow").send_oneway().unwrap();
    }

    // Colocated: the same rules.
    with_sync(&slow, SYNC_WITH_SERVER)
        .request("slow")
        .send_oneway()
        .unwrap();
    assert_eq!(calls.recv_timeout(Duration::from_secs(5)), Ok("slow"));
    let e = with_sync(&missing, SYNC_WITH_TARGET)
        .request("slow")
        .send_oneway()
        .unwrap_err();
    assert!(e.is(SystemExceptionKind::OBJECT_NOT_EXIST));
    client.destroy().unwrap();
    server.destroy().unwrap();
}

/// Records the sync scope each interception point sees.
#[derive(Default)]
struct Seen {
    log: Mutex<Vec<String>>,
}

impl Seen {
    fn push(&self, s: String) {
        self.log.lock().unwrap().push(s);
    }

    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.log.lock().unwrap())
    }
}

struct Watch(Arc<Seen>);

impl Interceptor for Watch {
    fn name(&self) -> &str {
        "watch"
    }
}

impl ClientRequestInterceptor for Watch {
    fn send_request(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        let policy = ri
            .get_request_policy(SYNC_SCOPE_POLICY_TYPE)
            .unwrap()
            .and_then(|p| p.synchronization());
        self.0.push(format!(
            "send_request {} {} {policy:?}",
            ri.response_expected(),
            ri.sync_scope()
        ));
        Ok(())
    }

    fn receive_reply(&self, ri: &ClientRequestInfo<'_>) -> Result<(), SystemException> {
        self.0.push(format!("receive_reply {}", ri.sync_scope()));
        Ok(())
    }

    fn receive_other(&self, ri: &ClientRequestInfo<'_>) -> Result<(), InterceptError> {
        self.0.push(format!("receive_other {}", ri.sync_scope()));
        Ok(())
    }
}

impl ServerRequestInterceptor for Watch {
    fn receive_request(&self, ri: &ServerRequestInfo<'_>) -> Result<(), InterceptError> {
        self.0.push(format!(
            "server {} {}",
            ri.response_expected(),
            ri.sync_scope()
        ));
        Ok(())
    }
}

struct WatchInit(Arc<Seen>);

impl OrbInitializer for WatchInit {
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        info.add_client_request_interceptor(Arc::new(Watch(self.0.clone())))
            .unwrap();
        info.add_server_request_interceptor(Arc::new(Watch(self.0.clone())))
            .unwrap();
        Ok(())
    }
}

#[test]
fn interceptors_see_the_sync_scope() {
    let client_seen = Arc::new(Seen::default());
    let server_seen = Arc::new(Seen::default());
    let server = server_orb(OrbOptions {
        initializers: vec![Arc::new(WatchInit(server_seen.clone()))],
        ..OrbOptions::default()
    });
    let poa = server.root_poa().unwrap();
    poa.the_poa_manager().activate().unwrap();
    let (tx, calls) = mpsc::channel();
    let id = poa.activate_object(Arc::new(Slow { calls: tx })).unwrap();
    let slow = poa.id_to_reference(&id).unwrap();
    let client = Orb::with_options(OrbOptions {
        initializers: vec![Arc::new(WatchInit(client_seen.clone()))],
        ..OrbOptions::default()
    })
    .unwrap();
    let obj = client
        .string_to_object(&server.object_to_string(Some(&slow)).unwrap())
        .unwrap()
        .unwrap();

    for (v, server_line) in [
        (SYNC_WITH_TRANSPORT, "server false 0"),
        (SYNC_WITH_SERVER, "server true 2"),
        (SYNC_WITH_TARGET, "server true 3"),
    ] {
        with_sync(&obj, v).request("x").send_oneway().unwrap();
        assert_eq!(calls.recv_timeout(Duration::from_secs(5)), Ok("other"));
        assert_eq!(
            client_seen.take(),
            vec![
                format!("send_request false {v} Some({v})"),
                format!("receive_other {v}"),
            ]
        );
        assert_eq!(server_seen.take(), vec![server_line.to_owned()]);
    }
    obj.request("x").invoke().unwrap();
    assert_eq!(calls.recv_timeout(Duration::from_secs(5)), Ok("other"));
    assert_eq!(
        client_seen.take(),
        vec![
            "send_request true 3 None".to_owned(),
            "receive_reply 3".to_owned()
        ]
    );
    client.destroy().unwrap();
    server.destroy().unwrap();
}
