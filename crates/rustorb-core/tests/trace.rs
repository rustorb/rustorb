//! Trace output (docs/tracing.md): events reach a `tracing` subscriber,
//! and without one `-ORBtraceLevel` writes them to standard error.

use std::process::Command;
use std::sync::{Arc, Mutex};

use rustorb_core::{
    CompletionStatus, EndpointAddr, InStream, ObjectRef, Orb, OrbOptions, Servant, ServerReply,
    SystemException, SystemExceptionKind,
};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

const ECHO_ID: &str = "IDL:test/Echo:1.0";

struct Echo;

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
            _ => Err(SystemException::new(
                SystemExceptionKind::NO_PERMISSION,
                3,
                CompletionStatus::No,
            )),
        }
    }
}

/// A server ORB with one Echo object, a client ORB, and the object's
/// reference in the client.
/// Both take the ORB options in `args`.
fn pair(mut args: Vec<String>) -> (Orb, Orb, ObjectRef) {
    let mut options = OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..OrbOptions::default()
    };
    options.parse_args(&mut args.clone()).unwrap();
    let server = Orb::with_options(options).unwrap();
    let root = server.root_poa().unwrap();
    let id = root.activate_object(Arc::new(Echo)).unwrap();
    root.the_poa_manager().activate().unwrap();
    let local = root.id_to_reference(&id).unwrap();
    let client = Orb::init(&mut args).unwrap();
    let ior = server.object_to_string(Some(&local)).unwrap();
    let obj = client.string_to_object(&ior).unwrap().unwrap();
    (server, client, obj)
}

fn calls(obj: &ObjectRef) {
    let reply = obj
        .request("echo_long")
        .args(|out| {
            out.write_i32(42);
            Ok(())
        })
        .invoke()
        .unwrap();
    assert_eq!(reply.input().read::<i32>().unwrap(), 42);
    assert!(obj.request("refused").invoke().is_err());
}

const CHILD: &str = "RUSTORB_TRACE_TEST_CHILD";

/// Run by [`stderr_without_subscriber`] in a process of its own.
#[test]
fn child_writes_trace() {
    let Ok(args) = std::env::var(CHILD) else {
        return;
    };
    let args = args.split(' ').map(str::to_owned).collect();
    let (server, client, obj) = pair(args);
    calls(&obj);
    client.destroy().unwrap();
    server.destroy().unwrap();
}

fn child_stderr(args: &str) -> String {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["child_writes_trace", "--exact", "--nocapture"])
        .env(CHILD, args)
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stderr).unwrap()
}

#[test]
fn stderr_without_subscriber() {
    let err = child_stderr("prog -ORBtraceLevel 25");
    for line in [
        "rustorb: info: listening on 127.0.0.1:",
        "rustorb: info: connected to 127.0.0.1:",
        "rustorb: info: accepted a connection from 127.0.0.1:",
        "rustorb: debug: request \"echo_long\" (id 0) to 127.0.0.1:",
        "rustorb: trace: sent Request (GIOP 1.2, ",
        "rustorb: trace: received Reply (GIOP 1.2, ",
        "rustorb: debug: reply to request \"echo_long\" (id 0): NoException",
        "rustorb: debug: request id 1: replying CORBA::NO_PERMISSION (minor 0x00000003, COMPLETED_NO)",
        "rustorb: debug: request \"refused\" raised CORBA::NO_PERMISSION",
        "rustorb: info: closing the connection to 127.0.0.1:",
    ] {
        assert!(err.contains(line), "no {line:?} in:\n{err}");
    }

    // Level 5 leaves out the requests and messages; the time comes first
    // with -ORBtraceTime.
    let err = child_stderr("prog -ORBtraceLevel 5 -ORBtraceTime 1");
    assert!(err.contains(" rustorb: info: connected to"), "{err}");
    assert!(!err.contains("debug:") && !err.contains("trace:"), "{err}");
    let first = err.lines().find(|l| l.contains("rustorb:")).unwrap();
    assert!(
        first.starts_with("20") && first.as_bytes()[10] == b' ',
        "{first}"
    );

    // The default level, 1, has nothing to say about successful calls.
    let err = child_stderr("prog");
    assert!(!err.contains("rustorb:"), "{err}");
}

/// What the collecting subscriber saw: level, target, message.
type Seen = Vec<(Level, String, String)>;

static SEEN: Mutex<Seen> = Mutex::new(Vec::new());

struct Collect;

struct Message(String);

impl Visit for Message {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

impl Subscriber for Collect {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut msg = Message(String::new());
        event.record(&mut msg);
        let meta = event.metadata();
        SEEN.lock()
            .unwrap()
            .push((*meta.level(), meta.target().to_owned(), msg.0));
    }
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}
}

#[test]
fn events_reach_a_subscriber() {
    tracing::subscriber::set_global_default(Collect).unwrap();
    // The subscriber gets every event, whatever the trace level.
    let (server, client, obj) = pair(vec!["-ORBtraceLevel".into(), "0".into()]);
    calls(&obj);
    client.destroy().unwrap();
    server.destroy().unwrap();
    let seen = SEEN.lock().unwrap();
    let has = |level: Level, target: &str, text: &str| {
        seen.iter()
            .any(|(l, t, m)| *l == level && t == target && m.contains(text))
    };
    assert!(
        has(Level::INFO, "rustorb_core::connection", "connected to"),
        "{seen:?}"
    );
    assert!(
        has(Level::INFO, "rustorb_core::server", "listening on"),
        "{seen:?}"
    );
    assert!(
        has(
            Level::DEBUG,
            "rustorb_core::object",
            "reply to request \"echo_long\""
        ),
        "{seen:?}"
    );
    assert!(
        has(
            Level::DEBUG,
            "rustorb_core::server",
            "request \"echo_long\""
        ),
        "{seen:?}"
    );
    assert!(
        has(Level::TRACE, "rustorb_core::wire", "received Request"),
        "{seen:?}"
    );
    assert!(
        has(Level::DEBUG, "rustorb_core::poa", "activated object"),
        "{seen:?}"
    );
}
