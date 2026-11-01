//! The largest GIOP message (`-ORBgiopMaxMsgSize`, docs/message-size.md):
//! requests and replies over it are not sent.

use std::sync::Arc;

use rustorb_core::{
    CompletionStatus, EndpointAddr, InStream, ObjectRef, Orb, OrbOptions, Servant, ServerReply,
    SystemException, SystemExceptionKind,
};

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
            // octets n: n bytes back.
            "octets" => {
                let n = input.read::<u32>()? as usize;
                reply.output().write_octet_seq(&vec![7; n])?;
                Ok(())
            }
            "echo" => {
                let data = input.read_octet_seq()?.to_vec();
                reply.output().write_octet_seq(&data)?;
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

fn args(s: &str) -> Vec<String> {
    s.split_whitespace().map(str::to_owned).collect()
}

/// A server ORB with one Echo object and a client ORB with the object's
/// reference, each with its own ORB arguments.
fn pair(server_args: &str, client_args: &str) -> (Orb, Orb, ObjectRef) {
    let mut options = OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..OrbOptions::default()
    };
    options.parse_args(&mut args(server_args)).unwrap();
    let server = Orb::with_options(options).unwrap();
    let root = server.root_poa().unwrap();
    let id = root.activate_object(Arc::new(Echo)).unwrap();
    root.the_poa_manager().activate().unwrap();
    let local = root.id_to_reference(&id).unwrap();
    let client = Orb::init(&mut args(client_args)).unwrap();
    let ior = server.object_to_string(Some(&local)).unwrap();
    let obj = client.string_to_object(&ior).unwrap().unwrap();
    (server, client, obj)
}

fn octets(obj: &ObjectRef, n: u32) -> Result<usize, SystemException> {
    let reply = obj
        .request("octets")
        .args(|out| {
            out.write_u32(n);
            Ok(())
        })
        .invoke()
        .map_err(|e| e.as_system().cloned().unwrap())?;
    let len = reply.input().read_octet_seq()?.len();
    Ok(len)
}

fn echo(obj: &ObjectRef, n: usize) -> Result<usize, SystemException> {
    let reply = obj
        .request("echo")
        .args(|out| {
            out.write_octet_seq(&vec![3; n])?;
            Ok(())
        })
        .invoke()
        .map_err(|e| e.as_system().cloned().unwrap())?;
    let len = reply.input().read_octet_seq()?.len();
    Ok(len)
}

#[test]
fn option_is_parsed() {
    let mut options = OrbOptions::default();
    assert_eq!(options.max_message_size, 64 * 1024 * 1024);
    let mut a = args("prog -ORBgiopMaxMsgSize 2097152 rest");
    options.parse_args(&mut a).unwrap();
    assert_eq!(a, args("prog rest"));
    assert_eq!(options.max_message_size, 2 * 1024 * 1024);
    options
        .parse_args(&mut args("-ORBgiopMaxMsgSize=4096"))
        .unwrap();
    assert_eq!(options.max_message_size, 4096);
    for bad in [
        "-ORBgiopMaxMsgSize 1023",
        "-ORBgiopMaxMsgSize big",
        "-ORBgiopMaxMsgSize",
    ] {
        assert!(options.parse_args(&mut args(bad)).is_err(), "{bad}");
    }
}

#[test]
fn request_over_the_limit_is_not_sent() {
    let (server, client, obj) = pair("", "-ORBgiopMaxMsgSize 65536");
    assert_eq!(echo(&obj, 60_000).unwrap(), 60_000);
    let e = echo(&obj, 70_000).unwrap_err();
    assert!(e.is(SystemExceptionKind::MARSHAL), "{e}");
    assert_eq!(e.completed, CompletionStatus::No);
    // The connection is still good.
    assert_eq!(echo(&obj, 10).unwrap(), 10);
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn reply_over_the_limit_is_marshal_completed_yes() {
    let (server, client, obj) = pair("-ORBgiopMaxMsgSize 65536", "");
    assert_eq!(octets(&obj, 60_000).unwrap(), 60_000);
    let e = octets(&obj, 70_000).unwrap_err();
    assert!(e.is(SystemExceptionKind::MARSHAL), "{e}");
    assert_eq!(e.completed, CompletionStatus::Yes);
    assert_eq!(octets(&obj, 10).unwrap(), 10);
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn request_over_the_servers_limit_fails() {
    // The client does not know the server's limit: the server closes the
    // connection, and the call fails.
    let (server, client, obj) = pair("-ORBgiopMaxMsgSize 65536", "");
    assert!(echo(&obj, 70_000).is_err());
    assert_eq!(echo(&obj, 10).unwrap(), 10);
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn fragment_size_option() {
    let mut options = OrbOptions::default();
    assert_eq!(options.fragment_size, 0);
    options
        .parse_args(&mut args("-ORBgiopFragmentSize 4096"))
        .unwrap();
    assert_eq!(options.fragment_size, 4096);
    options
        .parse_args(&mut args("-ORBgiopFragmentSize=0"))
        .unwrap();
    assert_eq!(options.fragment_size, 0);
    for bad in ["-ORBgiopFragmentSize 100", "-ORBgiopFragmentSize x"] {
        assert!(options.parse_args(&mut args(bad)).is_err(), "{bad}");
    }
}

#[test]
fn fragmented_requests_and_replies() {
    for (server_args, client_args) in [
        ("-ORBgiopFragmentSize 1024", "-ORBgiopFragmentSize 1024"),
        ("-ORBgiopFragmentSize 1031", ""),
        ("", "-ORBgiopFragmentSize 2048"),
        // GIOP 1.1 is never fragmented.
        (
            "-ORBgiopFragmentSize 1024",
            "-ORBgiopFragmentSize 1024 -ORBmaxGIOPVersion 1.1",
        ),
    ] {
        let (server, client, obj) = pair(server_args, client_args);
        for n in [0, 10, 1000, 1024, 5000, 200_000] {
            assert_eq!(echo(&obj, n).unwrap(), n, "{server_args} / {client_args}");
            assert_eq!(octets(&obj, n as u32).unwrap(), n);
        }
        client.destroy().unwrap();
        server.destroy().unwrap();
    }
}

#[test]
fn limit_applies_to_the_whole_fragmented_message() {
    let (server, client, obj) = pair("", "-ORBgiopFragmentSize 1024 -ORBgiopMaxMsgSize 65536");
    assert_eq!(echo(&obj, 60_000).unwrap(), 60_000);
    let e = echo(&obj, 70_000).unwrap_err();
    assert!(e.is(SystemExceptionKind::MARSHAL), "{e}");
    client.destroy().unwrap();
    server.destroy().unwrap();
}
