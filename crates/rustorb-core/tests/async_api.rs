//! The async API and ORBs on the application's tokio runtime.

use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use rustorb_core::{
    CompletionStatus, EndpointAddr, Exception, InStream, ObjectRef, Orb, OrbOptions, Servant,
    ServerReply, SystemException, SystemExceptionKind, minor,
};

const TEST_ID: &str = "IDL:test/Async:1.0";

#[derive(Default)]
struct TestServant {
    this: OnceLock<ObjectRef>,
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
            // A blocking call from the servant, to its own object.
            "call_self" => {
                let v = input.read::<i32>()?;
                let got = call_long(self.this.get().unwrap(), v).map_err(|e| match e {
                    Exception::System(e) => e,
                    Exception::User(_) => sys(SystemExceptionKind::UNKNOWN),
                })?;
                reply.output().write_i32(got + 1);
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

fn sys(kind: SystemExceptionKind) -> SystemException {
    SystemException::new(kind, 0, CompletionStatus::No)
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

/// `servant` active in the RootPOA, and the manager activated.
fn serve(orb: &Orb, servant: Arc<TestServant>) -> ObjectRef {
    let root = orb.root_poa().unwrap();
    let id = root.activate_object(servant.clone()).unwrap();
    let obj = root.id_to_reference(&id).unwrap();
    let _ = servant.this.set(obj.clone());
    root.the_poa_manager().activate().unwrap();
    obj
}

/// A reference of `client` to `obj` of another ORB.
fn remote(client: &Orb, server: &Orb, obj: &ObjectRef) -> ObjectRef {
    client
        .string_to_object(&server.object_to_string(Some(obj)).unwrap())
        .unwrap()
        .unwrap()
}

fn call_long(obj: &ObjectRef, v: i32) -> Result<i32, Exception> {
    let reply = obj
        .request("echo_long")
        .args(move |out| {
            out.write_i32(v);
            Ok(())
        })
        .invoke()?;
    Ok(reply.input().read::<i32>()?)
}

async fn call_long_async(obj: &ObjectRef, op: &str, v: i32) -> Result<i32, Exception> {
    let reply = obj
        .request_async(op)
        .args(move |out| {
            out.write_i32(v);
            Ok(())
        })
        .invoke()
        .await?;
    Ok(reply.input().read::<i32>()?)
}

fn system<T: std::fmt::Debug>(result: Result<T, Exception>) -> SystemException {
    match result {
        Err(Exception::System(e)) => e,
        other => panic!("expected a system exception, got {other:?}"),
    }
}

fn multi_thread() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn async_calls_on_a_shared_runtime() {
    let rt = multi_thread();
    let server = Orb::with_runtime(server_options(), rt.handle().clone());
    let servant = Arc::new(TestServant::default());
    let obj = serve(&server, servant.clone());
    let client = Orb::with_runtime(OrbOptions::default(), rt.handle().clone());
    assert_eq!(client.runtime_handle().id(), rt.handle().id());
    let target = remote(&client, &server, &obj);

    rt.block_on(async {
        assert_eq!(call_long_async(&target, "echo_long", 7).await.unwrap(), 7);
        // Many calls at once on one connection, from tasks.
        let tasks: Vec<_> = (0..32)
            .map(|i| {
                let target = target.clone();
                tokio::spawn(async move { call_long_async(&target, "echo_long", i).await })
            })
            .collect();
        for (i, task) in tasks.into_iter().enumerate() {
            assert_eq!(task.await.unwrap().unwrap(), i as i32);
        }
        assert!(target.is_a_async(TEST_ID).await.unwrap());
        assert!(!target.is_a_async("IDL:test/Other:1.0").await.unwrap());
        assert!(!target.non_existent_async().await.unwrap());
        assert!(target.locate_async().await.unwrap());
        target
            .request_async("post")
            .args(|out| {
                out.write_i32(5);
                Ok(())
            })
            .send_oneway()
            .await
            .unwrap();
        // A user of the async API is not refused, but the blocking API is
        // (it would block a worker of the runtime it needs).
        let e = system(call_long(&target, 1));
        assert!(e.is(SystemExceptionKind::BAD_INV_ORDER));
        let e = system(call_long_async(&target, "nope", 1).await);
        assert!(e.is(SystemExceptionKind::BAD_OPERATION));
    });
    // The blocking API from a thread of its own.
    assert_eq!(call_long(&target, 9).unwrap(), 9);
    let posted = servant.posted.lock().unwrap().clone();
    assert!(posted == [5] || posted.is_empty(), "{posted:?}");

    client.destroy().unwrap();
    server.destroy().unwrap();
    // The runtime keeps none of the ORBs' tasks.
    let alive = (0..100)
        .map(|_| {
            thread::sleep(Duration::from_millis(10));
            rt.metrics().num_alive_tasks()
        })
        .find(|&n| n == 0);
    assert_eq!(alive, Some(0));
}

#[test]
fn async_calls_polled_outside_the_orbs_runtime() {
    // The ORBs have runtimes of their own; the futures are polled by a
    // runtime with neither I/O nor timers.
    let server = Orb::with_options(server_options()).unwrap();
    let obj = serve(&server, Arc::new(TestServant::default()));
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let target = remote(&client, &server, &obj);
    let bare = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    bare.block_on(async {
        assert_eq!(call_long_async(&target, "echo_long", -3).await.unwrap(), -3);
        let e = system(
            target
                .request_async("sleep")
                .args(|out| {
                    out.write_u32(2000);
                    Ok(())
                })
                .timeout(Some(Duration::from_millis(100)))
                .invoke()
                .await,
        );
        assert!(e.is(SystemExceptionKind::TIMEOUT), "{e:?}");
        assert_eq!(e.completed, CompletionStatus::Maybe);
    });
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn async_calls_to_the_orbs_own_objects() {
    let rt = multi_thread();
    let orb = Orb::with_runtime(server_options(), rt.handle().clone());
    let obj = serve(&orb, Arc::new(TestServant::default()));
    rt.block_on(async {
        assert_eq!(call_long_async(&obj, "echo_long", 4).await.unwrap(), 4);
        // The servant runs on an upcall thread, where it may block.
        assert_eq!(call_long_async(&obj, "call_self", 4).await.unwrap(), 5);
        assert!(obj.locate_async().await.unwrap());
    });
    orb.destroy().unwrap();
}

#[test]
fn a_current_thread_runtime() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = Orb::with_runtime(server_options(), rt.handle().clone());
    let obj = serve(&server, Arc::new(TestServant::default()));
    let client = Orb::with_runtime(OrbOptions::default(), rt.handle().clone());
    let target = remote(&client, &server, &obj);
    rt.block_on(async {
        assert_eq!(call_long_async(&target, "echo_long", 11).await.unwrap(), 11);
        assert_eq!(call_long_async(&target, "call_self", 1).await.unwrap(), 2);
        // Shutting down needs the runtime to run: from inside it, without
        // blocking it.
        for orb in [&client, &server] {
            orb.shutdown(false).unwrap();
            orb.run_async().await;
        }
    });
    client.destroy().unwrap();
    server.destroy().unwrap();
}

#[test]
fn run_async_resolves_at_shutdown() {
    let rt = multi_thread();
    let orb = Orb::with_runtime(server_options(), rt.handle().clone());
    let obj = serve(&orb, Arc::new(TestServant::default()));
    rt.block_on(async {
        let waiter = tokio::spawn({
            let orb = orb.clone();
            async move { orb.run_async().await }
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!waiter.is_finished());
        orb.shutdown(false).unwrap();
        tokio::time::timeout(Duration::from_secs(10), waiter)
            .await
            .unwrap()
            .unwrap();
        // Once shut down, it resolves at once.
        orb.run_async().await;
        let e = system(call_long_async(&obj, "echo_long", 1).await);
        assert!(e.is(SystemExceptionKind::BAD_INV_ORDER));
    });
    orb.destroy().unwrap();
}

#[test]
fn the_runtime_shut_down_first() {
    let rt = multi_thread();
    let server = Orb::with_runtime(server_options(), rt.handle().clone());
    let obj = serve(&server, Arc::new(TestServant::default()));
    let client = Orb::with_runtime(OrbOptions::default(), rt.handle().clone());
    let target = remote(&client, &server, &obj);
    assert_eq!(call_long(&target, 1).unwrap(), 1);
    drop(rt);
    // Calls fail, and destroying the ORBs still returns.
    let e = system(call_long(&target, 2));
    assert!(
        e.is(SystemExceptionKind::COMM_FAILURE) || e.is(SystemExceptionKind::TRANSIENT),
        "{e:?}"
    );
    client.destroy().unwrap();
    server.destroy().unwrap();
}
