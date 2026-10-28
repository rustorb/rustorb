//! The CosNaming stubs and `corbaname:` against a scripted naming service
//! on localhost.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use rustorb_core::cdr::{ByteOrder, CdrDecode, CdrEncode, Decoder, Encoder};
use rustorb_core::giop::{
    Body, IiopProfile, Ior, Message, MessageEncoder, MessageHeader, ReplyHeader, ReplyStatus,
    SystemExceptionBody, Version, decode_message,
};
use rustorb_core::{CompletionStatus, Orb, OrbOptions, SystemExceptionKind, minor};
use rustorb_naming::{
    BindingType, NameComponent, NamingContext, NamingContextExt, NamingError, NotFoundReason,
    repository_id, to_name,
};

/// A request the server received: operation, object key and body.
#[derive(Debug, Clone)]
struct Call {
    operation: String,
    key: Vec<u8>,
    body: Body,
}

impl Call {
    fn decoder(&self) -> Decoder<'_> {
        self.body.decoder()
    }
}

type BodyFn = Box<dyn FnOnce(&mut Encoder)>;

/// What the server answers.
enum Answer {
    Ok(BodyFn),
    User(&'static str, BodyFn),
    System(SystemExceptionKind),
}

fn ok(f: impl FnOnce(&mut Encoder) + 'static) -> Answer {
    Answer::Ok(Box::new(f))
}

fn user(id: &'static str, f: impl FnOnce(&mut Encoder) + 'static) -> Answer {
    Answer::User(id, Box::new(f))
}

type Handler = dyn Fn(&Call, u16) -> Answer + Send + Sync;

/// A GIOP 1.2 server that answers each request with the handler and
/// records the requests.
struct Server {
    port: u16,
    calls: Arc<Mutex<Vec<Call>>>,
}

impl Server {
    fn start(handler: impl Fn(&Call, u16) -> Answer + Send + Sync + 'static) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let calls: Arc<Mutex<Vec<Call>>> = Arc::default();
        let handler: Arc<Handler> = Arc::new(handler);
        let c = Arc::clone(&calls);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let handler = Arc::clone(&handler);
                let calls = Arc::clone(&c);
                thread::spawn(move || serve(stream, port, &*handler, &calls));
            }
        });
        Server { port, calls }
    }

    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    fn url(&self) -> String {
        format!("corbaloc::127.0.0.1:{}/NameService", self.port)
    }
}

fn serve(mut stream: TcpStream, port: u16, handler: &Handler, calls: &Mutex<Vec<Call>>) {
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
        let Message::Request { header, body } = message else {
            continue;
        };
        let call = Call {
            operation: header.operation.clone(),
            key: match &header.target {
                rustorb_core::giop::TargetAddress::Key(key) => key.clone(),
                other => panic!("unexpected target {other:?}"),
            },
            body,
        };
        calls.lock().unwrap().push(call.clone());
        let (status, write): (ReplyStatus, BodyFn) = match handler(&call, port) {
            Answer::Ok(f) => (ReplyStatus::NoException, f),
            Answer::User(id, f) => (
                ReplyStatus::UserException,
                Box::new(move |enc: &mut Encoder| {
                    enc.write_string(id).unwrap();
                    f(enc);
                }),
            ),
            Answer::System(kind) => (
                ReplyStatus::SystemException,
                Box::new(move |enc: &mut Encoder| {
                    SystemExceptionBody {
                        exception_id: kind.repository_id().into(),
                        minor_code_value: 0,
                        completion_status: CompletionStatus::No,
                    }
                    .encode(enc)
                    .unwrap();
                }),
            ),
        };
        let mut m = MessageEncoder::reply(
            h.version,
            ByteOrder::Little,
            &ReplyHeader {
                request_id: header.request_id,
                reply_status: status,
                service_context: Vec::new(),
            },
        )
        .unwrap();
        write(m.body());
        if stream.write_all(&m.finish().unwrap()).is_err() {
            return;
        }
    }
}

/// An IOR for an object with `key` on the server.
fn ior(port: u16, type_id: &str, key: &[u8]) -> Ior {
    let profile = IiopProfile {
        version: Version::V1_2,
        host: "127.0.0.1".into(),
        port,
        object_key: key.to_vec(),
        components: Vec::new(),
    };
    Ior {
        type_id: type_id.into(),
        profiles: vec![profile.to_tagged_profile(ByteOrder::Big).unwrap()],
    }
}

fn read_name(dec: &mut Decoder<'_>) -> Vec<NameComponent> {
    let len = dec.read_u32().unwrap();
    (0..len)
        .map(|_| NameComponent::new(dec.read_string().unwrap(), dec.read_string().unwrap()))
        .collect()
}

fn write_name(enc: &mut Encoder, name: &[NameComponent]) {
    enc.write_u32(name.len() as u32);
    for c in name {
        enc.write_string(&c.id).unwrap();
        enc.write_string(&c.kind).unwrap();
    }
}

fn nc(id: &str, kind: &str) -> NameComponent {
    NameComponent::new(id, kind)
}

fn orb() -> Orb {
    Orb::with_options(OrbOptions::default()).unwrap()
}

fn root(orb: &Orb, server: &Server) -> NamingContextExt {
    let obj = orb.string_to_object(&server.url()).unwrap().unwrap();
    NamingContextExt::unchecked_narrow(obj)
}

const ECHO_ID: &str = "IDL:test/Echo:1.0";

/// Resolves `a/b.k` to an Echo object, and fails for anything else.
fn resolver(call: &Call, port: u16) -> Answer {
    match call.operation.as_str() {
        "resolve" => {
            let name = read_name(&mut call.decoder());
            if name == [nc("a", ""), nc("b", "k")] {
                let ior = ior(port, ECHO_ID, b"echo");
                ok(move |enc| ior.encode(enc).unwrap())
            } else if name.is_empty() {
                user(repository_id::INVALID_NAME, |_| {})
            } else {
                let rest = name[1..].to_vec();
                user(repository_id::NOT_FOUND, move |enc| {
                    enc.write_u32(1); // not_context
                    write_name(enc, &rest);
                })
            }
        }
        "_is_a" => {
            let id = call.decoder().read_string().unwrap();
            ok(move |enc| enc.write_bool(id == repository_id::NAMING_CONTEXT))
        }
        _ => Answer::System(SystemExceptionKind::BAD_OPERATION),
    }
}

#[test]
fn resolve_marshals_the_name_and_reads_the_object() {
    let server = Server::start(resolver);
    let orb = orb();
    let root = root(&orb, &server);
    let obj = root.resolve(&to_name("a/b.k").unwrap()).unwrap().unwrap();
    assert_eq!(obj.type_id(), ECHO_ID);
    assert_eq!(
        obj.ior().profiles,
        ior(server.port, ECHO_ID, b"echo").profiles
    );

    let calls = server.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].operation, "resolve");
    assert_eq!(calls[0].key, b"NameService");
}

#[test]
fn user_exceptions_are_decoded() {
    let server = Server::start(resolver);
    let orb = orb();
    let root = root(&orb, &server);
    match root.resolve(&to_name("x/y/z").unwrap()) {
        Err(NamingError::NotFound { why, rest_of_name }) => {
            assert_eq!(why, NotFoundReason::NotContext);
            assert_eq!(rest_of_name, [nc("y", ""), nc("z", "")]);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(root.resolve(&[]), Err(NamingError::InvalidName)));
    // A system exception is passed on.
    match root.unbind(&to_name("a").unwrap()) {
        Err(NamingError::System(e)) => assert!(e.is(SystemExceptionKind::BAD_OPERATION)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn undeclared_user_exceptions_become_unknown() {
    let server = Server::start(|_, _| user(repository_id::NOT_EMPTY, |_| {}));
    let orb = orb();
    let root = root(&orb, &server);
    match root.resolve(&to_name("a").unwrap()) {
        Err(NamingError::System(e)) => {
            assert!(e.is(SystemExceptionKind::UNKNOWN));
            assert_eq!(e.minor, minor::UNLISTED_USER_EXCEPTION);
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(root.destroy(), Err(NamingError::NotEmpty)));
}

#[test]
fn bind_sends_name_and_object() {
    let server = Server::start(|call, _| match call.operation.as_str() {
        "bind" => user(repository_id::ALREADY_BOUND, |_| {}),
        _ => ok(|_| {}),
    });
    let orb = orb();
    let root = root(&orb, &server);
    let echo = orb
        .string_to_object(
            &ior(server.port, ECHO_ID, b"echo")
                .to_ior_string(ByteOrder::Big)
                .unwrap(),
        )
        .unwrap()
        .unwrap();
    assert!(matches!(
        root.bind(&to_name("a.b").unwrap(), &echo),
        Err(NamingError::AlreadyBound)
    ));
    root.rebind(&to_name("a.b").unwrap(), &echo).unwrap();

    let calls = server.calls();
    let ops: Vec<_> = calls.iter().map(|c| c.operation.as_str()).collect();
    assert_eq!(ops, ["bind", "rebind"]);
    let mut dec = calls[1].decoder();
    assert_eq!(read_name(&mut dec), [nc("a", "b")]);
    let sent = Ior::decode(&mut dec).unwrap();
    assert_eq!(sent.type_id, ECHO_ID);
    assert!(dec.is_empty());
}

#[test]
fn list_and_iterate() {
    let server = Server::start(
        |call, port| match (call.key.as_slice(), call.operation.as_str()) {
            (b"NameService", "list") => {
                assert_eq!(call.decoder().read_u32().unwrap(), 1);
                let it = ior(port, repository_id::BINDING_ITERATOR, b"iter");
                ok(move |enc| {
                    enc.write_u32(1);
                    write_name(enc, &[nc("a", "")]);
                    enc.write_u32(0);
                    it.encode(enc).unwrap();
                })
            }
            (b"iter", "next_n") => ok(|enc| {
                enc.write_bool(true);
                enc.write_u32(2);
                write_name(enc, &[nc("b", "kind")]);
                enc.write_u32(1);
                write_name(enc, &[nc("c", "")]);
                enc.write_u32(0);
            }),
            (b"iter", "next_one") => ok(|enc| {
                enc.write_bool(false);
                write_name(enc, &[]);
                enc.write_u32(0);
            }),
            (b"iter", "destroy") => ok(|_| {}),
            other => panic!("unexpected {other:?}"),
        },
    );
    let orb = orb();
    let root = root(&orb, &server);
    let (first, it) = root.list(1).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].binding_name, [nc("a", "")]);
    assert_eq!(first[0].binding_type, BindingType::Object);
    let it = it.expect("an iterator");
    let (more, rest) = it.next_n(2).unwrap();
    assert!(more);
    assert_eq!(rest.len(), 2);
    assert_eq!(rest[0].binding_type, BindingType::Context);
    assert_eq!(rest[1].binding_name, [nc("c", "")]);
    assert_eq!(it.next_one().unwrap(), None);
    it.destroy().unwrap();
}

#[test]
fn ext_operations() {
    let server = Server::start(|call, _| {
        let mut dec = call.decoder();
        match call.operation.as_str() {
            "to_string" => {
                let name = read_name(&mut dec);
                ok(move |enc| {
                    enc.write_string(&rustorb_naming::to_string(&name).unwrap())
                        .unwrap()
                })
            }
            "to_name" => {
                let s = dec.read_string().unwrap();
                match to_name(&s) {
                    Ok(name) => ok(move |enc| write_name(enc, &name)),
                    Err(_) => user(repository_id::INVALID_NAME, |_| {}),
                }
            }
            "to_url" => {
                let (addr, sn) = (dec.read_string().unwrap(), dec.read_string().unwrap());
                match rustorb_naming::to_url(&addr, &sn) {
                    Ok(url) => ok(move |enc| enc.write_string(&url).unwrap()),
                    Err(_) => user(repository_id::INVALID_ADDRESS, |_| {}),
                }
            }
            op => panic!("unexpected {op}"),
        }
    });
    let orb = orb();
    let root = root(&orb, &server);
    let name = [nc("a", "b"), nc("c/d", ""), nc("", "e")];
    assert_eq!(root.to_string(&name).unwrap(), "a.b/c\\/d/.e");
    assert_eq!(root.to_name("a.b/c\\/d/.e").unwrap(), name);
    assert!(matches!(
        root.to_name("a//b"),
        Err(NamingError::InvalidName)
    ));
    assert_eq!(
        root.to_url(":host:2809", "a/b c").unwrap(),
        "corbaname::host:2809#a/b%20c"
    );
    assert!(matches!(
        root.to_url("host:2809", "a"),
        Err(NamingError::InvalidAddress)
    ));
}

#[test]
fn narrow_asks_the_object_unless_the_type_id_says() {
    let server = Server::start(resolver);
    let orb = orb();
    // corbaloc gives an empty type id, so narrowing asks with _is_a.
    let obj = orb.string_to_object(&server.url()).unwrap().unwrap();
    assert!(NamingContext::narrow(obj.clone()).unwrap().is_some());
    assert!(NamingContextExt::narrow(obj).unwrap().is_none());
    assert_eq!(server.calls().len(), 2);

    let typed = ior(
        server.port,
        repository_id::NAMING_CONTEXT_EXT,
        b"NameService",
    );
    let typed = orb
        .string_to_object(&typed.to_ior_string(ByteOrder::Big).unwrap())
        .unwrap()
        .unwrap();
    assert!(NamingContext::narrow(typed.clone()).unwrap().is_some());
    assert!(NamingContextExt::narrow(typed).unwrap().is_some());
    assert_eq!(server.calls().len(), 2);
}

#[test]
fn corbaname_resolves_through_the_orb() {
    let server = Server::start(resolver);
    let orb = orb();
    let base = format!("corbaname::127.0.0.1:{}", server.port);

    let obj = orb
        .string_to_object(&format!("{base}#a/b.k"))
        .unwrap()
        .unwrap();
    assert_eq!(obj.type_id(), ECHO_ID);
    // URL escapes are undone before the name is parsed.
    let obj = orb
        .string_to_object(&format!("{base}#%61/b%2Ek"))
        .unwrap()
        .unwrap();
    assert_eq!(obj.type_id(), ECHO_ID);
    let calls = server.calls();
    assert_eq!(calls.len(), 2);
    assert!(
        calls
            .iter()
            .all(|c| c.operation == "resolve" && c.key == b"NameService")
    );

    // No name: the context itself, with no request.
    let ctx = orb
        .string_to_object(&format!("{base}/Other"))
        .unwrap()
        .unwrap();
    let profile = ctx.ior().iiop_profiles().next().unwrap().unwrap();
    assert_eq!(
        (profile.port, profile.object_key.as_slice()),
        (server.port, &b"Other"[..])
    );
    assert_eq!(server.calls().len(), 2);

    // Not bound, and a malformed name: the same exceptions and minor codes
    // as omniORB 4.3.4 raises (measured with omniORBpy against omniNames).
    let e = orb.string_to_object(&format!("{base}#x/y")).unwrap_err();
    assert!(e.is(SystemExceptionKind::BAD_PARAM));
    assert_eq!(e.minor, minor::STRING_TO_OBJECT_OTHER);
    let e = orb.string_to_object(&format!("{base}#a//b")).unwrap_err();
    assert!(e.is(SystemExceptionKind::BAD_PARAM));
    assert_eq!(e.minor, minor::BAD_SCHEME_SPECIFIC_PART);
    assert_eq!(server.calls().len(), 3);
}

#[test]
fn corbaname_passes_system_exceptions_on() {
    let server = Server::start(|_, _| Answer::System(SystemExceptionKind::OBJECT_NOT_EXIST));
    let orb = orb();
    let e = orb
        .string_to_object(&format!("corbaname::127.0.0.1:{}#a", server.port))
        .unwrap_err();
    assert!(e.is(SystemExceptionKind::OBJECT_NOT_EXIST));
}
