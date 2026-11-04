//! `CallbackRequest`: requests whose reply goes to a callback, the base of
//! AMI (`docs/ami.md`).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rustorb_core::{
    CompletionStatus, EndpointAddr, Exception, InStream, ObjectRef, Orb, OrbOptions, Reply,
    Servant, ServerReply, SystemException, SystemExceptionKind, minor,
};

const TEST_ID: &str = "IDL:test/Ami:1.0";
const OOPS_ID: &str = "IDL:test/Oops:1.0";

#[derive(Default)]
struct TestServant {
    posted: Mutex<Vec<i32>>,
}

impl Servant for TestServant {
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
            "echo_string" => {
                let v = input.read_string()?;
                reply.output().write_string(&v)?;
                Ok(())
            }
            "fail" => {
                let v = input.read::<i32>()?;
                reply.user_exception(OOPS_ID)?.write_i32(v);
                Ok(())
            }
            "sleep" => {
                let ms = input.read::<u32>()?;
                thread::sleep(Duration::from_millis(u64::from(ms)));
                Ok(())
            }
            "post" => {
                let v = input.read::<i32>()?;
                self.posted.lock().unwrap().push(v);
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

fn server_options() -> OrbOptions {
    OrbOptions {
        endpoints: vec![EndpointAddr {
            host: "127.0.0.1".into(),
            port: 0,
        }],
        ..OrbOptions::default()
    }
}

fn serve(orb: &Orb, servant: Arc<TestServant>) -> ObjectRef {
    let root = orb.root_poa().unwrap();
    let id = root.activate_object(servant).unwrap();
    let obj = root.id_to_reference(&id).unwrap();
    root.the_poa_manager().activate().unwrap();
    obj
}

fn remote(client: &Orb, server: &Orb, obj: &ObjectRef) -> ObjectRef {
    client
        .string_to_object(&server.object_to_string(Some(obj)).unwrap())
        .unwrap()
        .unwrap()
}

/// What a callback saw: the result and the name of its thread.
type Seen = (Result<Reply, Exception>, Option<String>);

/// Sends `op` with the long `v`; the receiver gets what the callback saw.
fn send_long(obj: &ObjectRef, op: &str, v: i32) -> mpsc::Receiver<Seen> {
    let (tx, rx) = mpsc::channel();
    obj.request_callback(op)
        .args(move |out| {
            out.write_i32(v);
            Ok(())
        })
        .send(move |result| {
            let name = thread::current().name().map(str::to_owned);
            let _ = tx.send((result, name));
        })
        .unwrap();
    rx
}

fn wait(rx: &mpsc::Receiver<Seen>) -> Seen {
    rx.recv_timeout(Duration::from_secs(10))
        .expect("the callback was not called")
}

fn long_of(result: Result<Reply, Exception>) -> i32 {
    result.unwrap().input().read::<i32>().unwrap()
}

fn system_of(result: Result<Reply, Exception>) -> SystemException {
    match result {
        Err(Exception::System(e)) => e,
        Err(Exception::User(u)) => panic!("user exception {}", u.repository_id()),
        Ok(_) => panic!("a reply"),
    }
}

#[test]
fn replies_reach_the_callback_on_the_reply_pool() {
    let server = Orb::with_options(server_options()).unwrap();
    let obj = serve(&server, Arc::default());
    // A client without a server side.
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let target = remote(&client, &server, &obj);

    let receivers: Vec<_> = (0..50)
        .map(|i| send_long(&target, "echo_long", i))
        .collect();
    for (i, rx) in receivers.iter().enumerate() {
        let (result, name) = wait(rx);
        assert_eq!(long_of(result), i as i32);
        assert_eq!(name.as_deref(), Some("rustorb-upcall"));
    }

    // Arguments the function owns, written after send returned.
    let text = String::from("héllo");
    let (tx, rx) = mpsc::channel();
    target
        .request_callback("echo_string")
        .args(move |out| out.write_string(&text))
        .send(move |r| {
            let _ = tx.send(r.map(|reply| reply.input().read_string().unwrap()));
        })
        .unwrap();
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap(),
        "héllo"
    );

    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn exceptions_reach_the_callback() {
    let server = Orb::with_options(server_options()).unwrap();
    let obj = serve(&server, Arc::default());
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let target = remote(&client, &server, &obj);

    match wait(&send_long(&target, "fail", 7)).0 {
        Err(Exception::User(u)) => {
            assert_eq!(u.repository_id(), OOPS_ID);
            let mut input = u.input().unwrap();
            assert_eq!(input.read::<i32>().unwrap(), 7);
        }
        _ => panic!("expected the user exception"),
    }

    let e = system_of(wait(&send_long(&target, "nope", 0)).0);
    assert_eq!(e.kind(), Some(SystemExceptionKind::BAD_OPERATION));

    // A server that is gone.
    let dead = {
        let other = Orb::with_options(server_options()).unwrap();
        let o = serve(&other, Arc::default());
        let r = remote(&client, &other, &o);
        other.destroy().unwrap();
        r
    };
    let e = system_of(wait(&send_long(&dead, "echo_long", 1)).0);
    assert!(
        matches!(
            e.kind(),
            Some(SystemExceptionKind::TRANSIENT | SystemExceptionKind::COMM_FAILURE)
        ),
        "{e:?}"
    );

    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn timeouts_apply() {
    let server = Orb::with_options(server_options()).unwrap();
    let obj = serve(&server, Arc::default());
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let target = remote(&client, &server, &obj);

    let (tx, rx) = mpsc::channel();
    target
        .request_callback("sleep")
        .args(|out| {
            out.write_u32(2000);
            Ok(())
        })
        .timeout(Some(Duration::from_millis(100)))
        .send(move |r| {
            let _ = tx.send(r);
        })
        .unwrap();
    let e = system_of(rx.recv_timeout(Duration::from_secs(10)).unwrap());
    assert_eq!(e.kind(), Some(SystemExceptionKind::TIMEOUT));
    assert_eq!(e.completed, CompletionStatus::Maybe);

    // The timeout of the reference, taken when the request is sent.
    target.set_call_timeout(Some(Some(Duration::from_millis(100))));
    let rx = {
        let (tx, rx) = mpsc::channel();
        target
            .request_callback("sleep")
            .args(|out| {
                out.write_u32(2000);
                Ok(())
            })
            .send(move |r| {
                let _ = tx.send(r);
            })
            .unwrap();
        rx
    };
    target.set_call_timeout(None);
    let e = system_of(rx.recv_timeout(Duration::from_secs(10)).unwrap());
    assert_eq!(e.kind(), Some(SystemExceptionKind::TIMEOUT));

    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn objects_of_the_same_orb() {
    let orb = Orb::with_options(server_options()).unwrap();
    let servant = Arc::new(TestServant::default());
    let obj = serve(&orb, servant.clone());

    assert_eq!(long_of(wait(&send_long(&obj, "echo_long", 5)).0), 5);

    // The callback makes a synchronous call: the reply pool is not a
    // runtime worker.
    let (tx, rx) = mpsc::channel();
    let again = obj.clone();
    obj.request_callback("echo_long")
        .args(|out| {
            out.write_i32(1);
            Ok(())
        })
        .send(move |r| {
            let first = long_of(r);
            let reply = again
                .request("post")
                .args(move |out| {
                    out.write_i32(first + 1);
                    Ok(())
                })
                .invoke();
            let _ = tx.send(reply.is_ok());
        })
        .unwrap();
    assert!(rx.recv_timeout(Duration::from_secs(10)).unwrap());
    assert_eq!(*servant.posted.lock().unwrap(), vec![2]);

    orb.destroy().unwrap();
}

#[test]
fn shut_down_orbs() {
    let server = Orb::with_options(server_options()).unwrap();
    let obj = serve(&server, Arc::default());
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let target = remote(&client, &server, &obj);

    // Waiting for a reply when the client is destroyed: the callback is
    // still called, once.
    let calls = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = mpsc::channel();
    let counted = Arc::clone(&calls);
    target
        .request_callback("sleep")
        .args(|out| {
            out.write_u32(1000);
            Ok(())
        })
        .send(move |r| {
            counted.fetch_add(1, Ordering::SeqCst);
            let _ = tx.send(r);
        })
        .unwrap();
    thread::sleep(Duration::from_millis(100));
    client.destroy().unwrap();
    let e = system_of(rx.recv_timeout(Duration::from_secs(10)).unwrap());
    assert!(
        matches!(
            e.kind(),
            Some(SystemExceptionKind::TRANSIENT | SystemExceptionKind::COMM_FAILURE)
        ),
        "{e:?}"
    );
    thread::sleep(Duration::from_millis(100));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // Sending once shut down fails, without calling the callback.
    let e = target
        .request_callback("echo_long")
        .send(|_| panic!("called"))
        .unwrap_err();
    assert_eq!(e.kind(), Some(SystemExceptionKind::BAD_INV_ORDER));
    assert_eq!(e.minor, minor::ORB_SHUT_DOWN);

    server.destroy().unwrap();
}

#[test]
fn from_async_code() {
    let server = Orb::with_options(server_options()).unwrap();
    let obj = serve(&server, Arc::default());
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let client = Orb::with_runtime(OrbOptions::default(), rt.handle().clone());
    let target = remote(&client, &server, &obj);
    // Sent from a worker of the runtime the ORB shares.
    let rx = rt.block_on(async { send_long(&target, "echo_long", 9) });
    let (result, name) = wait(&rx);
    assert_eq!(long_of(result), 9);
    assert_eq!(name.as_deref(), Some("rustorb-upcall"));
    client.destroy().unwrap();
    drop(rt);
    server.destroy().unwrap();
}
