//! Closing idle connections (docs/connections.md): the client and the
//! server close connections nothing went over for a while, and calls made
//! afterwards reconnect without the caller seeing it.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use rustorb_core::{
    CompletionStatus, EndpointAddr, InStream, ObjectRef, Orb, OrbOptions, Servant, ServerReply,
    SystemException, SystemExceptionKind,
};

const ECHO_ID: &str = "IDL:test/Echo:1.0";

struct Echo {
    oneways: AtomicUsize,
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
            "echo_long" => {
                let v = input.read::<i32>()?;
                reply.output().write_i32(v);
                Ok(())
            }
            // sleep ms: answers after ms milliseconds.
            "sleep" => {
                let ms = input.read::<u32>()?;
                thread::sleep(Duration::from_millis(ms.into()));
                Ok(())
            }
            "note" => {
                self.oneways.fetch_add(1, Ordering::SeqCst);
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

/// A TCP proxy in front of a server that counts the connections through
/// it: accepted, and still open in both directions.
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

    fn accepted(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }

    fn open(&self) -> usize {
        self.open.load(Ordering::SeqCst)
    }
}

/// Waits up to two seconds for `cond`.
fn eventually(cond: impl Fn() -> bool) -> bool {
    let until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < until {
        if cond() {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    cond()
}

struct Setup {
    server: Orb,
    client: Orb,
    obj: ObjectRef,
    proxy: Proxy,
    echo: Arc<Echo>,
}

impl Drop for Setup {
    fn drop(&mut self) {
        let _ = self.client.destroy();
        let _ = self.server.destroy();
    }
}

/// A server ORB with one Echo object, a proxy in front of it, and a client
/// ORB with a reference to the object through the proxy.
fn setup(server_options: OrbOptions, client_options: OrbOptions) -> Setup {
    let server = Orb::with_options(OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..server_options
    })
    .unwrap();
    let root = server.root_poa().unwrap();
    let echo = Arc::new(Echo {
        oneways: AtomicUsize::new(0),
    });
    let id = root.activate_object(echo.clone()).unwrap();
    root.the_poa_manager().activate().unwrap();
    let local = root.id_to_reference(&id).unwrap();
    let ior = server.object_to_string(Some(&local)).unwrap();
    let port = server_port(&server, &ior);
    let proxy = Proxy::start(port);
    let client = Orb::with_options(client_options).unwrap();
    let url = format!(
        "corbaloc:iiop:1.2@127.0.0.1:{}/{}",
        proxy.port,
        object_key_url(&server, &ior)
    );
    let obj = client.string_to_object(&url).unwrap().unwrap();
    Setup {
        server,
        client,
        obj,
        proxy,
        echo,
    }
}

/// The port of the IIOP profile of `ior`.
fn server_port(orb: &Orb, ior: &str) -> u16 {
    let obj = orb.string_to_object(ior).unwrap().unwrap();
    obj.ior().iiop_profiles().next().unwrap().unwrap().port
}

/// The object key of `ior`, %-escaped for a corbaloc URL.
fn object_key_url(orb: &Orb, ior: &str) -> String {
    let obj = orb.string_to_object(ior).unwrap().unwrap();
    let profile = obj.ior().iiop_profiles().next().unwrap().unwrap();
    profile
        .object_key
        .iter()
        .map(|b| format!("%{b:02x}"))
        .collect()
}

fn echo(obj: &ObjectRef, v: i32) -> Result<i32, rustorb_core::Exception> {
    let reply = obj
        .request("echo_long")
        .args(|out| {
            out.write_i32(v);
            Ok(())
        })
        .invoke()?;
    Ok(reply.input().read::<i32>().unwrap())
}

fn options(client_idle: Option<u64>, server_idle: Option<u64>) -> OrbOptions {
    OrbOptions {
        client_idle_timeout: client_idle.map(Duration::from_millis),
        server_idle_timeout: server_idle.map(Duration::from_millis),
        ..OrbOptions::default()
    }
}

#[test]
fn client_closes_idle_connection_and_reconnects() {
    let s = setup(options(None, None), options(Some(100), None));
    assert_eq!(echo(&s.obj, 1).unwrap(), 1);
    assert_eq!(s.proxy.accepted(), 1);
    assert!(eventually(|| s.proxy.open() == 0), "not closed");
    assert_eq!(echo(&s.obj, 2).unwrap(), 2);
    assert_eq!(s.proxy.accepted(), 2);
}

#[test]
fn server_closes_idle_connection_and_client_reconnects() {
    let s = setup(options(None, Some(100)), options(None, None));
    assert_eq!(echo(&s.obj, 1).unwrap(), 1);
    assert!(eventually(|| s.proxy.open() == 0), "not closed");
    assert_eq!(echo(&s.obj, 2).unwrap(), 2);
    assert_eq!(s.proxy.accepted(), 2);
}

#[test]
fn not_closed_while_a_reply_is_awaited() {
    let s = setup(options(None, Some(50)), options(Some(50), None));
    s.obj
        .request("sleep")
        .args(|out| {
            out.write_u32(400);
            Ok(())
        })
        .invoke()
        .unwrap();
    assert_eq!(s.proxy.accepted(), 1);
}

#[test]
fn no_timeout_keeps_connections() {
    let s = setup(options(None, None), options(None, None));
    assert_eq!(echo(&s.obj, 1).unwrap(), 1);
    thread::sleep(Duration::from_millis(300));
    assert_eq!(s.proxy.open(), 1);
    assert_eq!(echo(&s.obj, 2).unwrap(), 2);
    assert_eq!(s.proxy.accepted(), 1);
}

/// Calls spaced around the idle timeout race the closing: every call must
/// still succeed, and every oneway arrive.
#[test]
fn calls_racing_the_close_succeed() {
    for (client, server) in [(None, Some(20)), (Some(20), None), (Some(20), Some(20))] {
        let s = setup(options(None, server), options(client, None));
        let threads = 4;
        let rounds = 40;
        let start = Arc::new(Barrier::new(threads));
        let workers: Vec<_> = (0..threads)
            .map(|t| {
                let (obj, start) = (s.obj.clone(), Arc::clone(&start));
                thread::spawn(move || {
                    start.wait();
                    for i in 0..rounds {
                        let v = i32::try_from(t * 1000 + i).unwrap();
                        assert_eq!(echo(&obj, v).unwrap(), v);
                        obj.request("note").send_oneway().unwrap();
                        thread::sleep(Duration::from_millis(((t + i) % 5 * 8) as u64));
                    }
                })
            })
            .collect();
        for w in workers {
            w.join().unwrap();
        }
        assert!(
            eventually(|| s.echo.oneways.load(Ordering::SeqCst) == threads * rounds),
            "oneways lost: {} of {}",
            s.echo.oneways.load(Ordering::SeqCst),
            threads * rounds
        );
        assert!(s.proxy.accepted() > 1, "never closed");
    }
}

/// A caller that read its reply keeps the reading for its next call
/// (`-ORBfollowLinger`, docs/read-ownership.md). With a linger far longer
/// than the test, nothing gives the reading back to the I/O task on its
/// own: the calls must still work when the server closes the connection
/// meanwhile, and when other threads call at the same time.
fn keeping(client_idle: Option<u64>) -> OrbOptions {
    OrbOptions {
        follow_linger: Duration::from_secs(30),
        ..options(client_idle, None)
    }
}

#[test]
fn server_closes_the_connection_a_caller_keeps_reading() {
    let s = setup(options(None, Some(100)), keeping(None));
    let started = Instant::now();
    assert_eq!(echo(&s.obj, 1).unwrap(), 1);
    // The server's CloseConnection is read with the next call.
    thread::sleep(Duration::from_millis(400));
    assert_eq!(echo(&s.obj, 2).unwrap(), 2);
    assert_eq!(s.proxy.accepted(), 2);
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn calls_from_many_threads_while_a_caller_keeps_reading() {
    for server in [None, Some(20)] {
        let s = setup(options(None, server), keeping(None));
        let started = Instant::now();
        let threads = 4;
        let rounds = 100;
        let start = Arc::new(Barrier::new(threads));
        let workers: Vec<_> = (0..threads)
            .map(|t| {
                let (obj, start) = (s.obj.clone(), Arc::clone(&start));
                thread::spawn(move || {
                    start.wait();
                    for i in 0..rounds {
                        let v = i32::try_from(t * 1000 + i).unwrap();
                        assert_eq!(echo(&obj, v).unwrap(), v);
                        if (t + i) % 7 == 0 {
                            thread::sleep(Duration::from_millis(((t + i) % 3 * 10) as u64));
                        }
                    }
                })
            })
            .collect();
        for w in workers {
            w.join().unwrap();
        }
        // No call waited for the linger.
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "{:?}",
            started.elapsed()
        );
    }
}

/// The server sends `CloseConnection` on a connection that sent nothing.
#[test]
fn server_sends_close_connection() {
    let server = Orb::with_options(OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..options(None, Some(50))
    })
    .unwrap();
    let root = server.root_poa().unwrap();
    let id = root
        .activate_object(Arc::new(Echo {
            oneways: AtomicUsize::new(0),
        }))
        .unwrap();
    let ior = server
        .object_to_string(Some(&root.id_to_reference(&id).unwrap()))
        .unwrap();
    let port = server_port(&server, &ior);
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut header = [0; 12];
    stream.read_exact(&mut header).unwrap();
    assert_eq!(&header[..4], b"GIOP");
    assert_eq!(header[7], 5, "not CloseConnection");
    let mut rest = Vec::new();
    assert_eq!(stream.read_to_end(&mut rest).unwrap(), 0);
    server.destroy().unwrap();
}

/// At `-ORBmaxServerConnections`, the server closes the connection used
/// least recently to take a new one.
#[test]
fn connection_limit_closes_least_used() {
    let s = setup(
        OrbOptions {
            max_server_connections: 1,
            ..options(None, None)
        },
        options(None, None),
    );
    let other = Orb::with_options(options(None, None)).unwrap();
    let ior = s.client.object_to_string(Some(&s.obj)).unwrap();
    let obj2 = other.string_to_object(&ior).unwrap().unwrap();
    for i in 0..5 {
        assert_eq!(echo(&s.obj, i).unwrap(), i);
        assert_eq!(echo(&obj2, i).unwrap(), i);
    }
    assert!(s.proxy.accepted() >= 10);
    assert!(eventually(|| s.proxy.open() <= 1));
    other.destroy().unwrap();
}

fn sleep(obj: &ObjectRef, ms: u32) {
    obj.request("sleep")
        .args(|out| {
            out.write_u32(ms);
            Ok(())
        })
        .invoke()
        .unwrap();
}

/// Calls `sleep(ms)` from `threads` threads at once, `rounds` times each.
fn sleep_from(obj: &ObjectRef, threads: usize, rounds: usize, ms: u32) {
    let start = Arc::new(Barrier::new(threads));
    let workers: Vec<_> = (0..threads)
        .map(|_| {
            let (obj, start) = (obj.clone(), Arc::clone(&start));
            thread::spawn(move || {
                start.wait();
                for _ in 0..rounds {
                    sleep(&obj, ms);
                }
            })
        })
        .collect();
    for w in workers {
        w.join().unwrap();
    }
}

/// Calls at the same time open connections up to
/// `-ORBmaxConnectionsPerServer` (default 4), and share them beyond it.
#[test]
fn concurrent_calls_open_connections_up_to_the_limit() {
    for (max, threads, expected) in [(4, 2, 2), (4, 4, 4), (4, 8, 4), (1, 4, 1), (2, 6, 2)] {
        let s = setup(
            options(None, None),
            OrbOptions {
                max_connections_per_server: max,
                ..options(None, None)
            },
        );
        let started = Instant::now();
        sleep_from(&s.obj, threads, 3, 100);
        assert_eq!(s.proxy.accepted(), expected, "max {max}, {threads} threads");
        // The calls ran side by side as far as connections went (the
        // server reads a connection's requests while one runs, so even
        // a shared connection does not serialize them).
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(s.proxy.open(), expected);
    }
}

/// One thread calling in turn uses one connection.
#[test]
fn calls_in_turn_use_one_connection() {
    let s = setup(options(None, None), options(None, None));
    for i in 0..50 {
        assert_eq!(echo(&s.obj, i).unwrap(), i);
    }
    sleep_from(&s.obj, 1, 3, 1);
    assert_eq!(s.proxy.accepted(), 1);
}

/// Idle connections beyond the first close like any other, and calls
/// afterwards open what they need again.
#[test]
fn extra_connections_close_when_idle() {
    let s = setup(options(None, None), options(Some(100), None));
    sleep_from(&s.obj, 4, 2, 50);
    assert_eq!(s.proxy.accepted(), 4);
    assert!(eventually(|| s.proxy.open() == 0));
    assert_eq!(echo(&s.obj, 7).unwrap(), 7);
    assert_eq!(s.proxy.accepted(), 5);
}

/// The server closing its idle connections closes all of them at once:
/// a call meeting one sends again without running out of attempts.
#[test]
fn server_closing_every_connection_is_survived() {
    let s = setup(options(None, Some(50)), options(None, None));
    for _ in 0..5 {
        sleep_from(&s.obj, 4, 1, 20);
        thread::sleep(Duration::from_millis(150));
    }
    assert_eq!(echo(&s.obj, 1).unwrap(), 1);
}

#[test]
fn orb_arguments() {
    let mut o = OrbOptions::default();
    assert_eq!(o.client_idle_timeout, Some(Duration::from_secs(120)));
    assert_eq!(o.server_idle_timeout, Some(Duration::from_secs(180)));
    assert_eq!(o.max_server_connections, 0);
    assert_eq!(o.max_connections_per_server, 4);
    let mut args: Vec<String> = [
        "prog",
        "-ORBoutConScanPeriod",
        "0",
        "-ORBinConScanPeriod=30",
        "-ORBmaxServerConnections",
        "100",
        "-ORBmaxConnectionsPerServer",
        "1",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    o.parse_args(&mut args).unwrap();
    assert_eq!(args, ["prog"]);
    assert_eq!(o.client_idle_timeout, None);
    assert_eq!(o.server_idle_timeout, Some(Duration::from_secs(30)));
    assert_eq!(o.max_server_connections, 100);
    assert_eq!(o.max_connections_per_server, 1);
    o.parse_args(&mut vec![
        "-ORBmaxGIOPConnectionPerServer".into(),
        "5".into(),
    ])
    .unwrap();
    assert_eq!(o.max_connections_per_server, 5);
    for bad in ["0", "x"] {
        assert!(
            o.parse_args(&mut vec!["-ORBmaxConnectionsPerServer".into(), bad.into()])
                .is_err()
        );
    }
    assert!(
        o.parse_args(&mut vec!["-ORBinConScanPeriod".into(), "x".into()])
            .is_err()
    );
}
