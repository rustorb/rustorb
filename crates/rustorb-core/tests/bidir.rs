//! Bidirectional GIOP (docs/bidir-giop.md): a client offers the connection
//! it opened, and the server calls the client's objects back over it, so
//! that the client need not be reachable.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use rustorb_core::policy::BIDIRECTIONAL_POLICY_TYPE;
use rustorb_core::{
    Any, Bidirectional, CompletionStatus, EndpointAddr, Exception, InStream, ObjectRef, Orb,
    OrbOptions, Policy, PolicyValue, Servant, ServerReply, SetOverrideType, SystemException,
    SystemExceptionKind, TCKind, TypeCode, Value,
};

/// The server: `call_back(cb, n)` returns `cb.notify(n) + 1`;
/// `call_back_later(cb, ms)` (oneway) calls `cb.slow(ms)` from a thread of
/// its own after 50 ms and records how that ended.
struct Server {
    later: Arc<Mutex<Option<Result<(), SystemException>>>>,
}

fn system(e: Exception) -> SystemException {
    match e {
        Exception::System(e) => e,
        Exception::User(_) => {
            SystemException::new(SystemExceptionKind::UNKNOWN, 0, CompletionStatus::Maybe)
        }
    }
}

impl Servant for Server {
    fn repository_id(&self) -> &str {
        "IDL:test/Server:1.0"
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        match operation {
            "call_back" => {
                let cb = input.read_object()?.expect("a callback");
                let n = input.read::<i32>()?;
                let r = cb
                    .request("notify")
                    .args(|out| {
                        out.write_i32(n);
                        Ok(())
                    })
                    .invoke()
                    .map_err(system)?;
                let v = r.input().read::<i32>()?;
                reply.output().write_i32(v + 1);
                Ok(())
            }
            "call_back_later" => {
                let cb = input.read_object()?.expect("a callback");
                let ms = input.read::<u32>()?;
                let later = Arc::clone(&self.later);
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(50));
                    let r = cb
                        .request("slow")
                        .args(|out| {
                            out.write_u32(ms);
                            Ok(())
                        })
                        .invoke()
                        .map(|_| ())
                        .map_err(system);
                    *later.lock().unwrap() = Some(r);
                });
                Ok(())
            }
            _ => Err(SystemException::new(
                SystemExceptionKind::BAD_OPERATION,
                0,
                CompletionStatus::No,
            )),
        }
    }
}

/// The client's callback object.
struct Callback {
    calls: AtomicUsize,
}

impl Servant for Callback {
    fn repository_id(&self) -> &str {
        "IDL:test/Callback:1.0"
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match operation {
            "notify" => {
                let n = input.read::<i32>()?;
                reply.output().write_i32(n * 10);
                Ok(())
            }
            "slow" => {
                let ms = input.read::<u32>()?;
                thread::sleep(Duration::from_millis(ms.into()));
                Ok(())
            }
            _ => Err(SystemException::new(
                SystemExceptionKind::BAD_OPERATION,
                0,
                CompletionStatus::No,
            )),
        }
    }
}

/// A TCP proxy in front of the server that counts the connections through
/// it, accepted and still open.
struct Proxy {
    port: u16,
    accepted: Arc<AtomicUsize>,
    open: Arc<AtomicUsize>,
}

impl Proxy {
    fn start(upstream: u16) -> Proxy {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let accepted = Arc::new(AtomicUsize::new(0));
        let open = Arc::new(AtomicUsize::new(0));
        let (a, o) = (Arc::clone(&accepted), Arc::clone(&open));
        thread::spawn(move || {
            for client in listener.incoming() {
                let Ok(client) = client else { return };
                let server = TcpStream::connect(("127.0.0.1", upstream)).unwrap();
                a.fetch_add(1, Ordering::SeqCst);
                o.fetch_add(1, Ordering::SeqCst);
                let left = Arc::new(AtomicUsize::new(2));
                for (mut from, mut to) in [
                    (client.try_clone().unwrap(), server.try_clone().unwrap()),
                    (server, client),
                ] {
                    let (left, o) = (Arc::clone(&left), Arc::clone(&o));
                    thread::spawn(move || {
                        let mut buf = [0; 16384];
                        while let Ok(n @ 1..) = from.read(&mut buf) {
                            if to.write_all(&buf[..n]).is_err() {
                                break;
                            }
                        }
                        let _ = to.shutdown(Shutdown::Write);
                        if left.fetch_sub(1, Ordering::SeqCst) == 1 {
                            o.fetch_sub(1, Ordering::SeqCst);
                        }
                    });
                }
            }
        });
        Proxy {
            port,
            accepted,
            open,
        }
    }
}

struct Setup {
    server: Orb,
    client: Orb,
    /// The server object, through the proxy.
    obj: ObjectRef,
    /// The callback object, in a POA of the client.
    cb: ObjectRef,
    callback: Arc<Callback>,
    later: Arc<Mutex<Option<Result<(), SystemException>>>>,
    proxy: Proxy,
}

impl Drop for Setup {
    fn drop(&mut self) {
        let _ = self.client.destroy();
        let _ = self.server.destroy();
    }
}

fn local_endpoint() -> Vec<EndpointAddr> {
    vec![EndpointAddr {
        host: "127.0.0.1".into(),
        port: 0,
    }]
}

/// A server ORB with the Server object behind a proxy, and a client ORB
/// whose callback object is published at an address nobody listens on
/// (`127.0.0.1:1`), in a POA with the BidirectionalPolicy `cb_policy`.
fn setup(
    server_options: OrbOptions,
    client_options: OrbOptions,
    cb_policy: Bidirectional,
) -> Setup {
    let server = Orb::with_options(OrbOptions {
        endpoints: local_endpoint(),
        ..server_options
    })
    .unwrap();
    let root = server.root_poa().unwrap();
    let later = Arc::new(Mutex::new(None));
    let id = root
        .activate_object(Arc::new(Server {
            later: Arc::clone(&later),
        }))
        .unwrap();
    root.the_poa_manager().activate().unwrap();
    let local = root.id_to_reference(&id).unwrap();
    let profile = local.ior().iiop_profiles().next().unwrap().unwrap();
    let proxy = Proxy::start(profile.port);
    let key: String = profile
        .object_key
        .iter()
        .map(|b| format!("%{b:02x}"))
        .collect();

    let client = Orb::with_options(OrbOptions {
        endpoints: local_endpoint(),
        endpoint_publish: Some(EndpointAddr {
            host: "127.0.0.1".into(),
            port: 1,
        }),
        ..client_options
    })
    .unwrap();
    let croot = client.root_poa().unwrap();
    let poa = croot
        .create_poa(
            "callbacks",
            Some(&croot.the_poa_manager()),
            &[Policy::Bidirectional(cb_policy)],
        )
        .unwrap();
    let callback = Arc::new(Callback {
        calls: AtomicUsize::new(0),
    });
    let cid = poa.activate_object(callback.clone()).unwrap();
    croot.the_poa_manager().activate().unwrap();
    let cb = poa.id_to_reference(&cid).unwrap();
    let url = format!("corbaloc:iiop:1.2@127.0.0.1:{}/{key}", proxy.port);
    let obj = client.string_to_object(&url).unwrap().unwrap();
    Setup {
        server,
        client,
        obj,
        cb,
        callback,
        later,
        proxy,
    }
}

fn call_back(s: &Setup, n: i32) -> Result<i32, Exception> {
    let reply = s
        .obj
        .request("call_back")
        .args(|out| {
            out.write_object(Some(&s.cb))?;
            out.write_i32(n);
            Ok(())
        })
        .invoke()?;
    Ok(reply.input().read::<i32>().unwrap())
}

fn call_back_later(s: &Setup, ms: u32) {
    s.obj
        .request("call_back_later")
        .args(|out| {
            out.write_object(Some(&s.cb))?;
            out.write_u32(ms);
            Ok(())
        })
        .send_oneway()
        .unwrap();
}

/// Waits up to five seconds for the outcome of `call_back_later`.
fn later(s: &Setup) -> Result<(), SystemException> {
    let until = Instant::now() + Duration::from_secs(5);
    while Instant::now() < until {
        if let Some(r) = s.later.lock().unwrap().take() {
            return r;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("the server did not call back");
}

fn offering() -> OrbOptions {
    OrbOptions {
        offer_bidir: true,
        ..OrbOptions::default()
    }
}

fn accepting() -> OrbOptions {
    OrbOptions {
        accept_bidir: true,
        ..OrbOptions::default()
    }
}

fn kind_of(e: Exception) -> SystemExceptionKind {
    match e {
        Exception::System(e) => e.kind().unwrap(),
        Exception::User(u) => panic!("unexpected user exception {}", u.repository_id()),
    }
}

#[test]
fn server_calls_back_over_the_clients_connection() {
    let s = setup(accepting(), offering(), Bidirectional::Both);
    assert_eq!(call_back(&s, 4).unwrap(), 41);
    assert_eq!(call_back(&s, 5).unwrap(), 51);
    assert_eq!(s.callback.calls.load(Ordering::SeqCst), 2);
    // One connection, opened by the client, both ways.
    assert_eq!(s.proxy.accepted.load(Ordering::SeqCst), 1);
}

#[test]
fn bidir_through_policy_overrides() {
    let s = setup(
        OrbOptions::default(),
        OrbOptions::default(),
        Bidirectional::Both,
    );
    let both = [PolicyValue::Poa(Policy::Bidirectional(Bidirectional::Both))];
    s.client
        .set_policy_overrides(&both, SetOverrideType::Add)
        .unwrap();
    s.server
        .set_policy_overrides(&both, SetOverrideType::Add)
        .unwrap();
    assert_eq!(
        s.client.policy_overrides(&[BIDIRECTIONAL_POLICY_TYPE]),
        both.to_vec()
    );
    assert_eq!(call_back(&s, 1).unwrap(), 11);
}

#[test]
fn server_that_does_not_accept_connects_to_the_client() {
    // The client is published at a port nobody listens on.
    let s = setup(OrbOptions::default(), offering(), Bidirectional::Both);
    assert_eq!(
        kind_of(call_back(&s, 1).unwrap_err()),
        SystemExceptionKind::TRANSIENT
    );
    assert_eq!(s.callback.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn client_that_does_not_offer_is_not_called_back_over_its_connection() {
    let s = setup(accepting(), OrbOptions::default(), Bidirectional::Both);
    assert_eq!(
        kind_of(call_back(&s, 1).unwrap_err()),
        SystemExceptionKind::TRANSIENT
    );
}

#[test]
fn poa_without_bidirectional_policy_refuses_callbacks() {
    let s = setup(accepting(), offering(), Bidirectional::Normal);
    assert_eq!(
        kind_of(call_back(&s, 1).unwrap_err()),
        SystemExceptionKind::NO_PERMISSION
    );
    assert_eq!(s.callback.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn client_keeps_the_connection_while_a_callback_runs() {
    // The client closes connections idle for 100 ms; the callback runs
    // for 400 ms while the client waits for nothing.
    let s = setup(
        OrbOptions {
            server_idle_timeout: None,
            ..accepting()
        },
        OrbOptions {
            client_idle_timeout: Some(Duration::from_millis(100)),
            ..offering()
        },
        Bidirectional::Both,
    );
    call_back_later(&s, 400);
    later(&s).unwrap();
    assert_eq!(s.proxy.accepted.load(Ordering::SeqCst), 1);
    // Then it is idle both ways and closed.
    let until = Instant::now() + Duration::from_secs(3);
    while s.proxy.open.load(Ordering::SeqCst) != 0 && Instant::now() < until {
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(s.proxy.open.load(Ordering::SeqCst), 0, "not closed");
}

#[test]
fn server_keeps_the_connection_while_it_awaits_a_callback_reply() {
    // The server closes connections idle for 100 ms; its call to the
    // client takes 400 ms.
    let s = setup(
        OrbOptions {
            server_idle_timeout: Some(Duration::from_millis(100)),
            ..accepting()
        },
        OrbOptions {
            client_idle_timeout: None,
            ..offering()
        },
        Bidirectional::Both,
    );
    call_back_later(&s, 400);
    later(&s).unwrap();
    assert_eq!(s.proxy.accepted.load(Ordering::SeqCst), 1);
}

#[test]
fn after_the_connection_closes_the_server_cannot_call_back() {
    let s = setup(
        OrbOptions {
            server_idle_timeout: Some(Duration::from_millis(100)),
            ..accepting()
        },
        OrbOptions {
            client_idle_timeout: None,
            ..offering()
        },
        Bidirectional::Both,
    );
    assert_eq!(call_back(&s, 2).unwrap(), 21);
    let until = Instant::now() + Duration::from_secs(3);
    while s.proxy.open.load(Ordering::SeqCst) != 0 && Instant::now() < until {
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(s.proxy.open.load(Ordering::SeqCst), 0, "not closed");
    // A new connection is offered again with its first request.
    assert_eq!(call_back(&s, 3).unwrap(), 31);
    assert_eq!(s.proxy.accepted.load(Ordering::SeqCst), 2);
}

#[test]
fn create_policy_takes_an_unsigned_short() {
    let orb = Orb::with_options(OrbOptions::default()).unwrap();
    let any = Any::new(TypeCode::simple(TCKind::UShort).unwrap(), Value::UShort(1));
    assert_eq!(
        orb.create_policy(BIDIRECTIONAL_POLICY_TYPE, &any).unwrap(),
        PolicyValue::Poa(Policy::Bidirectional(Bidirectional::Both))
    );
    let two = Any::new(TypeCode::simple(TCKind::UShort).unwrap(), Value::UShort(2));
    assert!(orb.create_policy(BIDIRECTIONAL_POLICY_TYPE, &two).is_err());
    let long = Any::new(TypeCode::simple(TCKind::Long).unwrap(), Value::Long(1));
    assert!(orb.create_policy(BIDIRECTIONAL_POLICY_TYPE, &long).is_err());
}

#[test]
fn options_parse() {
    let mut args: Vec<String> = [
        "prog",
        "-ORBofferBiDirectionalGIOP",
        "1",
        "-ORBacceptBiDirectionalGIOP=1",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut options = OrbOptions::default();
    options.parse_args(&mut args).unwrap();
    assert!(options.offer_bidir && options.accept_bidir);
    assert_eq!(args, ["prog"]);
}
