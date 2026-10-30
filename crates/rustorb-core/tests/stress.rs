//! Load tests: many connections, many requests on one connection, large
//! messages, nested calls with a small upcall pool, server restarts,
//! dropped connections and a stalled network (issue #49).
//!
//! The defaults keep each test to a few seconds so that `cargo test` runs
//! them every time. `RUSTORB_STRESS_SCALE=N` multiplies the number of
//! clients, requests and restarts; the scheduled workflow
//! (.github/workflows/soak.yml) runs them with a larger N.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use rustorb_core::cdr::ByteOrder;
use rustorb_core::giop::{IiopProfile, Ior, Version};
use rustorb_core::{
    CompletionStatus, EndpointAddr, Exception, IdAssignment, InStream, Lifespan, ObjectRef, Octets,
    Orb, OrbOptions, Policy, Servant, ServerReply, SystemException, SystemExceptionKind, minor,
};

const LOAD_ID: &str = "IDL:test/Load:1.0";

fn scale() -> usize {
    std::env::var("RUSTORB_STRESS_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(1)
}

fn sys(kind: SystemExceptionKind) -> SystemException {
    SystemException::new(kind, 0, CompletionStatus::No)
}

/// The servant of every test. `relay(depth)` calls `peer.relay(depth - 1)`
/// (or returns 0 at depth 0) and returns the depth it was given.
#[derive(Default)]
struct LoadServant {
    peer: OnceLock<ObjectRef>,
    calls: AtomicUsize,
    oneways: AtomicUsize,
}

impl Servant for LoadServant {
    fn repository_id(&self) -> &str {
        LOAD_ID
    }

    fn dispatch(
        &self,
        operation: &str,
        input: &mut InStream<'_>,
        reply: &mut ServerReply,
    ) -> Result<(), SystemException> {
        self.calls.fetch_add(1, Ordering::Relaxed);
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
            "echo_octets" => {
                let v = input.read_octet_seq()?.to_vec();
                reply.output().write_octet_seq(&v)?;
                Ok(())
            }
            "echo_octets_shared" => {
                let v = input.read_octet_seq_shared()?;
                reply.output().write_octet_seq_shared(&v)?;
                Ok(())
            }
            "sleep" => {
                let ms = input.read::<u32>()?;
                thread::sleep(Duration::from_millis(u64::from(ms)));
                Ok(())
            }
            "relay" => {
                let depth = input.read::<u32>()?;
                if depth > 0 {
                    let peer = self.peer.get().unwrap();
                    peer.request("relay")
                        .args(move |out| {
                            out.write_u32(depth - 1);
                            Ok(())
                        })
                        .invoke()
                        .map_err(|e| match e {
                            Exception::System(e) => e,
                            Exception::User(_) => sys(SystemExceptionKind::UNKNOWN),
                        })?;
                }
                reply.output().write_u32(depth);
                Ok(())
            }
            "post" => {
                let _ = input.read::<u32>()?;
                self.oneways.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            _ => Err(SystemException::new(
                SystemExceptionKind::BAD_OPERATION,
                minor::UNKNOWN_OPERATION,
                CompletionStatus::No,
            )),
        }
    }
}

fn server_options(port: u16) -> OrbOptions {
    OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port,
        }],
        ..OrbOptions::default()
    }
}

fn client_options() -> OrbOptions {
    OrbOptions {
        io_threads: 1,
        ..OrbOptions::default()
    }
}

/// `servant` active as `obj` in a USER_ID POA, PERSISTENT when the ORB
/// listens on a fixed port so that the reference survives a restart.
fn serve(orb: &Orb, servant: Arc<LoadServant>) -> ObjectRef {
    let root = orb.root_poa().unwrap();
    let lifespan = if orb.options().endpoints.iter().all(|e| e.port != 0) {
        Lifespan::Persistent
    } else {
        Lifespan::Transient
    };
    let poa = root
        .create_poa(
            "load",
            Some(&root.the_poa_manager()),
            &[
                Policy::Lifespan(lifespan),
                Policy::IdAssignment(IdAssignment::User),
            ],
        )
        .unwrap();
    poa.activate_object_with_id(b"obj", servant).unwrap();
    root.the_poa_manager().activate().unwrap();
    poa.id_to_reference(b"obj").unwrap()
}

/// `obj` as seen by `client` (a reference that goes over TCP).
fn remote(client: &Orb, server: &Orb, obj: &ObjectRef) -> ObjectRef {
    client
        .string_to_object(&server.object_to_string(Some(obj)).unwrap())
        .unwrap()
        .unwrap()
}

/// `obj` with its IIOP profile pointing to `port` on localhost.
fn via_port(orb: &Orb, obj: &ObjectRef, port: u16) -> ObjectRef {
    let mut profile: IiopProfile = obj.ior().iiop_profiles().next().unwrap().unwrap();
    profile.host = "127.0.0.1".into();
    profile.port = port;
    let ior = Ior {
        type_id: LOAD_ID.into(),
        profiles: vec![profile.to_tagged_profile(ByteOrder::Big).unwrap()],
    };
    orb.string_to_object(&ior.to_ior_string(ByteOrder::Big).unwrap())
        .unwrap()
        .unwrap()
}

fn echo_long(obj: &ObjectRef, v: i32) -> Result<i32, Exception> {
    let reply = obj
        .request("echo_long")
        .args(move |out| {
            out.write_i32(v);
            Ok(())
        })
        .invoke()?;
    Ok(reply.input().read::<i32>()?)
}

/// Like [`echo_octets`], with the octets shared rather than copied on both
/// sides (vectored writes of the large ones).
fn echo_octets_shared(obj: &ObjectRef, data: &Octets) -> Result<Octets, Exception> {
    let reply = obj
        .request("echo_octets_shared")
        .args(|out| Ok(out.write_octet_seq_shared(data)?))
        .invoke()?;
    Ok(reply
        .input()
        .read_octet_seq_shared()
        .map_err(SystemException::from)?)
}

fn echo_octets(obj: &ObjectRef, data: &[u8]) -> Result<Vec<u8>, Exception> {
    let reply = obj
        .request("echo_octets")
        .args(|out| Ok(out.write_octet_seq(data)?))
        .invoke()?;
    Ok(reply
        .input()
        .read_octet_seq()
        .map_err(SystemException::from)?
        .to_vec())
}

fn relay(obj: &ObjectRef, depth: u32) -> Result<u32, Exception> {
    let reply = obj
        .request("relay")
        .args(move |out| {
            out.write_u32(depth);
            Ok(())
        })
        .invoke()?;
    Ok(reply.input().read::<u32>()?)
}

fn kind_of(e: &Exception) -> SystemExceptionKind {
    match e {
        Exception::System(e) => e.kind().unwrap(),
        Exception::User(u) => panic!("unexpected user exception {u:?}"),
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A server ORB on `port`, retried while the port is still taken by the
/// previous one.
fn server_on(port: u16, servant: Arc<LoadServant>) -> (Orb, ObjectRef) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let orb = Orb::with_options(server_options(port)).unwrap();
        match orb.root_poa() {
            Ok(_) => {
                let obj = serve(&orb, servant);
                return (orb, obj);
            }
            Err(e) if Instant::now() < deadline => {
                let _ = orb.destroy();
                eprintln!("port {port} not free yet: {e:?}");
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => panic!("cannot listen on {port}: {e:?}"),
        }
    }
}

fn rate(n: usize, d: Duration) -> String {
    format!(
        "{n} calls in {d:.2?} ({:.0} calls/s)",
        n as f64 / d.as_secs_f64()
    )
}

#[test]
fn many_clients_each_on_its_own_connection() {
    let server = Orb::with_options(server_options(0)).unwrap();
    let servant = Arc::new(LoadServant::default());
    let obj = serve(&server, servant.clone());
    let ior = server.object_to_string(Some(&obj)).unwrap();
    let clients = 32 * scale();
    let calls = 50;
    let start = Instant::now();
    let threads: Vec<_> = (0..clients)
        .map(|c| {
            let ior = ior.clone();
            thread::spawn(move || {
                let orb = Orb::with_options(client_options()).unwrap();
                let obj = orb.string_to_object(&ior).unwrap().unwrap();
                for i in 0..calls {
                    let v = (c * 1000 + i) as i32;
                    assert_eq!(echo_long(&obj, v).unwrap(), v);
                }
                orb.destroy().unwrap();
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    eprintln!(
        "{clients} connections: {}",
        rate(clients * calls, start.elapsed())
    );
    assert_eq!(servant.calls.load(Ordering::Relaxed), clients * calls);
    server.destroy().unwrap();
}

/// Long runs of calls, one after another, on each of several connections:
/// the caller reads its own reply, or the reader task does, and the
/// server's upcall thread reads the next request, or the I/O task does
/// (docs/read-ownership.md). A reply or request lost in a hand-over shows
/// up as `TIMEOUT`.
#[test]
fn back_to_back_calls_hand_the_reading_over() {
    let server = Orb::with_options(server_options(0)).unwrap();
    let servant = Arc::new(LoadServant::default());
    let obj = serve(&server, servant.clone());
    let ior = server.object_to_string(Some(&obj)).unwrap();
    let clients = 8;
    let calls = 2000 * scale();
    let start = Instant::now();
    let threads: Vec<_> = (0..clients)
        .map(|c| {
            let ior = ior.clone();
            thread::spawn(move || {
                let orb = Orb::with_options(client_options()).unwrap();
                let obj = orb.string_to_object(&ior).unwrap().unwrap();
                obj.set_call_timeout(Some(Some(Duration::from_secs(10))));
                for i in 0..calls {
                    let v = (c * 100_000 + i) as i32;
                    assert_eq!(echo_long(&obj, v).unwrap(), v);
                }
                orb.destroy().unwrap();
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    eprintln!(
        "{clients} connections back to back: {}",
        rate(clients * calls, start.elapsed())
    );
    assert_eq!(servant.calls.load(Ordering::Relaxed), clients * calls);
    server.destroy().unwrap();
}

#[test]
fn many_threads_share_one_connection() {
    let server = Orb::with_options(server_options(0)).unwrap();
    let servant = Arc::new(LoadServant::default());
    let obj = serve(&server, servant.clone());
    let client = Orb::with_options(client_options()).unwrap();
    let obj = remote(&client, &server, &obj);
    let threads = 16;
    let calls = 500 * scale();
    let start = Instant::now();
    let workers: Vec<_> = (0..threads)
        .map(|t| {
            let obj = obj.clone();
            thread::spawn(move || {
                for i in 0..calls {
                    match i % 3 {
                        0 => {
                            let v = (t * calls + i) as i32;
                            assert_eq!(echo_long(&obj, v).unwrap(), v);
                        }
                        1 => {
                            let s = format!("thread {t} call {i}");
                            let reply = obj
                                .request("echo_string")
                                .args(|out| out.write_string(&s))
                                .invoke()
                                .unwrap();
                            assert_eq!(reply.input().read_string().unwrap(), s);
                        }
                        _ => obj
                            .request("post")
                            .args(move |out| {
                                out.write_u32(i as u32);
                                Ok(())
                            })
                            .send_oneway()
                            .unwrap(),
                    }
                }
            })
        })
        .collect();
    for w in workers {
        w.join().unwrap();
    }
    eprintln!("one connection: {}", rate(threads * calls, start.elapsed()));
    let oneways = threads * calls.div_ceil(3);
    let deadline = Instant::now() + Duration::from_secs(20);
    while servant.oneways.load(Ordering::Relaxed) < threads * (calls / 3) {
        assert!(Instant::now() < deadline, "oneway requests were lost");
        thread::sleep(Duration::from_millis(10));
    }
    assert!(servant.oneways.load(Ordering::Relaxed) <= oneways);
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn large_messages_in_both_directions() {
    let server = Orb::with_options(server_options(0)).unwrap();
    let obj = serve(&server, Arc::new(LoadServant::default()));
    let sizes = [0, 1, 4093, 65_536, 1 << 20, 16 << 20];
    for version in [Version::V1_0, Version::V1_1, Version::V1_2] {
        let client = Orb::with_options(OrbOptions {
            max_giop_version: version,
            ..client_options()
        })
        .unwrap();
        let obj = remote(&client, &server, &obj);
        // Several large messages at once on the one connection, copied
        // and shared (written from where they are, read in place).
        let workers: Vec<_> = sizes
            .iter()
            .flat_map(|&size| [(size, false), (size, true)])
            .map(|(size, shared)| {
                let obj = obj.clone();
                thread::spawn(move || {
                    let data: Vec<u8> = (0..size).map(|i| (i * 7 + size) as u8).collect();
                    let start = Instant::now();
                    if shared {
                        let back = echo_octets_shared(&obj, &Octets::from(data.clone())).unwrap();
                        assert!(back == data, "{size} shared octets came back changed");
                    } else {
                        let back = echo_octets(&obj, &data).unwrap();
                        assert!(back == data, "{size} octets came back changed");
                    }
                    (size, shared, start.elapsed())
                })
            })
            .collect();
        for w in workers {
            let (size, shared, took) = w.join().unwrap();
            if size >= 1 << 20 {
                let how = if shared { "shared" } else { "copied" };
                eprintln!("GIOP {version:?}: {size} octets ({how}) there and back in {took:.2?}");
            }
        }
        client.destroy().unwrap();
    }
    server.destroy().unwrap();
}

#[test]
fn oversized_requests_fail_and_the_server_carries_on() {
    let server = Orb::with_options(OrbOptions {
        max_message_size: 1 << 20,
        ..server_options(0)
    })
    .unwrap();
    let obj = serve(&server, Arc::new(LoadServant::default()));
    let client = Orb::with_options(client_options()).unwrap();
    let obj = remote(&client, &server, &obj);
    let big = vec![0x5a; 2 << 20];
    let e = echo_octets(&obj, &big).unwrap_err();
    eprintln!("oversized request: {e:?}");
    // The connection may be gone; the next call opens another one.
    assert_eq!(echo_long(&obj, 5).unwrap(), 5);
    let fits = vec![0xa5; 512 << 10];
    assert_eq!(echo_octets(&obj, &fits).unwrap(), fits);
    client.destroy().unwrap();
    server.destroy().unwrap();
}

/// Two ORBs whose servants call each other: a call of depth `d` holds
/// `d + 1` upcall threads, alternately of each ORB, until it returns.
fn ping_pong(pool: usize, timeout: Option<Duration>) -> (Orb, Orb, ObjectRef) {
    let options = |port| OrbOptions {
        min_server_threads: 1,
        max_server_threads: pool,
        server_thread_idle_timeout: Duration::from_millis(200),
        request_timeout: timeout,
        ..server_options(port)
    };
    let a = Orb::with_options(options(0)).unwrap();
    let b = Orb::with_options(options(0)).unwrap();
    let sa = Arc::new(LoadServant::default());
    let sb = Arc::new(LoadServant::default());
    let obj_a = serve(&a, sa.clone());
    let obj_b = serve(&b, sb.clone());
    let _ = sa.peer.set(remote(&a, &b, &obj_b));
    let _ = sb.peer.set(remote(&b, &a, &obj_a));
    (a, b, obj_a)
}

#[test]
fn nested_calls_within_the_pool_size_complete() {
    let pool = 4;
    let (a, b, obj_a) = ping_pong(pool, None);
    let client = Orb::with_options(client_options()).unwrap();
    let obj = remote(&client, &a, &obj_a);
    // A chain of depth 3 runs relay(3) and relay(1) on A and relay(2) and
    // relay(0) on B; two of them hold the whole pool of each ORB.
    for _ in 0..10 * scale() {
        let chains: Vec<_> = (0..2)
            .map(|_| {
                let obj = obj.clone();
                thread::spawn(move || relay(&obj, 3))
            })
            .collect();
        for c in chains {
            assert_eq!(c.join().unwrap().unwrap(), 3);
        }
    }
    client.destroy().unwrap();
    a.destroy().unwrap();
    b.destroy().unwrap();
}

#[test]
fn an_exhausted_pool_times_out_and_recovers() {
    let pool = 4;
    let timeout = Duration::from_secs(2);
    let (a, b, obj_a) = ping_pong(pool, Some(timeout));
    let client = Orb::with_options(OrbOptions {
        request_timeout: Some(timeout),
        ..client_options()
    })
    .unwrap();
    let obj = remote(&client, &a, &obj_a);
    // Eight chains of depth 6 want 32 threads of A and 24 of B: the pools fill
    // with upcalls that wait for each other, and the queued requests wait
    // behind them. Every call must end with TIMEOUT (or succeed), not hang.
    let start = Instant::now();
    let chains: Vec<_> = (0..8)
        .map(|_| {
            let obj = obj.clone();
            thread::spawn(move || relay(&obj, 6))
        })
        .collect();
    let mut timeouts = 0;
    for c in chains {
        match c.join().unwrap() {
            Ok(depth) => assert_eq!(depth, 6),
            Err(e) => {
                assert_eq!(kind_of(&e), SystemExceptionKind::TIMEOUT, "{e:?}");
                timeouts += 1;
            }
        }
    }
    let took = start.elapsed();
    eprintln!("exhausted pool: {timeouts} of 8 chains timed out after {took:.2?}");
    assert!(took < timeout * 4, "the calls took {took:?}");
    // The upcalls unwind as their own nested calls time out; then the
    // pools serve again.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match relay(&obj, 4) {
            Ok(4) => break,
            Ok(d) => panic!("relay returned {d}"),
            Err(e) => {
                assert!(
                    Instant::now() < deadline,
                    "the pools did not recover: {e:?}"
                );
                thread::sleep(Duration::from_millis(200));
            }
        }
    }
    client.destroy().unwrap();
    a.destroy().unwrap();
    b.destroy().unwrap();
}

#[test]
fn nested_calls_within_one_orb_do_not_use_the_pool() {
    // A servant that calls itself through its own ORB runs the inner call
    // on its own thread, so even a pool of two serves deep recursion from
    // many clients at once.
    let server = Orb::with_options(OrbOptions {
        min_server_threads: 1,
        max_server_threads: 2,
        request_timeout: Some(Duration::from_secs(10)),
        ..server_options(0)
    })
    .unwrap();
    let servant = Arc::new(LoadServant::default());
    let obj = serve(&server, servant.clone());
    let _ = servant.peer.set(obj.clone());
    let client = Orb::with_options(client_options()).unwrap();
    let obj = remote(&client, &server, &obj);
    let workers: Vec<_> = (0..16)
        .map(|_| {
            let obj = obj.clone();
            thread::spawn(move || {
                for _ in 0..5 * scale() {
                    assert_eq!(relay(&obj, 16).unwrap(), 16);
                }
            })
        })
        .collect();
    for w in workers {
        w.join().unwrap();
    }
    client.destroy().unwrap();
    server.destroy().unwrap();
}

/// Calls `echo_long` on `obj` from `threads` threads until `stop` is set,
/// counting successes and failures by kind. Only the kinds in `allowed`
/// may fail.
struct Callers {
    stop: Arc<AtomicBool>,
    ok: Arc<AtomicUsize>,
    failed: Arc<Mutex<Vec<SystemExceptionKind>>>,
    threads: Vec<thread::JoinHandle<()>>,
}

impl Callers {
    fn start(obj: &ObjectRef, threads: usize) -> Callers {
        let stop = Arc::new(AtomicBool::new(false));
        let ok = Arc::new(AtomicUsize::new(0));
        let failed = Arc::new(Mutex::new(Vec::new()));
        let threads = (0..threads)
            .map(|t| {
                let (obj, stop, ok, failed) =
                    (obj.clone(), stop.clone(), ok.clone(), failed.clone());
                thread::spawn(move || {
                    let mut i = 0;
                    while !stop.load(Ordering::Relaxed) {
                        let v = (t * 1_000_000 + i) as i32;
                        match echo_long(&obj, v) {
                            Ok(got) => {
                                assert_eq!(got, v);
                                ok.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(e) => {
                                failed.lock().unwrap().push(kind_of(&e));
                                thread::sleep(Duration::from_millis(5));
                            }
                        }
                        i += 1;
                    }
                })
            })
            .collect();
        Callers {
            stop,
            ok,
            failed,
            threads,
        }
    }

    /// Waits until `n` more calls have succeeded.
    fn wait_for(&self, n: usize, what: &str) {
        let target = self.ok.load(Ordering::Relaxed) + n;
        let deadline = Instant::now() + Duration::from_secs(30);
        while self.ok.load(Ordering::Relaxed) < target {
            assert!(Instant::now() < deadline, "no calls succeed {what}");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn finish(self, allowed: &[SystemExceptionKind]) -> (usize, Vec<SystemExceptionKind>) {
        self.stop.store(true, Ordering::Relaxed);
        for t in self.threads {
            t.join().unwrap();
        }
        let failed = self.failed.lock().unwrap().clone();
        for kind in &failed {
            assert!(allowed.contains(kind), "unexpected {kind:?}");
        }
        (self.ok.load(Ordering::Relaxed), failed)
    }
}

#[test]
fn clients_reconnect_after_the_server_restarts() {
    let port = free_port();
    let servant = Arc::new(LoadServant::default());
    let (mut server, obj) = server_on(port, servant.clone());
    let client = Orb::with_options(OrbOptions {
        request_timeout: Some(Duration::from_secs(5)),
        ..client_options()
    })
    .unwrap();
    let obj = remote(&client, &server, &obj);
    let callers = Callers::start(&obj, 4);
    callers.wait_for(100, "at first");
    let restarts = 3 * scale();
    for _ in 0..restarts {
        // The shutdown lets running requests reply and closes the
        // connections with CloseConnection; calls in between find the
        // port closed.
        server.destroy().unwrap();
        thread::sleep(Duration::from_millis(50));
        server = server_on(port, servant.clone()).0;
        callers.wait_for(100, "after a restart");
    }
    // A request that reaches the old server after its shutdown began,
    // on a connection not yet closed, finds the POA manager inactive:
    // OBJ_ADAPTER (see docs/load-testing.md).
    let (ok, failed) = callers.finish(&[
        SystemExceptionKind::TRANSIENT,
        SystemExceptionKind::COMM_FAILURE,
        SystemExceptionKind::OBJ_ADAPTER,
    ]);
    eprintln!(
        "{restarts} restarts: {ok} calls succeeded, {} failed",
        failed.len()
    );
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn calls_in_flight_end_when_the_client_shuts_down() {
    let server = Orb::with_options(server_options(0)).unwrap();
    let obj = serve(&server, Arc::new(LoadServant::default()));
    let client = Orb::with_options(client_options()).unwrap();
    let obj = remote(&client, &server, &obj);
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let obj = obj.clone();
            thread::spawn(move || {
                loop {
                    let r = obj
                        .request("sleep")
                        .args(|out| {
                            out.write_u32(20);
                            Ok(())
                        })
                        .invoke();
                    if let Err(e) = r {
                        return kind_of(&e);
                    }
                }
            })
        })
        .collect();
    thread::sleep(Duration::from_millis(200));
    let start = Instant::now();
    client.shutdown(true).unwrap();
    for w in workers {
        let kind = w.join().unwrap();
        assert!(
            [
                SystemExceptionKind::BAD_INV_ORDER,
                SystemExceptionKind::COMM_FAILURE,
                SystemExceptionKind::TRANSIENT,
            ]
            .contains(&kind),
            "{kind:?}"
        );
    }
    assert!(start.elapsed() < Duration::from_secs(10));
    client.destroy().unwrap();
    // The server is still fine.
    let other = Orb::with_options(client_options()).unwrap();
    assert_eq!(echo_long(&remote(&other, &server, &obj), 3).unwrap(), 3);
    other.destroy().unwrap();
    server.destroy().unwrap();
}

/// A TCP relay between the client and the server that can drop every
/// connection (`cut`), or stop moving bytes for a while: both ways
/// (`stalled`) or only from the server to the client (`replies_stalled`).
struct Proxy {
    port: u16,
    stalled: Arc<AtomicBool>,
    replies_stalled: Arc<AtomicBool>,
    conns: Arc<Mutex<Vec<TcpStream>>>,
}

impl Proxy {
    fn start(target: u16) -> Proxy {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let stalled = Arc::new(AtomicBool::new(false));
        let replies_stalled = Arc::new(AtomicBool::new(false));
        let conns = Arc::new(Mutex::new(Vec::new()));
        let (both, replies, c) = (stalled.clone(), replies_stalled.clone(), conns.clone());
        thread::spawn(move || {
            for down in listener.incoming() {
                let Ok(down) = down else { return };
                let Ok(up) = TcpStream::connect(("127.0.0.1", target)) else {
                    continue;
                };
                let mut list = c.lock().unwrap();
                list.push(down.try_clone().unwrap());
                list.push(up.try_clone().unwrap());
                drop(list);
                let requests = (down.try_clone().unwrap(), up.try_clone().unwrap());
                let flags = vec![both.clone()];
                thread::spawn(move || pump(requests.0, requests.1, &flags));
                let flags = vec![both.clone(), replies.clone()];
                thread::spawn(move || pump(up, down, &flags));
            }
        });
        Proxy {
            port,
            stalled,
            replies_stalled,
            conns,
        }
    }

    fn cut(&self) {
        for conn in self.conns.lock().unwrap().drain(..) {
            let _ = conn.shutdown(Shutdown::Both);
        }
    }
}

fn pump(mut from: TcpStream, mut to: TcpStream, stalled: &[Arc<AtomicBool>]) {
    let wait = || {
        while stalled.iter().any(|s| s.load(Ordering::Relaxed)) {
            thread::sleep(Duration::from_millis(5));
        }
    };
    let mut buf = vec![0; 65_536];
    loop {
        wait();
        match from.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                wait();
                if to.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
        }
    }
    let _ = to.shutdown(Shutdown::Both);
    let _ = from.shutdown(Shutdown::Both);
}

#[test]
fn dropped_connections_fail_the_calls_on_them_and_the_next_ones_reconnect() {
    let server = Orb::with_options(server_options(0)).unwrap();
    let obj = serve(&server, Arc::new(LoadServant::default()));
    let port = obj.ior().iiop_profiles().next().unwrap().unwrap().port;
    let proxy = Proxy::start(port);
    let client = Orb::with_options(OrbOptions {
        request_timeout: Some(Duration::from_secs(5)),
        ..client_options()
    })
    .unwrap();
    let obj = via_port(&client, &obj, proxy.port);
    let callers = Callers::start(&obj, 4);
    callers.wait_for(100, "at first");
    let cuts = 5 * scale();
    for _ in 0..cuts {
        proxy.cut();
        callers.wait_for(100, "after the connection was dropped");
    }
    // A call that was sent may or may not have run: COMM_FAILURE,
    // completed maybe. Calls not yet sent go to a new connection.
    let (ok, failed) = callers.finish(&[
        SystemExceptionKind::COMM_FAILURE,
        SystemExceptionKind::TRANSIENT,
    ]);
    eprintln!("{cuts} cuts: {ok} calls succeeded, {} failed", failed.len());
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn a_stalled_network_times_out_and_the_connection_recovers() {
    let server = Orb::with_options(server_options(0)).unwrap();
    let obj = serve(&server, Arc::new(LoadServant::default()));
    let port = obj.ior().iiop_profiles().next().unwrap().unwrap().port;
    let proxy = Proxy::start(port);
    let timeout = Duration::from_millis(500);
    let client = Orb::with_options(OrbOptions {
        request_timeout: Some(timeout),
        ..client_options()
    })
    .unwrap();
    let obj = via_port(&client, &obj, proxy.port);
    let callers = Callers::start(&obj, 4);
    callers.wait_for(100, "at first");
    for _ in 0..2 * scale() {
        // Nothing moves for longer than the call timeout: the calls time
        // out. The bytes held meanwhile arrive afterwards, and the late
        // replies are dropped.
        proxy.stalled.store(true, Ordering::Relaxed);
        thread::sleep(timeout * 3);
        proxy.stalled.store(false, Ordering::Relaxed);
        callers.wait_for(100, "after the network came back");
    }
    let (ok, failed) = callers.finish(&[SystemExceptionKind::TIMEOUT]);
    assert!(!failed.is_empty(), "no call timed out");
    eprintln!("stalls: {ok} calls succeeded, {} timed out", failed.len());
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn a_stalled_connection_does_not_hold_up_the_others() {
    let server = Orb::with_options(server_options(0)).unwrap();
    let obj = serve(&server, Arc::new(LoadServant::default()));
    let client = Orb::with_options(client_options()).unwrap();
    let direct = remote(&client, &server, &obj);
    let port = obj.ior().iiop_profiles().next().unwrap().unwrap().port;
    let proxy = Proxy::start(port);
    let stuck_client = Orb::with_options(OrbOptions {
        request_timeout: Some(Duration::from_millis(500)),
        ..client_options()
    })
    .unwrap();
    let stuck = via_port(&stuck_client, &obj, proxy.port);
    assert_eq!(echo_long(&stuck, 1).unwrap(), 1);
    let big = vec![1; 16 << 20];

    // The server stops reading: the client cannot write the whole request
    // and gives up at the call timeout. The request did not reach the
    // server (completed no), and the cut-off stream is closed. Where the
    // socket buffers take the whole request (Windows' loopback can), the
    // write ends and the call times out waiting for the reply instead
    // (completed maybe).
    proxy.stalled.store(true, Ordering::Relaxed);
    let start = Instant::now();
    let e = system(echo_octets(&stuck, &big));
    assert!(e.is(SystemExceptionKind::TIMEOUT), "{e:?}");
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "{:?}",
        start.elapsed()
    );
    if cfg!(target_os = "linux") {
        assert_eq!(e.completed, CompletionStatus::No);
    }
    for i in 0..100 {
        assert_eq!(echo_long(&direct, i).unwrap(), i);
    }
    proxy.stalled.store(false, Ordering::Relaxed);
    assert_eq!(echo_long(&stuck, 2).unwrap(), 2);

    // The client stops reading: the server cannot write the whole reply.
    // That holds up neither the server's other connections nor the
    // client's next calls on a new connection once the stream is dropped.
    proxy.replies_stalled.store(true, Ordering::Relaxed);
    let e = system(echo_octets(&stuck, &big));
    assert!(e.is(SystemExceptionKind::TIMEOUT), "{e:?}");
    for i in 0..100 {
        assert_eq!(echo_long(&direct, i).unwrap(), i);
    }
    proxy.cut();
    proxy.replies_stalled.store(false, Ordering::Relaxed);
    let deadline = Instant::now() + Duration::from_secs(10);
    while echo_long(&stuck, 3).ok() != Some(3) {
        assert!(Instant::now() < deadline, "no new connection");
    }
    stuck_client.destroy().unwrap();
    client.destroy().unwrap();
    server.destroy().unwrap();
}

fn system<T: std::fmt::Debug>(result: Result<T, Exception>) -> SystemException {
    match result {
        Err(Exception::System(e)) => e,
        other => panic!("expected a system exception, got {other:?}"),
    }
}
