//! The internal counters (`rustorb_core::stats`, `-ORBstatistics`): off
//! at first, turned on by the ORB option, and counting the paths a call
//! takes. One test: the counters are process-wide.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use rustorb_core::stats::{self, Counter};
use rustorb_core::{
    CompletionStatus, EndpointAddr, InStream, Orb, OrbOptions, Servant, ServerReply,
    SystemException, SystemExceptionKind, minor,
};

struct Echo;

impl Servant for Echo {
    fn repository_id(&self) -> &str {
        "IDL:test/Echo:1.0"
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
                SystemExceptionKind::BAD_OPERATION,
                minor::UNKNOWN_OPERATION,
                CompletionStatus::No,
            )),
        }
    }
}

#[test]
fn counts_the_paths_of_calls_once_enabled() {
    assert!(!stats::enabled());

    let mut args = vec![
        "prog".to_owned(),
        "-ORBstatistics".to_owned(),
        "1".to_owned(),
    ];
    let mut options = OrbOptions::default();
    options.parse_args(&mut args).unwrap();
    assert_eq!(args, ["prog"]);
    assert_eq!(options.statistics, Some(true));
    options.endpoints = vec![EndpointAddr {
        host: "127.0.0.1".into(),
        port: 0,
    }];
    let server = Orb::with_options(options).unwrap();
    assert!(stats::enabled());

    let root = server.root_poa().unwrap();
    let id = root.activate_object(Arc::new(Echo)).unwrap();
    root.the_poa_manager().activate().unwrap();
    let ior = server
        .object_to_string(Some(&root.id_to_reference(&id).unwrap()))
        .unwrap();
    let client = Orb::with_options(OrbOptions::default()).unwrap();
    let obj = client.string_to_object(&ior).unwrap().unwrap();

    let before = stats::snapshot();
    for i in 0..20 {
        let reply = obj
            .request("echo_long")
            .args(move |out| {
                out.write_i32(i);
                Ok(())
            })
            .invoke()
            .unwrap();
        assert_eq!(reply.input().read::<i32>().unwrap(), i);
    }
    // The server counts a reply after writing it: the last one may be
    // counted after the caller has it.
    let deadline = Instant::now() + Duration::from_secs(5);
    let written =
        |s: &stats::Snapshot| s.get(Counter::outbox_direct) + s.get(Counter::outbox_queued);
    while written(&stats::snapshot().since(&before)) < 20 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(1));
    }
    let used = stats::snapshot().since(&before);

    // Every reply reached its caller one way or the other, every request
    // its servant through the pool or a following thread, and every reply
    // was written.
    assert_eq!(
        used.get(Counter::replies_read_here) + used.get(Counter::replies_from_task),
        20,
        "{used}"
    );
    assert!(used.get(Counter::pool_jobs) >= 1, "{used}");
    assert_eq!(
        used.get(Counter::pool_jobs) + used.get(Counter::follow_requests),
        20,
        "{used}"
    );
    assert_eq!(written(&used), 20, "{used}");
    assert!(used.get(Counter::recv_calls) > 0, "{used}");

    // Unix: a thread that takes the reading takes it off the runtime's
    // poller, so data arriving meanwhile does not wake the I/O task (an
    // event the poller had already seen may still get through). The
    // server's following thread keeps it off while it handles a request,
    // so it takes it off once, when the I/O task leaves the reading to it.
    #[cfg(unix)]
    {
        assert!(used.get(Counter::claims) > 0, "{used}");
        assert!(used.get(Counter::follow_pauses) >= 1, "{used}");
        assert!(
            used.get(Counter::unwatches)
                <= used.get(Counter::claims) - used.get(Counter::follow_requests)
                    + used.get(Counter::follow_pauses),
            "{used}"
        );
        assert!(used.get(Counter::task_wakeups_thread_reads) <= 2, "{used}");
    }

    // Off: the caller counts no more (the server's threads may still see
    // the counting on for a moment).
    stats::set_enabled(false);
    let replies =
        |s: &stats::Snapshot| s.get(Counter::replies_read_here) + s.get(Counter::replies_from_task);
    let before = stats::snapshot();
    obj.request("echo_long")
        .args(|out| {
            out.write_i32(1);
            Ok(())
        })
        .invoke()
        .unwrap();
    assert_eq!(replies(&stats::snapshot()), replies(&before));

    let _ = client.destroy();
    let _ = server.destroy();
}
