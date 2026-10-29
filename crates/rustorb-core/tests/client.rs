//! Client invocations against a scripted GIOP server on localhost.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use rustorb_core::cdr::{ByteOrder, CdrEncode, Encoder};
use rustorb_core::giop::{
    AlternateIiopAddress, CodeSetComponent, CodeSetComponentInfo, CodeSetContext, IiopProfile, Ior,
    LocateReplyHeader, LocateStatus, Message, MessageEncoder, MessageHeader, ReplyHeader,
    ReplyStatus, SystemExceptionBody, TargetAddress, Version, code_set, decode_message,
};
use rustorb_core::{CompletionStatus, Exception, Orb, OrbOptions, SystemExceptionKind, minor};

const KEY: &[u8] = b"test-key";

/// What the scripted server does with one message.
enum Action {
    Send(Vec<u8>),
    /// Drops the connection.
    Close,
}

/// A message the server received.
#[derive(Debug, Clone)]
struct Received {
    connection: usize,
    version: Version,
    message: Message,
}

type Handler = dyn Fn(&Received, &Server) -> Vec<Action> + Send + Sync;

/// A GIOP server on an ephemeral port that answers each message with the
/// handler's actions and records everything it receives.
#[derive(Clone)]
struct Server {
    port: u16,
    received: Arc<Mutex<Vec<Received>>>,
    calls: Arc<AtomicUsize>,
}

impl Server {
    fn start(
        handler: impl Fn(&Received, &Server) -> Vec<Action> + Send + Sync + 'static,
    ) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server = Server {
            port: listener.local_addr().unwrap().port(),
            received: Arc::default(),
            calls: Arc::default(),
        };
        let handler: Arc<Handler> = Arc::new(handler);
        let s = server.clone();
        thread::spawn(move || {
            for (connection, stream) in listener.incoming().enumerate() {
                let Ok(stream) = stream else { return };
                let handler = Arc::clone(&handler);
                let s = s.clone();
                thread::spawn(move || serve(stream, connection, &*handler, &s));
            }
        });
        server
    }

    fn received(&self) -> Vec<Received> {
        self.received.lock().unwrap().clone()
    }

    /// Requests and locate requests received so far.
    fn requests(&self) -> Vec<Received> {
        self.received()
            .into_iter()
            .filter(|r| {
                matches!(
                    r.message,
                    Message::Request { .. } | Message::LocateRequest(_)
                )
            })
            .collect()
    }

    fn ior(&self, version: Version) -> Ior {
        ior_for(self.port, version, true)
    }
}

fn serve(mut stream: TcpStream, connection: usize, handler: &Handler, server: &Server) {
    loop {
        let mut header = [0u8; MessageHeader::SIZE];
        if stream.read_exact(&mut header).is_err() {
            return;
        }
        let parsed = MessageHeader::parse(&header).unwrap();
        let mut msg = header.to_vec();
        msg.resize(parsed.total_len(), 0);
        stream.read_exact(&mut msg[MessageHeader::SIZE..]).unwrap();
        let (h, message) = decode_message(&msg).unwrap();
        let received = Received {
            connection,
            version: h.version,
            message,
        };
        server.received.lock().unwrap().push(received.clone());
        if matches!(
            received.message,
            Message::Request { .. } | Message::LocateRequest(_)
        ) {
            server.calls.fetch_add(1, Ordering::SeqCst);
        }
        for action in handler(&received, server) {
            match action {
                Action::Send(bytes) => stream.write_all(&bytes).unwrap(),
                Action::Close => return,
            }
        }
    }
}

/// omniORB's code set component: ISO 8859-1 converting to UTF-8, UTF-16.
fn code_sets() -> CodeSetComponentInfo {
    CodeSetComponentInfo {
        for_char_data: CodeSetComponent {
            native_code_set: code_set::ISO_8859_1,
            conversion_code_sets: vec![code_set::UTF_8],
        },
        for_wchar_data: CodeSetComponent {
            native_code_set: code_set::UTF_16,
            conversion_code_sets: vec![code_set::UCS_2_LEVEL_1],
        },
    }
}

fn ior_for(port: u16, version: Version, with_code_sets: bool) -> Ior {
    profile_ior(port, version, with_code_sets, Vec::new())
}

fn profile_ior(
    port: u16,
    version: Version,
    with_code_sets: bool,
    mut components: Vec<rustorb_core::giop::TaggedComponent>,
) -> Ior {
    if with_code_sets && version.minor > 0 {
        components.insert(0, code_sets().to_component(ByteOrder::Big).unwrap());
    }
    let profile = IiopProfile {
        version,
        host: "127.0.0.1".into(),
        port,
        object_key: KEY.to_vec(),
        components,
    };
    Ior {
        type_id: "IDL:test/Echo:1.0".into(),
        profiles: vec![profile.to_tagged_profile(ByteOrder::Big).unwrap()],
    }
}

/// A port nothing listens on.
fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn reply(r: &Received, status: ReplyStatus, body: impl FnOnce(&mut Encoder)) -> Vec<Action> {
    let Message::Request { header, .. } = &r.message else {
        panic!("not a request: {:?}", r.message)
    };
    let mut m = MessageEncoder::reply(
        r.version,
        ByteOrder::Little,
        &ReplyHeader {
            request_id: header.request_id,
            reply_status: status,
            service_context: Vec::new(),
        },
    )
    .unwrap();
    body(m.body());
    vec![Action::Send(m.finish().unwrap())]
}

fn locate_reply(
    r: &Received,
    status: LocateStatus,
    body: impl FnOnce(&mut Encoder),
) -> Vec<Action> {
    let Message::LocateRequest(header) = &r.message else {
        panic!("not a locate request: {:?}", r.message)
    };
    let mut m = MessageEncoder::locate_reply(
        r.version,
        ByteOrder::Little,
        &LocateReplyHeader {
            request_id: header.request_id,
            locate_status: status,
        },
    )
    .unwrap();
    body(m.body());
    vec![Action::Send(m.finish().unwrap())]
}

/// Echoes the `long` argument of every request.
fn echo_long(r: &Received, _: &Server) -> Vec<Action> {
    match &r.message {
        Message::Request { header, body } if header.response_expected() => {
            let v = body.decoder().read_i32().unwrap();
            reply(r, ReplyStatus::NoException, |enc| enc.write_i32(v))
        }
        _ => Vec::new(),
    }
}

fn orb() -> Orb {
    Orb::with_options(OrbOptions::default()).unwrap()
}

fn call_long(obj: &rustorb_core::ObjectRef, v: i32) -> Result<i32, Exception> {
    let reply = obj
        .request("echo_long")
        .args(move |out| {
            out.write_i32(v);
            Ok(())
        })
        .invoke()?;
    Ok(reply.input().read::<i32>()?)
}

fn object(orb: &Orb, ior: &Ior) -> rustorb_core::ObjectRef {
    orb.string_to_object(&ior.to_ior_string(ByteOrder::Big).unwrap())
        .unwrap()
        .unwrap()
}

#[test]
fn two_way_request_over_giop_1_2() {
    let server = Server::start(echo_long);
    let orb = orb();
    let obj = object(&orb, &server.ior(Version::V1_2));
    assert_eq!(call_long(&obj, 42).unwrap(), 42);
    assert_eq!(call_long(&obj, -7).unwrap(), -7);

    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    // One connection, distinct request ids, key addressing, code sets.
    let mut ids = Vec::new();
    for r in &requests {
        assert_eq!(r.connection, 0);
        assert_eq!(r.version, Version::V1_2);
        let Message::Request { header, .. } = &r.message else {
            unreachable!()
        };
        assert_eq!(header.operation, "echo_long");
        assert_eq!(header.target, TargetAddress::Key(KEY.to_vec()));
        let cs = CodeSetContext::find(&header.service_context)
            .unwrap()
            .unwrap();
        assert_eq!(cs.char_data, code_set::UTF_8);
        assert_eq!(cs.wchar_data, code_set::UTF_16);
        ids.push(header.request_id);
    }
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn giop_version_follows_the_profile_and_the_orb_limit() {
    let server = Server::start(echo_long);
    let orb = orb();
    // corbaloc gives an IIOP 1.0 profile without code sets.
    let obj = orb
        .string_to_object(&format!("corbaloc::127.0.0.1:{}/test-key", server.port))
        .unwrap()
        .unwrap();
    assert_eq!(call_long(&obj, 1).unwrap(), 1);

    let options = OrbOptions {
        max_giop_version: Version::V1_1,
        ..OrbOptions::default()
    };
    let orb11 = Orb::with_options(options).unwrap();
    let obj11 = object(&orb11, &server.ior(Version::V1_2));
    assert_eq!(call_long(&obj11, 2).unwrap(), 2);

    let requests = server.requests();
    assert_eq!(requests[0].version, Version::V1_0);
    let Message::Request { header, .. } = &requests[0].message else {
        unreachable!()
    };
    assert!(header.service_context.is_empty());
    assert_eq!(requests[1].version, Version::V1_1);
    let Message::Request { header, .. } = &requests[1].message else {
        unreachable!()
    };
    assert!(CodeSetContext::find(&header.service_context).is_some());
}

#[test]
fn strings_use_the_negotiated_code_set() {
    // A server without UTF-8 gets ISO 8859-1.
    let server = Server::start(|r, _| {
        let Message::Request { body, .. } = &r.message else {
            return Vec::new();
        };
        let bytes = body.decoder().read_string_bytes().unwrap().to_vec();
        reply(r, ReplyStatus::NoException, |enc| {
            enc.write_string_bytes(&bytes).unwrap()
        })
    });
    let mut components = vec![
        CodeSetComponentInfo {
            for_char_data: CodeSetComponent {
                native_code_set: code_set::ISO_8859_1,
                conversion_code_sets: Vec::new(),
            },
            for_wchar_data: CodeSetComponent::default(),
        }
        .to_component(ByteOrder::Big)
        .unwrap(),
    ];
    let ior = profile_ior(
        server.port,
        Version::V1_2,
        false,
        std::mem::take(&mut components),
    );
    let orb = orb();
    let obj = object(&orb, &ior);
    let reply = obj
        .request("echo_string")
        .args(|out| out.write_string("caf\u{e9}"))
        .invoke()
        .unwrap();
    assert_eq!(reply.input().read_string().unwrap(), "caf\u{e9}");
    let Message::Request { body, .. } = &server.requests()[0].message else {
        unreachable!()
    };
    assert_eq!(body.decoder().read_string_bytes().unwrap(), b"caf\xe9");

    // No wchar code set: wstring is refused before sending.
    let e = obj
        .request("echo_wstring")
        .args(|out| out.write_wstring("x"))
        .invoke()
        .unwrap_err();
    let e = e.as_system().unwrap();
    assert!(e.is(SystemExceptionKind::BAD_PARAM));
    assert_eq!(e.minor, minor::WCHAR_TCS_NOT_KNOWN);
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn system_and_user_exceptions() {
    let server = Server::start(|r, _| {
        let Message::Request { header, .. } = &r.message else {
            return Vec::new();
        };
        match header.operation.as_str() {
            "_non_existent" => reply(r, ReplyStatus::SystemException, |enc| {
                SystemExceptionBody {
                    exception_id: "IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0".into(),
                    minor_code_value: 0x4f4d_0002,
                    completion_status: CompletionStatus::No,
                }
                .encode(enc)
                .unwrap()
            }),
            "fail_system" => reply(r, ReplyStatus::SystemException, |enc| {
                SystemExceptionBody {
                    exception_id: "IDL:omg.org/CORBA/BAD_PARAM:1.0".into(),
                    minor_code_value: 42,
                    completion_status: CompletionStatus::No,
                }
                .encode(enc)
                .unwrap()
            }),
            "fail" => reply(r, ReplyStatus::UserException, |enc| {
                enc.write_string("IDL:test/Oops:1.0").unwrap();
                enc.write_i32(7);
                enc.write_string("boom").unwrap();
            }),
            _ => Vec::new(),
        }
    });
    let orb = orb();
    let obj = object(&orb, &server.ior(Version::V1_2));
    assert!(obj.non_existent().unwrap());

    let e = obj.request("fail_system").invoke().unwrap_err();
    let sys = e.as_system().unwrap();
    assert!(sys.is(SystemExceptionKind::BAD_PARAM));
    assert_eq!((sys.minor, sys.completed), (42, CompletionStatus::No));
    assert_eq!(
        sys.to_string(),
        "CORBA::BAD_PARAM (minor 0x0000002a, COMPLETED_NO)"
    );

    let e = obj.request("fail").invoke().unwrap_err();
    let user = e.as_user().unwrap();
    assert_eq!(user.repository_id(), "IDL:test/Oops:1.0");
    let mut input = user.input().unwrap();
    assert_eq!(input.read::<i32>().unwrap(), 7);
    assert_eq!(input.read_string().unwrap(), "boom");
}

#[test]
fn location_forward_is_followed_and_remembered() {
    let target = Server::start(echo_long);
    let forward_to = target.ior(Version::V1_2);
    let forwarder = Server::start(move |r, _| {
        reply(r, ReplyStatus::LocationForward, |enc| {
            forward_to.encode(enc).unwrap()
        })
    });
    let orb = orb();
    let obj = object(&orb, &forwarder.ior(Version::V1_2));
    assert_eq!(call_long(&obj, 5).unwrap(), 5);
    assert_eq!(call_long(&obj, 6).unwrap(), 6);
    assert_eq!(forwarder.requests().len(), 1);
    assert_eq!(target.requests().len(), 2);
    assert_eq!(obj.effective_ior(), target.ior(Version::V1_2));
}

#[test]
fn unreachable_forward_falls_back_to_the_original() {
    let gone = ior_for(closed_port(), Version::V1_2, true);
    let server = Server::start(move |r, s| {
        if s.calls.load(Ordering::SeqCst) == 1 {
            reply(r, ReplyStatus::LocationForwardPerm, |enc| {
                gone.encode(enc).unwrap()
            })
        } else {
            echo_long(r, s)
        }
    });
    let orb = orb();
    let obj = object(&orb, &server.ior(Version::V1_2));
    assert_eq!(call_long(&obj, 9).unwrap(), 9);
    assert_eq!(server.requests().len(), 2);
    assert_eq!(obj.effective_ior(), *obj.ior());
}

#[test]
fn forward_loops_end_with_transient() {
    let server = Server::start(|r, s| {
        let me = s.ior(Version::V1_2);
        reply(r, ReplyStatus::LocationForward, |enc| {
            me.encode(enc).unwrap()
        })
    });
    let options = OrbOptions {
        max_forwards: 3,
        ..OrbOptions::default()
    };
    let orb = Orb::with_options(options).unwrap();
    let obj = object(&orb, &server.ior(Version::V1_2));
    let e = call_long(&obj, 1).unwrap_err();
    assert!(e.as_system().unwrap().is(SystemExceptionKind::TRANSIENT));
    assert_eq!(server.requests().len(), 4);
}

#[test]
fn needs_addressing_mode_resends_with_the_reference() {
    let server = Server::start(|r, s| {
        let Message::Request { header, .. } = &r.message else {
            return Vec::new();
        };
        match &header.target {
            TargetAddress::Key(_) => {
                reply(r, ReplyStatus::NeedsAddressingMode, |enc| enc.write_i16(2))
            }
            TargetAddress::Reference {
                selected_profile_index,
                ior,
            } => {
                assert_eq!(*selected_profile_index, 0);
                assert_eq!(*ior, s.ior(Version::V1_2));
                echo_long(r, s)
            }
            TargetAddress::Profile(_) => panic!("profile addressing was not asked for"),
        }
    });
    let orb = orb();
    let obj = object(&orb, &server.ior(Version::V1_2));
    assert_eq!(call_long(&obj, 3).unwrap(), 3);
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn close_connection_before_the_reply_retries_on_a_new_connection() {
    let server = Server::start(|r, s| {
        if r.connection == 0 {
            vec![
                Action::Send(MessageEncoder::close_connection(r.version, ByteOrder::Big).unwrap()),
                Action::Close,
            ]
        } else {
            echo_long(r, s)
        }
    });
    let orb = orb();
    let obj = object(&orb, &server.ior(Version::V1_2));
    assert_eq!(call_long(&obj, 11).unwrap(), 11);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].connection, 1);
}

#[test]
fn a_dropped_connection_is_comm_failure_and_the_next_call_reconnects() {
    let server = Server::start(|r, s| {
        if r.connection == 0 {
            vec![Action::Close]
        } else {
            echo_long(r, s)
        }
    });
    let orb = orb();
    let obj = object(&orb, &server.ior(Version::V1_2));
    let e = call_long(&obj, 1).unwrap_err();
    let e = e.as_system().unwrap();
    assert!(e.is(SystemExceptionKind::COMM_FAILURE));
    assert_eq!(e.completed, CompletionStatus::Maybe);
    assert_eq!(call_long(&obj, 2).unwrap(), 2);
}

#[test]
fn unreachable_server_is_transient() {
    let orb = orb();
    let obj = object(&orb, &ior_for(closed_port(), Version::V1_2, true));
    let e = call_long(&obj, 1).unwrap_err();
    let e = e.as_system().unwrap();
    assert!(e.is(SystemExceptionKind::TRANSIENT));
    assert_eq!(e.completed, CompletionStatus::No);
}

#[test]
fn alternate_addresses_are_tried_in_order() {
    let server = Server::start(echo_long);
    let alternate = AlternateIiopAddress {
        host: "127.0.0.1".into(),
        port: server.port,
    }
    .to_component(ByteOrder::Big)
    .unwrap();
    let ior = profile_ior(closed_port(), Version::V1_2, true, vec![alternate]);
    let orb = orb();
    let obj = object(&orb, &ior);
    assert_eq!(call_long(&obj, 8).unwrap(), 8);
}

#[test]
fn request_timeout_cancels_the_request() {
    let server = Server::start(|_, _| Vec::new());
    let options = OrbOptions {
        request_timeout: Some(Duration::from_millis(200)),
        ..OrbOptions::default()
    };
    let orb = Orb::with_options(options).unwrap();
    let obj = object(&orb, &server.ior(Version::V1_2));
    let e = call_long(&obj, 1).unwrap_err();
    let e = e.as_system().unwrap();
    assert!(e.is(SystemExceptionKind::TIMEOUT));
    assert_eq!(e.completed, CompletionStatus::Maybe);
    thread::sleep(Duration::from_millis(100));
    let received = server.received();
    let Message::Request { header, .. } = &received[0].message else {
        unreachable!()
    };
    assert!(matches!(
        received[1].message,
        Message::CancelRequest(c) if c.request_id == header.request_id
    ));
}

#[test]
fn oneway_requests_expect_no_reply() {
    let server = Server::start(echo_long);
    let orb = orb();
    let obj = object(&orb, &server.ior(Version::V1_2));
    obj.request("post")
        .args(|out| out.write_string("hi"))
        .send_oneway()
        .unwrap();
    assert_eq!(call_long(&obj, 1).unwrap(), 1);
    let requests = server.requests();
    let Message::Request { header, .. } = &requests[0].message else {
        unreachable!()
    };
    assert_eq!(header.operation, "post");
    assert!(!header.response_expected());
    assert_eq!(header.response_flags, 0);
}

#[test]
fn locate_requests() {
    let here = Server::start(|r, _| locate_reply(r, LocateStatus::ObjectHere, |_| {}));
    let here_ior = here.ior(Version::V1_2);
    let forwarder = Server::start(move |r, _| {
        locate_reply(r, LocateStatus::ObjectForward, |enc| {
            here_ior.encode(enc).unwrap()
        })
    });
    let unknown = Server::start(|r, _| locate_reply(r, LocateStatus::UnknownObject, |_| {}));
    let orb = orb();
    assert!(
        object(&orb, &forwarder.ior(Version::V1_2))
            .locate()
            .unwrap()
    );
    assert!(!object(&orb, &unknown.ior(Version::V1_1)).locate().unwrap());
    assert_eq!(here.requests().len(), 1);
    assert_eq!(unknown.requests()[0].version, Version::V1_1);
}

#[test]
fn fragmented_replies_are_joined() {
    let server = Server::start(|r, _| {
        let Message::Request { .. } = &r.message else {
            return Vec::new();
        };
        let Action::Send(whole) = reply(r, ReplyStatus::NoException, |enc| {
            let values: Vec<f64> = (0..100).map(f64::from).collect();
            values.encode(enc).unwrap();
        })
        .remove(0) else {
            unreachable!()
        };
        // Split after 64 bytes: a Reply with more fragments, then a
        // Fragment carrying the rest.
        let split = 64;
        let mut first = whole[..split].to_vec();
        first[6] |= 0x02;
        first[8..12].copy_from_slice(&((split - 12) as u32).to_le_bytes());
        let request_id = u32::from_le_bytes(whole[12..16].try_into().unwrap());
        let mut m =
            MessageEncoder::fragment(r.version, ByteOrder::Little, request_id, false).unwrap();
        m.body().write_octets(&whole[split..]);
        vec![Action::Send(first), Action::Send(m.finish().unwrap())]
    });
    let orb = orb();
    let obj = object(&orb, &server.ior(Version::V1_2));
    let reply = obj.request("values").invoke().unwrap();
    let values: Vec<f64> = reply.input().read().unwrap();
    assert_eq!(values, (0..100).map(f64::from).collect::<Vec<_>>());
}

#[test]
fn string_to_object_and_back() {
    let orb = orb();
    let ior = ior_for(2809, Version::V1_2, true);
    let s = ior.to_ior_string(ByteOrder::Little).unwrap();
    let obj = orb.string_to_object(&s).unwrap().unwrap();
    assert_eq!(obj.type_id(), "IDL:test/Echo:1.0");
    assert_eq!(orb.object_to_string(Some(&obj)).unwrap(), s);
    assert!(
        orb.string_to_object(&Ior::nil().to_ior_string(ByteOrder::Big).unwrap())
            .unwrap()
            .is_none()
    );

    let bad = |s: &str| orb.string_to_object(s).unwrap_err();
    let e = bad("http://example.com/");
    assert!(e.is(SystemExceptionKind::BAD_PARAM));
    assert_eq!(e.minor, minor::BAD_SCHEME_NAME);
    assert_eq!(bad("IOR:0").minor, minor::BAD_SCHEME_SPECIFIC_PART);
    // corbaname: asks the naming service, which is not there; a malformed
    // name fails before that.
    let port = closed_port();
    assert!(bad(&format!("corbaname::127.0.0.1:{port}#a/b")).is(SystemExceptionKind::TRANSIENT));
    assert_eq!(
        bad(&format!("corbaname::127.0.0.1:{port}#a//b")).minor,
        minor::BAD_SCHEME_SPECIFIC_PART
    );
    // With no name it is the context itself.
    let ctx = orb
        .string_to_object(&format!("corbaname::127.0.0.1:{port}"))
        .unwrap()
        .unwrap();
    assert!(ctx.type_id().is_empty());
}

#[test]
fn initial_references() {
    let mut args: Vec<String> = [
        "prog",
        "-ORBInitRef",
        "NameService=corbaloc::127.0.0.1:2809/NameService",
        "-ORBDefaultInitRef",
        "corbaloc::10.0.0.1:2809",
        "-ORBmaxGIOPVersion=1.1",
        "-ORBclientCallTimeOutPeriod",
        "1500",
        "-ORBsupportPerThreadTimeOut",
        "1",
        "-ORBunknown",
        "x",
    ]
    .map(String::from)
    .to_vec();
    let orb = Orb::init(&mut args).unwrap();
    assert_eq!(args, ["prog", "-ORBunknown", "x"]);
    assert_eq!(orb.options().max_giop_version, Version::V1_1);
    assert_eq!(
        orb.options().request_timeout,
        Some(Duration::from_millis(1500))
    );
    assert_eq!(orb.list_initial_services(), ["NameService"]);

    let ns = orb.resolve_initial_references("NameService").unwrap();
    let profile = ns.ior().iiop_profiles().next().unwrap().unwrap();
    assert_eq!((profile.host.as_str(), profile.port), ("127.0.0.1", 2809));
    let other = orb.resolve_initial_references("Other").unwrap();
    let profile = other.ior().iiop_profiles().next().unwrap().unwrap();
    assert_eq!(profile.host, "10.0.0.1");
    assert_eq!(profile.object_key, b"Other");
    let rir = orb
        .string_to_object("corbaloc:rir:/NameService")
        .unwrap()
        .unwrap();
    assert!(rir.is_equivalent(&ns));

    let plain = self::orb();
    assert!(matches!(
        plain.resolve_initial_references("NameService"),
        Err(rustorb_core::ResolveError::InvalidName(_))
    ));
}

#[test]
fn shutdown_refuses_new_calls() {
    let server = Server::start(echo_long);
    let orb = orb();
    let obj = object(&orb, &server.ior(Version::V1_2));
    assert_eq!(call_long(&obj, 1).unwrap(), 1);
    orb.shutdown(true).unwrap();
    let e = call_long(&obj, 2).unwrap_err();
    let e = e.as_system().unwrap();
    assert!(e.is(SystemExceptionKind::BAD_INV_ORDER));
    assert_eq!(e.minor, minor::ORB_SHUT_DOWN);
    thread::sleep(Duration::from_millis(100));
    assert!(
        server
            .received()
            .iter()
            .any(|r| matches!(r.message, Message::CloseConnection))
    );
}

/// How long `call_long` on `obj` takes to fail with `TIMEOUT`, and its
/// completion status.
fn time_out(obj: &rustorb_core::ObjectRef) -> (Duration, CompletionStatus) {
    let start = Instant::now();
    let e = call_long(obj, 1).unwrap_err();
    let e = e.as_system().unwrap().clone();
    assert!(e.is(SystemExceptionKind::TIMEOUT), "{e:?}");
    (start.elapsed(), e.completed)
}

/// Whether `d` is about `ms` milliseconds (well below the next setting
/// the tests use).
fn about(d: Duration, ms: u64) -> bool {
    d >= Duration::from_millis(ms) && d < Duration::from_millis(ms + 400)
}

#[test]
fn call_timeouts_of_the_orb_the_thread_and_the_reference() {
    let server = Server::start(|_, _| Vec::new());
    let orb = orb();
    let ior = server.ior(Version::V1_2);
    assert_eq!(orb.call_timeout(), None);

    orb.set_call_timeout(Some(Duration::from_millis(200)));
    let obj = object(&orb, &ior);
    let (d, completed) = time_out(&obj);
    assert!(about(d, 200), "{d:?}");
    assert_eq!(completed, CompletionStatus::Maybe);

    // The thread's timeout overrides the ORB's, on this thread only.
    orb.set_call_timeout(Some(Duration::from_secs(30)));
    assert_eq!(
        rustorb_core::set_thread_call_timeout(Some(Some(Duration::from_millis(300)))),
        None
    );
    assert!(about(time_out(&obj).0, 300));
    let other = obj.clone();
    let t = thread::spawn(move || {
        rustorb_core::set_thread_call_timeout(Some(Some(Duration::from_millis(100))));
        time_out(&other).0
    });
    assert!(about(t.join().unwrap(), 100));

    // The reference's overrides the thread's, and is shared by clones but
    // not by other references to the IOR.
    obj.set_call_timeout(Some(Some(Duration::from_millis(100))));
    assert!(about(time_out(&obj.clone()).0, 100));
    let fresh = object(&orb, &ior);
    assert_eq!(fresh.call_timeout(), None);
    assert!(about(time_out(&fresh).0, 300));

    // The request's overrides everything.
    let start = Instant::now();
    let e = obj
        .request("echo_long")
        .args(|out| {
            out.write_i32(1);
            Ok(())
        })
        .timeout(Some(Duration::from_millis(500)))
        .invoke()
        .unwrap_err();
    assert!(e.as_system().unwrap().is(SystemExceptionKind::TIMEOUT));
    assert!(about(start.elapsed(), 500));

    // Unsetting falls back to the next one.
    obj.set_call_timeout(None);
    assert!(about(time_out(&obj).0, 300));
    rustorb_core::set_thread_call_timeout(None);
    orb.set_call_timeout(Some(Duration::from_millis(200)));
    assert!(about(time_out(&obj).0, 200));
}

/// A listener whose accept queue is full, so that connecting to it hangs.
fn stuck_listener() -> (TcpListener, Vec<TcpStream>, u16) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .unwrap();
    let listener = rt.block_on(async {
        let socket = tokio::net::TcpSocket::new_v4().unwrap();
        socket.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        socket.listen(0).unwrap().into_std().unwrap()
    });
    let addr = listener.local_addr().unwrap();
    // Fill the queue until a connection hangs; how many fit depends on
    // the operating system (one on Linux, more on macOS).
    let mut fillers = Vec::new();
    while let Ok(c) = TcpStream::connect_timeout(&addr, Duration::from_millis(100)) {
        fillers.push(c);
        assert!(fillers.len() < 4096, "the accept queue does not fill");
    }
    (listener, fillers, addr.port())
}

#[test]
fn timeouts_while_connecting() {
    let (_listener, _fillers, port) = stuck_listener();
    let ior = profile_ior(port, Version::V1_2, true, Vec::new());
    let orb = orb();
    let obj = object(&orb, &ior);

    // The call timeout covers connecting.
    orb.set_call_timeout(Some(Duration::from_millis(200)));
    let (d, completed) = time_out(&obj);
    assert!(about(d, 200), "{d:?}");
    assert_eq!(completed, CompletionStatus::No);

    // A connect timeout replaces it while connecting.
    orb.set_connect_timeout(Some(Duration::from_millis(500)));
    assert_eq!(orb.connect_timeout(), Some(Duration::from_millis(500)));
    let (d, completed) = time_out(&obj);
    assert!(about(d, 500), "{d:?}");
    assert_eq!(completed, CompletionStatus::No);
}
