//! IIOP over TLS (SSLIOP): TLS endpoints and their IORs, certificates,
//! client authentication, and calls through the TLS record layer. The
//! certificates are in tests/tls (make-certs.sh).

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use rustorb_core::giop::{SslSecTrans, association_options, component_tag};
use rustorb_core::{
    ClientAuth, CompletionStatus, EndpointAddr, Exception, InStream, ObjectRef, Orb, OrbOptions,
    Servant, ServerReply, SystemException, SystemExceptionKind, TlsOptions, minor,
};

const TEST_ID: &str = "IDL:test/Tls:1.0";

struct Echo;

impl Servant for Echo {
    fn repository_id(&self) -> &str {
        TEST_ID
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
            "echo_octets" => {
                let v = input.read_octet_seq()?.to_vec();
                reply.output().write_octet_seq(&v)?;
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

fn file(name: &str) -> Option<PathBuf> {
    Some(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/tls")
            .join(name),
    )
}

fn local(port: u16) -> EndpointAddr {
    EndpointAddr {
        host: "127.0.0.1".into(),
        port,
    }
}

/// A server with one TLS endpoint, `cert` and its key, and the test CA
/// for client certificates.
fn tls_server(cert: &str, client_auth: ClientAuth, plain: bool) -> OrbOptions {
    OrbOptions {
        endpoints: if plain { vec![local(0)] } else { Vec::new() },
        tls_endpoints: vec![local(0)],
        tls: TlsOptions {
            ca_file: file("ca.pem"),
            cert_file: file(&format!("{cert}.pem")),
            key_file: file(&format!("{cert}.key")),
            client_auth,
            ..TlsOptions::default()
        },
        ..OrbOptions::default()
    }
}

/// A client trusting `ca`, with the client certificate if `cert`.
fn tls_client(ca: &str, cert: bool) -> OrbOptions {
    OrbOptions {
        tls: TlsOptions {
            ca_file: file(ca),
            key_file: if cert {
                file("client-bundle.pem")
            } else {
                None
            },
            ..TlsOptions::default()
        },
        ..OrbOptions::default()
    }
}

fn serve(orb: &Orb) -> ObjectRef {
    let root = orb.root_poa().unwrap();
    let id = root.activate_object(Arc::new(Echo)).unwrap();
    root.the_poa_manager().activate().unwrap();
    root.id_to_reference(&id).unwrap()
}

fn remote(client: &Orb, server: &Orb, obj: &ObjectRef) -> ObjectRef {
    client
        .string_to_object(&server.object_to_string(Some(obj)).unwrap())
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

fn system<T: std::fmt::Debug>(result: Result<T, Exception>) -> SystemException {
    match result {
        Err(Exception::System(e)) => e,
        other => panic!("expected a system exception, got {other:?}"),
    }
}

/// The `TAG_SSL_SEC_TRANS` component and the port of each IIOP profile.
fn ssl_of(obj: &ObjectRef) -> Vec<(u16, Option<SslSecTrans>)> {
    obj.ior()
        .iiop_profiles()
        .map(|p| {
            let p = p.unwrap();
            let ssl = p
                .components
                .iter()
                .find(|c| c.tag == component_tag::SSL_SEC_TRANS)
                .map(|c| SslSecTrans::from_component(c).unwrap());
            (p.port, ssl)
        })
        .collect()
}

#[test]
fn calls_over_tls() {
    let server = Orb::with_options(tls_server("server", ClientAuth::None, false)).unwrap();
    let obj = serve(&server);
    let tls_port = server.tls_endpoints()[0].1;
    // TLS only: port 0 in the profile, the TLS port in the component.
    let profiles = ssl_of(&obj);
    assert_eq!(profiles.len(), 1);
    let (port, ssl) = profiles[0];
    assert_eq!(port, 0);
    let ssl = ssl.unwrap();
    assert_eq!(ssl.port, tls_port);
    assert_eq!(ssl.target_requires & association_options::NO_PROTECTION, 0);
    assert_ne!(
        ssl.target_requires & association_options::CONFIDENTIALITY,
        0
    );

    let client = Orb::with_options(tls_client("ca.pem", false)).unwrap();
    let target = remote(&client, &server, &obj);
    assert_eq!(echo_long(&target, 42).unwrap(), 42);
    // Messages of many records, both ways.
    let big: Vec<u8> = (0..1 << 20).map(|i| i as u8).collect();
    assert_eq!(echo_octets(&target, &big).unwrap(), big);
    // Calls from many threads on the one connection.
    thread::scope(|s| {
        for t in 0..8 {
            let target = &target;
            s.spawn(move || {
                for i in 0..200 {
                    assert_eq!(echo_long(target, t * 1000 + i).unwrap(), t * 1000 + i);
                }
                let v = vec![t as u8; 100_000];
                assert_eq!(echo_octets(target, &v).unwrap(), v);
            });
        }
    });
    // The server's own object, called without the network.
    assert_eq!(echo_long(&obj, 7).unwrap(), 7);
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn a_client_without_tls_cannot_reach_a_tls_only_server() {
    let server = Orb::with_options(tls_server("server", ClientAuth::None, false)).unwrap();
    let obj = serve(&server);
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let e = system(echo_long(&remote(&client, &server, &obj), 1));
    assert!(e.is(SystemExceptionKind::TRANSIENT), "{e:?}");
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn plain_and_tls_endpoints_together() {
    let server = Orb::with_options(tls_server("server", ClientAuth::None, true)).unwrap();
    let obj = serve(&server);
    let profiles = ssl_of(&obj);
    let (port, ssl) = profiles[0];
    assert_eq!(port, server.endpoints()[0].1);
    let ssl = ssl.unwrap();
    assert_eq!(ssl.port, server.tls_endpoints()[0].1);
    assert_ne!(ssl.target_supports & association_options::NO_PROTECTION, 0);

    let plain = Orb::with_options(OrbOptions::default()).unwrap();
    assert_eq!(echo_long(&remote(&plain, &server, &obj), 1).unwrap(), 1);
    let tls = Orb::with_options(tls_client("ca.pem", false)).unwrap();
    assert_eq!(echo_long(&remote(&tls, &server, &obj), 2).unwrap(), 2);
    plain.destroy().unwrap();
    tls.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn client_certificates() {
    // Required: refused without one, accepted with one.
    let server = Orb::with_options(tls_server("server", ClientAuth::Required, false)).unwrap();
    let obj = serve(&server);
    let (_, ssl) = ssl_of(&obj)[0];
    assert_ne!(
        ssl.unwrap().target_requires & association_options::ESTABLISH_TRUST_IN_CLIENT,
        0
    );
    let without = Orb::with_options(tls_client("ca.pem", false)).unwrap();
    let e = system(echo_long(&remote(&without, &server, &obj), 1));
    assert!(
        e.is(SystemExceptionKind::TRANSIENT) || e.is(SystemExceptionKind::COMM_FAILURE),
        "{e:?}"
    );
    let with = Orb::with_options(tls_client("ca.pem", true)).unwrap();
    assert_eq!(echo_long(&remote(&with, &server, &obj), 2).unwrap(), 2);
    without.destroy().unwrap();
    with.destroy().unwrap();
    server.destroy().unwrap();

    // Optional: accepted without one.
    let server = Orb::with_options(tls_server("server", ClientAuth::Optional, false)).unwrap();
    let obj = serve(&server);
    let without = Orb::with_options(tls_client("ca.pem", false)).unwrap();
    assert_eq!(echo_long(&remote(&without, &server, &obj), 3).unwrap(), 3);
    let with = Orb::with_options(tls_client("ca.pem", true)).unwrap();
    assert_eq!(echo_long(&remote(&with, &server, &obj), 4).unwrap(), 4);
    without.destroy().unwrap();
    with.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn a_server_certificate_of_another_ca_is_refused() {
    let server = Orb::with_options(tls_server("other-server", ClientAuth::None, false)).unwrap();
    let obj = serve(&server);
    let client = Orb::with_options(tls_client("ca.pem", false)).unwrap();
    let e = system(echo_long(&remote(&client, &server, &obj), 1));
    assert!(e.is(SystemExceptionKind::TRANSIENT), "{e:?}");
    // Trusting that CA instead, it is accepted.
    let other = Orb::with_options(tls_client("other-ca.pem", false)).unwrap();
    assert_eq!(echo_long(&remote(&other, &server, &obj), 2).unwrap(), 2);
    client.destroy().unwrap();
    other.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn the_host_name_is_checked_unless_turned_off() {
    let server = Orb::with_options(tls_server("wrong-name", ClientAuth::None, false)).unwrap();
    let obj = serve(&server);
    let client = Orb::with_options(tls_client("ca.pem", false)).unwrap();
    let e = system(echo_long(&remote(&client, &server, &obj), 1));
    assert!(e.is(SystemExceptionKind::TRANSIENT), "{e:?}");
    let mut options = tls_client("ca.pem", false);
    options.tls.verify_host = false;
    let lax = Orb::with_options(options).unwrap();
    assert_eq!(echo_long(&remote(&lax, &server, &obj), 2).unwrap(), 2);
    client.destroy().unwrap();
    lax.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn a_tls_endpoint_needs_a_key() {
    let mut options = tls_server("server", ClientAuth::None, false);
    options.tls.key_file = None;
    let server = Orb::with_options(options).unwrap();
    let e = server.root_poa().err().unwrap();
    assert!(e.is(SystemExceptionKind::INITIALIZE), "{e:?}");
}

#[test]
fn tls_options_from_arguments() {
    let mut args: Vec<String> = [
        "prog",
        "-ORBendPoint",
        "giop:ssl::2809",
        "-ORBendPoint",
        "giop:tcp::0",
        "-ORBendPointPublish",
        "giop:ssl:example.org:443",
        "-ORBsslCAFile",
        "ca.pem",
        "-ORBsslCertFile=cert.pem",
        "-ORBsslKeyFile",
        "key.pem",
        "-ORBsslVerifyClient",
        "required",
        "-ORBsslVerifyHost",
        "0",
        "rest",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut options = OrbOptions::default();
    options.parse_args(&mut args).unwrap();
    assert_eq!(args, ["prog", "rest"]);
    assert_eq!(
        options.tls_endpoints,
        [EndpointAddr {
            host: String::new(),
            port: 2809
        }]
    );
    assert_eq!(options.endpoints.len(), 1);
    assert_eq!(
        options.tls_endpoint_publish,
        Some(EndpointAddr {
            host: "example.org".into(),
            port: 443
        })
    );
    assert_eq!(
        options.tls,
        TlsOptions {
            ca_file: Some("ca.pem".into()),
            cert_file: Some("cert.pem".into()),
            key_file: Some("key.pem".into()),
            client_auth: ClientAuth::Required,
            verify_host: false,
        }
    );
    let mut bad = vec!["-ORBsslVerifyClient".to_string(), "sometimes".to_string()];
    assert!(OrbOptions::default().parse_args(&mut bad).is_err());
}

/// Idle TLS connections are closed by either side (docs/connections.md),
/// and the next call opens a new one.
#[test]
fn idle_tls_connections_close_and_reconnect() {
    for (client_idle, server_idle) in [(Some(50), None), (None, Some(50))] {
        let server = Orb::with_options(OrbOptions {
            server_idle_timeout: server_idle.map(std::time::Duration::from_millis),
            ..tls_server("server", ClientAuth::None, false)
        })
        .unwrap();
        let obj = serve(&server);
        let client = Orb::with_options(OrbOptions {
            client_idle_timeout: client_idle.map(std::time::Duration::from_millis),
            ..tls_client("ca.pem", false)
        })
        .unwrap();
        let target = remote(&client, &server, &obj);
        for i in 0..5 {
            assert_eq!(echo_long(&target, i).unwrap(), i);
            thread::sleep(std::time::Duration::from_millis(120));
        }
        client.destroy().unwrap();
        server.destroy().unwrap();
    }
}
