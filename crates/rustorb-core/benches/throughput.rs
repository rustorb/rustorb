//! Throughput benchmarks for the cases issue #64 measured against omniORB:
//! many connections with small calls, and large messages. Run with
//! `cargo bench -p rustorb-core`; criterion keeps the last run in
//! `target/criterion` and reports the change against it, so a slowdown
//! shows up when the benchmark is run before and after a change
//! (`--save-baseline NAME` / `--baseline NAME` to compare with a named
//! run). See docs/load-testing.md.
//!
//! - `giop/*`: reading a message into a buffer the ORB keeps, without the
//!   network (`FragmentAssembler` and decoding). Catches copies coming
//!   back.
//! - `orb/*`: an ORB calling another in the same process over loopback
//!   TCP: one call at a time, 1 MiB octet sequences (copied, and shared
//!   with `read_octet_seq_shared` / `write_octet_seq_shared`), and 16 client ORBs
//!   (a connection each) calling at once. `echo_long_noop_interceptors` is
//!   `echo_long` with a Portable Interceptor that does nothing on each side.
//! - `orb/oneway`: oneway calls as fast as the client sends them, timed
//!   until the servant has run every one (the server's rate).
//! - `orb/echo_long_threads_one_ref/4`: 4 threads calling through one
//!   object reference (one connection) at once.
//! - `orb/echo_long_in_process`: the caller and the servant in one ORB.
//!
//! The speed plan (docs/perf-plan.md, #203) compares these before and
//! after each step.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rustorb_core::cdr::{ByteOrder, Octets};
use rustorb_core::giop::{
    FragmentAssembler, Message, MessageEncoder, RequestHeader, TargetAddress, Version,
    response_flags,
};
use rustorb_core::pi::{
    ClientRequestInterceptor, Interceptor, OrbInitInfo, OrbInitializer, ServerRequestInterceptor,
};
use rustorb_core::{
    CompletionStatus, EndpointAddr, InStream, ObjectRef, Orb, OrbOptions, Servant, ServerReply,
    SystemException, SystemExceptionKind, minor,
};

const ECHO_ID: &str = "IDL:bench/Echo:1.0";
const MIB: usize = 1 << 20;

/// The oneway `post` calls the servants have run.
static POSTED: AtomicU64 = AtomicU64::new(0);

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
            "echo_octets_shared" => {
                let v = input.read_octet_seq_shared()?;
                reply.output().write_octet_seq_shared(&v)?;
                Ok(())
            }
            "echo_octets" => {
                let v = input.read_octet_seq()?;
                reply.output().write_octet_seq(v)?;
                Ok(())
            }
            "post" => {
                input.read::<i32>()?;
                POSTED.fetch_add(1, Ordering::Relaxed);
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

/// A request carrying `len` octets, as it arrives off the wire.
fn request_bytes(len: usize) -> Vec<u8> {
    let header = RequestHeader {
        request_id: 1,
        response_flags: response_flags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(b"obj".to_vec()),
        operation: "echo_octets".into(),
        service_context: Vec::new(),
        requesting_principal: Vec::new(),
    };
    let mut msg = MessageEncoder::request(Version::V1_2, ByteOrder::Little, &header).unwrap();
    msg.body().write_octet_seq(&vec![7u8; len]).unwrap();
    msg.finish().unwrap()
}

fn giop(c: &mut Criterion) {
    let mut group = c.benchmark_group("giop");
    let msg = request_bytes(MIB);
    group.throughput(Throughput::Bytes(msg.len() as u64));
    group.bench_function("assemble_and_decode_1MiB", |b| {
        let mut assembler = FragmentAssembler::default();
        b.iter_batched(
            || msg.clone(),
            |msg| {
                let whole = assembler.push_owned(msg).unwrap().unwrap();
                let (_, message) = whole.into_decoded().unwrap();
                let Message::Request { body, .. } = message else {
                    unreachable!()
                };
                body.decoder().read_octet_seq().unwrap().len()
            },
            criterion::BatchSize::LargeInput,
        );
    });
    group.finish();
}

struct Pair {
    server: Orb,
    client: Orb,
    /// The object, through the client ORB.
    obj: ObjectRef,
    /// The object, in the server ORB.
    local: ObjectRef,
}

/// Client and server interceptors that do nothing.
struct Noop;

impl Interceptor for Noop {
    fn name(&self) -> &str {
        "noop"
    }
}

impl ClientRequestInterceptor for Noop {}
impl ServerRequestInterceptor for Noop {}

impl OrbInitializer for Noop {
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), SystemException> {
        info.add_client_request_interceptor(Arc::new(Noop)).unwrap();
        info.add_server_request_interceptor(Arc::new(Noop)).unwrap();
        Ok(())
    }
}

impl Pair {
    fn new() -> Pair {
        Pair::with(Vec::new())
    }

    fn with(initializers: Vec<Arc<dyn OrbInitializer>>) -> Pair {
        let server = Orb::with_options(OrbOptions {
            endpoints: vec![EndpointAddr {
                host: "127.0.0.1".into(),
                port: 0,
            }],
            initializers: initializers.clone(),
            ..OrbOptions::default()
        })
        .unwrap();
        let root = server.root_poa().unwrap();
        let id = root.activate_object(Arc::new(Echo)).unwrap();
        root.the_poa_manager().activate().unwrap();
        let local = root.id_to_reference(&id).unwrap();
        let client = Orb::with_options(OrbOptions {
            initializers,
            ..OrbOptions::default()
        })
        .unwrap();
        let obj = remote(&client, &server, &local);
        Pair {
            server,
            client,
            obj,
            local,
        }
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        let _ = self.client.destroy();
        let _ = self.server.destroy();
    }
}

fn remote(client: &Orb, server: &Orb, obj: &ObjectRef) -> ObjectRef {
    client
        .string_to_object(&server.object_to_string(Some(obj)).unwrap())
        .unwrap()
        .unwrap()
}

fn echo_long(obj: &ObjectRef, v: i32) -> i32 {
    let reply = obj
        .request("echo_long")
        .args(move |out| {
            out.write_i32(v);
            Ok(())
        })
        .invoke()
        .unwrap();
    reply.input().read::<i32>().unwrap()
}

/// Like [`echo_octets`] with the shared octet API on both sides: nothing
/// is copied outside the kernel.
fn echo_octets_shared(obj: &ObjectRef, data: &Octets) -> usize {
    let reply = obj
        .request("echo_octets_shared")
        .args(|out| Ok(out.write_octet_seq_shared(data)?))
        .invoke()
        .unwrap();
    reply.input().read_octet_seq_shared().unwrap().len()
}

/// Sends `n` oneway calls, then waits until the servant has run them all.
fn oneway(obj: &ObjectRef, n: u64) {
    let target = POSTED.load(Ordering::Relaxed) + n;
    for i in 0..n {
        obj.request("post")
            .args(move |out| {
                out.write_i32(i as i32);
                Ok(())
            })
            .send_oneway()
            .unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    while POSTED.load(Ordering::Relaxed) < target {
        assert!(Instant::now() < deadline, "oneway calls lost");
        thread::yield_now();
    }
}

fn echo_octets(obj: &ObjectRef, data: &[u8]) -> usize {
    let reply = obj
        .request("echo_octets")
        .args(|out| Ok(out.write_octet_seq(data)?))
        .invoke()
        .unwrap();
    reply.input().read_octet_seq().unwrap().len()
}

fn orb(c: &mut Criterion) {
    let pair = Pair::new();
    let mut group = c.benchmark_group("orb");
    group.measurement_time(Duration::from_secs(5));

    group.throughput(Throughput::Elements(1));
    group.bench_function("echo_long", |b| {
        b.iter(|| assert_eq!(echo_long(&pair.obj, 7), 7));
    });
    group.bench_function("echo_long_in_process", |b| {
        b.iter(|| assert_eq!(echo_long(&pair.local, 7), 7));
    });
    group.bench_function("oneway", |b| {
        b.iter_custom(|iters| {
            let t = Instant::now();
            oneway(&pair.obj, iters);
            t.elapsed()
        });
    });
    // Each iteration is one call from each thread, all at once.
    {
        let threads = 4;
        let refs = vec![pair.obj.clone(); threads];
        group.throughput(Throughput::Elements(threads as u64));
        group.bench_with_input(
            BenchmarkId::new("echo_long_threads_one_ref", threads),
            &refs,
            |b, refs| b.iter_custom(|iters| concurrent(refs, iters)),
        );
    }
    group.throughput(Throughput::Elements(1));
    let intercepted = Pair::with(vec![Arc::new(Noop)]);
    group.bench_function("echo_long_noop_interceptors", |b| {
        b.iter(|| assert_eq!(echo_long(&intercepted.obj, 7), 7));
    });
    drop(intercepted);

    let data = vec![7u8; MIB];
    group.throughput(Throughput::Bytes(2 * MIB as u64));
    group.bench_function("echo_octets_1MiB", |b| {
        b.iter(|| assert_eq!(echo_octets(&pair.obj, &data), MIB));
    });
    let shared = Octets::from(data.clone());
    group.bench_function("echo_octets_shared_1MiB", |b| {
        b.iter(|| assert_eq!(echo_octets_shared(&pair.obj, &shared), MIB));
    });

    // Each iteration is one call from each client, all at once.
    for clients in [16] {
        let orbs: Vec<(Orb, ObjectRef)> = (0..clients)
            .map(|_| {
                let o = Orb::with_options(OrbOptions {
                    io_threads: 1,
                    ..OrbOptions::default()
                })
                .unwrap();
                let obj = remote(&o, &pair.server, &pair.obj);
                (o, obj)
            })
            .collect();
        let refs: Vec<ObjectRef> = orbs.iter().map(|(_, obj)| obj.clone()).collect();
        group.throughput(Throughput::Elements(clients as u64));
        group.bench_with_input(
            BenchmarkId::new("echo_long_connections", clients),
            &refs,
            |b, refs| b.iter_custom(|iters| concurrent(refs, iters)),
        );
        for (o, _) in orbs {
            let _ = o.destroy();
        }
    }
    group.finish();
}

/// Has a thread for each reference make `iters` calls, all at once;
/// returns the wall time.
fn concurrent(refs: &[ObjectRef], iters: u64) -> Duration {
    let start = Arc::new(Barrier::new(refs.len() + 1));
    let handles: Vec<_> = refs
        .iter()
        .map(|obj| {
            let obj = obj.clone();
            let start = Arc::clone(&start);
            thread::spawn(move || {
                start.wait();
                for i in 0..iters {
                    assert_eq!(echo_long(&obj, i as i32), i as i32);
                }
            })
        })
        .collect();
    start.wait();
    let t = Instant::now();
    for h in handles {
        h.join().unwrap();
    }
    t.elapsed()
}

criterion_group!(benches, giop, orb);
criterion_main!(benches);
