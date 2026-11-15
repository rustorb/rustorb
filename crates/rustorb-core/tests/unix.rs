//! `giop:unix` endpoints (docs/unix-transport.md): the socket files, the
//! `TAG_OMNIORB_UNIX_TRANS` components of IORs, calls over Unix sockets,
//! and which address a client picks.
#![cfg(unix)]

use std::io::Read;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rustorb_core::cdr::ByteOrder;
use rustorb_core::giop::{IiopProfile, Ior, UnixTrans, Version, component_tag};
use rustorb_core::{
    Bidirectional, CompletionStatus, EndpointAddr, Exception, InStream, ObjectRef, Orb, OrbOptions,
    Policy, Servant, ServerReply, SystemException, SystemExceptionKind, minor,
};

const ECHO_ID: &str = "IDL:test/UnixEcho:1.0";

/// `echo_long(n)` and `echo_octets(s)` return their argument;
/// `call_back(cb, n)` returns `cb.echo_long(n) + 1`.
struct Echo {
    calls: AtomicUsize,
}

impl Echo {
    fn new() -> Arc<Echo> {
        Arc::new(Echo {
            calls: AtomicUsize::new(0),
        })
    }
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
        self.calls.fetch_add(1, Ordering::SeqCst);
        match operation {
            "echo_long" => {
                let v = input.read::<i32>()?;
                reply.output().write_i32(v);
                Ok(())
            }
            "echo_octets" => {
                let v = input.read_octet_seq()?.to_vec();
                reply.output().write_octet_seq(&v)?;
                Ok(())
            }
            "call_back" => {
                let cb = input.read_object()?.expect("a callback");
                let n = input.read::<i32>()?;
                let v = echo_long(&cb, n).map_err(|e| match e {
                    Exception::System(e) => e,
                    Exception::User(_) => SystemException::new(
                        SystemExceptionKind::UNKNOWN,
                        0,
                        CompletionStatus::Maybe,
                    ),
                })?;
                reply.output().write_i32(v + 1);
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

/// A new empty directory for sockets, removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        // Under /tmp rather than the temporary directory, whose path can be
        // long on macOS: a socket path has at most 104 bytes there.
        let dir = PathBuf::from(format!("/tmp/rustorb-unix-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn spec(&self, name: &str) -> String {
        self.path(name).to_str().unwrap().to_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn unix_options(paths: &[String], tcp: bool) -> OrbOptions {
    OrbOptions {
        endpoints: if tcp {
            vec![EndpointAddr {
                host: "127.0.0.1".into(),
                port: 0,
            }]
        } else {
            Vec::new()
        },
        unix_endpoints: paths.to_vec(),
        ..OrbOptions::default()
    }
}

fn serve(orb: &Orb, servant: Arc<Echo>) -> ObjectRef {
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

fn unix_components(obj: &ObjectRef) -> (IiopProfile, Vec<UnixTrans>) {
    let profile = obj.ior().iiop_profiles().next().unwrap().unwrap();
    let unix = profile
        .components
        .iter()
        .filter(|c| c.tag == component_tag::OMNIORB_UNIX_TRANS)
        .map(|c| UnixTrans::from_component(c).unwrap())
        .collect();
    (profile, unix)
}

fn host_name() -> String {
    let out = std::process::Command::new("hostname").output().unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn is_socket(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_socket())
}

#[test]
fn options_parse() {
    let mut options = OrbOptions::default();
    let mut args: Vec<String> = [
        "-ORBendPoint",
        "giop:unix:/tmp/a.sock",
        "-ORBendPoint",
        "giop:unix:",
        "-ORBendPoint",
        "giop:tcp::0",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    options.parse_args(&mut args).unwrap();
    assert!(args.is_empty());
    assert_eq!(options.unix_endpoints, ["/tmp/a.sock", ""]);
    assert_eq!(options.endpoints.len(), 1);
}

#[test]
fn unix_only_server_publishes_the_socket_with_port_0() {
    let dir = TempDir::new("only");
    let path = dir.spec("s.sock");
    let server = Orb::with_options(unix_options(std::slice::from_ref(&path), false)).unwrap();
    let obj = serve(&server, Echo::new());
    let (profile, unix) = unix_components(&obj);
    assert_eq!(profile.version, Version::V1_2);
    assert_eq!(profile.port, 0);
    assert_eq!(profile.host, host_name());
    assert_eq!(
        unix,
        [UnixTrans {
            host: host_name(),
            path: path.clone(),
        }]
    );
    assert_eq!(server.unix_endpoints(), std::slice::from_ref(&path));
    assert!(server.endpoints().is_empty());

    // Like omniORB's: a socket anybody may connect to.
    let meta = std::fs::symlink_metadata(&path).unwrap();
    assert!(meta.file_type().is_socket());
    assert_eq!(meta.permissions().mode() & 0o777, 0o777);

    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let r = remote(&client, &server, &obj);
    assert_eq!(echo_long(&r, 7).unwrap(), 7);
    assert_eq!(echo_long(&r, -3).unwrap(), -3);
    // Large enough to take many reads and writes.
    let big: Vec<u8> = (0..1_500_000u32).map(|i| i as u8).collect();
    let reply = r
        .request("echo_octets")
        .args(|out| {
            out.write_octet_seq(&big)?;
            Ok(())
        })
        .invoke()
        .unwrap();
    assert_eq!(reply.input().read_octet_seq().unwrap(), &big[..]);
    assert_eq!(echo_long(&r, 9).unwrap(), 9);

    client.destroy().unwrap();
    server.destroy().unwrap();
    // The socket goes with the ORB.
    assert!(!path_exists(&path));
}

fn path_exists(path: &str) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

#[test]
fn tcp_and_unix_endpoints_share_one_profile() {
    let dir = TempDir::new("both");
    let paths = [dir.spec("a.sock"), dir.spec("b.sock")];
    let server = Orb::with_options(unix_options(&paths, true)).unwrap();
    let obj = serve(&server, Echo::new());
    let (profile, unix) = unix_components(&obj);
    assert_eq!(profile.host, "127.0.0.1");
    assert_ne!(profile.port, 0);
    let got: Vec<_> = unix.iter().map(|u| u.path.clone()).collect();
    assert_eq!(got, paths);
    server.destroy().unwrap();
    assert!(!path_exists(&paths[0]) && !path_exists(&paths[1]));
}

#[test]
fn a_path_ending_in_a_slash_is_a_directory() {
    let dir = TempDir::new("dir");
    let spec = format!("{}/", dir.0.display());
    let server = Orb::with_options(unix_options(&[spec.clone(), spec], false)).unwrap();
    let _obj = serve(&server, Echo::new());
    let paths = server.unix_endpoints();
    assert_eq!(paths.len(), 2);
    for p in &paths {
        let p = Path::new(p);
        assert_eq!(p.parent().unwrap(), dir.0);
        // PID-SECONDS, the pid in nine digits.
        let name = p.file_name().unwrap().to_str().unwrap();
        let pid = format!("{:09}-", std::process::id());
        assert!(name.starts_with(&pid), "{name}");
        assert!(is_socket(p));
    }
    assert_ne!(paths[0], paths[1]);
    server.destroy().unwrap();
    assert!(paths.iter().all(|p| !path_exists(p)));
}

#[test]
fn the_default_socket_is_in_tmp_omni_user() {
    let server = Orb::with_options(unix_options(&[String::new()], false)).unwrap();
    let _obj = serve(&server, Echo::new());
    let paths = server.unix_endpoints();
    assert_eq!(paths.len(), 1);
    let path = Path::new(&paths[0]);
    let dir = path.parent().unwrap().to_str().unwrap();
    assert!(dir.starts_with("/tmp/omni-"), "{dir}");
    assert!(is_socket(path));
    server.destroy().unwrap();
    assert!(!path.exists());
}

#[test]
fn a_stale_socket_is_replaced_but_nothing_else() {
    let dir = TempDir::new("stale");

    // A socket nobody listens on any more.
    let stale = dir.path("stale.sock");
    drop(UnixListener::bind(&stale).unwrap());
    assert!(is_socket(&stale));
    let spec = stale.to_str().unwrap().to_owned();
    let server = Orb::with_options(unix_options(std::slice::from_ref(&spec), false)).unwrap();
    let obj = serve(&server, Echo::new());
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    assert_eq!(echo_long(&remote(&client, &server, &obj), 1).unwrap(), 1);

    // A socket in use: by the server above.
    let other = Orb::with_options(unix_options(std::slice::from_ref(&spec), false)).unwrap();
    assert_initialize(other.root_poa().err());
    assert!(is_socket(&stale));
    assert_eq!(echo_long(&remote(&client, &server, &obj), 2).unwrap(), 2);

    // A file that is not a socket.
    let plain = dir.path("plain");
    std::fs::write(&plain, b"keep me").unwrap();
    let orb = Orb::with_options(unix_options(&[dir.spec("plain")], false)).unwrap();
    assert_initialize(orb.root_poa().err());
    assert_eq!(std::fs::read(&plain).unwrap(), b"keep me");

    // A directory that does not exist.
    let orb = Orb::with_options(unix_options(&[dir.spec("no/such.sock")], false)).unwrap();
    assert_initialize(orb.root_poa().err());

    client.destroy().unwrap();
    server.destroy().unwrap();
}

fn assert_initialize(e: Option<SystemException>) {
    let e = e.expect("the endpoint fails");
    assert_eq!(e.kind(), Some(SystemExceptionKind::INITIALIZE));
}

#[test]
fn calls_within_the_orb_stay_local() {
    let dir = TempDir::new("local");
    let server = Orb::with_options(unix_options(&[dir.spec("s.sock")], false)).unwrap();
    let echo = Echo::new();
    let obj = serve(&server, echo.clone());
    let again = server
        .string_to_object(&server.object_to_string(Some(&obj)).unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(echo_long(&again, 5).unwrap(), 5);
    assert_eq!(echo.calls.load(Ordering::SeqCst), 1);
    server.destroy().unwrap();
}

/// An IOR with one IIOP 1.2 profile at `127.0.0.1:tcp_port` and a
/// `TAG_OMNIORB_UNIX_TRANS` component for `path` on `host`.
fn crafted(tcp_port: u16, host: &str, path: &Path) -> String {
    let unix = UnixTrans {
        host: host.into(),
        path: path.to_str().unwrap().into(),
    };
    let profile = IiopProfile {
        version: Version::V1_2,
        host: "127.0.0.1".into(),
        port: tcp_port,
        object_key: b"key".to_vec(),
        components: vec![unix.to_component(ByteOrder::Big).unwrap()],
    };
    Ior {
        type_id: ECHO_ID.into(),
        profiles: vec![profile.to_tagged_profile(ByteOrder::Big).unwrap()],
    }
    .to_ior_string(ByteOrder::Big)
    .unwrap()
}

/// Which listener a client connects to for `ior`: a call is started on a
/// thread of its own (the listeners never answer), and the first
/// connection is reported.
fn first_connection(
    tcp: &std::net::TcpListener,
    unix: Option<&UnixListener>,
    ior: &str,
) -> &'static str {
    tcp.set_nonblocking(true).unwrap();
    if let Some(u) = unix {
        u.set_nonblocking(true).unwrap();
    }
    let client = Orb::with_options(OrbOptions {
        request_timeout: Some(Duration::from_secs(2)),
        ..OrbOptions::default()
    })
    .unwrap();
    let obj = client.string_to_object(ior).unwrap().unwrap();
    let call = thread::spawn(move || {
        let _ = obj.non_existent();
        drop(obj);
        client.destroy().unwrap();
    });
    let until = Instant::now() + Duration::from_secs(5);
    let which = loop {
        if let Some(u) = unix {
            if let Ok((mut s, _)) = u.accept() {
                let mut b = [0u8; 4];
                s.set_nonblocking(false).unwrap();
                s.read_exact(&mut b).unwrap();
                assert_eq!(&b, b"GIOP");
                break "unix";
            }
        }
        if let Ok((mut s, _)) = tcp.accept() {
            let mut b = [0u8; 4];
            s.set_nonblocking(false).unwrap();
            s.read_exact(&mut b).unwrap();
            assert_eq!(&b, b"GIOP");
            break "tcp";
        }
        assert!(Instant::now() < until, "no connection");
        thread::sleep(Duration::from_millis(5));
    };
    call.join().unwrap();
    which
}

#[test]
fn a_client_uses_the_socket_on_its_own_host_first() {
    let dir = TempDir::new("pick");
    let path = dir.path("p.sock");
    let unix = UnixListener::bind(&path).unwrap();
    let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = tcp.local_addr().unwrap().port();
    let ior = crafted(port, &host_name(), &path);
    assert_eq!(first_connection(&tcp, Some(&unix), &ior), "unix");
}

#[test]
fn a_socket_on_another_host_is_not_used() {
    let dir = TempDir::new("other");
    let path = dir.path("p.sock");
    let unix = UnixListener::bind(&path).unwrap();
    let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = tcp.local_addr().unwrap().port();
    let ior = crafted(port, "not-this-host.invalid", &path);
    assert_eq!(first_connection(&tcp, Some(&unix), &ior), "tcp");
}

#[test]
fn a_dead_socket_falls_back_to_tcp() {
    let dir = TempDir::new("dead");
    let path = dir.path("gone.sock");
    let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = tcp.local_addr().unwrap().port();
    let ior = crafted(port, &host_name(), &path);
    assert_eq!(first_connection(&tcp, None, &ior), "tcp");
}

#[test]
fn a_unix_only_object_on_another_host_is_unreachable() {
    let dir = TempDir::new("unreach");
    let path = dir.path("p.sock");
    let _unix = UnixListener::bind(&path).unwrap();
    let ior = crafted(0, "not-this-host.invalid", &path);
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let obj = client.string_to_object(&ior).unwrap().unwrap();
    match obj.non_existent() {
        Err(Exception::System(e)) => {
            assert_eq!(e.kind(), Some(SystemExceptionKind::TRANSIENT));
        }
        other => panic!("{other:?}"),
    }
    client.destroy().unwrap();
}

#[test]
fn bidirectional_giop_over_a_unix_socket() {
    let dir = TempDir::new("bidir");
    let server = Orb::with_options(OrbOptions {
        accept_bidir: true,
        ..unix_options(&[dir.spec("s.sock")], false)
    })
    .unwrap();
    let obj = serve(&server, Echo::new());

    // The client's callback object is published at an address nobody
    // listens on: only the connection the client offered reaches it.
    let client = Orb::with_options(OrbOptions {
        offer_bidir: true,
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        endpoint_publish: Some(EndpointAddr {
            host: "127.0.0.1".into(),
            port: 1,
        }),
        ..OrbOptions::default()
    })
    .unwrap();
    let croot = client.root_poa().unwrap();
    let poa = croot
        .create_poa(
            "callbacks",
            Some(&croot.the_poa_manager()),
            &[Policy::Bidirectional(Bidirectional::Both)],
        )
        .unwrap();
    let callback = Echo::new();
    let cid = poa.activate_object(callback.clone()).unwrap();
    croot.the_poa_manager().activate().unwrap();
    let cb = poa.id_to_reference(&cid).unwrap();

    let r = remote(&client, &server, &obj);
    let reply = r
        .request("call_back")
        .args(|out| {
            out.write_object(Some(&cb))?;
            out.write_i32(40);
            Ok(())
        })
        .invoke()
        .unwrap();
    assert_eq!(reply.input().read::<i32>().unwrap(), 41);
    assert_eq!(callback.calls.load(Ordering::SeqCst), 1);
    client.destroy().unwrap();
    server.destroy().unwrap();
}
